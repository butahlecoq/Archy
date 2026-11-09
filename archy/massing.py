"""Validation and measurement for rectangular massing alternatives."""

from dataclasses import dataclass
import hashlib
import json
import math
from pathlib import Path
import re
import sys
from typing import Any

import yaml
from .geometry import BuildableArea, Footprint
try:
    from jsonschema import Draft202012Validator
except ImportError:
    Draft202012Validator = None


class BriefError(ValueError):
    """The input brief is malformed or outside the supported domain."""


def _schema_path() -> Path:
    """Locate the checked-in schema or its installed data-files copy."""
    candidates = (
        Path(__file__).resolve().parent.parent / "docs" / "massing-brief.schema.json",
        Path(sys.prefix) / "share" / "archy" / "schemas" / "massing-brief.schema.json",
    )
    for candidate in candidates:
        if candidate.is_file():
            return candidate
    raise BriefError("massing brief schema is not installed")


def _validate_schema(data: Any) -> None:
    if Draft202012Validator is None:
        return
    try:
        schema = json.loads(_schema_path().read_text(encoding="utf-8"))
        Draft202012Validator.check_schema(schema)
        error = next(iter(sorted(Draft202012Validator(schema).iter_errors(data), key=lambda item: (list(map(str, item.absolute_path)), item.message))), None)
    except (OSError, json.JSONDecodeError, ValueError) as error:
        raise BriefError(f"cannot load massing brief schema: {error}") from error
    if error is not None:
        field = ".".join(str(part) for part in error.absolute_path) or "brief"
        raise BriefError(f"{field}: {error.message}")


@dataclass(frozen=True)
class Site:
    width: float
    depth: float
    north: float
    east: float
    south: float
    west: float
    max_footprint_area: float
    target_gross_floor_area: float
    max_building_height: float | None = None


@dataclass(frozen=True)
class Alternative:
    id: str
    footprint: Footprint
    floor_count: int
    floor_to_floor_height: float


def _number(value: Any, field: str, *, positive: bool = True) -> float:
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        raise BriefError(f"{field} must be a number")
    result = float(value)
    if not math.isfinite(result) or (positive and result <= 0):
        raise BriefError(f"{field} must be finite and {'positive' if positive else 'non-negative'}")
    if not positive and result < 0:
        raise BriefError(f"{field} must be non-negative")
    return result


def _mapping(value: Any, field: str) -> dict[str, Any]:
    if not isinstance(value, dict):
        raise BriefError(f"{field} must be a mapping")
    return value


def _keys(value: dict[str, Any], required: set[str], optional: set[str], field: str) -> None:
    missing = required - value.keys()
    extra = value.keys() - required - optional
    if missing:
        raise BriefError(f"{field} is missing fields: {', '.join(sorted(missing))}")
    if extra:
        raise BriefError(f"{field} has unknown fields: {', '.join(sorted(extra))}")


def _parse_brief(data: Any) -> tuple[dict[str, str], Site, list[Alternative]]:
    root = _mapping(data, "brief")
    _keys(root, {"schema_version", "contract_version", "project", "units", "coordinate_system", "site", "alternatives"}, set(), "brief")
    if root.get("schema_version") != 1 or root.get("contract_version") != 1:
        raise BriefError("schema_version and contract_version must both be 1")
    project = _mapping(root["project"], "project")
    _keys(project, {"id", "name"}, set(), "project")
    for field in ("id", "name"):
        if not isinstance(project[field], str) or not project[field].strip():
            raise BriefError(f"project.{field} must be a non-empty string")
    if root.get("units") != {"length": "m", "area": "m2"}:
        raise BriefError("units must be metres and square metres")
    coordinate_system = _mapping(root["coordinate_system"], "coordinate_system")
    _keys(coordinate_system, {"origin", "x_axis", "y_axis", "z_axis"}, set(), "coordinate_system")
    if coordinate_system != {"origin": "southwest_site_corner", "x_axis": "east", "y_axis": "north", "z_axis": "up"}:
        raise BriefError("coordinate_system must use southwest_site_corner, east, north, and up")
    site_data = _mapping(root.get("site"), "site")
    _keys(
        site_data,
        {"width_m", "depth_m", "setbacks_m", "max_footprint_area_m2", "target_gross_floor_area_m2"},
        {"max_building_height_m"},
        "site",
    )
    setbacks = _mapping(site_data.get("setbacks_m"), "site.setbacks_m")
    _keys(setbacks, {"north", "east", "south", "west"}, set(), "site.setbacks_m")
    site = Site(
        _number(site_data.get("width_m"), "site.width_m"),
        _number(site_data.get("depth_m"), "site.depth_m"),
        _number(setbacks.get("north"), "setbacks.north", positive=False),
        _number(setbacks.get("east"), "setbacks.east", positive=False),
        _number(setbacks.get("south"), "setbacks.south", positive=False),
        _number(setbacks.get("west"), "setbacks.west", positive=False),
        _number(site_data.get("max_footprint_area_m2"), "site.max_footprint_area_m2"),
        _number(site_data.get("target_gross_floor_area_m2"), "site.target_gross_floor_area_m2"),
        _number(site_data["max_building_height_m"], "site.max_building_height_m")
        if "max_building_height_m" in site_data
        else None,
    )
    if site.west + site.east >= site.width or site.south + site.north >= site.depth:
        raise BriefError("setbacks must leave a positive buildable rectangle")
    raw_alternatives = root.get("alternatives")
    if not isinstance(raw_alternatives, list) or not raw_alternatives:
        raise BriefError("alternatives must be a non-empty list")
    alternatives = []
    seen_ids = set()
    for index, raw in enumerate(raw_alternatives):
        item = _mapping(raw, f"alternatives[{index}]")
        _keys(item, {"id", "footprint", "floor_count", "floor_to_floor_height_m"}, set(), f"alternatives[{index}]")
        alt_id = item.get("id")
        if not isinstance(alt_id, str) or re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9_-]*", alt_id) is None or alt_id in seen_ids:
            raise BriefError(f"alternatives[{index}].id must be a unique path-safe identifier")
        seen_ids.add(alt_id)
        footprint = _mapping(item.get("footprint"), f"alternatives[{index}].footprint")
        _keys(footprint, {"width_m", "depth_m", "origin_x_m", "origin_y_m"}, {"rotation_degrees"}, f"alternatives[{index}].footprint")
        try:
            parsed_footprint = Footprint.from_mapping(footprint)
        except ValueError as error:
            raise BriefError(f"{alt_id}.footprint: {error}") from error
        floors = item.get("floor_count")
        if isinstance(floors, bool) or not isinstance(floors, int) or floors < 1:
            raise BriefError(f"alternatives[{index}].floor_count must be a positive integer")
        alternatives.append(Alternative(
            alt_id,
            parsed_footprint,
            floors,
            _number(item.get("floor_to_floor_height_m"), f"{alt_id}.floor_to_floor_height_m"),
        ))
    return {"id": project["id"], "name": project["name"]}, site, alternatives


