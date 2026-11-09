"""Combine evaluator, Blender inspection, and preview outputs."""

import argparse
from html import escape
import json
from pathlib import Path
import struct
from typing import Any
import zlib

from .geometry import Footprint

MAX_PNG_BYTES = 64 * 1024 * 1024
MAX_PNG_PIXELS = 16_000_000
MAX_PNG_DECOMPRESSED_BYTES = 128 * 1024 * 1024


def build_comparison(evaluation: dict[str, Any], inspection: dict[str, Any], render_dir: Path, *, ifc_model: str | None = None) -> dict[str, Any]:
    if evaluation.get("contract_version") != 1 or inspection.get("contract_version") != 1:
        raise ValueError("evaluation and inspection must use contract version 1")
    evaluated = {row["id"]: row for row in evaluation.get("results", [])}
    inspected = {row["id"]: row for row in inspection.get("checks", [])}
    if not evaluated or evaluated.keys() != inspected.keys():
        raise ValueError("inspection alternative IDs must exactly match evaluation IDs")
    alternatives = []
    for alt_id, result in evaluated.items():
        render = render_dir / f"{alt_id}.png"
        check = inspected[alt_id]
        if not check.get("matches_input") or not _is_png(render):
            raise ValueError(f"alternative {alt_id} is missing verified geometry or preview render")
        alternatives.append({
            "id": alt_id,
            "valid": result["valid"],
            "violations": result["violations"],
            "constraints": result["constraints"],
            "input": result["input"],
            "measurements": result["measurements"],
            "formulas": result["formulas"],
            "geometry_check": check,
            "render": render.relative_to(render_dir.parent).as_posix(),
        })
    alternatives.sort(key=lambda row: row["id"])
    return {
        "contract_version": 1,
        "project": evaluation["project"],
        "brief": evaluation["brief"],
        "source": evaluation["source"],
        "units": evaluation["units"],
        "site": evaluation["site"],
        **({"ifc_model": ifc_model} if ifc_model is not None else {}),
        "alternatives": alternatives,
    }


def _is_png(path: Path) -> bool:
    try:
        with path.open("rb") as png_file:
            data = png_file.read(MAX_PNG_BYTES + 1)
    except OSError:
        return False
    if len(data) > MAX_PNG_BYTES:
        return False
    if not data.startswith(b"\x89PNG\r\n\x1a\n"):
        return False
    offset = 8
    width = height = 0
    decompressor = zlib.decompressobj()
    inflated_size = 0
    saw_header = saw_data = saw_end = False
    try:
        while offset + 12 <= len(data):
            length = struct.unpack_from(">I", data, offset)[0]
            end = offset + 12 + length
            if end > len(data):
                return False
            kind = data[offset + 4:offset + 8]
            payload = data[offset + 8:offset + 8 + length]
            checksum = struct.unpack_from(">I", data, offset + 8 + length)[0]
            if zlib.crc32(kind + payload) & 0xFFFFFFFF != checksum:
                return False
            if not saw_header:
                if kind != b"IHDR" or length != 13:
                    return False
                width, height, bit_depth, color_type, compression, filter_method, interlace = struct.unpack(">IIBBBBB", payload)
                channels_by_type = {0: 1, 2: 3, 3: 1, 4: 2, 6: 4}
                valid_depths = {0: {1, 2, 4, 8, 16}, 2: {8, 16}, 3: {1, 2, 4, 8}, 4: {8, 16}, 6: {8, 16}}
                if (
                    width == 0 or height == 0 or width * height > MAX_PNG_PIXELS
                    or color_type not in channels_by_type or bit_depth not in valid_depths[color_type]
                    or compression != 0 or filter_method != 0 or interlace != 0
                ):
                    return False
                row_size = (width * channels_by_type[color_type] * bit_depth + 7) // 8 + 1
                expected_size = row_size * height
                if expected_size > MAX_PNG_DECOMPRESSED_BYTES:
                    return False
                saw_header = True
            elif kind == b"IDAT":
                pending = payload
                while True:
                    remaining = expected_size - inflated_size
                    limit = min(1024 * 1024, remaining + 1)
                    inflated = decompressor.decompress(pending, limit)
                    inflated_size += len(inflated)
                    if inflated_size > expected_size:
                        return False
                    pending = decompressor.unconsumed_tail
                    if pending:
                        continue
                    if len(inflated) == limit:
                        pending = b""
                        continue
                    break
                saw_data = True
            elif kind == b"IEND":
                if length != 0 or not saw_data:
                    return False
                saw_end = True
                offset = end
                break
            offset = end
        if not saw_end or offset != len(data):
            return False
        return decompressor.eof and not decompressor.unused_data and inflated_size == expected_size
    except (ValueError, zlib.error, struct.error):
        return False


