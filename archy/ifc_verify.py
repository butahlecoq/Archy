"""Verify Archy massing properties and represented geometry in an IFC4 file."""

import argparse
from itertools import permutations
import json
import math
from pathlib import Path
import re
import sys
from typing import Any

from .geometry import Footprint


TOLERANCE_M = 1e-6


def _same_points(actual: list[tuple[float, float]], expected: list[tuple[float, float]]) -> bool:
    return len(actual) == len(expected) and any(
        all(
            all(_close(a, e) for a, e in zip(actual_point, expected_point))
            for actual_point, expected_point in zip(actual, expected_order)
        )
        for expected_order in permutations(expected)
    )


def _properties(ifc: Any, element: Any, names: tuple[str, ...] = ("ArchyMassing", "Pset_ArchyMassing")) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for relation in getattr(element, "IsDefinedBy", ()):
        definition = getattr(relation, "RelatingPropertyDefinition", None)
        if definition is None or not definition.is_a("IfcPropertySet"):
            continue
        if definition.Name in names:
            for prop in definition.HasProperties:
                value = getattr(prop, "NominalValue", None)
                result[prop.Name] = getattr(value, "wrappedValue", value)
    return result


def _property_sets(element: Any, name: str) -> list[Any]:
    sets = []
    for relation in getattr(element, "IsDefinedBy", ()):
        definition = getattr(relation, "RelatingPropertyDefinition", None)
        if definition is not None and definition.is_a("IfcPropertySet") and definition.Name == name:
            sets.append(definition)
    return sets


def _aggregate_children(element: Any) -> list[Any]:
    return [
        child
        for relation in getattr(element, "IsDecomposedBy", ())
        if relation.is_a("IfcRelAggregates")
        for child in relation.RelatedObjects
    ]


def _has_only_aggregate_parent(element: Any, parent_type: str, parent_id: int) -> bool:
    relations = getattr(element, "Decomposes", ())
    if len(relations) != 1 or not relations[0].is_a("IfcRelAggregates"):
        return False
    parent = relations[0].RelatingObject
    return parent.is_a(parent_type) and parent.id() == parent_id


def _contained_products(element: Any) -> list[Any]:
    return [child for relation in getattr(element, "ContainsElements", ()) for child in relation.RelatedElements]


