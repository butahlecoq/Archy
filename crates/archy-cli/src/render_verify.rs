//! Independent, bounded parsing of the exact bytes returned by A09 capture.
use archy_adapters::CapturedOutputs;
use archy_core::{ArchyError, ErrorCode, EvaluationReport, Variant};
use std::collections::{BTreeMap, BTreeSet};
use std::io::Cursor;

pub const VERIFIER_VERSION: &str = "archy-rust-png-glb-v1";

fn invalid(reason: impl Into<String>) -> ArchyError {
    ArchyError::new(ErrorCode::ArtifactInvalid, "render", reason)
}
fn near(a: f64, b: f64, tol: f64) -> Result<(), ArchyError> {
    if !a.is_finite() || (a - b).abs() > tol {
        Err(invalid(format!("geometry mismatch: {a} vs {b} ±{tol}")))
    } else {
        Ok(())
    }
}
fn metric(report: &EvaluationReport, name: &str) -> Result<f64, ArchyError> {
    report
        .metrics
        .iter()
        .find(|m| m.name == name)
        .map(|m| m.value)
        .ok_or_else(|| invalid(format!("missing metric {name}")))
}
fn polygon_contains(p: [f64; 2], polygon: &[[f64; 2]], tol: f64) -> bool {
    let mut inside = false;
    for pair in polygon.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        let cross = ((p[0] - a[0]) * (b[1] - a[1]) - (p[1] - a[1]) * (b[0] - a[0])).abs();
        let edge = (b[0] - a[0]).hypot(b[1] - a[1]);
        if cross <= tol * edge
            && p[0] >= a[0].min(b[0]) - tol
            && p[0] <= a[0].max(b[0]) + tol
            && p[1] >= a[1].min(b[1]) - tol
            && p[1] <= a[1].max(b[1]) + tol
        {
            return true;
        }
        if (a[1] > p[1]) != (b[1] > p[1])
            && p[0] < a[0] + (p[1] - a[1]) * (b[0] - a[0]) / (b[1] - a[1])
        {
            inside = !inside;
        }
    }
    inside
}
// Preview contrast is measured after straight-alpha compositing on opaque white.
fn white_composite(pixel: &[u8]) -> [u8; 3] {
    let alpha = u16::from(*pixel.get(3).unwrap_or(&255));
    std::array::from_fn(|channel| {
        (255 - ((255 - u16::from(pixel[channel])) * alpha + 127) / 255) as u8
    })
}
fn verify_png(bytes: &[u8]) -> Result<u32, ArchyError> {
    if bytes.len() > 4 * 1024 * 1024 {
        return Err(invalid("PNG input size"));
    }
    let mut decoder = png::Decoder::new_with_limits(
        Cursor::new(bytes),
        png::Limits {
            bytes: 8 * 1024 * 1024,
        },
    );
    decoder.set_transformations(png::Transformations::IDENTITY);
    let mut reader = decoder
        .read_info()
        .map_err(|e| invalid(format!("PNG header: {e}")))?;
    if reader.info().width != 512
        || reader.info().height != 512
        || reader.info().animation_control.is_some()
        || reader.info().bit_depth != png::BitDepth::Eight
        || !matches!(
            reader.info().color_type,
            png::ColorType::Rgb | png::ColorType::Rgba
        )
    {
        return Err(invalid("PNG dimensions, color or animation"));
    }
    let size = reader
        .output_buffer_size()
        .ok_or_else(|| invalid("PNG output size"))?;
    if size > 512 * 512 * 4 {
        return Err(invalid("PNG decoded size"));
    }
    let mut decoded = vec![0; size];
    let frame = reader
        .next_frame(&mut decoded)
        .map_err(|e| invalid(format!("PNG decode: {e}")))?;
    reader
        .finish()
        .map_err(|e| invalid(format!("PNG completion: {e}")))?;
    let channels = match frame.color_type {
        png::ColorType::Rgb => 3,
        png::ColorType::Rgba => 4,
        _ => return Err(invalid("PNG must decode RGB or RGBA")),
    };
    let pixels = &decoded[..frame.buffer_size()];
    if pixels.len() != 512 * 512 * channels {
        return Err(invalid("PNG pixel length"));
    }
    // The fixed orthographic cameras leave the opaque, uniform world at the edge.
    // A border median tolerates isolated damaged pixels, including (0, 0).
    let mut border_histogram = [[0_u32; 256]; 3];
    let mut border_len = 0_u32;
    for (index, pixel) in pixels.chunks_exact(channels).enumerate() {
        let x = index % 512;
        let y = index / 512;
        if x == 0 || x == 511 || y == 0 || y == 511 {
            for (channel, value) in white_composite(pixel).into_iter().enumerate() {
                border_histogram[channel][usize::from(value)] += 1;
            }
            border_len += 1;
        }
    }
    let bg = border_histogram.map(|histogram| {
        let mut cumulative = 0;
        histogram
            .into_iter()
            .enumerate()
            .find_map(|(value, count)| {
                cumulative += count;
                (cumulative > border_len / 2).then_some(value as u8)
            })
            .unwrap()
    });
    // Spatially varying borders are outside that render profile, even if their
    // contrast would otherwise exceed the foreground threshold.
    let unstable_border = pixels
        .chunks_exact(channels)
        .enumerate()
        .filter(|(index, pixel)| {
            let x = index % 512;
            let y = index / 512;
            (x == 0 || x == 511 || y == 0 || y == 511)
                && white_composite(pixel)
                    .iter()
                    .zip(bg.iter())
                    .any(|(value, reference)| value.abs_diff(*reference) > 8)
        })
        .count();
    if unstable_border > border_len as usize / 100 {
        return Err(invalid("PNG background varies across camera border"));
    }
    let mut foreground = 0_usize;
    let (mut min_x, mut min_y, mut max_x, mut max_y) = (512, 512, 0, 0);
    for (index, pixel) in pixels.chunks_exact(channels).enumerate() {
        let visible = white_composite(pixel)
            .iter()
            .zip(bg.iter())
            .map(|(a, b)| a.abs_diff(*b))
            .max()
            .unwrap_or(0)
            > 8;
        if visible {
            foreground += 1;
            let (x, y) = (index % 512, index / 512);
            min_x = min_x.min(x);
            min_y = min_y.min(y);
            max_x = max_x.max(x);
            max_y = max_y.max(y);
        }
    }
    let width = max_x.saturating_sub(min_x) + 1;
    let height = max_y.saturating_sub(min_y) + 1;
    // A08's 512-pixel orthographic camera can turn a supported 100:1 roof
    // into a few rows across most of the image. An iso view makes the same
    // roof diagonal, so its axis-aligned box is mostly background. Require
    // resolved extent and contrast density within that box.
    let box_area = width * height;
    if foreground < 128
        || width.min(height) < 2
        || width.max(height) < 16
        || foreground * 50 < box_area
    {
        return Err(invalid("blank PNG preview"));
    }
    Ok(foreground as u32)
}

