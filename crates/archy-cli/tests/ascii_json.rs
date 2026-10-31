use archy_cli::{ascii_json_compact, ascii_json_pretty};
use serde_json::{Value, json};
use std::process::Command;

#[test]
fn unicode_bmp_and_non_bmp_roundtrip_without_non_ascii_output_bytes() {
    let value = json!({"bmp":"é漢字", "non_bmp":"😀𝄞", "nested":["ASCII", "é😀"]});
    for output in [
        ascii_json_compact(&value).unwrap(),
        ascii_json_pretty(&value).unwrap(),
    ] {
        assert!(output.is_ascii());
        assert!(output.contains("\\u00e9"));
        assert!(output.contains("\\u6f22"));
        assert!(output.contains("\\ud83d\\ude00"));
        assert!(output.contains("\\ud834\\udd1e"));
        assert_eq!(serde_json::from_str::<Value>(&output).unwrap(), value);
        for cut in 0..=output.len() {
            let pieces = [&output.as_bytes()[..cut], &output.as_bytes()[cut..]];
            let decoded = pieces
                .iter()
                .map(|part| String::from_utf8_lossy(part).into_owned())
                .collect::<String>();
            assert_eq!(decoded, output);
        }
    }
}

#[test]
fn ascii_quotes_backslashes_controls_and_literal_escape_text_stay_exact() {
    let value = json!({"quote":"\"", "slash":"\\", "control":"\u{0000}\n\t\r", "literal":"\\u00e9", "ascii":"A/Z_0"});
    let pretty = ascii_json_pretty(&value).unwrap();
    let compact = ascii_json_compact(&value).unwrap();
    assert_eq!(pretty, serde_json::to_string_pretty(&value).unwrap());
    assert_eq!(compact, serde_json::to_string(&value).unwrap());
    assert_eq!(serde_json::from_str::<Value>(&pretty).unwrap(), value);
    assert_eq!(serde_json::from_str::<Value>(&compact).unwrap(), value);
}

#[test]
fn binary_reader_json_and_unicode_error_are_ascii_and_parseable() {
    let binary = env!("CARGO_BIN_EXE_archy");
    let project = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/courtyard-housing/project.yaml");
    for args in [
        vec!["doctor".to_string(), "--json".to_string()],
        vec![
            "desktop".to_string(),
            "capabilities".to_string(),
            "--json".to_string(),
        ],
        vec![
            "desktop".to_string(),
            "status".to_string(),
            "--project".to_string(),
            project.to_string_lossy().into_owned(),
            "--json".to_string(),
        ],
    ] {
        let output = Command::new(binary).args(&args).output().unwrap();
        assert!(
            output.status.code() == Some(0)
                || args[0] == "doctor" && output.status.code() == Some(1)
        );
        assert!(output.stdout.is_ascii(), "{args:?}");
        serde_json::from_slice::<Value>(&output.stdout).unwrap();
    }
    let missing =
        std::env::temp_dir().join(format!("archy-missing-é-{}.json", uuid::Uuid::new_v4()));
    let output = Command::new(binary)
        .args(["validate", "--project", missing.to_str().unwrap(), "--json"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(output.stdout.is_ascii());
    assert!(
        String::from_utf8(output.stdout.clone())
            .unwrap()
            .contains("\\u00e9")
    );
    let error: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(error["field"], missing.to_str().unwrap());
}