def _verify_spatial_structure(model: Any, expected: dict[str, dict[str, Any]], digest: str) -> list[str]:
    failures: list[str] = []
    buildings = model.by_type("IfcBuilding")
    by_id: dict[str, list[Any]] = {}
    for building in buildings:
        props = _properties(model, building, ("ArchyAlternative",))
        alt_id = props.get("AlternativeId")
        if isinstance(alt_id, str):
            by_id.setdefault(alt_id, []).append(building)
    if len(buildings) != len(expected) or set(by_id) != set(expected) or any(len(items) != 1 for items in by_id.values()):
        failures.append("IFC alternative building IDs do not match evaluation")

    projects = model.by_type("IfcProject")
    sites = model.by_type("IfcSite")
    if len(projects) != 1:
        failures.append(f"expected exactly one IfcProject, found {len(projects)}")
    if len(sites) != 1:
        failures.append(f"expected exactly one IfcSite, found {len(sites)}")
    else:
        site = sites[0]
        if len(projects) == 1 and not _has_only_aggregate_parent(site, "IfcProject", projects[0].id()):
            failures.append("IfcSite must be aggregated by the project")
        site_buildings = [child for child in _aggregate_children(site) if child.is_a("IfcBuilding")]
        if len(site_buildings) != len(expected) or {child.id() for child in site_buildings} != {b.id() for values in by_id.values() for b in values}:
            failures.append("IfcSite must aggregate exactly the alternative buildings")
        for building in buildings:
            if not _has_only_aggregate_parent(building, "IfcSite", site.id()):
                failures.append(f"{_properties(model, building, ('ArchyAlternative',)).get('AlternativeId', 'building')}: building must be aggregated by the site")
    if len(model.by_type("IfcBuildingStorey")) != sum(
        row.get("input", {}).get("floor_count", 0) if isinstance(row.get("input"), dict) else 0
        for row in expected.values()
    ):
        failures.append("IFC storey count does not match evaluation")

    for alt_id, row in expected.items():
        matches = by_id.get(alt_id, [])
        if len(matches) != 1:
            continue
        building = matches[0]
        props = _properties(model, building, ("ArchyAlternative",))
        inputs = row.get("input", {})
        measurements = row.get("measurements", {})
        floors = inputs.get("floor_count") if isinstance(inputs, dict) else None
        floor_height = inputs.get("floor_to_floor_height_m") if isinstance(inputs, dict) else None
        height = measurements.get("building_height_m") if isinstance(measurements, dict) else None
        if len(_property_sets(building, "ArchyAlternative")) != 1:
            failures.append(f"{alt_id}: expected exactly one ArchyAlternative property set")
        if props.get("SourceBriefSHA256") != digest:
            failures.append(f"{alt_id}: building source digest property does not match evaluation")
        if props.get("FloorCount") != floors or isinstance(props.get("FloorCount"), bool):
            failures.append(f"{alt_id}: building FloorCount does not match evaluation")
        if not isinstance(floor_height, (int, float)) or not _close(props.get("FloorToFloorHeight_m"), float(floor_height)):
            failures.append(f"{alt_id}: building FloorToFloorHeight_m does not match evaluation")
        if not isinstance(height, (int, float)) or not _close(props.get("BuildingHeight_m"), float(height)):
            failures.append(f"{alt_id}: building BuildingHeight_m does not match evaluation")
        if props.get("SpatialStructureVersion") != 1 or isinstance(props.get("SpatialStructureVersion"), bool):
            failures.append(f"{alt_id}: SpatialStructureVersion must be 1")
        if not isinstance(floors, int) or isinstance(floors, bool) or floors < 1:
            continue
        storeys = [child for child in _aggregate_children(building) if child.is_a("IfcBuildingStorey")]
        if len(storeys) != floors:
            failures.append(f"{alt_id}: expected {floors} storeys, found {len(storeys)}")
        for storey in storeys:
            if not _has_only_aggregate_parent(storey, "IfcBuilding", building.id()):
                failures.append(f"{alt_id}: {storey.Name or 'storey'} must be aggregated only by its alternative building")
        storeys_by_name: dict[str, list[Any]] = {}
        for storey in storeys:
            storeys_by_name.setdefault(storey.Name or "", []).append(storey)
        for index in range(1, floors + 1):
            matching_storeys = storeys_by_name.get(f"Level {index}", [])
            if len(matching_storeys) != 1:
                failures.append(f"{alt_id}: expected exactly one Level {index} storey")
                continue
            storey = matching_storeys[0]
            elevation = (index - 1) * floor_height if isinstance(floor_height, (int, float)) else math.nan
            if not _close(storey.Elevation, float(elevation)):
                failures.append(f"{alt_id}: Level {index} elevation does not match evaluation")
            if getattr(storey, "Representation", None) is not None:
                failures.append(f"{alt_id}: Level {index} storey must not have geometry")
        proxies = [child for child in _contained_products(building) if child.is_a("IfcBuildingElementProxy")]
        if len(proxies) != 1:
            failures.append(f"{alt_id}: expected exactly one contained mass proxy, found {len(proxies)}")
        elif _properties(model, proxies[0]).get("AlternativeId") != alt_id:
            failures.append(f"{alt_id}: contained proxy alternative ID does not match building")
    return failures


def _close(actual: Any, expected: float) -> bool:
    return (
        isinstance(actual, (int, float))
        and not isinstance(actual, bool)
        and math.isfinite(actual)
        and math.isclose(float(actual), expected, rel_tol=0.0, abs_tol=TOLERANCE_M)
    )


