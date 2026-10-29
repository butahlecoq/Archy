mod blender;
mod ifc;

use archy_core::{ArchyError, ErrorCode};
use schemars::JsonSchema;
use serde::Deserialize;
use serde::de::{self, DeserializeOwned, MapAccess, SeqAccess, Visitor};
use std::fmt;

pub use blender::{BlenderResponse, decode_blender_response};
pub use ifc::{IfcResponse, decode_ifc_response};

pub(crate) const MAX_RESPONSE_BYTES: usize = 1024 * 1024;

pub(crate) fn bad(field: &str, message: &str) -> ArchyError {
    ArchyError::new(ErrorCode::ArtifactInvalid, field, message)
}

pub(crate) fn decode<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, ArchyError> {
    if bytes.len() > MAX_RESPONSE_BYTES {
        return Err(bad("$", "worker response exceeds 1 MiB"));
    }
    let unique: UniqueValue = serde_json::from_slice(bytes)
        .map_err(|e| bad("$", &format!("invalid worker JSON: {e}")))?;
    serde_json::from_value(unique.0).map_err(|e| bad("$", &format!("invalid worker response: {e}")))
}

struct UniqueValue(serde_json::Value);
impl<'de> Deserialize<'de> for UniqueValue {
    fn deserialize<D: de::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct UniqueVisitor;
        impl<'de> Visitor<'de> for UniqueVisitor {
            type Value = UniqueValue;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("JSON without duplicate keys")
            }
            fn visit_bool<E: de::Error>(self, v: bool) -> Result<Self::Value, E> {
                Ok(UniqueValue(v.into()))
            }
            fn visit_i64<E: de::Error>(self, v: i64) -> Result<Self::Value, E> {
                Ok(UniqueValue(v.into()))
            }
            fn visit_u64<E: de::Error>(self, v: u64) -> Result<Self::Value, E> {
                Ok(UniqueValue(v.into()))
            }
            fn visit_f64<E: de::Error>(self, v: f64) -> Result<Self::Value, E> {
                if !v.is_finite() {
                    return Err(E::custom("nonfinite number"));
                }
                Ok(UniqueValue(serde_json::Value::from(v)))
            }
            fn visit_str<E: de::Error>(self, v: &str) -> Result<Self::Value, E> {
                Ok(UniqueValue(v.into()))
            }
            fn visit_string<E: de::Error>(self, v: String) -> Result<Self::Value, E> {
                Ok(UniqueValue(v.into()))
            }
            fn visit_none<E: de::Error>(self) -> Result<Self::Value, E> {
                Ok(UniqueValue(serde_json::Value::Null))
            }
            fn visit_unit<E: de::Error>(self) -> Result<Self::Value, E> {
                Ok(UniqueValue(serde_json::Value::Null))
            }
            fn visit_seq<A: SeqAccess<'de>>(self, mut access: A) -> Result<Self::Value, A::Error> {
                let mut values = Vec::new();
                while let Some(value) = access.next_element::<UniqueValue>()? {
                    values.push(value.0);
                }
                Ok(UniqueValue(values.into()))
            }
            fn visit_map<A: MapAccess<'de>>(self, mut access: A) -> Result<Self::Value, A::Error> {
                let mut values = serde_json::Map::new();
                while let Some(key) = access.next_key::<String>()? {
                    if values.contains_key(&key) {
                        return Err(de::Error::custom("duplicate JSON key"));
                    }
                    values.insert(key, access.next_value::<UniqueValue>()?.0);
                }
                Ok(UniqueValue(serde_json::Value::Object(values)))
            }
        }
        deserializer.deserialize_any(UniqueVisitor)
    }
}

pub(crate) fn finite(values: impl IntoIterator<Item = f64>, field: &str) -> Result<(), ArchyError> {
    if values.into_iter().any(|v| !v.is_finite()) {
        return Err(bad(field, "nonfinite worker measurement"));
    }
    Ok(())
}

pub(crate) fn matrix(value: &[[f64; 4]; 4], field: &str) -> Result<(), ArchyError> {
    finite(value.iter().flatten().copied(), field)?;
    if value[3]
        .iter()
        .zip([0., 0., 0., 1.])
        .any(|(got, want)| (got - want).abs() > 1e-6)
    {
        return Err(bad(field, "nonaffine transform"));
    }
    Ok(())
}

pub(crate) fn same(got: &str, expected: &str, field: &str) -> Result<(), ArchyError> {
    if got != expected {
        return Err(bad(field, "worker identity or fixed setting mismatch"));
    }
    Ok(())
}

pub(crate) fn near(got: f64, expected: f64, tol: f64, field: &str) -> Result<(), ArchyError> {
    if !got.is_finite() || (got - expected).abs() > tol {
        return Err(bad(field, "worker numeric setting mismatch"));
    }
    Ok(())
}

/// A worker claim only. A09 must open the file independently and recompute size/hash.
#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ClaimedArtifact {
    pub path: String,
    pub sha256: String,
    pub media_type: String,
    pub producer: String,
    pub size_bytes: u64,
}

pub(crate) fn artifacts(
    items: &[ClaimedArtifact],
    expected: &[(&str, &str)],
    producer: &str,
) -> Result<(), ArchyError> {
    if items.len() != expected.len() {
        return Err(bad("artifacts", "incomplete worker artifact inventory"));
    }
    for (item, (name, media)) in items.iter().zip(expected) {
        same(&item.path, name, "artifacts.path")?;
        same(&item.media_type, media, "artifacts.media_type")?;
        same(&item.producer, producer, "artifacts.producer")?;
        if item.size_bytes == 0
            || item.sha256.len() != 64
            || !item
                .sha256
                .bytes()
                .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
        {
            return Err(bad("artifacts", "invalid claimed size or SHA-256"));
        }
    }
    Ok(())
}

#[derive(Debug, JsonSchema)]
pub struct BackendContract {
    pub blender_request: crate::request::BlenderWire,
    pub ifc_request: crate::request::IfcWire,
    pub blender_response: BlenderResponse,
    pub ifc_response: IfcResponse,
}
