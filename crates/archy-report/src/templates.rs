use crate::ReportDocument;
use archy_core::{ConstraintKind, NormalizedSpec};
use std::fmt::Write;

fn check_unit(spec: &NormalizedSpec, constraint_id: &str) -> Option<&'static str> {
    match constraint_id {
        "site_containment" => Some("m"),
        "target_gfa" => Some("m2"),
        _ => spec
            .constraints
            .iter()
            .find(|constraint| constraint.id == constraint_id)
            .map(|constraint| match constraint.kind {
                ConstraintKind::MaxHeight | ConstraintKind::MinEdgeSetback => "m",
            }),
    }
}

fn check_value(value: Option<f64>, unit: Option<&str>) -> String {
    match (value, unit) {
        (Some(value), Some(unit)) => format!("{value} {unit}"),
        (Some(value), None) => value.to_string(),
        (None, _) => "uncomputed".into(),
    }
}

fn escaped(value: &str) -> String {
    let mut out = String::new();
    for ch in value.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            '\n' | '\r' => out.push('⏎'),
            '\t' => out.push(' '),
            '\\' | '`' | '*' | '_' | '{' | '}' | '[' | ']' | '(' | ')' | '#' | '+' | '-' | '.'
            | '!' | '|' | '~' => {
                out.push('\\');
                out.push(ch);
            }
            character if character.is_control() => out.push('�'),
            character => out.push(character),
        }
    }
    out
}

pub(crate) fn markdown(doc: &ReportDocument) -> String {
    let mut out = String::from("# Archy schematic report\n\n");
    let context = &doc.context;
    writeln!(out, "Report version: {}  ", escaped(&doc.schema_version)).unwrap();
    writeln!(out, "Project: {}  ", escaped(&doc.spec.project_id)).unwrap();
    writeln!(out, "Run: {}  ", escaped(&context.run_id)).unwrap();
    writeln!(out, "Operation: {}  ", escaped(&context.operation)).unwrap();
    writeln!(
        out,
        "Parent run: {}  ",
        context
            .parent_run_id
            .as_deref()
            .map(escaped)
            .unwrap_or_else(|| "none".into())
    )
    .unwrap();
    writeln!(
        out,
        "Parent variant: {}  ",
        context
            .parent_variant_id
            .as_deref()
            .map(escaped)
            .unwrap_or_else(|| "none".into())
    )
    .unwrap();
    writeln!(out, "Input SHA-256: {}  ", escaped(&context.input_sha256)).unwrap();
    writeln!(
        out,
        "Normalized SHA-256: {}  ",
        escaped(&context.normalized_sha256)
    )
    .unwrap();
    writeln!(out, "Algorithm: {}  ", escaped(&context.algorithm_version)).unwrap();
    writeln!(out, "Build: {}\n", escaped(&context.build_version)).unwrap();
    writeln!(
        out,
        "Requested count: {}  ",
        context
            .requested_count
            .map(|value| value.to_string())
            .unwrap_or_else(|| "not recorded".into())
    )
    .unwrap();
    writeln!(
        out,
        "Attempts: {}\n",
        context
            .attempts
            .map(|value| value.to_string())
            .unwrap_or_else(|| "not recorded".into())
    )
    .unwrap();
    out.push_str("## Exhaustion details\n\n");
    for explanation in &context.exhaustion_explanations {
        writeln!(out, "- {}", escaped(explanation)).unwrap();
    }
    if context.exhaustion_explanations.is_empty() {
        out.push_str("None recorded.\n");
    }
    out.push_str("\n## Preferences\n\n");
    for preference in &doc.spec.preferences {
        writeln!(out, "- {}", escaped(preference)).unwrap();
    }
    if doc.spec.preferences.is_empty() {
        out.push_str("None declared.\n");
    }
    out.push_str("\n## Declared constraints\n\n");
    for constraint in &doc.spec.constraints {
        writeln!(
            out,
            "- {}: {:?}, {} m; source: {}; authority: {} (unverified declaration)",
            escaped(&constraint.id),
            constraint.kind,
            constraint.value_m,
            escaped(&constraint.source),
            escaped(&constraint.authority)
        )
        .unwrap();
    }
    out.push_str("\n## Variants and measurements\n\n");
    for item in &doc.variants {
        writeln!(out, "### {}\n", escaped(&item.variant.id)).unwrap();
        writeln!(
            out,
            "Shape: {:?}; storeys: {}; floor-to-floor: {} m\n",
            item.variant.shape, item.variant.storeys, item.variant.floor_to_floor_m
        )
        .unwrap();
        out.push_str("Metrics:\n\n");
        for metric in &item.evaluation.metrics {
            writeln!(
                out,
                "- {}: {} {} (method: {})",
                escaped(&metric.name),
                metric.value,
                escaped(&metric.unit),
                escaped(&metric.method_version)
            )
            .unwrap();
        }
        out.push_str("\nChecks:\n\n");
        for check in &item.evaluation.checks {
            let unit = check_unit(&doc.spec, &check.constraint_id);
            writeln!(
                out,
                "- {}: {:?}; measured: {}; limit: {}; required: {}; {}",
                escaped(&check.constraint_id),
                check.result,
                check_value(check.measured, unit),
                check_value(check.limit, unit),
                check.required,
                escaped(&check.explanation)
            )
            .unwrap();
        }
        writeln!(
            out,
            "\nAll required checks passed: {}\n",
            item.evaluation.required_checks_passed
        )
        .unwrap();
    }
    out.push_str("## Uncomputed analyses\n\nDaylight: uncomputed  \nCost: uncomputed\n\n## Professional limits\n\n");
    out.push_str(&escaped(&doc.professional_limits));
    out.push('\n');
    out
}
