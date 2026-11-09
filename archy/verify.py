"""Verify an Archy workflow output directory against its artifact manifest."""

import argparse
import hashlib
import json
from itertools import permutations
import math
from pathlib import Path
from typing import Any

from .comparison import _is_png, render_html
from .geometry import BuildableArea, Footprint

MAX_JSON_BYTES = 64 * 1024 * 1024
MAX_MANIFEST_BYTES = 16 * 1024 * 1024
MAX_FLOAT32_COORDINATE_TOLERANCE_M = 1e-4


def _coordinate_close(actual: float, expected: float, *, float32: bool) -> bool:
    tolerance = 1e-6
    if float32:
        scaled = max(abs(actual), abs(expected)) * 1e-7
        tolerance = min(max(tolerance, scaled), MAX_FLOAT32_COORDINATE_TOLERANCE_M)
    return math.isclose(actual, expected, rel_tol=0.0, abs_tol=tolerance)


def _same_points(
    actual: list[tuple[float, float]],
    expected: list[tuple[float, float]],
    *,
    float32: bool,
) -> bool:
    return len(actual) == len(expected) and any(
        all(
            all(_coordinate_close(a, e, float32=float32) for a, e in zip(actual_point, expected_point))
            for actual_point, expected_point in zip(actual, expected_order)
        )
        for expected_order in permutations(expected)
    )


