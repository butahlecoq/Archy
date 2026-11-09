"""Export an Archy evaluation report as a simple IFC4 massing model."""

import argparse
import json
import math
import os
from pathlib import Path
import re
import tempfile
from typing import Any

from .geometry import Footprint

MINIMUM_BUILDING_EXTRUSION_DEPTH_M = 1e-4


class IfcExportError(ValueError):
    """Invalid report or unavailable IFC export capability."""


def require_ifc_support() -> None:
    """Check the IFC backend is importable before a workflow starts Blender."""
    try:
        import ifcopenshell.api  # noqa: F401
        import ifcopenshell.geom  # noqa: F401
    except ImportError as error:
        raise IfcExportError("IfcOpenShell API and geometry support are required; install it in the Archy Python environment") from error


def export_evaluation(report: dict[str, Any], output: Path) -> Path:
    validate_evaluation(report)
    try:
        import ifcopenshell.api
    except ImportError as error:
        raise IfcExportError("IfcOpenShell is required; install it in the Archy Python environment") from error
    try:
        model = ifcopenshell.api.run("project.create_file", version="IFC4")
        project = ifcopenshell.api.run("root.create_entity", model, ifc_class="IfcProject", name=report["project"]["name"])
        ifcopenshell.api.run("unit.assign_unit", model, units=[
            ifcopenshell.api.run("unit.add_si_unit", model, unit_type="LENGTHUNIT"),
            ifcopenshell.api.run("unit.add_si_unit", model, unit_type="AREAUNIT"),
            ifcopenshell.api.run("unit.add_si_unit", model, unit_type="VOLUMEUNIT"),
        ])
        context = ifcopenshell.api.run("context.add_context", model, context_type="Model")
        body = ifcopenshell.api.run("context.add_context", model, context_type="Model", context_identifier="Body", target_view="MODEL_VIEW", parent=context)
        site = ifcopenshell.api.run("root.create_entity", model, ifc_class="IfcSite", name="Archy Site")
        ifcopenshell.api.run("aggregate.assign_object", model, products=[site], relating_object=project)

        for row in report["results"]:
            alt_id = row["id"]
            inputs = row["input"]
            measurements = row["measurements"]
            building = ifcopenshell.api.run("root.create_entity", model, ifc_class="IfcBuilding", name=f"Alternative {alt_id}")
            ifcopenshell.api.run("aggregate.assign_object", model, products=[building], relating_object=site)
            alternative_pset = ifcopenshell.api.run("pset.add_pset", model, product=building, name="ArchyAlternative")
            ifcopenshell.api.run("pset.edit_pset", model, pset=alternative_pset, properties={
                "AlternativeId": alt_id,
                "SourceBriefSHA256": report["source"]["sha256"],
                "FloorCount": inputs["floor_count"],
                "FloorToFloorHeight_m": inputs["floor_to_floor_height_m"],
                "BuildingHeight_m": measurements["building_height_m"],
                "SpatialStructureVersion": 1,
            })
            storeys = []
            for level_index in range(1, inputs["floor_count"] + 1):
                storey = ifcopenshell.api.run(
                    "root.create_entity", model,
                    ifc_class="IfcBuildingStorey",
                    name=f"Level {level_index}",
                )
                storey.Elevation = (level_index - 1) * inputs["floor_to_floor_height_m"]
                storeys.append(storey)
            ifcopenshell.api.run("aggregate.assign_object", model, products=storeys, relating_object=building)

            product = ifcopenshell.api.run("root.create_entity", model, ifc_class="IfcBuildingElementProxy", name=f"Archy mass {alt_id}")
            product.Description = f"ArchyAlternativeId={alt_id}"
            footprint = inputs["footprint"]
            profile = ifcopenshell.api.run(
                "profile.add_parameterized_profile", model,
                ifc_class="IfcRectangleProfileDef",
            )
            profile.XDim = footprint["width_m"]
            profile.YDim = footprint["depth_m"]
            profile.Position = model.createIfcAxis2Placement2D(
                Location=model.createIfcCartesianPoint((footprint["width_m"] / 2, footprint["depth_m"] / 2))
            )
            representation = ifcopenshell.api.run(
                "geometry.add_profile_representation", model,
                context=body,
                profile=profile,
                depth=measurements["building_height_m"],
            )
            ifcopenshell.api.run("geometry.assign_representation", model, product=product, representation=representation)
            geometry = Footprint.from_mapping(footprint)
            matrix = _placement_matrix(geometry.origin_x_m, geometry.origin_y_m, 0.0, geometry.rotation_degrees)
            ifcopenshell.api.run("geometry.edit_object_placement", model, product=product, matrix=matrix, is_si=True)
            pset = ifcopenshell.api.run("pset.add_pset", model, product=product, name="ArchyMassing")
            properties = {
                "AlternativeId": alt_id,
                "SourceBriefSHA256": report["source"]["sha256"],
                **footprint,
                "rotation_degrees": geometry.rotation_degrees,
                "FloorCount": inputs["floor_count"],
                "FloorToFloorHeight_m": inputs["floor_to_floor_height_m"],
                **measurements,
            }
            ifcopenshell.api.run("pset.edit_pset", model, pset=pset, properties=properties)
            ifcopenshell.api.run("spatial.assign_container", model, products=[product], relating_structure=building)
        output.parent.mkdir(parents=True, exist_ok=True)
        with tempfile.NamedTemporaryFile(prefix=f".{output.name}.", suffix=".tmp", dir=output.parent, delete=False) as temporary:
            temporary_path = Path(temporary.name)
        try:
            model.write(str(temporary_path))
            os.replace(temporary_path, output)
        finally:
            temporary_path.unlink(missing_ok=True)
    except Exception as error:
        raise IfcExportError(f"IFC generation failed: {error}") from error
    return output