type Matrix = [[f64; 4]; 4];
fn identity() -> Matrix {
    [
        [1., 0., 0., 0.],
        [0., 1., 0., 0.],
        [0., 0., 1., 0.],
        [0., 0., 0., 1.],
    ]
}
fn product(a: Matrix, b: Matrix) -> Matrix {
    let mut out = [[0.; 4]; 4];
    for c in 0..4 {
        for r in 0..4 {
            out[c][r] = (0..4).map(|k| a[k][r] * b[c][k]).sum();
        }
    }
    out
}
fn world(m: Matrix, p: [f32; 3]) -> [f64; 3] {
    let v = [p[0] as f64, p[1] as f64, p[2] as f64, 1.];
    let q: [f64; 3] = std::array::from_fn(|r| (0..4).map(|c| m[c][r] * v[c]).sum());
    [q[0], -q[2], q[1]]
}
fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

#[derive(Default)]
struct Solid {
    vertices: Vec<[f64; 3]>,
    triangles: Vec<[usize; 3]>,
}
fn collect_node(
    node: gltf::Node<'_>,
    parent: Matrix,
    blob: &[u8],
    solids: &mut Vec<Solid>,
    depth: usize,
) -> Result<(), ArchyError> {
    if depth > 16 || solids.len() >= 32 {
        return Err(invalid("GLB hierarchy limit"));
    }
    let local = node.transform().matrix().map(|c| c.map(f64::from));
    let transform = product(parent, local);
    if let Some(mesh) = node.mesh() {
        for primitive in mesh.primitives() {
            if solids.len() >= 32 {
                return Err(invalid("GLB solid count"));
            }
            if primitive.mode() != gltf::mesh::Mode::Triangles
                || primitive.morph_targets().next().is_some()
            {
                return Err(invalid("unsupported GLB primitive"));
            }
            let reader = primitive.reader(|buffer| (buffer.index() == 0).then_some(blob));
            let used_vertices: usize = solids.iter().map(|s| s.vertices.len()).sum();
            let used_indices: usize = solids.iter().map(|s| s.triangles.len() * 3).sum();
            let positions: Vec<_> = reader
                .read_positions()
                .ok_or_else(|| invalid("GLB positions"))?
                .take(100_001 - used_vertices)
                .map(|p| world(transform, p))
                .collect();
            if positions.len() < 4
                || positions.len() + used_vertices > 100_000
                || positions.iter().flatten().any(|v| !v.is_finite())
            {
                return Err(invalid("GLB vertex count or coordinate"));
            }
            let indices: Vec<_> = reader
                .read_indices()
                .ok_or_else(|| invalid("GLB indices"))?
                .into_u32()
                .take(300_001 - used_indices)
                .collect();
            if indices.is_empty()
                || indices.len() % 3 != 0
                || indices.len() + used_indices > 300_000
            {
                return Err(invalid("GLB triangle count"));
            }
            let triangles = indices
                .as_chunks::<3>()
                .0
                .iter()
                .map(|t| [t[0] as usize, t[1] as usize, t[2] as usize])
                .collect::<Vec<_>>();
            if triangles.iter().flatten().any(|i| *i >= positions.len()) {
                return Err(invalid("GLB index range"));
            }
            solids.push(Solid {
                vertices: positions,
                triangles,
            });
        }
    }
    for child in node.children() {
        collect_node(child, transform, blob, solids, depth + 1)?;
    }
    Ok(())
}
fn verify_solid(
    solid: &Solid,
    variant: &Variant,
    report: &EvaluationReport,
) -> Result<(usize, f64, f64), ArchyError> {
    let tol = report.geometry_parity_budget.coordinate_length_m;
    if tol != 1e-6 {
        return Err(invalid("unsupported geometry tolerance"));
    }
    let mut low = [f64::INFINITY; 3];
    let mut high = [f64::NEG_INFINITY; 3];
    for p in &solid.vertices {
        for k in 0..3 {
            low[k] = low[k].min(p[k]);
            high[k] = high[k].max(p[k]);
        }
        if !polygon_contains([p[0], p[1]], &variant.footprint_xy_m, tol) {
            return Err(invalid("GLB vertex outside footprint"));
        }
    }
    let level = (low[2] / variant.floor_to_floor_m).round() as i64;
    if level < 0 || level >= i64::from(variant.storeys) {
        return Err(invalid("GLB storey elevation"));
    }
    near(low[2], level as f64 * variant.floor_to_floor_m, tol)?;
    near(high[2], (level + 1) as f64 * variant.floor_to_floor_m, tol)?;
    for (axis, min, max) in [(0, "xmin", "xmax"), (1, "ymin", "ymax")] {
        near(low[axis], metric(report, min)?, tol)?;
        near(high[axis], metric(report, max)?, tol)?;
    }
    for corner in variant
        .footprint_xy_m
        .iter()
        .take(variant.footprint_xy_m.len() - 1)
    {
        for z in [low[2], high[2]] {
            if !solid.vertices.iter().any(|p| {
                (p[0] - corner[0]).abs() <= tol
                    && (p[1] - corner[1]).abs() <= tol
                    && (p[2] - z).abs() <= tol
            }) {
                return Err(invalid("GLB missing footprint corner"));
            }
        }
    }
    let mut weld = BTreeMap::new();
    let mut ids = Vec::new();
    for p in &solid.vertices {
        let key = p.map(f64::to_bits);
        let next = weld.len();
        ids.push(*weld.entry(key).or_insert(next));
    }
    let mut edges = BTreeMap::<(usize, usize), (u32, i32)>::new();
    let mut volume = 0.;
    let mut roof = 0.;
    let mut roof_triangles = 0;
    for face in &solid.triangles {
        let tri = face.map(|i| solid.vertices[i]);
        let ids = face.map(|i| ids[i]);
        if ids[0] == ids[1] || ids[1] == ids[2] || ids[2] == ids[0] {
            return Err(invalid("degenerate GLB triangle"));
        }
        for (a, b) in [(ids[0], ids[1]), (ids[1], ids[2]), (ids[2], ids[0])] {
            let key = (a.min(b), a.max(b));
            let entry = edges.entry(key).or_default();
            entry.0 += 1;
            entry.1 += if a < b { 1 } else { -1 };
        }
        volume += dot(tri[0], cross(tri[1], tri[2])) / 6.;
        if tri.iter().all(|p| (p[2] - high[2]).abs() <= tol) {
            roof_triangles += 1;
            let center = [
                (tri[0][0] + tri[1][0] + tri[2][0]) / 3.,
                (tri[0][1] + tri[1][1] + tri[2][1]) / 3.,
            ];
            if !polygon_contains(center, &variant.footprint_xy_m, tol) {
                return Err(invalid("GLB roof fills footprint notch"));
            }
            let area = cross(sub(tri[1], tri[0]), sub(tri[2], tri[0]));
            roof += area[2].abs() / 2.;
        }
    }
    if roof_triangles == 0
        || edges
            .values()
            .any(|(count, orientation)| *count != 2 || *orientation != 0)
        || volume <= 0.
    {
        return Err(invalid("GLB solid is open or inverted"));
    }
    let area = metric(report, "footprint_area")?;
    near(roof, area, report.geometry_parity_budget.footprint_area_m2)?;
    near(
        volume,
        area * variant.floor_to_floor_m,
        report.geometry_parity_budget.footprint_area_m2 * variant.floor_to_floor_m
            + 2. * area * tol,
    )?;
    Ok((level as usize, roof, volume))
}
fn verify_glb(
    bytes: &[u8],
    variant: &Variant,
    report: &EvaluationReport,
) -> Result<(), ArchyError> {
    if bytes.len() < 20 || bytes.len() > 8 * 1024 * 1024 || !bytes.starts_with(b"glTF") {
        return Err(invalid("GLB header or input size"));
    }
    let glb =
        gltf::binary::Glb::from_slice(bytes).map_err(|e| invalid(format!("GLB container: {e}")))?;
    // The JSON is parsed twice (profile inspection and glTF validation).
    if glb.json.len() > 1024 * 1024 {
        return Err(invalid("GLB JSON profile size"));
    }
    let document: serde_json::Value =
        serde_json::from_slice(&glb.json).map_err(|e| invalid(format!("GLB JSON: {e}")))?;
    if document
        .get("extensionsRequired")
        .is_some_and(|v| v.as_array().is_none_or(|a| !a.is_empty()))
        || document.get("skins").is_some()
        || document.get("animations").is_some()
        || document["buffers"]
            .as_array()
            .is_none_or(|a| a.len() != 1 || a[0].get("uri").is_some())
        || document["images"]
            .as_array()
            .is_some_and(|a| a.iter().any(|v| v.get("uri").is_some()))
    {
        return Err(invalid("GLB external or unsupported resources"));
    }
    let parsed = gltf::Gltf::from_slice(bytes).map_err(|e| invalid(format!("GLB parse: {e}")))?;
    let blob = parsed
        .blob
        .as_deref()
        .ok_or_else(|| invalid("GLB binary data absent"))?;
    let scene = parsed
        .default_scene()
        .ok_or_else(|| invalid("GLB default scene absent"))?;
    let mut solids = Vec::new();
    for node in scene.nodes() {
        collect_node(node, identity(), blob, &mut solids, 0)?;
    }
    if solids.len() != variant.storeys as usize {
        return Err(invalid("GLB storey instance count"));
    }
    let mut levels = BTreeSet::new();
    let mut area_sum = 0.;
    for solid in &solids {
        let (level, area, _) = verify_solid(solid, variant, report)?;
        if !levels.insert(level) {
            return Err(invalid("duplicate GLB storey"));
        }
        area_sum += area;
    }
    if levels != (0..variant.storeys as usize).collect() {
        return Err(invalid("missing GLB storey"));
    }
    near(
        area_sum,
        metric(report, "gross_floor_area")?,
        report.geometry_parity_budget.gross_floor_area_m2,
    )?;
    near(
        variant.storeys as f64 * variant.floor_to_floor_m,
        metric(report, "height")?,
        report.geometry_parity_budget.coordinate_length_m,
    )?;
    Ok(())
}