def load_brief(path: str | Path) -> tuple[dict[str, str], Site, list[Alternative]]:
    """Load and validate the supported YAML brief structure."""
    try:
        source_bytes = Path(path).read_bytes()
        data = yaml.safe_load(source_bytes)
    except (OSError, UnicodeDecodeError, yaml.YAMLError) as error:
        raise BriefError(f"cannot read brief: {error}") from error
    _validate_schema(data)
    return _parse_brief(data)


def evaluate_brief(path: str | Path) -> dict[str, Any]:
    """Return stable validation and measurement results for every alternative."""
    path = Path(path)
    try:
        source_bytes = path.read_bytes()
        data = yaml.safe_load(source_bytes)
    except (OSError, UnicodeDecodeError, yaml.YAMLError) as error:
        raise BriefError(f"cannot read brief: {error}") from error
    _validate_schema(data)
    project, site, alternatives = _parse_brief(data)
    results = []
    for alt in alternatives:
        footprint = alt.footprint
        area = footprint.width_m * footprint.depth_m
        gross = area * alt.floor_count
        height = alt.floor_to_floor_height * alt.floor_count
        violations = []
        buildable_bounds = {
            "west_m": site.west,
            "east_m": site.width - site.east,
            "south_m": site.south,
            "north_m": site.depth - site.north,
        }
        measured_bounds = footprint.bounds().as_mapping()
        buildable_area = BuildableArea(
            west_m=buildable_bounds["west_m"],
            south_m=buildable_bounds["south_m"],
            east_m=buildable_bounds["east_m"],
            north_m=buildable_bounds["north_m"],
        )
        inside_buildable_area = buildable_area.contains(footprint)
        constraints = [
            {"code": "footprint_area_maximum", "measured_m2": area, "operator": "<=", "limit_m2": site.max_footprint_area, "passed": area <= site.max_footprint_area},
            {"code": "gross_floor_area_minimum", "measured_m2": gross, "operator": ">=", "limit_m2": site.target_gross_floor_area, "passed": gross >= site.target_gross_floor_area},
        ]
        if site.max_building_height is not None:
            constraints.append({
                "code": "building_height_maximum",
                "measured_m": height,
                "operator": "<=",
                "limit_m": site.max_building_height,
                "passed": height <= site.max_building_height,
            })
        if area > site.max_footprint_area:
            violations.append({"code": "footprint_area_exceeds_maximum", "measured_m2": area, "allowed_m2": site.max_footprint_area})
        if gross < site.target_gross_floor_area:
            violations.append({"code": "gross_floor_area_below_target", "measured_m2": gross, "allowed_m2": site.target_gross_floor_area})
        if site.max_building_height is not None and height > site.max_building_height:
            violations.append({
                "code": "building_height_exceeds_maximum",
                "measured_m": height,
                "allowed_m": site.max_building_height,
            })
        if not inside_buildable_area:
            measured_polygon = [
                {"x_m": point.x_m, "y_m": point.y_m}
                for point in footprint.corners()
            ]
            violations.append({"code": "footprint_outside_buildable_area", "measured": measured_bounds, "allowed": buildable_bounds, "measured_polygon": measured_polygon})
        violations.sort(key=lambda violation: violation["code"])
        results.append({"id": alt.id, "valid": not violations, "violations": violations, "constraints": constraints, "input": {"footprint": footprint.as_mapping(), "floor_count": alt.floor_count, "floor_to_floor_height_m": alt.floor_to_floor_height}, "measurements": {"footprint_area_m2": area, "gross_floor_area_m2": gross, "building_height_m": height}, "formulas": {"footprint_area_m2": "width_m * depth_m", "gross_floor_area_m2": "footprint_area_m2 * floor_count", "building_height_m": "floor_to_floor_height_m * floor_count"}})
    return {
        "project": project,
        "brief": data,
        "source": {"sha256": hashlib.sha256(source_bytes).hexdigest()},
        "contract_version": 1,
        "units": {"length": "m", "area": "m2"},
        "site": {
            "width_m": site.width,
            "depth_m": site.depth,
            "setbacks_m": {"north": site.north, "east": site.east, "south": site.south, "west": site.west},
            "max_footprint_area_m2": site.max_footprint_area,
            "target_gross_floor_area_m2": site.target_gross_floor_area,
            **({"max_building_height_m": site.max_building_height} if site.max_building_height is not None else {}),
        },
        "results": results,
    }
