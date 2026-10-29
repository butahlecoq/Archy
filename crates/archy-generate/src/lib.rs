//! Deterministic bounded massing alternatives.
use archy_core::*;
use rand_chacha::ChaCha8Rng;
use rand_core::{Rng, SeedableRng};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

pub const ALGORITHM_VERSION: &str = "chacha8-u53-massing-v1";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GenerationResult {
    pub variants: Vec<Variant>,
    pub requested_count: u32,
    pub attempts: u32,
    pub algorithm_version: String,
    pub exhaustion_explanations: Vec<String>,
}

fn fraction(random: &mut ChaCha8Rng) -> f64 {
    (random.next_u64() >> 11) as f64 / 9_007_199_254_740_992.0
}

fn between(random: &mut ChaCha8Rng, low: f64, high: f64) -> f64 {
    low + (high - low) * fraction(random)
}

pub fn generate_variants(
    spec: &NormalizedSpec,
    count: u32,
    seed: u64,
) -> Result<GenerationResult, ArchyError> {
    if !(1..=1000).contains(&count) {
        return Err(ArchyError::schema(
            "count",
            "Requested count must be 1..1000",
        ));
    }
    let [site_left, site_bottom, site_right, site_top] = site_bounds(spec)?;
    if spec.generation.shapes.is_empty() {
        return Err(ArchyError::schema(
            "generation.shapes",
            "At least one shape is required",
        ));
    }
    let setback = spec
        .constraints
        .iter()
        .filter(|constraint| constraint.kind == ConstraintKind::MinEdgeSetback)
        .map(|constraint| constraint.value_m)
        .fold(0.0_f64, f64::max);
    let [left, bottom, right, top] = [
        site_left + setback,
        site_bottom + setback,
        site_right - setback,
        site_top - setback,
    ];
    let (available_width, available_depth) = (right - left, top - bottom);
    let mut result = GenerationResult {
        variants: Vec::new(),
        requested_count: count,
        attempts: 0,
        algorithm_version: ALGORITHM_VERSION.into(),
        exhaustion_explanations: Vec::new(),
    };
    if ![left, bottom, right, top, available_width, available_depth]
        .iter()
        .all(|value| value.is_finite())
        || available_width <= 2.0 * MIN_DIMENSION_M
        || available_depth <= 2.0 * MIN_DIMENSION_M
    {
        result.exhaustion_explanations.push(
            "Buildable dimensions are absent or below supported numeric resolution after declared setbacks".into(),
        );
        return Ok(result);
    }
    let mut key = [0_u8; 32];
    key[..8].copy_from_slice(&seed.to_le_bytes());
    let mut random = ChaCha8Rng::from_seed(key);
    let mut seen = BTreeSet::new();
    let max_attempts = count.saturating_mul(100);
    for attempt in 0..max_attempts {
        result.attempts = attempt + 1;
        let shape_index = (random.next_u64() % spec.generation.shapes.len() as u64) as usize;
        let shape = spec.generation.shapes[shape_index];
        let sampled_width = available_width * between(&mut random, 0.45, 0.98);
        let target_depth =
            (spec.brief.target_gfa_m2 / f64::from(spec.brief.storeys)) / sampled_width;
        let target_fraction = if target_depth.is_finite() {
            (target_depth / available_depth).clamp(0.35, 0.95)
        } else {
            0.65
        };
        let sampled_depth = available_depth
            * between(
                &mut random,
                (target_fraction - 0.2).max(0.3),
                (target_fraction + 0.2).min(0.98),
            );
        let origin_x = between(&mut random, left, right - sampled_width);
        let origin_y = between(&mut random, bottom, top - sampled_depth);
        let parameters = match shape {
            Shape::Rectangle => ShapeParameters::Rectangle {
                origin_xy_m: [origin_x, origin_y],
                width_m: sampled_width,
                depth_m: sampled_depth,
            },
            Shape::LShape => ShapeParameters::LShape {
                origin_xy_m: [origin_x, origin_y],
                width_m: sampled_width,
                depth_m: sampled_depth,
                notch_width_m: sampled_width * between(&mut random, 0.2, 0.6),
                notch_depth_m: sampled_depth * between(&mut random, 0.2, 0.6),
            },
        };
        let Ok(signature) = parameters.geometric_signature() else {
            continue;
        };
        if !seen.insert(signature) {
            continue;
        }
        if let Ok(variant) = Variant::new(
            spec.project_id.clone(),
            seed,
            ALGORITHM_VERSION.into(),
            parameters,
            spec.brief.storeys,
            spec.brief.floor_to_floor_m,
        ) {
            result.variants.push(variant);
            if result.variants.len() == count as usize {
                break;
            }
        }
    }
    if result.variants.len() < count as usize {
        result.exhaustion_explanations.push(format!(
            "Only {} distinct representable canonical footprints found in {} bounded attempts",
            result.variants.len(),
            result.attempts
        ));
    }
    Ok(result)
}