pub fn verify_captured(
    captured: &CapturedOutputs,
    variant: &Variant,
    report: &EvaluationReport,
) -> Result<Vec<(String, u32)>, ArchyError> {
    verify_bytes(
        |name| {
            captured
                .artifacts()
                .iter()
                .find(|a| a.path() == name)
                .map(|a| a.bytes())
        },
        variant,
        report,
    )
}

pub fn verify_bytes<'a>(
    get: impl Fn(&str) -> Option<&'a [u8]>,
    variant: &Variant,
    report: &EvaluationReport,
) -> Result<Vec<(String, u32)>, ArchyError> {
    verify_glb(
        get("model.glb").ok_or_else(|| invalid("missing GLB"))?,
        variant,
        report,
    )?;
    ["view-top.png", "view-front.png", "view-iso.png"]
        .into_iter()
        .map(|name| {
            Ok((
                name.to_owned(),
                verify_png(get(name).ok_or_else(|| invalid("missing preview"))?)?,
            ))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rgba_probe(alpha: u8, hidden_rgb: bool) -> Vec<u8> {
        let mut pixels = vec![0; 512 * 512 * 4];
        for (i, pixel) in pixels.as_chunks_mut::<4>().0.iter_mut().enumerate() {
            if hidden_rgb {
                pixel[..3].copy_from_slice(&[(i / 512) as u8, i as u8, 128]);
                pixel[3] = alpha;
            } else {
                pixel[..3].fill(255);
                pixel[3] = 255;
                if (128..384).contains(&(i / 512)) && (128..384).contains(&(i % 512)) {
                    pixel[..3].fill(0);
                    pixel[3] = alpha;
                }
            }
        }
        let mut encoded = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut encoded, 512, 512);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            let mut writer = encoder.write_header().unwrap();
            writer.write_image_data(&pixels).unwrap();
        }
        encoded
    }

    fn single_outlier_png(x: usize, y: usize) -> Vec<u8> {
        let mut pixels = vec![255; 512 * 512 * 4];
        for pixel in pixels.as_chunks_mut::<4>().0 {
            pixel[3] = 255;
        }
        pixels[(y * 512 + x) * 4..(y * 512 + x) * 4 + 3].fill(0);
        let mut encoded = Vec::new();
        let mut encoder = png::Encoder::new(&mut encoded, 512, 512);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder
            .write_header()
            .unwrap()
            .write_image_data(&pixels)
            .unwrap();
        encoded
    }

    fn varying_background_png(bands: bool) -> Vec<u8> {
        let mut pixels = vec![255; 512 * 512 * 4];
        for (index, pixel) in pixels.as_chunks_mut::<4>().0.iter_mut().enumerate() {
            let x = index % 512;
            let value = if bands {
                255 - 10 * u8::from(x >= 256)
            } else {
                255 - (x / 8) as u8
            };
            pixel[..3].fill(value);
        }
        let mut encoded = Vec::new();
        let mut encoder = png::Encoder::new(&mut encoded, 512, 512);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder
            .write_header()
            .unwrap()
            .write_image_data(&pixels)
            .unwrap();
        encoded
    }

    #[test]
    fn varying_background_is_not_preview_evidence() {
        for bands in [false, true] {
            let error = verify_png(&varying_background_png(bands)).unwrap_err();
            assert_eq!(error.code, ErrorCode::ArtifactInvalid);
            assert!(error.message.contains("background"), "{error:?}");
        }
    }

    #[test]
    fn thin_framed_massing_remains_visible_at_512_pixels() {
        for vertical in [false, true] {
            let mut pixels = vec![255; 512 * 512 * 4];
            for (index, pixel) in pixels.as_chunks_mut::<4>().0.iter_mut().enumerate() {
                let (x, y) = (index % 512, index / 512);
                let (long, short) = if vertical { (y, x) } else { (x, y) };
                if (27..485).contains(&long) && (254..258).contains(&short) {
                    pixel[..3].fill(0);
                }
            }
            let mut encoded = Vec::new();
            let mut encoder = png::Encoder::new(&mut encoded, 512, 512);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            encoder
                .write_header()
                .unwrap()
                .write_image_data(&pixels)
                .unwrap();
            assert_eq!(verify_png(&encoded).unwrap(), 458 * 4);
        }
    }

    #[test]
    fn scattered_contrast_is_not_a_silhouette() {
        let mut pixels = vec![255; 512 * 512 * 4];
        for y in (32..480).step_by(16) {
            for x in (32..480).step_by(16) {
                pixels[(y * 512 + x) * 4..(y * 512 + x) * 4 + 3].fill(0);
            }
        }
        let mut encoded = Vec::new();
        let mut encoder = png::Encoder::new(&mut encoded, 512, 512);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder
            .write_header()
            .unwrap()
            .write_image_data(&pixels)
            .unwrap();
        let error = verify_png(&encoded).unwrap_err();
        assert!(error.message.contains("blank PNG preview"));
    }

    #[test]
    fn diagonal_thin_massing_has_sparse_axis_aligned_bounds() {
        let mut pixels = vec![255; 512 * 512 * 4];
        for (index, pixel) in pixels.as_chunks_mut::<4>().0.iter_mut().enumerate() {
            let (x, y) = (index % 512, index / 512);
            if (32..480).contains(&x) && y.abs_diff(120 + (x - 32) / 2) < 7 {
                pixel[..3].fill(0);
            }
        }
        let mut encoded = Vec::new();
        let mut encoder = png::Encoder::new(&mut encoded, 512, 512);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder
            .write_header()
            .unwrap()
            .write_image_data(&pixels)
            .unwrap();
        assert!(verify_png(&encoded).unwrap() > 2621);
    }

    #[test]
    fn isolated_pixel_never_counts_as_visible_preview() {
        for (x, y) in [
            (0, 0),
            (511, 0),
            (0, 511),
            (511, 511),
            (256, 0),
            (0, 256),
            (256, 256),
        ] {
            let error = verify_png(&single_outlier_png(x, y)).unwrap_err();
            assert_eq!(error.code, ErrorCode::ArtifactInvalid);
            assert!(
                error.message.contains("blank PNG preview"),
                "{x},{y}: {error:?}"
            );
        }
    }

    #[test]
    fn rgba_preview_requires_visible_foreground() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/a10-render-real");
        let variant: Variant =
            serde_json::from_slice(&std::fs::read(root.join("variant.json")).unwrap()).unwrap();
        let report: EvaluationReport =
            serde_json::from_slice(&std::fs::read(root.join("evaluation.json")).unwrap()).unwrap();
        let glb = std::fs::read(root.join("model.glb")).unwrap();
        for (alpha, hidden_rgb, accepted) in [
            (0, true, false),
            (0, false, false),
            (1, false, false),
            (8, false, false),
            (9, false, true),
            (128, false, true),
            (255, false, true),
        ] {
            let png = rgba_probe(alpha, hidden_rgb);
            let result = verify_bytes(
                |name| {
                    (name == "model.glb")
                        .then_some(glb.as_slice())
                        .or_else(|| name.ends_with(".png").then_some(png.as_slice()))
                },
                &variant,
                &report,
            );
            assert_eq!(
                result.is_ok(),
                accepted,
                "alpha={alpha}, hidden_rgb={hidden_rgb}: {result:?}"
            );
            if let Err(error) = result {
                assert_eq!(error.code, ErrorCode::ArtifactInvalid);
            }
        }
    }
    #[test]
    fn real_a10_export_decodes_and_corruption_refuses() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/a10-render-real");
        let variant: Variant =
            serde_json::from_slice(&std::fs::read(root.join("variant.json")).unwrap()).unwrap();
        let report: EvaluationReport =
            serde_json::from_slice(&std::fs::read(root.join("evaluation.json")).unwrap()).unwrap();
        let files: BTreeMap<_, _> = [
            "model.glb",
            "view-top.png",
            "view-front.png",
            "view-iso.png",
        ]
        .into_iter()
        .map(|name| (name.to_owned(), std::fs::read(root.join(name)).unwrap()))
        .collect();
        let get = |name: &str| files.get(name).map(Vec::as_slice);
        let previews = verify_bytes(get, &variant, &report).unwrap();
        assert_eq!(previews.len(), 3);
        assert!(previews.iter().all(|(_, count)| *count > 2621));
        let mut wrong_geometry = variant.clone();
        wrong_geometry.floor_to_floor_m += 0.25;
        assert_eq!(
            verify_bytes(
                |name| files.get(name).map(Vec::as_slice),
                &wrong_geometry,
                &report
            )
            .unwrap_err()
            .code,
            ErrorCode::ArtifactInvalid
        );
        let oversized_png = vec![0; 4 * 1024 * 1024 + 1];
        assert_eq!(
            verify_png(&oversized_png).unwrap_err().code,
            ErrorCode::ArtifactInvalid
        );
        let oversized_glb = vec![0; 8 * 1024 * 1024 + 1];
        assert_eq!(
            verify_glb(&oversized_glb, &variant, &report)
                .unwrap_err()
                .code,
            ErrorCode::ArtifactInvalid
        );
        let mut corrupt = files.clone();
        corrupt.get_mut("model.glb").unwrap()[0] = 0;
        assert_eq!(
            verify_bytes(
                |name| corrupt.get(name).map(Vec::as_slice),
                &variant,
                &report
            )
            .unwrap_err()
            .code,
            ErrorCode::ArtifactInvalid
        );
        let mut corrupt = files;
        corrupt.get_mut("view-top.png").unwrap().truncate(20);
        assert_eq!(
            verify_bytes(
                |name| corrupt.get(name).map(Vec::as_slice),
                &variant,
                &report
            )
            .unwrap_err()
            .code,
            ErrorCode::ArtifactInvalid
        );
    }

    #[test]
    fn glb_json_profile_is_bounded_before_parsing() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/a10-render-real");
        let bytes = std::fs::read(root.join("model.glb")).unwrap();
        let variant: Variant =
            serde_json::from_slice(&std::fs::read(root.join("variant.json")).unwrap()).unwrap();
        let report: EvaluationReport =
            serde_json::from_slice(&std::fs::read(root.join("evaluation.json")).unwrap()).unwrap();
        let mut glb = gltf::binary::Glb::from_slice(&bytes).unwrap();
        let mut json = glb.json.into_owned();
        json.resize(1024 * 1024 + 4, b' ');
        glb.json = std::borrow::Cow::Owned(json);
        let oversized = glb.to_vec().unwrap();
        let error = verify_glb(&oversized, &variant, &report).unwrap_err();
        assert_eq!(error.code, ErrorCode::ArtifactInvalid);
        assert!(error.message.contains("GLB JSON profile size"), "{error:?}");
    }
}