def verify(evaluation_path: Path, model_path: Path) -> list[str]:
    """Return failures comparing IFC4 Archy proxies with an evaluation report."""
    try:
        import ifcopenshell
        import ifcopenshell.geom
    except ImportError as error:
        raise RuntimeError(
            "IfcOpenShell is required for IFC verification; install it in the Python environment "
            "running `python -m archy.ifc_verify`"
        ) from error

    try:
        evaluation = json.loads(evaluation_path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        return [f"cannot read evaluation report: {error}"]
    if not isinstance(evaluation, dict) or evaluation.get("contract_version") != 1:
        return ["evaluation report must be a contract version 1 object"]
    source = evaluation.get("source")
    source_digest = source.get("sha256") if isinstance(source, dict) else None
    if not isinstance(source_digest, str) or re.fullmatch(r"[0-9a-f]{64}", source_digest) is None:
        return ["evaluation report has no valid source SHA-256"]
    results = evaluation.get("results")
    if not isinstance(results, list) or not results:
        return ["evaluation report must contain alternatives"]
    expected: dict[str, dict[str, Any]] = {}
    for row in results:
        if not isinstance(row, dict) or not isinstance(row.get("id"), str) or row["id"] in expected:
            return ["evaluation report has malformed or duplicate alternative IDs"]
        expected[row["id"]] = row

    try:
        model = ifcopenshell.open(str(model_path))
    except Exception as error:
        return [f"cannot open IFC model: {error}"]
    if model.schema != "IFC4":
        return [f"expected IFC4, found {model.schema}"]

    has_new_metadata = any(pset.Name == "ArchyAlternative" for pset in model.by_type("IfcPropertySet"))
    has_storeys = bool(model.by_type("IfcBuildingStorey"))
    new_structure = has_new_metadata or has_storeys
    if new_structure:
        failures = _verify_spatial_structure(model, expected, source_digest)
    else:
        failures = []

    proxies = model.by_type("IfcBuildingElementProxy")
    by_id: dict[str, list[Any]] = {}
    for proxy in proxies:
        props = _properties(model, proxy)
        alt_id = props.get("AlternativeId")
        if isinstance(alt_id, str):
            by_id.setdefault(alt_id, []).append(proxy)
    if len(proxies) != len(expected):
        failures.append("IFC proxy count does not match evaluation")
    if set(by_id) != set(expected):
        failures.append("IFC proxy alternative IDs do not match evaluation")

    if new_structure:
        for proxy in proxies:
            if len(_property_sets(proxy, "ArchyMassing")) != 1:
                failures.append(f"{_properties(model, proxy).get('AlternativeId', 'proxy')}: expected exactly one ArchyMassing property set")
            if _property_sets(proxy, "Pset_ArchyMassing"):
                failures.append(f"{_properties(model, proxy).get('AlternativeId', 'proxy')}: new structure cannot use legacy Pset_ArchyMassing")

    settings = ifcopenshell.geom.settings()
    settings.set(settings.USE_WORLD_COORDS, True)
    for alt_id, row in expected.items():
        matches = by_id.get(alt_id, [])
        if len(matches) != 1:
            failures.append(f"{alt_id}: expected exactly one Archy proxy, found {len(matches)}")
            continue
        proxy = matches[0]
        props = _properties(model, proxy)
        inputs = row.get("input", {})
        footprint = inputs.get("footprint", {}) if isinstance(inputs, dict) else {}
        try:
            geometry = Footprint.from_mapping(footprint)
        except (TypeError, ValueError) as error:
            failures.append(f"{alt_id}: invalid footprint in evaluation: {error}")
            continue
        measurements = row.get("measurements", {})
        if props.get("SourceBriefSHA256") != source_digest:
            failures.append(f"{alt_id}: source digest property does not match evaluation")
        property_keys = {
            "width_m": "width_m",
            "depth_m": "depth_m",
            "origin_x_m": "origin_x_m",
            "origin_y_m": "origin_y_m",
        }
        for key, property_key in property_keys.items():
            expected_value = footprint.get(key) if isinstance(footprint, dict) else None
            if not isinstance(expected_value, (int, float)) or not _close(props.get(property_key), float(expected_value)):
                failures.append(f"{alt_id}: Archy property {key} does not match evaluation")
        stored_rotation = props.get("rotation_degrees")
        if "rotation_degrees" in footprint and not _close(stored_rotation, geometry.rotation_degrees):
            failures.append(f"{alt_id}: Archy property rotation_degrees is missing or does not match evaluation")
        elif "rotation_degrees" not in footprint and stored_rotation is not None and not _close(stored_rotation, 0.0):
            failures.append(f"{alt_id}: Archy property rotation_degrees does not match evaluation")
        floors = inputs.get("floor_count") if isinstance(inputs, dict) else None
        if props.get("FloorCount") != floors or isinstance(props.get("FloorCount"), bool):
            failures.append(f"{alt_id}: Archy property floor_count does not match evaluation")
        floor_height = inputs.get("floor_to_floor_height_m") if isinstance(inputs, dict) else None
        if not isinstance(floor_height, (int, float)) or not _close(props.get("FloorToFloorHeight_m"), float(floor_height)):
            failures.append(f"{alt_id}: Archy property floor_to_floor_height_m does not match evaluation")
        measurement_properties = {
            "footprint_area_m2": "footprint_area_m2",
            "gross_floor_area_m2": "gross_floor_area_m2",
            "building_height_m": "building_height_m",
        }
        for key, property_key in measurement_properties.items():
            value = measurements.get(key) if isinstance(measurements, dict) else None
            if not isinstance(value, (int, float)) or not _close(props.get(property_key), float(value)):
                failures.append(f"{alt_id}: Archy property {key} does not match evaluation")
        try:
            representations = getattr(getattr(proxy, "Representation", None), "Representations", ()) or ()
            items = [item for representation in representations for item in representation.Items]
            if len(items) != 1 or not items[0].is_a("IfcExtrudedAreaSolid"):
                raise ValueError("expected exactly one IfcExtrudedAreaSolid representation item")
            solid = items[0]
            profile = solid.SweptArea
            if not profile.is_a("IfcRectangleProfileDef"):
                raise ValueError("extruded solid must use an IfcRectangleProfileDef")
            if not _close(profile.XDim, float(footprint["width_m"])) or not _close(profile.YDim, float(footprint["depth_m"])):
                failures.append(f"{alt_id}: rectangle profile dimensions do not match evaluation")
            position = profile.Position
            coordinates = tuple(position.Location.Coordinates) if position and position.Location else ()
            reference = position.RefDirection if position else None
            direction = tuple(reference.DirectionRatios) if reference else (1.0, 0.0)
            if (
                len(coordinates) != 2
                or not _close(coordinates[0], float(footprint["width_m"]) / 2)
                or not _close(coordinates[1], float(footprint["depth_m"]) / 2)
                or len(direction) != 2
                or not _close(direction[0], 1.0)
                or not _close(direction[1], 0.0)
            ):
                failures.append(f"{alt_id}: rectangle profile position does not match exporter contract")
            if not _close(solid.Depth, float(measurements["building_height_m"])):
                failures.append(f"{alt_id}: extrusion depth does not match evaluation")
            shape = ifcopenshell.geom.create_shape(settings, proxy)
            vertices = shape.geometry.verts
            if not vertices or len(vertices) % 3:
                raise ValueError("geometry has no usable vertices")
            coordinates = [vertices[index:index + 3] for index in range(0, len(vertices), 3)]
            bounds = (
                min(point[0] for point in coordinates), max(point[0] for point in coordinates),
                min(point[1] for point in coordinates), max(point[1] for point in coordinates),
                min(point[2] for point in coordinates), max(point[2] for point in coordinates),
            )
            expected_bounds = (
                geometry.bounds().min_x_m, geometry.bounds().max_x_m,
                geometry.bounds().min_y_m, geometry.bounds().max_y_m,
                0.0, measurements["building_height_m"],
            )
            if any(not _close(actual, float(want)) for actual, want in zip(bounds, expected_bounds)):
                failures.append(f"{alt_id}: represented IFC world extents do not match evaluation")
            minimum_z = min(point[2] for point in coordinates)
            bottom = list(dict.fromkeys(
                (point[0], point[1]) for point in coordinates if point[2] == minimum_z
            ))
            expected_corners = [(point.x_m, point.y_m) for point in geometry.corners()]
            if not _same_points(bottom, expected_corners):
                failures.append(f"{alt_id}: represented IFC world corners do not match evaluation")
        except Exception as error:
            failures.append(f"{alt_id}: cannot inspect represented geometry: {error}")
    return failures


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("evaluation", type=Path, help="Archy evaluation.json")
    parser.add_argument("model", type=Path, help="IFC4 model to verify")
    args = parser.parse_args(argv)
    try:
        failures = verify(args.evaluation, args.model)
    except RuntimeError as error:
        parser.exit(2, f"archy-ifc-verify: error: {error}\n")
    if failures:
        parser.exit(1, "archy-ifc-verify: verification failed\n" + "\n".join(f"- {item}" for item in failures) + "\n")
    print("Archy IFC geometry and properties verified")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