def render_html(comparison: dict[str, Any]) -> str:
    site = comparison["site"]
    setbacks = site["setbacks_m"]
    site_summary = (
        f"{site['width_m']:g} × {site['depth_m']:g} m; "
        f"setbacks N {setbacks['north']:g}, E {setbacks['east']:g}, "
        f"S {setbacks['south']:g}, W {setbacks['west']:g} m; "
        f"maximum footprint {site['max_footprint_area_m2']:g} m²; "
        f"target gross floor area {site['target_gross_floor_area_m2']:g} m²"
    )
    if "max_building_height_m" in site:
        site_summary += f"; maximum building height {site['max_building_height_m']:g} m"
    source_digest = escape(comparison["source"]["sha256"])
    rows = []
    for alternative in comparison["alternatives"]:
        status = "Pass" if alternative["valid"] else "Fail"
        violations = escape(", ".join(item["code"] for item in alternative["violations"]) or "None")
        alt_id = escape(alternative["id"])
        footprint = alternative["input"]["footprint"]
        geometry = Footprint.from_mapping(footprint)
        bounds = geometry.bounds()
        has_rotation = "rotation_degrees" in footprint
        if has_rotation:
            world_min_x = min(0.0, bounds.min_x_m)
            world_max_x = max(site["width_m"], bounds.max_x_m)
            world_min_y = min(0.0, bounds.min_y_m)
            world_max_y = max(site["depth_m"], bounds.max_y_m)
            view_width = world_max_x - world_min_x
            view_depth = world_max_y - world_min_y
        else:
            # Preserve the historical archived-report layout and arithmetic.
            world_min_x = world_min_y = 0.0
            world_max_x = max(site["width_m"], footprint["origin_x_m"] + footprint["width_m"])
            world_max_y = max(site["depth_m"], footprint["origin_y_m"] + footprint["depth_m"])
            view_width = world_max_x
            view_depth = world_max_y
        scale = min(360 / view_width, 260 / view_depth)
        view_height = view_depth * scale
        plan_width = site["width_m"] * scale
        plan_height = site["depth_m"] * scale
        plan_left = 60 + (360 - view_width * scale) / 2 - world_min_x * scale
        plan_top = 28 + (260 - view_height) / 2
        if has_rotation:
            display_max_y = world_max_y + (260 - view_height) / (2 * scale)
            plan_bottom = plan_top + display_max_y * scale
            site_top = plan_top + (display_max_y - site["depth_m"]) * scale
        else:
            display_max_y = plan_top + view_height / scale
            plan_bottom = plan_top + view_height
            site_top = plan_bottom - plan_height
        build_left = plan_left + setbacks["west"] * scale
        if has_rotation:
            build_top = plan_top + (display_max_y - site["depth_m"] + setbacks["north"]) * scale
        else:
            build_top = site_top + setbacks["north"] * scale
        points = geometry.corners()
        svg_points = " ".join(f"{plan_left + point.x_m * scale:g},{plan_top + (display_max_y - point.y_m) * scale:g}" for point in points)
        footprint_summary = (
            f"Footprint {footprint['width_m']:g} × {footprint['depth_m']:g} m at "
            f"E {footprint['origin_x_m']:g}, N {footprint['origin_y_m']:g} m"
        )
        if has_rotation:
            footprint_summary += f"; rotated {geometry.rotation_degrees:g}° counter-clockwise"
            footprint_shape = f'<polygon points="{svg_points}" class="footprint"/>'
            rotation_description = f' rotated {geometry.rotation_degrees:g} degrees counter-clockwise'
        else:
            # Keep the historical HTML byte-for-byte for archived comparison JSON.
            footprint_shape = (
                f'<rect x="{plan_left + footprint["origin_x_m"] * scale:g}" '
                f'y="{plan_bottom - (footprint["origin_y_m"] + footprint["depth_m"]) * scale:g}" '
                f'width="{footprint["width_m"] * scale:g}" height="{footprint["depth_m"] * scale:g}" class="footprint"/>'
            )
            rotation_description = ""
        plan_svg = (
            f'<svg class="site-plan" viewBox="0 0 480 320" role="img" '
            f'aria-labelledby="plan-title-{alt_id}" aria-describedby="plan-desc-{alt_id}" '
            f'xmlns="http://www.w3.org/2000/svg">'
            f'<title id="plan-title-{alt_id}">{alt_id} site plan</title>'
            f'<desc id="plan-desc-{alt_id}">Site {site["width_m"]:g} by {site["depth_m"]:g} metres. '
            f'Buildable envelope after north {setbacks["north"]:g}, east {setbacks["east"]:g}, '
            f'south {setbacks["south"]:g}, and west {setbacks["west"]:g} metre setbacks. '
            f'Footprint {footprint["width_m"]:g} by {footprint["depth_m"]:g} metres{rotation_description} at '
            f'east {footprint["origin_x_m"]:g}, north {footprint["origin_y_m"]:g} metres from the southwest corner. '
            'North is up and east is right.</desc>'
            f'<rect x="{plan_left:g}" y="{site_top:g}" width="{plan_width:g}" height="{plan_height:g}" class="site-boundary"/>'
            f'<rect x="{build_left:g}" y="{build_top:g}" width="{(site["width_m"] - setbacks["west"] - setbacks["east"]) * scale:g}" '
            f'height="{(site["depth_m"] - setbacks["north"] - setbacks["south"]) * scale:g}" class="buildable-area"/>'
            f'{footprint_shape}'
            f'<text x="440" y="38" class="axis">N ↑</text><text x="440" y="58" class="axis">E →</text>'
            f'<text x="{plan_left:g}" y="{plan_bottom + 18:g}" class="dimension">Site {site["width_m"]:g} × {site["depth_m"]:g} m</text>'
            '</svg>'
        )
        constraints = "<br>".join(
            (
                f"{escape(item['code'])}: {item['measured_m']:.1f} {escape(item['operator'])} {item['limit_m']:.1f} m"
                if item["code"] == "building_height_maximum"
                else f"{escape(item['code'])}: {item['measured_m2']:.1f} {escape(item['operator'])} {item['limit_m2']:.1f} m²"
            )
            for item in alternative["constraints"]
        )
        rows.append(
            "<tr>"
            f"<th>{alt_id}</th>"
            f"<td>{plan_svg}<p class=\"site-plan-caption\">{escape(footprint_summary)}</p></td>"
            f"<td><img src=\"{escape(alternative['render'], quote=True)}\" alt=\"{alt_id} massing preview\"></td>"
            f"<td>{alternative['measurements']['footprint_area_m2']:.1f} m²</td>"
            f"<td>{alternative['measurements']['gross_floor_area_m2']:.1f} m²</td>"
            f"<td>{alternative['measurements']['building_height_m']:.1f} m</td>"
            f"<td class=\"{'pass' if alternative['valid'] else 'fail'}\">{status}<br>{constraints}</td>"
            f"<td>{violations}</td></tr>"
        )
    ifc_link = f'<p><a href="{escape(comparison["ifc_model"], quote=True)}">Download IFC model</a></p>' if comparison.get("ifc_model") else ""
    return """<!doctype html>
<html lang="en"><meta charset="utf-8"><meta name="viewport" content="width=device-width">
<title>Archy massing comparison</title>
<style>body{font:16px system-ui,sans-serif;margin:2rem;color:#1e293b}h1{margin-bottom:.2rem}table{border-collapse:collapse;width:100%}th,td{border:1px solid #cbd5e1;padding:.7rem;text-align:left;vertical-align:middle}img{width:min(32vw,360px);height:auto;background:#111827}.site-plan{display:block;width:min(34vw,420px);height:auto}.site-plan-caption{font-size:.875rem;margin:.25rem 0}.site-boundary{fill:#fff;stroke:#334155;stroke-width:2}.buildable-area{fill:#dcfce7;stroke:#15803d;stroke-width:1.5;stroke-dasharray:5 3}.footprint{fill:#2563eb;fill-opacity:.82;stroke:#1e3a8a;stroke-width:1.5}.axis{font:14px system-ui,sans-serif;fill:#0f172a}.dimension{font:12px system-ui,sans-serif;fill:#334155}.pass{color:#166534;font-weight:700}.fail{color:#b91c1c;font-weight:700}</style>
<h1>Archy massing comparison</h1>
<p>""" + escape(comparison["project"]["name"]) + "</p>" + ifc_link + "<p><strong>Site:</strong> " + escape(site_summary) + "</p><p><strong>Brief SHA-256:</strong> <code>" + source_digest + "</code></p><table><thead><tr><th>Alternative</th><th>Site plan</th><th>Preview</th><th>Footprint</th><th>Gross floor area</th><th>Height</th><th>Constraint results</th><th>Violations</th></tr></thead><tbody>" + "".join(rows) + "</tbody></table></html>\n"


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("evaluation", type=Path)
    parser.add_argument("inspection", type=Path)
    parser.add_argument("render_dir", type=Path)
    parser.add_argument("output", type=Path)
    parser.add_argument("--html", type=Path, help="also write a visual HTML report")
    args = parser.parse_args(argv)
    evaluation = json.loads(args.evaluation.read_text(encoding="utf-8"))
    inspection = json.loads(args.inspection.read_text(encoding="utf-8"))
    comparison = build_comparison(evaluation, inspection, args.render_dir)
    args.output.write_text(json.dumps(comparison, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    if args.html:
        args.html.write_text(render_html(comparison), encoding="utf-8")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
