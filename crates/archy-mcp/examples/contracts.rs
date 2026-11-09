use archy_mcp::tools::definitions;
use serde_json::{Map, Value};
fn main() {
    let mut schemas = Map::new();
    for tool in definitions() {
        schemas.insert(
            tool.name.to_string(),
            serde_json::json!({"input": tool.input_schema, "output": tool.output_schema}),
        );
    }
    println!("{}", Value::Object(schemas));
}
