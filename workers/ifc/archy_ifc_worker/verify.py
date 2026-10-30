"""Reopen and measure the written IFC; this is an exporter-engine check, not A13."""

import argparse
from contextlib import redirect_stdout
from datetime import datetime
import json
import math
from pathlib import Path
import sys

from .geometry import footprint
from .protocol import MAX_REQUEST_BYTES, ProtocolError, parse_request


class ArtifactError(ValueError):
    pass


def require(condition, message):
    if not condition:
        raise ArtifactError(message)


def inspect(path, request):
    import ifcopenshell
    import ifcopenshell.geom
    import ifcopenshell.util.element
    import ifcopenshell.util.placement

    try:
        model = ifcopenshell.open(str(path))
    except Exception as exc:
        raise ArtifactError("IFC parser could not reopen artifact") from exc
    v = request["variant"]
    n, h = v["storeys"], v["floor_to_floor_m"]
    ox, oy = v["parameters"]["origin_xy_m"]
    local, area = footprint(v)
    require(model.schema == "IFC4", "wrong IFC schema")
    profiles = model.by_type("IfcArbitraryClosedProfileDef")
    require(len(profiles) == 1 and profiles[0].OuterCurve.is_a("IfcIndexedPolyCurve"),
            "wrong swept profile")
    observed = profiles[0].OuterCurve.Points.CoordList
    require(len(observed) == len(local) and
            all(all(abs(a - b) <= 1e-6 for a, b in zip(got, want))
                for got, want in zip(observed, local)), "profile vertices or notch mismatch")
    for name, count in (("IfcProject", 1), ("IfcSite", 1), ("IfcBuilding", 1),
                        ("IfcBuildingStorey", n), ("IfcBuildingElementProxy", n),
                        ("IfcRelAggregates", 3), ("IfcRelContainedInSpatialStructure", n)):
        require(len(model.by_type(name)) == count, "wrong hierarchy count: " + name)
    for forbidden in ("IfcWall", "IfcSlab", "IfcSpace"):
        require(not model.by_type(forbidden), "unexpected detailed element: " + forbidden)
    roots = model.by_type("IfcRoot")
    guids = [root.GlobalId for root in roots]
    require(len(guids) == len(set(guids)), "duplicate GUID")
    for guid in guids:
        try:
            expanded = ifcopenshell.guid.expand(guid)
        except Exception as exc:
            raise ArtifactError("invalid GUID") from exc
        require(len(guid) == 22 and len(expanded) == 32, "invalid GUID")
    units = model.by_type("IfcSIUnit")
    for kind, expected in (("LENGTHUNIT", "METRE"), ("AREAUNIT", "SQUARE_METRE"),
                           ("VOLUMEUNIT", "CUBIC_METRE")):
        matches = [u for u in units if u.UnitType == kind]
        require(len(matches) == 1 and matches[0].Name == expected and matches[0].Prefix is None,
                "wrong SI unit: " + kind)
    project = model.by_type("IfcProject")[0]
    assignment = project.UnitsInContext
    require(assignment is not None and assignment.is_a("IfcUnitAssignment"),
            "project has no active unit assignment")
    active = list(assignment.Units or ())
    expected_units = {"LENGTHUNIT": "METRE", "AREAUNIT": "SQUARE_METRE",
                      "VOLUMEUNIT": "CUBIC_METRE"}
    require(len(active) == 3 and all(unit.is_a() == "IfcSIUnit" for unit in active) and
            {unit.UnitType for unit in active} == set(expected_units),
            "project active units must be exact SI length/area/volume")
    for unit in active:
        require(unit.Name == expected_units[unit.UnitType] and unit.Prefix is None,
                "wrong active SI unit: " + unit.UnitType)
    site = model.by_type("IfcSite")[0]
    building = model.by_type("IfcBuilding")[0]
    require(site in project.IsDecomposedBy[0].RelatedObjects, "site aggregation wrong")
    require(building in site.IsDecomposedBy[0].RelatedObjects, "building aggregation wrong")

    def check_placement(entity, expected, label):
        matrix = ifcopenshell.util.placement.get_local_placement(entity.ObjectPlacement)
        require(all(abs(matrix[axis, 3] - expected[axis]) <= 1e-6 for axis in range(3)) and
                all(abs(matrix[row, col] - (1 if row == col else 0)) <= 1e-6
                    for row in range(3) for col in range(3)),
                "wrong " + label + " placement")

    check_placement(site, (0, 0, 0), "site")
    check_placement(building, (ox, oy, 0), "building")
    psets = ifcopenshell.util.element.get_psets(building)
    props = psets.get("Archy_SchematicMassing", {})
    require(props.get("ApprovalState") == "DRAFT" and props.get("Schematic") is True,
            "missing DRAFT schematic provenance")
    for field, expected in (("ProjectId", request["project_id"]),
                            ("VariantId", request["variant_id"]),
                            ("RunId", request["run_id"]),
                            ("CanonicalInputSha256", request["source_digest"])):
        require(props.get(field) == expected, "provenance mismatch: " + field)
    require(props.get("SourceArtifacts") == json.dumps(request["source_artifacts"], sort_keys=True),
            "source references mismatch")
    require(props.get("WorkerVersion") == "ifc-worker-v1" and
            props.get("LibraryVersion") == ifcopenshell.version and
            props.get("VariantSchemaVersion") == v["schema_version"], "version provenance mismatch")
    try:
        stamp = datetime.fromisoformat(props["ExportedAtUTC"])
    except (KeyError, TypeError, ValueError) as exc:
        raise ArtifactError("missing export timestamp") from exc
    require(stamp.tzinfo is not None and stamp.utcoffset().total_seconds() == 0,
            "export timestamp is not UTC")
    quantities = [rel.RelatingPropertyDefinition for rel in building.IsDefinedBy
                  if rel.RelatingPropertyDefinition.is_a("IfcElementQuantity")]
    require(len(quantities) == 1, "missing schematic quantities")
    require(quantities[0].Name == "Archy_SchematicQuantities" and
            quantities[0].MethodOfMeasurement.startswith("task02-analytic-massing-v1"),
            "wrong quantity method")
    names = {"FootprintArea", "ApproximateGFA", "MassVolume"}
    require(len(quantities[0].Quantities) == 3 and
            {q.Name for q in quantities[0].Quantities} == names, "wrong quantity set")
    expected_units = {"FootprintArea": "AREAUNIT", "ApproximateGFA": "AREAUNIT",
                      "MassVolume": "VOLUMEUNIT"}
    for q in quantities[0].Quantities:
        require(q.Unit is not None and q.Unit.UnitType == expected_units[q.Name] and
                q.Unit.Prefix is None and q.Formula, "wrong quantity unit or formula")
    values = {q.Name: q[3] for q in quantities[0].Quantities}
    require(math.isclose(values.get("FootprintArea", -1), area, abs_tol=1e-6) and
            math.isclose(values.get("ApproximateGFA", -1), area * n, abs_tol=1e-6) and
            math.isclose(values.get("MassVolume", -1), area * n * h, abs_tol=1e-6),
            "wrong schematic quantities")
    settings = ifcopenshell.geom.settings()
    settings.set(settings.USE_WORLD_COORDS, True)
    measurements = []
    for i, (storey, proxy) in enumerate(zip(model.by_type("IfcBuildingStorey"),
                                             model.by_type("IfcBuildingElementProxy"))):
        require(storey in building.IsDecomposedBy[0].RelatedObjects, "storey aggregation wrong")
        require(len(proxy.ContainedInStructure) == 1 and
                proxy.ContainedInStructure[0].RelatingStructure == storey,
                "proxy containment wrong")
        require(proxy.PredefinedType == "USERDEFINED" and
                proxy.ObjectType == "ArchySchematicStoreyMass", "proxy type wrong")
        representations = proxy.Representation.Representations
        require(len(representations) == 1 and representations[0].RepresentationIdentifier == "Body" and
                representations[0].RepresentationType == "SweptSolid" and
                len(representations[0].Items) == 1 and
                representations[0].Items[0].is_a("IfcExtrudedAreaSolid"),
                "proxy must have one Body/SweptSolid extrusion")
        for entity, expected in ((storey, (ox, oy, i * h)), (proxy, (ox, oy, i * h))):
            check_placement(entity, expected, "accumulated")
        try:
            shape = ifcopenshell.geom.create_shape(settings, proxy)
        except Exception as exc:
            raise ArtifactError("native tessellation failed") from exc
        vertices = list(zip(*(iter(shape.geometry.verts),) * 3))
        require(bool(vertices), "empty tessellation")
        bounds = [min(v[axis] for v in vertices) for axis in range(3)] + [
                  max(v[axis] for v in vertices) for axis in range(3)]
        expected = [min(p[0] for p in v["footprint_xy_m"]), min(p[1] for p in v["footprint_xy_m"]),
                    i * h, max(p[0] for p in v["footprint_xy_m"]),
                    max(p[1] for p in v["footprint_xy_m"]), (i + 1) * h]
        require(all(abs(a - b) <= 1e-6 for a, b in zip(bounds, expected)),
                "tessellated world bounds mismatch")
        faces = list(zip(*(iter(shape.geometry.faces),) * 3))
        volume = 0.0
        for ia, ib, ic in faces:
            a, b, c = vertices[ia], vertices[ib], vertices[ic]
            volume += (a[0] * (b[1] * c[2] - b[2] * c[1]) +
                       a[1] * (b[2] * c[0] - b[0] * c[2]) +
                       a[2] * (b[0] * c[1] - b[1] * c[0])) / 6
        require(abs(abs(volume) - area * h) <= max(1e-6, area * h * 1e-8),
                "tessellated mass volume mismatch")
        measurements.append({"bounds_m": bounds, "volume_m3": abs(volume), "triangles": len(faces)})
    return {"status": "ok", "schema": model.schema, "storeys": n,
            "footprint_area_m2": area, "expected_footprint_area_m2": area,
            "approximate_gfa_m2": area * n, "height_m": n * h,
            "tessellation": measurements, "draft": True,
            "engine": "IfcOpenShell reopen+tessellation; not independent A13"}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--ifc", required=True)
    parser.add_argument("--request", required=True)
    args = parser.parse_args()
    try:
        with Path(args.request).open("rb") as source:
            request = parse_request(source.read(MAX_REQUEST_BYTES + 1))
        with redirect_stdout(sys.stderr):
            report = inspect(Path(args.ifc), request)
    except (ArtifactError, ProtocolError, OSError) as exc:
        print(json.dumps({"status": "error", "message": str(exc)}))
        return 20
    print(json.dumps(report, sort_keys=True, allow_nan=False))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
