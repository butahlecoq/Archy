use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum Units {
    #[serde(rename = "m")]
    M,
    #[serde(rename = "mm")]
    Mm,
    #[serde(rename = "cm")]
    Cm,
    #[serde(rename = "ft")]
    Ft,
}
impl Units {
    pub fn meters(self) -> f64 {
        match self {
            Self::M => 1.0,
            Self::Mm => 0.001,
            Self::Cm => 0.01,
            Self::Ft => 0.3048,
        }
    }
}
