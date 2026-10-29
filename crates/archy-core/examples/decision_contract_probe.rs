//! Test driver for schema and actual serde/domain validation.
use archy_core::decision_contract::{DecisionRecord, Proposal};
use serde_json::Value;
use std::io::{self, Read};

fn main() {
    let mut input = String::new();
    io::stdin().read_to_string(&mut input).unwrap();
    let cases: Vec<Value> = serde_json::from_str(&input).unwrap();
    let results: Vec<_> = cases
        .into_iter()
        .map(|case| {
            let value = case["value"].clone();
            match case["kind"].as_str().unwrap() {
                "proposal" => {
                    serde_json::from_value::<Proposal>(value).is_ok_and(|v| v.validate().is_ok())
                }
                "decision" => serde_json::from_value::<DecisionRecord>(value)
                    .is_ok_and(|v| v.validate().is_ok()),
                _ => false,
            }
        })
        .collect();
    println!("{}", serde_json::to_string(&results).unwrap());
}
