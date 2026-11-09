use crate::{
    ArchyError,
    project::{v1, v2},
    validation::v1::{self as legacy, InputFormat, MAX_INPUT_BYTES},
};
use std::collections::HashSet;

#[derive(Debug, Clone)]
pub struct CurrentProject {
    pub canonical: v2::NormalizedSpec,
    pub numeric: v1::NormalizedSpec,
    pub source_schema_version: &'static str,
}

fn canonical_field(mut error: ArchyError, source: &v2::Project) -> ArchyError {
    error.field = match error.field.as_str() {
        "project_id" => "id".into(),
        "brief.floor_to_floor" if source.brief.floor_to_floor_m.is_some() => {
            "brief.floor_to_floor_m".into()
        }
        "brief.target_gfa" if source.brief.target_gfa_m2.is_some() => "brief.target_gfa_m2".into(),
        "site.polygon_xy" if source.site.polygon_xy_m.is_some() => "site.polygon_xy_m".into(),
        path if path.ends_with(".value") => format!("{}.bound", path.trim_end_matches(".value")),
        "site.width" if source.site.width_m.is_some() => "site.width_m".into(),
        "site.depth" if source.site.depth_m.is_some() => "site.depth_m".into(),
        path => path.into(),
    };
    error
}

fn parse_error(message: String) -> ArchyError {
    let mut field = message
        .split("ARCHY_PATH|")
        .nth(1)
        .and_then(|part| part.split('|').next())
        .filter(|part| !part.is_empty() && *part != ".")
        .unwrap_or("$")
        .to_owned();
    if field == "?" {
        field = message
            .split("duplicate mapping key: ")
            .nth(1)
            .and_then(|part| part.split([',', '\n']).next())
            .filter(|key| !key.is_empty() && key.len() <= 128)
            .unwrap_or("$")
            .into();
    }
    if field.starts_with("preferences") {
        let guidance = if field.ends_with(".kind") {
            "Use massing_character, landscape_intent, or architectural_intent"
        } else {
            "Use a bounded declarative preference with kind and value"
        };
        return ArchyError::schema(field, guidance);
    }
    let message = message
        .split("ARCHY_PATH|")
        .next()
        .filter(|part| !part.is_empty())
        .map(str::to_owned)
        .unwrap_or_else(|| {
            message
                .split("| ")
                .nth(1)
                .unwrap_or("Invalid project value")
                .to_owned()
        });
    if message.len() > 512 {
        return ArchyError::schema(field, "Invalid or oversized project value");
    }
    ArchyError::schema(field, message)
}

struct Tracked(v2::Project);
impl<'de> serde::Deserialize<'de> for Tracked {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        serde_path_to_error::deserialize(deserializer)
            .map(Self)
            .map_err(|error| {
                let mut path = error.path().to_string();
                let message = error.inner().to_string();
                for prefix in ["missing field `", "unknown field `", "duplicate field `"] {
                    if let Some(key) = message
                        .split(prefix)
                        .nth(1)
                        .and_then(|part| part.split('`').next())
                    {
                        if path == "." || path.is_empty() {
                            path = key.into();
                        } else if !path.ends_with(&format!(".{key}")) && path != key {
                            path.push_str(&format!(".{key}"));
                        }
                        break;
                    }
                }
                serde::de::Error::custom(format!("ARCHY_PATH|{path}| {message}"))
            })
    }
}

fn parse_v2(text: &str, format: InputFormat) -> Result<v2::Project, ArchyError> {
    let parsed: Tracked = match format {
        InputFormat::Json => serde_json::from_str(text).map_err(|e| parse_error(e.to_string()))?,
        InputFormat::Yaml => {
            let options = serde_saphyr::options! {
                strict_booleans: true, no_schema: true, reject_unsupported_tags: true,
                merge_keys: serde_saphyr::MergeKeyPolicy::Error,
                duplicate_keys: serde_saphyr::DuplicateKeyPolicy::Error,
                budget: serde_saphyr::budget! {
                    max_anchors: 0, max_aliases: 0, max_documents: 1,
                    max_depth: 32, flow_nesting_limit: 32, max_nodes: 20_000,
                    max_events: 40_000, max_total_scalar_bytes: MAX_INPUT_BYTES,
                    max_inclusion_depth: 0,
                },
            };
            serde_saphyr::from_str_with_options(text, options)
                .map_err(|e| parse_error(e.to_string()))?
        }
    };
    Ok(parsed.0)
}

/// Parsed form of a bounded preference value; only declared wishes enter the
/// canonical preference list. The grammar is lexical, not natural-language inference.
enum PreferenceForm {
    DeclaredWish,
    ExecutableInstruction,
}

