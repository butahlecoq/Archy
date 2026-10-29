use archy_core::{ArchyError, InputFormat, parse_project};
use std::{fs::File, io::Read, path::Path};

pub fn run(path: &Path) -> Result<serde_json::Value,ArchyError> {
    let format=match path.extension().and_then(|e| e.to_str()) {
        Some("json")=>InputFormat::Json,Some("yaml"|"yml")=>InputFormat::Yaml,
        _=>return Err(ArchyError::schema("project","Expected a .yaml, .yml or .json manifest file")),
    };
    let mut input=String::new();
    File::open(path).map_err(|e|crate::io_error(path,e))?
        .take((archy_core::validation::MAX_INPUT_BYTES+1) as u64)
        .read_to_string(&mut input).map_err(|e|crate::io_error(path,e))?;
    let normalized=parse_project(&input,format)?;
    serde_json::to_value(normalized).map_err(|e|ArchyError::schema("$",e.to_string()))
}
