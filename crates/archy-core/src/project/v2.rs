use super::v1;
use crate::{ArchyError, units::Units};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const SCHEMA_VERSION: &str = "0.2.0";
pub const NORMALIZATION_VERSION: &str = "si-rectangle-v2";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
pub enum SchemaVersion {
    #[serde(rename = "0.2.0")]
    V2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
pub enum NormalizationVersion {
    #[serde(rename = "si-rectangle-v2")]
    V2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
pub enum MeterUnit {
    #[serde(rename = "m")]
    M,
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Project {
    pub schema_version: SchemaVersion,
    pub id: String,
    pub units: Units,
    pub site: v1::Site,
    pub brief: v1::Brief,
    pub constraints: Vec<Constraint>,
    #[serde(default)]
    #[schemars(extend("maxItems" = 8, "uniqueItems" = true))]
    pub preferences: Vec<Preference>,
    /// Passive source text. It does not set a constraint or direct an operation.
    #[serde(default)]
    #[schemars(extend("maxItems" = 4))]
    #[schemars(extend("items" = {"type": "string", "maxLength": 512}))]
    pub notes: Vec<String>,
    #[serde(default)]
    pub generation: v1::Generation,
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Constraint {
    pub id: String,
    pub kind: v1::ConstraintKind,
    pub bound: f64,
    pub unit: Units,
    pub source: String,
    pub authority: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Preference {
    pub kind: PreferenceKind,
    #[schemars(extend("minLength" = 1, "maxLength" = 256, "pattern" = "^(?=.*\\S)[^\\u0000-\\u001f\\u007f-\\u009f<>`$;{}]*$"))]
    pub value: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PreferenceKind {
    MassingCharacter,
    LandscapeIntent,
    ArchitecturalIntent,
}

impl Preference {
    pub fn from_legacy(value: &str) -> Self {
        let kind = match value {
            "courtyard" => PreferenceKind::MassingCharacter,
            "retain trees" => PreferenceKind::LandscapeIntent,
            _ => PreferenceKind::ArchitecturalIntent,
        };
        Self {
            kind,
            value: if value == "retain trees" {
                "retain_trees"
            } else {
                value
            }
            .into(),
        }
    }
    pub fn legacy_label(&self) -> &str {
        if self.kind == PreferenceKind::LandscapeIntent && self.value == "retain_trees" {
            "retain trees"
        } else {
            &self.value
        }
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct NormalizedSpec {
    pub schema_version: SchemaVersion,
    pub id: String,
    pub units: MeterUnit,
    pub site: v1::NormalizedSite,
    pub brief: v1::NormalizedBrief,
    pub constraints: Vec<NormalizedConstraint>,
    #[schemars(extend("maxItems" = 8, "uniqueItems" = true))]
    pub preferences: Vec<Preference>,
    #[schemars(extend("maxItems" = 4))]
    pub notes: Vec<String>,
    pub generation: v1::Generation,
    pub normalization_version: NormalizationVersion,
    pub tolerance_m: f64,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct NormalizedConstraint {
    pub id: String,
    pub kind: v1::ConstraintKind,
    pub bound: f64,
    pub unit: MeterUnit,
    pub source: String,
    pub authority: String,
}

impl NormalizedSpec {
    pub(crate) fn from_numeric(
        spec: &v1::NormalizedSpec,
        preferences: Vec<Preference>,
        notes: Vec<String>,
    ) -> Self {
        Self {
            schema_version: SchemaVersion::V2,
            id: spec.project_id.clone(),
            units: MeterUnit::M,
            site: spec.site.clone(),
            brief: spec.brief.clone(),
            constraints: spec
                .constraints
                .iter()
                .map(|c| NormalizedConstraint {
                    id: c.id.clone(),
                    kind: c.kind,
                    bound: c.value_m,
                    unit: MeterUnit::M,
                    source: c.source.clone(),
                    authority: c.authority.clone(),
                })
                .collect(),
            preferences,
            notes,
            generation: spec.generation.clone(),
            normalization_version: NormalizationVersion::V2,
            tolerance_m: spec.tolerance_m,
        }
    }
    /// Revalidate a deserialized/public canonical value before numeric consumers use it.
    pub fn numeric_view(&self) -> Result<v1::NormalizedSpec, ArchyError> {
        crate::validation::v2::validate_preferences(&self.preferences)?;
        crate::validation::v2::validate_notes(&self.notes)?;
        let project = v1::Project {
            schema_version: "0.1.0".into(),
            project_id: self.id.clone(),
            units: Units::M,
            site: v1::Site {
                width: None,
                depth: None,
                width_m: None,
                depth_m: None,
                polygon_xy: None,
                polygon_xy_m: Some(self.site.polygon_xy_m.to_vec()),
                north_deg: self.site.north_angle_deg,
            },
            brief: v1::Brief {
                storeys: self.brief.storeys,
                floor_to_floor: None,
                floor_to_floor_m: Some(self.brief.floor_to_floor_m),
                target_gfa: None,
                target_gfa_m2: Some(self.brief.target_gfa_m2),
                target_gfa_hard: self.brief.target_gfa_hard,
            },
            constraints: self
                .constraints
                .iter()
                .map(|c| v1::Constraint {
                    id: c.id.clone(),
                    kind: c.kind,
                    value: None,
                    value_m: Some(c.bound),
                    unit: Some(Units::M),
                    source: c.source.clone(),
                    authority: Some(c.authority.clone()),
                })
                .collect(),
            preferences: self
                .preferences
                .iter()
                .map(|p| p.legacy_label().into())
                .collect(),
            generation: self.generation.clone(),
        };
        let numeric = crate::validation::v1::normalize(project)?;
        if Self::from_numeric(&numeric, self.preferences.clone(), self.notes.clone()) != *self {
            return Err(ArchyError::schema(
                "normalized",
                "Canonical project fields contradict normalized SI geometry or tolerance",
            ));
        }
        Ok(numeric)
    }
}