fn preference_form(value: &str) -> PreferenceForm {
    let words: Vec<String> = value
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|word| !word.is_empty())
        .map(str::to_ascii_lowercase)
        .collect();
    let executable = |word: &str| {
        matches!(
            word,
            "rm" | "sudo"
                | "bash"
                | "zsh"
                | "powershell"
                | "cmd"
                | "python"
                | "python3"
                | "node"
                | "npm"
                | "npx"
                | "curl"
                | "wget"
                | "chmod"
                | "chown"
                | "sh"
                | "shell"
                | "script"
        )
    };
    let action = |word: &str| {
        matches!(
            word,
            "run" | "execute" | "exec" | "launch" | "invoke" | "delete" | "erase"
        )
    };
    if words
        .windows(2)
        .any(|pair| pair[0] == "please" && action(&pair[1]))
        || words.windows(3).any(|triple| {
            matches!(triple[0].as_str(), "could" | "would" | "can")
                && triple[1] == "you"
                && action(&triple[2])
        })
        || (words.first().is_some_and(|word| action(word))
            && words.get(1).is_none_or(|word| word != "of"))
    {
        return PreferenceForm::ExecutableInstruction;
    }
    for (index, word) in words.iter().enumerate() {
        let following = words[index + 1..]
            .iter()
            .take(3)
            .map(String::as_str)
            .find(|next| !matches!(*next, "a" | "an" | "the" | "this" | "that"));
        if matches!(
            word.as_str(),
            "rm" | "sudo" | "chmod" | "chown" | "curl" | "wget"
        ) || (matches!(
            word.as_str(),
            "run" | "execute" | "exec" | "launch" | "invoke"
        ) && following.is_some_and(executable))
            || (matches!(word.as_str(), "delete" | "erase")
                && following.is_some_and(|next| {
                    matches!(next, "file" | "files" | "directory" | "directories")
                }))
        {
            return PreferenceForm::ExecutableInstruction;
        }
    }
    PreferenceForm::DeclaredWish
}

pub(crate) fn validate_preferences(preferences: &[v2::Preference]) -> Result<(), ArchyError> {
    if preferences.len() > 8 {
        return Err(ArchyError::schema(
            "preferences",
            "At most eight distinct preferences are supported in schema 0.2.0",
        ));
    }
    let mut seen = HashSet::new();
    for (index, preference) in preferences.iter().enumerate() {
        let value = preference.value.as_str();
        if value.trim().is_empty() || value.chars().count() > 256 || value.len() > 512 {
            return Err(ArchyError::schema(
                format!("preferences[{index}].value"),
                "Preference must have 1-256 characters and at most 512 UTF-8 bytes",
            ));
        }
        let lower = value.to_ascii_lowercase();
        let numeric_performance_claim = value.chars().any(|c| c.is_ascii_digit())
            && [
                "daylight",
                "energy",
                "carbon",
                "cost",
                "thermal",
                "acoustic",
                "efficiency",
                "compliance",
            ]
            .iter()
            .any(|term| lower.contains(term));
        if value
            .chars()
            .any(|c| c.is_control() || matches!(c, '<' | '>' | '`' | '$' | ';' | '{' | '}'))
            || lower.contains("execute:")
            || lower.contains("python3 -c")
            || lower.contains("guarantee")
            || numeric_performance_claim
            || matches!(
                preference_form(value),
                PreferenceForm::ExecutableInstruction
            )
        {
            return Err(ArchyError::schema(
                format!("preferences[{index}].value"),
                "Preference must describe an architectural intention without commands or performance guarantees",
            ));
        }
        if !seen.insert(preference) {
            return Err(ArchyError::schema(
                format!("preferences[{index}]"),
                "Duplicate preference",
            ));
        }
    }
    Ok(())
}

pub(crate) fn validate_notes(notes: &[String]) -> Result<(), ArchyError> {
    if notes.len() > 4 {
        return Err(ArchyError::schema(
            "notes",
            "At most four passive notes are supported",
        ));
    }
    let mut total = 0;
    for (index, note) in notes.iter().enumerate() {
        if note.len() > 512 {
            return Err(ArchyError::schema(
                format!("notes[{index}]"),
                "Note exceeds 512 UTF-8 bytes",
            ));
        }
        total += note.len();
    }
    if total > 1024 {
        return Err(ArchyError::schema(
            "notes",
            "Notes exceed 1024 UTF-8 bytes in total",
        ));
    }
    Ok(())
}