def _placement_matrix(x: float, y: float, z: float, rotation_degrees: float = 0.0):
    import numpy as np
    matrix = np.eye(4)
    angle = math.radians(rotation_degrees)
    matrix[:2, :2] = ((math.cos(angle), -math.sin(angle)), (math.sin(angle), math.cos(angle)))
    matrix[:3, 3] = (x, y, z)
    return matrix


def validate_evaluation(report: Any) -> None:
    """Validate an evaluation report for IFC export, including its extrusion depth."""
    if not isinstance(report, dict) or report.get("contract_version") != 1 or report.get("units") != {"length": "m", "area": "m2"}:
        raise IfcExportError("expected evaluation contract version 1 in metres")
    if not isinstance(report.get("project"), dict) or not isinstance(report.get("project", {}).get("name"), str):
        raise IfcExportError("evaluation project name is missing")
    digest = report.get("source", {}).get("sha256") if isinstance(report.get("source"), dict) else None
    if not isinstance(digest, str) or re.fullmatch(r"[0-9a-f]{64}", digest) is None:
        raise IfcExportError("evaluation source SHA-256 is missing or malformed")
    rows = report.get("results")
    if not isinstance(rows, list) or not rows:
        raise IfcExportError("evaluation results must be a non-empty list")
    seen = set()
    for row in rows:
        if not isinstance(row, dict) or not isinstance(row.get("id"), str) or not row["id"].strip() or row["id"] in seen:
            raise IfcExportError("alternative IDs must be unique non-empty strings")
        seen.add(row["id"])
        try:
            footprint = row["input"]["footprint"]
            geometry = Footprint.from_mapping(footprint)
            values = [footprint[key] for key in ("width_m", "depth_m", "origin_x_m", "origin_y_m")]
            values += [row["input"]["floor_to_floor_height_m"], row["measurements"]["building_height_m"], row["measurements"]["footprint_area_m2"], row["measurements"]["gross_floor_area_m2"]]
            floors = row["input"]["floor_count"]
        except (KeyError, TypeError, ValueError) as error:
            raise IfcExportError(f"alternative {row['id']} has incomplete input or measurements") from error
        if isinstance(floors, bool) or not isinstance(floors, int) or floors < 1 or any(isinstance(v, bool) or not isinstance(v, (int, float)) or not math.isfinite(v) for v in values):
            raise IfcExportError(f"alternative {row['id']} has invalid numeric data")
        if min(values[0], values[1], values[4], values[5], values[6], values[7]) <= 0 or min(values[2], values[3]) < 0:
            raise IfcExportError(f"alternative {row['id']} has invalid dimensions or origins")
        if values[5] < MINIMUM_BUILDING_EXTRUSION_DEPTH_M:
            raise IfcExportError(
                f"alternative {row['id']} building_height_m must be at least "
                f"{MINIMUM_BUILDING_EXTRUSION_DEPTH_M:g} m for IFC extrusion"
            )
        expected = (
            values[0] * values[1],
            values[0] * values[1] * floors,
            values[4] * floors,
        )
        supplied = (values[6], values[7], values[5])
        labels = ("footprint_area_m2", "gross_floor_area_m2", "building_height_m")
        for label, computed, measured in zip(labels, expected, supplied):
            if not math.isclose(computed, measured, rel_tol=1e-9, abs_tol=1e-9):
                raise IfcExportError(f"alternative {row['id']} {label} is inconsistent with its input")


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("evaluation", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args(argv)
    try:
        report = json.loads(args.evaluation.read_text(encoding="utf-8"))
        path = export_evaluation(report, args.output)
    except (OSError, json.JSONDecodeError, IfcExportError) as error:
        parser.exit(2, f"archy-ifc: error: {error}\n")
    print(path)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