def verify_output(output_dir: Path) -> list[str]:
    output_dir = output_dir.resolve()
    manifest_path = output_dir / "manifest.json"
    manifest = _read_json(manifest_path, "manifest", max_bytes=MAX_MANIFEST_BYTES)
    if manifest.get("contract_version") != 2 or not isinstance(manifest.get("artifacts"), dict):
        raise ValueError("unsupported or malformed Archy artifact manifest")
    artifact_paths = set(manifest["artifacts"])
    if any(not isinstance(path, str) or not isinstance(entry, dict) for path, entry in manifest["artifacts"].items()):
        return ["manifest artifact entries must map string paths to objects"]
    required = {"evaluation.json", "inspection.json", "massing.blend", "massing.ifc", "comparison.json", "comparison.html"}
    if not required.issubset(artifact_paths) or not any(path.startswith("renders/") and path.endswith(".png") for path in artifact_paths):
        return ["manifest is missing required workflow artifacts"]
    failures = []
    for relative, expected in sorted(manifest["artifacts"].items()):
        digest = expected.get("sha256")
        size = expected.get("size_bytes")
        if not isinstance(digest, str) or len(digest) != 64 or any(char not in "0123456789abcdef" for char in digest):
            failures.append(f"invalid SHA-256 digest: {relative}")
            continue
        if isinstance(size, bool) or not isinstance(size, int) or size < 0:
            failures.append(f"invalid artifact size: {relative}")
            continue
        relative_path = Path(relative)
        if relative_path.is_absolute() or ".." in relative_path.parts:
            failures.append(f"unsafe artifact path: {relative}")
            continue
        path = output_dir / relative_path
        try:
            path.resolve().relative_to(output_dir)
        except ValueError:
            failures.append(f"artifact escapes output directory: {relative}")
            continue
        try:
            if not path.is_file():
                raise OSError("not a regular file")
            if path.stat().st_size != size:
                failures.append(f"size mismatch: {relative}")
                continue
            artifact_digest = _sha256_file(path)
        except OSError:
            failures.append(f"missing artifact: {relative}")
            continue
        if artifact_digest != digest:
            failures.append(f"hash mismatch: {relative}")
    if failures:
        return failures
    try:
        evaluation = _read_json(output_dir / "evaluation.json", "evaluation")
        inspection = _read_json(output_dir / "inspection.json", "inspection")
        comparison = _read_json(output_dir / "comparison.json", "comparison")
    except ValueError as error:
        return [str(error)]
    evaluated = _rows(evaluation.get("results"), "evaluation.results")
    inspected = _rows(inspection.get("checks"), "inspection.checks")
    compared = _rows(comparison.get("alternatives"), "comparison.alternatives")
    if evaluated is None or inspected is None or compared is None:
        return ["malformed workflow result rows"]
    ids = set(evaluated)
    if not ids or set(inspected) != ids or set(compared) != ids:
        return ["evaluation, inspection, and comparison alternative IDs differ"]
    source = evaluation.get("source")
    if not isinstance(source, dict) or not isinstance(source.get("sha256"), str):
        return ["evaluation source digest is missing"]
    if (
        comparison.get("source") != source
        or comparison.get("project") != evaluation.get("project")
        or comparison.get("brief") != evaluation.get("brief")
        or comparison.get("units") != evaluation.get("units")
        or comparison.get("site") != evaluation.get("site")
    ):
        return ["comparison brief, project, source digest, units, or site do not match evaluation"]
    if comparison.get("ifc_model") != "massing.ifc":
        return ["comparison IFC model path does not match the workflow artifact"]
    try:
        with (output_dir / "comparison.html").open("rb") as html_file:
            html_bytes = html_file.read(MAX_JSON_BYTES + 1)
        if len(html_bytes) > MAX_JSON_BYTES:
            return ["comparison HTML exceeds the verifier size limit"]
        html = html_bytes.decode("utf-8")
    except (OSError, UnicodeDecodeError) as error:
        return [f"cannot read comparison HTML: {error}"]
    try:
        canonical_html = render_html(comparison)
    except (KeyError, TypeError, AttributeError) as error:
        return [f"comparison JSON cannot render HTML: {error}"]
    if html != canonical_html:
        return ["comparison HTML does not match comparison JSON"]
    for alt_id, result in evaluated.items():
        check = inspected[alt_id]
        compared_row = compared[alt_id]
        if check.get("matches_input") is not True:
            return [f"alternative {alt_id} geometry inspection failed"]
        if (
            compared_row.get("valid") != result.get("valid")
            or compared_row.get("constraints") != result.get("constraints")
            or compared_row.get("violations") != result.get("violations")
        ):
            return [f"alternative {alt_id} constraints or violations differ from evaluation"]
        if compared_row.get("input") != result.get("input"):
            return [f"alternative {alt_id} comparison input differs from evaluation"]
        inputs = result.get("input")
        measurements = result.get("measurements", {})
        if not isinstance(inputs, dict) or not isinstance(inputs.get("footprint"), dict) or not isinstance(measurements, dict):
            return [f"alternative {alt_id} has malformed input or measurements"]
        expected = inputs["footprint"]
        try:
            geometry = Footprint.from_mapping(expected)
            width = geometry.width_m
            depth = geometry.depth_m
            floors = inputs["floor_count"]
            floor_height = _finite_positive(inputs["floor_to_floor_height_m"])
            if isinstance(floors, bool) or not isinstance(floors, int) or floors < 1:
                raise ValueError("invalid floor count")
            origin_x = geometry.origin_x_m
            origin_y = geometry.origin_y_m
        except (KeyError, TypeError, ValueError):
            return [f"alternative {alt_id} has malformed input values"]
        computed = {
            "footprint_area_m2": width * depth,
            "gross_floor_area_m2": width * depth * floors,
            "building_height_m": floor_height * floors,
        }
        if any(not _close(measurements.get(key), value) for key, value in computed.items()):
            return [f"alternative {alt_id} measurements do not reproduce from input"]
        site = evaluation.get("site")
        if not isinstance(site, dict):
            return ["evaluation site is malformed"]
        try:
            max_footprint_area = _finite_positive(site.get("max_footprint_area_m2"))
            target_gross_floor_area = _finite_positive(site.get("target_gross_floor_area_m2"))
            site_width = _finite_positive(site.get("width_m"))
            site_depth = _finite_positive(site.get("depth_m"))
        except ValueError:
            return ["evaluation site dimensions or area limits are malformed"]
        brief = evaluation.get("brief")
        brief_site = brief.get("site") if isinstance(brief, dict) else None
        if not isinstance(brief_site, dict):
            return ["evaluation brief site is malformed"]
        if ("max_building_height_m" in site) != ("max_building_height_m" in brief_site):
            return ["evaluation building height limit does not match its brief"]
        max_building_height = None
        if "max_building_height_m" in brief_site:
            try:
                max_building_height = _finite_positive(brief_site["max_building_height_m"])
                site_max_height = _finite_positive(site.get("max_building_height_m"))
            except ValueError:
                return ["evaluation maximum building height is malformed"]
            if not _close(site_max_height, max_building_height):
                return ["evaluation building height limit does not match its brief"]
        constraints = result.get("constraints", [])
        expected_constraints = {
            "footprint_area_maximum": (computed["footprint_area_m2"], "<=", max_footprint_area, "measured_m2", "limit_m2"),
            "gross_floor_area_minimum": (computed["gross_floor_area_m2"], ">=", target_gross_floor_area, "measured_m2", "limit_m2"),
        }
        if max_building_height is not None:
            expected_constraints["building_height_maximum"] = (
                computed["building_height_m"], "<=", max_building_height, "measured_m", "limit_m"
            )
        if (
            not isinstance(constraints, list)
            or any(not isinstance(item, dict) for item in constraints)
            or {item.get("code") for item in constraints} != set(expected_constraints)
            or len(constraints) != len(expected_constraints)
        ):
            return [f"alternative {alt_id} constraint set is incomplete"]
        for constraint in constraints:
            measured, operator, limit, measured_field, limit_field = expected_constraints[constraint["code"]]
            if not _close(constraint.get(measured_field), measured) or constraint.get("operator") != operator or not _close(constraint.get(limit_field), limit):
                return [f"alternative {alt_id} constraint values do not reproduce"]
            passed = measured <= limit if operator == "<=" else measured >= limit
            if constraint.get("passed") is not passed:
                return [f"alternative {alt_id} constraint result is incorrect"]
        expected_violation_codes = set()
        if computed["footprint_area_m2"] > max_footprint_area:
            expected_violation_codes.add("footprint_area_exceeds_maximum")
        if computed["gross_floor_area_m2"] < target_gross_floor_area:
            expected_violation_codes.add("gross_floor_area_below_target")
        if max_building_height is not None and computed["building_height_m"] > max_building_height:
            expected_violation_codes.add("building_height_exceeds_maximum")
        setbacks = site.get("setbacks_m")
        if not isinstance(setbacks, dict):
            return [f"alternative {alt_id} site setbacks are malformed"]
        try:
            allowed_bounds = {
                "west_m": _finite_nonnegative(setbacks["west"]),
                "east_m": site_width - _finite_nonnegative(setbacks["east"]),
                "south_m": _finite_nonnegative(setbacks["south"]),
                "north_m": site_depth - _finite_nonnegative(setbacks["north"]),
            }
        except (KeyError, TypeError, ValueError):
            return [f"alternative {alt_id} site setbacks are malformed"]
        if allowed_bounds["west_m"] >= allowed_bounds["east_m"] or allowed_bounds["south_m"] >= allowed_bounds["north_m"]:
            return [f"alternative {alt_id} site setbacks leave no buildable area"]
        measured_bounds = geometry.bounds().as_mapping()
        try:
            buildable_area = BuildableArea(
                west_m=allowed_bounds["west_m"], south_m=allowed_bounds["south_m"],
                east_m=allowed_bounds["east_m"], north_m=allowed_bounds["north_m"],
            )
        except ValueError:
            return [f"alternative {alt_id} site setbacks are malformed"]
        outside = not buildable_area.contains(geometry)
        if outside:
            expected_violation_codes.add("footprint_outside_buildable_area")
        violations = result.get("violations")
        violation_codes = {item.get("code") for item in violations if isinstance(item, dict)} if isinstance(violations, list) else set()
        if violation_codes != expected_violation_codes or not isinstance(violations, list) or len(violations) != len(violation_codes):
            return [f"alternative {alt_id} violations do not match its constraints and setbacks"]
        for violation in violations:
            if violation["code"] == "footprint_outside_buildable_area":
                if violation.get("measured") != measured_bounds or violation.get("allowed") != allowed_bounds:
                    return [f"alternative {alt_id} setback violation bounds do not reproduce"]
                if "rotation_degrees" in expected and violation.get("measured_polygon") != [
                    {"x_m": point.x_m, "y_m": point.y_m} for point in geometry.corners()
                ]:
                    return [f"alternative {alt_id} setback violation polygon does not reproduce"]
                if "rotation_degrees" not in expected and "measured_polygon" in violation:
                    return [f"alternative {alt_id} legacy setback violation payload changed"]
            elif violation["code"] == "footprint_area_exceeds_maximum":
                if not _close(violation.get("measured_m2"), computed["footprint_area_m2"]) or not _close(violation.get("allowed_m2"), max_footprint_area):
                    return [f"alternative {alt_id} footprint violation values do not reproduce"]
            elif violation["code"] == "gross_floor_area_below_target":
                if not _close(violation.get("measured_m2"), computed["gross_floor_area_m2"]) or not _close(violation.get("allowed_m2"), target_gross_floor_area):
                    return [f"alternative {alt_id} gross floor area violation values do not reproduce"]
            elif violation["code"] == "building_height_exceeds_maximum":
                if max_building_height is None or not _close(violation.get("measured_m"), computed["building_height_m"]) or not _close(violation.get("allowed_m"), max_building_height):
                    return [f"alternative {alt_id} building height violation values do not reproduce"]
        if result.get("valid") is not (not expected_violation_codes):
            return [f"alternative {alt_id} validity does not match constraints"]
        bounds = check.get("bounds", {})
        expected_bounds = {
            "min_x_m": geometry.bounds().min_x_m,
            "max_x_m": geometry.bounds().max_x_m,
            "min_y_m": geometry.bounds().min_y_m,
            "max_y_m": geometry.bounds().max_y_m,
            "min_z_m": 0.0,
            "max_z_m": computed["building_height_m"],
        }
        has_rotation = "rotation_degrees" in expected
        if any(
            isinstance(bounds.get(key), bool)
            or not isinstance(bounds.get(key), (int, float))
            or not math.isfinite(bounds[key])
            or not _coordinate_close(bounds[key], value, float32=has_rotation)
            for key, value in expected_bounds.items()
        ):
            return [f"alternative {alt_id} geometry bounds do not match evaluation"]
        if "rotation_degrees" in expected:
            corners = check.get("corners")
            expected_corners = [(point.x_m, point.y_m) for point in geometry.corners()]
            if not isinstance(corners, list) or len(corners) != 4:
                return [f"alternative {alt_id} transformed geometry corners are missing"]
            try:
                actual_corners = [
                    (point["x_m"], point["y_m"])
                    for point in corners
                    if isinstance(point, dict)
                ]
            except (KeyError, TypeError, ValueError):
                return [f"alternative {alt_id} transformed geometry corners are malformed"]
            if len(actual_corners) != 4 or any(
                isinstance(coordinate, bool) or not isinstance(coordinate, (int, float)) or not math.isfinite(coordinate)
                for point in actual_corners for coordinate in point
            ):
                return [f"alternative {alt_id} transformed geometry corners are malformed"]
            if not _same_points(actual_corners, expected_corners, float32=has_rotation):
                return [f"alternative {alt_id} transformed geometry corners do not match evaluation"]
        render = compared_row.get("render")
        if not isinstance(render, str):
            return [f"alternative {alt_id} render path is missing"]
        render_path = Path(render)
        if render_path.is_absolute() or ".." in render_path.parts:
            return [f"alternative {alt_id} has unsafe render path"]
        if render_path.as_posix() not in artifact_paths:
            return [f"alternative {alt_id} render is absent from manifest"]
        render_file = output_dir / render_path
        try:
            render_file.resolve().relative_to(output_dir)
        except ValueError:
            return [f"alternative {alt_id} render escapes output directory"]
        if not _is_png(render_file):
            return [f"alternative {alt_id} render is missing or invalid"]
    try:
        from .ifc_verify import verify as verify_ifc
        ifc_failures = verify_ifc(output_dir / "evaluation.json", output_dir / "massing.ifc")
    except (ImportError, RuntimeError, ValueError) as error:
        return [f"IFC verification unavailable: {error}"]
    if ifc_failures:
        return [f"IFC verification failed: {failure}" for failure in ifc_failures]
    return failures


