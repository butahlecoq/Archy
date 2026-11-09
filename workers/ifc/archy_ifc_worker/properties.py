"""Schematic, DRAFT provenance and explicitly named approximate quantities."""

import json
from datetime import datetime, timezone

from . import VERSION


def attach(model, request, building, proxies, area):
    import ifcopenshell.api.pset
    v = request["variant"]
    values = {
        "ProjectId": request["project_id"], "VariantId": request["variant_id"],
        "RunId": request["run_id"], "CanonicalInputSha256": request["source_digest"],
        "ExportedAtUTC": datetime.now(timezone.utc).isoformat(),
        "SourceArtifacts": json.dumps(request["source_artifacts"], sort_keys=True),
        "WorkerVersion": VERSION, "LibraryVersion": __import__("ifcopenshell").version,
        "VariantSchemaVersion": v["schema_version"],
        "Coordinates": "metres; right-handed XY, +Z; north is metadata",
        "MethodVersion": "task02-analytic-massing-v1",
        "ToleranceVersion": "polygon-coordinate-parity-v1",
        "Schematic": True, "ApprovalState": "DRAFT",
        "NorthAngleDegrees": float(request["site"]["north_angle_deg"]),
    }
    pset = ifcopenshell.api.pset.add_pset(model, product=building, name="Archy_SchematicMassing")
    ifcopenshell.api.pset.edit_pset(model, pset=pset, properties=values)
    model.header.file_name.name = "model.ifc"
    model.header.file_name.time_stamp = values["ExportedAtUTC"]
    # The single named set is attached to the building; proxies inherit context by containment.
    square = next(u for u in model.by_type("IfcSIUnit") if u.UnitType == "AREAUNIT")
    cubic = next(u for u in model.by_type("IfcSIUnit") if u.UnitType == "VOLUMEUNIT")
    quantities = model.create_entity("IfcElementQuantity", Name="Archy_SchematicQuantities",
        MethodOfMeasurement="task02-analytic-massing-v1; approximate, not net or certified",
        Quantities=[
            model.create_entity("IfcQuantityArea", Name="FootprintArea", Unit=square, AreaValue=area,
                                Formula="canonical footprint shoelace; m2"),
            model.create_entity("IfcQuantityArea", Name="ApproximateGFA", Unit=square,
                                AreaValue=area * v["storeys"], Formula="footprint area * storeys; m2"),
            model.create_entity("IfcQuantityVolume", Name="MassVolume", Unit=cubic,
                                VolumeValue=area * v["storeys"] * v["floor_to_floor_m"],
                                Formula="footprint area * total height; m3")])
    model.create_entity("IfcRelDefinesByProperties", RelatedObjects=[building],
                        RelatingPropertyDefinition=quantities)
