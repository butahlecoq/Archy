//! Deterministic bounded massing alternatives.
use archy_core::*;
use rand_chacha::ChaCha8Rng;
use rand_core::{Rng, SeedableRng};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

pub const ALGORITHM_VERSION: &str = "chacha8-u53-massing-v1";
pub const DIAGNOSTIC_ALGORITHM_VERSION: &str = "chacha8-u53-massing-diagnostics-v2";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RejectionPhase {
    Footprint,
    Variant,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RejectionSummary {
    pub phase: RejectionPhase,
    pub code: ErrorCode,
    pub field: String,
    pub message: String,
    pub count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AttemptDiagnostics {
    pub rejections: Vec<RejectionSummary>,
    pub duplicates: u32,
}

impl AttemptDiagnostics {
    fn record(&mut self, phase: RejectionPhase, error: ArchyError) {
        if let Some(existing) = self.rejections.iter_mut().find(|entry| {
            entry.phase == phase
                && entry.code == error.code
                && entry.field == error.field
                && entry.message == error.message
        }) {
            existing.count += 1;
        } else {
            self.rejections.push(RejectionSummary {
                phase,
                code: error.code,
                field: error.field,
                message: error.message,
                count: 1,
            });
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GenerationResult {
    pub variants: Vec<Variant>,
    pub requested_count: u32,
    pub attempts: u32,
    pub algorithm_version: String,
    pub exhaustion_explanations: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attempt_diagnostics: Option<AttemptDiagnostics>,
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
    generate(spec, count, seed, true)
}

/// Reproduce the original published generation bytes for historical run verification.
pub fn generate_variants_v1(
    spec: &NormalizedSpec,
    count: u32,
    seed: u64,
) -> Result<GenerationResult, ArchyError> {
    generate(spec, count, seed, false)
}

fn generate(
    spec: &NormalizedSpec,
    count: u32,
    seed: u64,
    diagnostics_enabled: bool,
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
        attempt_diagnostics: None,
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
    let mut diagnostics = AttemptDiagnostics {
        rejections: Vec::new(),
        duplicates: 0,
    };
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
        let signature = match parameters.geometric_signature() {
            Ok(signature) => signature,
            Err(error) => {
                if diagnostics_enabled {
                    diagnostics.record(RejectionPhase::Footprint, error);
                }
                continue;
            }
        };
        if !seen.insert(signature) {
            diagnostics.duplicates += 1;
            continue;
        }
        match Variant::new(
            spec.project_id.clone(),
            seed,
            ALGORITHM_VERSION.into(),
            parameters,
            spec.brief.storeys,
            spec.brief.floor_to_floor_m,
        ) {
            Ok(variant) => {
                result.variants.push(variant);
                if result.variants.len() == count as usize {
                    break;
                }
            }
            Err(error) => {
                if diagnostics_enabled {
                    diagnostics.record(RejectionPhase::Variant, error);
                }
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
    if diagnostics_enabled && (!diagnostics.rejections.is_empty() || diagnostics.duplicates != 0) {
        result.algorithm_version = DIAGNOSTIC_ALGORITHM_VERSION.into();
        result.attempt_diagnostics = Some(diagnostics);
    }
    Ok(result)
}