def _finite_positive(value: Any) -> float:
    if isinstance(value, bool) or not isinstance(value, (int, float)) or not math.isfinite(value) or value <= 0:
        raise ValueError("expected finite positive number")
    return float(value)


def _sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as artifact:
        for chunk in iter(lambda: artifact.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def _finite_nonnegative(value: Any) -> float:
    if isinstance(value, bool) or not isinstance(value, (int, float)) or not math.isfinite(value) or value < 0:
        raise ValueError("expected finite non-negative number")
    return float(value)


def _close(actual: Any, expected: Any) -> bool:
    return (
        isinstance(actual, (int, float))
        and not isinstance(actual, bool)
        and math.isfinite(actual)
        and isinstance(expected, (int, float))
        and math.isfinite(expected)
        and math.isclose(actual, expected, rel_tol=0.0, abs_tol=1e-6)
    )


def _read_json(path: Path, label: str, *, max_bytes: int = MAX_JSON_BYTES) -> dict[str, Any]:
    try:
        if not path.is_file():
            raise OSError("not a regular file")
        with path.open("rb") as json_file:
            json_bytes = json_file.read(max_bytes + 1)
        if len(json_bytes) > max_bytes:
            raise ValueError(f"{label} exceeds the {max_bytes}-byte limit")
        value = json.loads(json_bytes)
    except (OSError, json.JSONDecodeError, UnicodeDecodeError) as error:
        raise ValueError(f"cannot read {label}: {error}") from error
    if not isinstance(value, dict):
        raise ValueError(f"{label} must be a JSON object")
    return value


def _rows(value: Any, label: str) -> dict[str, dict[str, Any]] | None:
    if not isinstance(value, list):
        return None
    rows: dict[str, dict[str, Any]] = {}
    for row in value:
        if not isinstance(row, dict) or not isinstance(row.get("id"), str) or row["id"] in rows:
            return None
        rows[row["id"]] = row
    return rows


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("output_dir", type=Path)
    args = parser.parse_args(argv)
    try:
        failures = verify_output(args.output_dir)
    except (OSError, json.JSONDecodeError, ValueError) as error:
        parser.exit(2, f"archy-verify: error: {error}\n")
    if failures:
        parser.exit(1, "archy-verify: integrity check failed\n" + "\n".join(f"- {failure}" for failure in failures) + "\n")
    print("Archy workflow artifacts verified")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
