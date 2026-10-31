//! Source-derived backend schema and exact Rust request serializations.
use archy_adapters::{BlenderRequest, IfcRequest, backend_schema};
use archy_core::{NormalizedSpec, Variant};

fn main() {
    let spec: NormalizedSpec = serde_json::from_str(include_str!(
        "../../../tests/fixtures/briefs/courtyard.normalized.json"
    ))
    .unwrap();
    let generation: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tests/fixtures/generation/courtyard.seed42.json"
    ))
    .unwrap();
    let variant: Variant = serde_json::from_value(generation["variants"][0].clone()).unwrap();
    let blender = BlenderRequest::new(
        &spec,
        &variant,
        "request-1",
        "run-1",
        &"a".repeat(64),
        "runs/run-1/blender",
    )
    .unwrap();
    let ifc = IfcRequest::new(
        &spec,
        &variant,
        "req-1",
        "run-1",
        &"a".repeat(64),
        "runs/run-1/ifc",
        vec![],
    )
    .unwrap();
    println!(
        "{}",
        serde_json::json!({
            "schemas": {"backend": backend_schema()},
            "serialized": {"blender": String::from_utf8(blender.to_bytes().unwrap()).unwrap(),
                           "ifc": String::from_utf8(ifc.to_bytes().unwrap()).unwrap()}
        })
    );
}