// The discriminator only chooses a parser. The selected parser reads the
// original document with duplicate-key, YAML alias and field-path checks.
fn source_version(text: &str, format: InputFormat) -> Option<String> {
    match format {
        InputFormat::Yaml => text
            .lines()
            .find_map(|line| {
                if line.starts_with(char::is_whitespace) {
                    return None;
                }
                let (key, value) = line.split_once(':')?;
                if key.trim().trim_matches(['\'', '"']) != "schema_version" {
                    return None;
                }
                Some(
                    value
                        .trim()
                        .split('#')
                        .next()?
                        .trim()
                        .trim_matches(['\'', '"'])
                        .to_owned(),
                )
            })
            .or_else(|| {
                let value: serde_json::Value = serde_saphyr::from_str(text).ok()?;
                value.get("schema_version")?.as_str().map(str::to_owned)
            }),
        InputFormat::Json => {
            let bytes = text.as_bytes();
            let mut depth = 0usize;
            let mut index = 0usize;
            while index < bytes.len() {
                match bytes[index] {
                    b'{' | b'[' => depth += 1,
                    b'}' | b']' => depth = depth.saturating_sub(1),
                    b'"' => {
                        let start = index;
                        index += 1;
                        while index < bytes.len() {
                            if bytes[index] == b'\\' {
                                index += 2;
                                continue;
                            }
                            if bytes[index] == b'"' {
                                break;
                            }
                            index += 1;
                        }
                        if index >= bytes.len() {
                            return None;
                        }
                        if depth == 1 {
                            let key = serde_json::from_str::<String>(&text[start..=index]).ok()?;
                            let mut next = index + 1;
                            while next < bytes.len() && bytes[next].is_ascii_whitespace() {
                                next += 1;
                            }
                            if key == "schema_version" && bytes.get(next) == Some(&b':') {
                                next += 1;
                                while next < bytes.len() && bytes[next].is_ascii_whitespace() {
                                    next += 1;
                                }
                                if bytes.get(next) != Some(&b'"') {
                                    return None;
                                }
                                let value_start = next;
                                next += 1;
                                while next < bytes.len() {
                                    if bytes[next] == b'\\' {
                                        next += 2;
                                        continue;
                                    }
                                    if bytes[next] == b'"' {
                                        break;
                                    }
                                    next += 1;
                                }
                                if next >= bytes.len() {
                                    return None;
                                }
                                return serde_json::from_str::<String>(&text[value_start..=next])
                                    .ok();
                            }
                        }
                    }
                    _ => {}
                }
                index += 1;
            }
            None
        }
    }
}

pub fn parse_project_current(
    text: &str,
    format: InputFormat,
) -> Result<CurrentProject, ArchyError> {
    if text.len() > MAX_INPUT_BYTES {
        return Err(ArchyError::schema("$", "Project exceeds 1 MiB input limit"));
    }
    match source_version(text, format).as_deref() {
        Some("0.1.0") => {
            let numeric = legacy::parse_project(text, format).map_err(|error| {
                if error.field == "id" || error.field == "notes" || error.field.ends_with(".bound")
                {
                    return ArchyError::schema(
                        "schema_version",
                        "Canonical v2 fields require schema_version 0.2.0",
                    );
                }
                if let Some((_, public_message)) = error.message.split_once("| ") {
                    ArchyError::schema(error.field, public_message.to_owned())
                } else {
                    error
                }
            })?;
            let mut preferences = Vec::new();
            for value in &numeric.preferences {
                let preference = v2::Preference::from_legacy(value);
                preferences.push(preference);
            }
            validate_preferences(&preferences)?;
            let canonical = v2::NormalizedSpec::from_numeric(&numeric, preferences, Vec::new());
            Ok(CurrentProject {
                canonical,
                numeric,
                source_schema_version: "0.1.0",
            })
        }
        Some(v2::SCHEMA_VERSION) => {
            let project = parse_v2(text, format)?;
            validate_preferences(&project.preferences)?;
            validate_notes(&project.notes)?;
            let preferences = project.preferences.clone();
            let notes = project.notes.clone();
            let source = project.clone();
            let v1_project = v1::Project {
                schema_version: "0.1.0".into(),
                project_id: project.id,
                units: project.units,
                site: project.site,
                brief: project.brief,
                constraints: project
                    .constraints
                    .into_iter()
                    .map(|c| v1::Constraint {
                        id: c.id,
                        kind: c.kind,
                        value: Some(c.bound),
                        value_m: None,
                        unit: Some(c.unit),
                        source: c.source,
                        authority: Some(c.authority),
                    })
                    .collect(),
                preferences: preferences
                    .iter()
                    .map(|p| p.legacy_label().into())
                    .collect(),
                generation: project.generation,
            };
            let numeric =
                legacy::normalize(v1_project).map_err(|error| canonical_field(error, &source))?;
            let canonical = v2::NormalizedSpec::from_numeric(&numeric, preferences, notes);
            Ok(CurrentProject {
                canonical,
                numeric,
                source_schema_version: v2::SCHEMA_VERSION,
            })
        }
        _ => Err(ArchyError::schema(
            "schema_version",
            "Supported source versions are 0.1.0 and 0.2.0",
        )),
    }
}
