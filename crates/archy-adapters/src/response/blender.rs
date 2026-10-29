use super::{ClaimedArtifact, artifacts, bad, decode, finite, matrix, near, same};
use crate::request::BlenderRequest;
use archy_core::ArchyError;
use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum BlenderResponse {
    Ok(Box<BlenderSuccess>),
    Error(BlenderFailure),
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum OkStatus {
    Ok,
}
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum ErrorStatus {
    Error,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BlenderSuccess {
    #[serde(rename = "status")]
    _status: OkStatus,
    schema_version: String,
    request_id: String,
    project_id: String,
    variant_id: String,
    run_id: String,
    request_digest: String,
    worker_version: String,
    blender_version: String,
    build_hash: String,
    python_version: String,
    warnings: Vec<String>,
    pub preset: BlenderPreset,
    pub geometry: BlenderGeometry,
    pub artifacts: Vec<ClaimedArtifact>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BlenderFailure {
    #[serde(rename = "status")]
    _status: ErrorStatus,
    schema_version: String,
    pub error: ArchyError,
    pub artifacts: Vec<ClaimedArtifact>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BlenderGeometry {
    pub storeys: u32,
    pub footprint_area_m2: f64,
    pub volume_m3: f64,
    pub bounds_m: [f64; 6],
    pub world_coordinate_max_error_m: f64,
    pub cap_area_error_m2: f64,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BlenderPreset {
    preset_id: String,
    engine: String,
    device: String,
    threads: u32,
    threads_mode: String,
    samples: u32,
    seed: u32,
    adaptive_sampling: bool,
    denoising: bool,
    resolution: [u32; 2],
    resolution_percentage: u32,
    format: String,
    image_compression: u32,
    film_transparent: bool,
    display_device: String,
    view_transform: String,
    look: String,
    exposure: f64,
    gamma: f64,
    axis_conversion: String,
    frame: u32,
    unit: Unit,
    world: World,
    material: Material,
    cameras: Cameras,
    objects: Vec<MassingObject>,
    light: Light,
    glb_export: GlbExport,
    view_bounds_m: [f64; 6],
    saved_render_path: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Unit {
    system: String,
    scale_length: f64,
}
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct World {
    use_nodes: bool,
    color_rgba: [f64; 4],
    strength: f64,
}
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Material {
    use_nodes: bool,
    diffuse_rgba: [f64; 4],
    base_color_rgba: [f64; 4],
    roughness: f64,
}
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Cameras {
    top: Camera,
    front: Camera,
    iso: Camera,
}
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Camera {
    name: String,
    #[serde(rename = "type")]
    kind: String,
    matrix_world: [[f64; 4]; 4],
    ortho_scale: f64,
    clip_start: f64,
    clip_end: f64,
}
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct MassingObject {
    name: String,
    location: [f64; 3],
    rotation_euler: [f64; 3],
    scale: [f64; 3],
    matrix_world: [[f64; 4]; 4],
}
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Light {
    #[serde(rename = "type")]
    kind: String,
    shape: String,
    color_rgb: [f64; 3],
    location: [f64; 3],
    rotation_euler: [f64; 3],
    matrix_world: [[f64; 4]; 4],
    energy: f64,
    size: f64,
}
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct GlbExport {
    format: String,
    selected_only: bool,
    export_yup: bool,
    animations: bool,
    cameras: bool,
    lights: bool,
    compression: bool,
}

pub fn decode_blender_response(
    bytes: &[u8],
    request: &BlenderRequest,
) -> Result<BlenderResponse, ArchyError> {
    let response: BlenderResponse = decode(bytes)?;
    let wire = request.wire();
    match &response {
        BlenderResponse::Error(failure) => {
            same(
                &failure.schema_version,
                "blender-worker-v1",
                "schema_version",
            )?;
            if !failure.artifacts.is_empty() {
                return Err(bad("artifacts", "error lists partial artifacts"));
            }
        }
        BlenderResponse::Ok(success) => {
            for (got, expected, field) in [
                (
                    &success.schema_version,
                    "blender-worker-v1",
                    "schema_version",
                ),
                (&success.request_id, wire.request_id.as_str(), "request_id"),
                (&success.project_id, wire.project_id.as_str(), "project_id"),
                (&success.variant_id, wire.variant_id.as_str(), "variant_id"),
                (&success.run_id, wire.run_id.as_str(), "run_id"),
                (
                    &success.request_digest,
                    wire.input_digest.as_str(),
                    "request_digest",
                ),
                (
                    &success.worker_version,
                    "blender-worker-v1",
                    "worker_version",
                ),
                (&success.blender_version, "4.5.15 LTS", "blender_version"),
                (&success.build_hash, "5afeabba6097", "build_hash"),
            ] {
                same(got, expected, field)?;
            }
            if !success.python_version.starts_with("3.11.15 ") || success.warnings.len() > 16 {
                return Err(bad(
                    "python_version",
                    "unsupported Blender runtime or warnings count",
                ));
            }
            artifacts(
                &success.artifacts,
                &[
                    ("scene.blend", "application/x-blender"),
                    ("model.glb", "model/gltf-binary"),
                    ("view-top.png", "image/png"),
                    ("view-front.png", "image/png"),
                    ("view-iso.png", "image/png"),
                ],
                "blender-worker-v1",
            )?;
            validate_geometry(&success.geometry, request)?;
            validate_preset(&success.preset, request)?;
        }
    }
    Ok(response)
}

fn validate_geometry(g: &BlenderGeometry, request: &BlenderRequest) -> Result<(), ArchyError> {
    let wire = request.wire();
    if g.storeys != wire.params.storeys
        || g.footprint_area_m2 <= 0.
        || g.volume_m3 <= 0.
        || g.world_coordinate_max_error_m < 0.
        || g.world_coordinate_max_error_m > 1e-6
        || g.cap_area_error_m2 < 0.
    {
        return Err(bad("geometry", "invalid geometry summary or parity"));
    }
    finite(
        [
            g.footprint_area_m2,
            g.volume_m3,
            g.world_coordinate_max_error_m,
            g.cap_area_error_m2,
        ]
        .into_iter()
        .chain(g.bounds_m),
        "geometry",
    )?;
    let area = request
        .evaluation()
        .metrics
        .iter()
        .find(|m| m.name == "footprint_area")
        .ok_or_else(|| bad("geometry", "missing canonical area metric"))?
        .value;
    let budget = &request.evaluation().geometry_parity_budget;
    near(
        g.footprint_area_m2,
        area,
        budget.footprint_area_m2,
        "geometry.footprint_area_m2",
    )?;
    if g.cap_area_error_m2 > budget.footprint_area_m2 {
        return Err(bad(
            "geometry.cap_area_error_m2",
            "cap area exceeds Task02 propagated budget",
        ));
    }
    let height = wire.params.floor_to_floor_m * f64::from(wire.params.storeys);
    let volume_budget =
        budget.footprint_area_m2 * height + area * 1e-6 * f64::from(wire.params.storeys);
    near(
        g.volume_m3,
        area * height,
        volume_budget,
        "geometry.volume_m3",
    )?;
    let ring = &wire.params.footprint_xy_m;
    let expected = [
        ring.iter().map(|p| p[0]).fold(f64::INFINITY, f64::min),
        ring.iter().map(|p| p[1]).fold(f64::INFINITY, f64::min),
        0.,
        ring.iter().map(|p| p[0]).fold(f64::NEG_INFINITY, f64::max),
        ring.iter().map(|p| p[1]).fold(f64::NEG_INFINITY, f64::max),
        height,
    ];
    for (got, want) in g.bounds_m.iter().zip(expected) {
        near(*got, want, 1e-6, "geometry.bounds_m")?;
    }
    Ok(())
}

fn validate_preset(p: &BlenderPreset, request: &BlenderRequest) -> Result<(), ArchyError> {
    for (got, want, field) in [
        (&p.preset_id, "smoke-cpu-v1", "preset.preset_id"),
        (&p.engine, "CYCLES", "preset.engine"),
        (&p.device, "CPU", "preset.device"),
        (&p.threads_mode, "FIXED", "preset.threads_mode"),
        (&p.format, "PNG_RGBA8", "preset.format"),
        (&p.display_device, "sRGB", "preset.display_device"),
        (&p.view_transform, "Standard", "preset.view_transform"),
        (&p.look, "Medium High Contrast", "preset.look"),
        (
            &p.axis_conversion,
            "Blender Z-up to glTF Y-up",
            "preset.axis_conversion",
        ),
        (
            &p.saved_render_path,
            "//view-iso.png",
            "preset.saved_render_path",
        ),
    ] {
        same(got, want, field)?;
    }
    if p.threads != 1
        || p.samples != 16
        || p.seed != 42
        || p.adaptive_sampling
        || p.denoising
        || p.resolution != [512, 512]
        || p.resolution_percentage != 100
        || p.image_compression != 15
        || p.film_transparent
        || p.frame != 1
    {
        return Err(bad("preset", "unsupported fixed CPU render setting"));
    }
    near(p.exposure, 0., 1e-6, "preset.exposure")?;
    near(p.gamma, 1., 1e-6, "preset.gamma")?;
    same(&p.unit.system, "METRIC", "preset.unit.system")?;
    near(p.unit.scale_length, 1., 1e-6, "preset.unit.scale_length")?;
    if !p.world.use_nodes || !p.material.use_nodes {
        return Err(bad("preset", "missing fixed shader nodes"));
    }
    for (got, want) in p.world.color_rgba.iter().zip([0.12, 0.15, 0.2, 1.]) {
        near(*got, want, 1e-6, "preset.world.color_rgba")?;
    }
    near(p.world.strength, 0.6, 1e-6, "preset.world.strength")?;
    for values in [p.material.diffuse_rgba, p.material.base_color_rgba] {
        for (got, want) in values.iter().zip([0.3, 0.55, 0.8, 1.]) {
            near(*got, want, 1e-6, "preset.material.color")?;
        }
    }
    near(p.material.roughness, 0.8, 1e-6, "preset.material.roughness")?;
    let export = &p.glb_export;
    same(&export.format, "GLB", "preset.glb_export.format")?;
    if !export.selected_only
        || !export.export_yup
        || export.animations
        || export.cameras
        || export.lights
        || export.compression
    {
        return Err(bad("preset.glb_export", "unsupported GLB export flags"));
    }
    finite(p.view_bounds_m, "preset.view_bounds_m")?;
    if (0..3).any(|i| p.view_bounds_m[i] >= p.view_bounds_m[i + 3]) {
        return Err(bad("preset.view_bounds_m", "invalid view bounds"));
    }
    for (camera, name) in [
        (&p.cameras.top, "Camera_top"),
        (&p.cameras.front, "Camera_front"),
        (&p.cameras.iso, "Camera_iso"),
    ] {
        same(&camera.name, name, "preset.cameras.name")?;
        same(&camera.kind, "ORTHO", "preset.cameras.type")?;
        matrix(&camera.matrix_world, "preset.cameras.matrix_world")?;
        near(camera.clip_start, 0.01, 1e-6, "preset.cameras.clip_start")?;
        finite([camera.ortho_scale, camera.clip_end], "preset.cameras")?;
        if camera.ortho_scale <= 0. || camera.clip_end <= camera.clip_start {
            return Err(bad("preset.cameras", "invalid orthographic camera"));
        }
    }
    if p.objects.len() != request.wire().params.storeys as usize {
        return Err(bad("preset.objects", "storey object count differs"));
    }
    let origin = request.wire().params.footprint_xy_m[0];
    for (i, obj) in p.objects.iter().enumerate() {
        same(&obj.name, &format!("Storey_{i:03}"), "preset.objects.name")?;
        finite(
            obj.location
                .into_iter()
                .chain(obj.rotation_euler)
                .chain(obj.scale),
            "preset.objects",
        )?;
        matrix(&obj.matrix_world, "preset.objects.matrix_world")?;
        let expected_location = [
            origin[0],
            origin[1],
            i as f64 * request.wire().params.floor_to_floor_m,
        ];
        for (axis, expected) in expected_location.iter().enumerate() {
            near(
                obj.rotation_euler[axis],
                0.,
                1e-6,
                "preset.objects.rotation_euler",
            )?;
            near(obj.scale[axis], 1., 1e-6, "preset.objects.scale")?;
            near(
                obj.location[axis],
                *expected,
                1e-6,
                "preset.objects.location",
            )?;
        }
    }
    let light = &p.light;
    same(&light.kind, "AREA", "preset.light.type")?;
    same(&light.shape, "DISK", "preset.light.shape")?;
    finite(
        light
            .location
            .into_iter()
            .chain(light.rotation_euler)
            .chain(light.color_rgb),
        "preset.light",
    )?;
    matrix(&light.matrix_world, "preset.light.matrix_world")?;
    near(light.energy, 3000., 1e-6, "preset.light.energy")?;
    near(light.size, 10., 1e-6, "preset.light.size")?;
    for color in light.color_rgb {
        near(color, 1., 1e-6, "preset.light.color_rgb")?;
    }
    for rotation in light.rotation_euler {
        near(rotation, 0., 1e-6, "preset.light.rotation_euler")?;
    }
    Ok(())
}
