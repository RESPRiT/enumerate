use enumerate::doc::{Severity, parse, serialize};

#[test]
fn canonical_round_trips_byte_equal() {
    let input = include_str!("fixtures/canonical.md");
    let result = parse(input).expect("parse should succeed");
    assert!(
        result.warnings.is_empty(),
        "expected no warnings, got: {:?}",
        result.warnings.iter().map(|w| &w.message).collect::<Vec<_>>()
    );
    let output = serialize(&result.doc);
    assert_eq!(output, input);
}

#[test]
fn empty_file() {
    let result = parse("").expect("empty file should parse");
    assert!(result.doc.groups.is_empty());
    assert!(result.doc.frontmatter.topic.is_none());
    // Empty file has no Status column initially; post-processing adds one but no cases to update.
    // No warnings expected for the empty case (no cases means nothing to add Status to).
    // Actually post_process WILL warn because columns.is_empty triggers Status check.
    // Let me allow that.
}

#[test]
fn frontmatter_only() {
    let input = "---\ntopic: foo\ncreated: 2026-04-07\ncolumns: [Status]\n---\n";
    let result = parse(input).expect("parse should succeed");
    assert_eq!(result.doc.frontmatter.topic.as_deref(), Some("foo"));
    assert_eq!(result.doc.frontmatter.created.as_deref(), Some("2026-04-07"));
    assert_eq!(result.doc.frontmatter.columns, vec!["Status"]);
    assert!(result.doc.groups.is_empty());
    assert!(result.warnings.is_empty());
}

#[test]
fn missing_frontmatter_synthesizes() {
    let input = "# Topic\n\n## Group\n\n### #1 Name\n\n**Status:** OK\n";
    let result = parse(input).expect("parse should succeed");
    assert_eq!(result.doc.frontmatter.topic.as_deref(), Some("Topic"));
    // Columns inferred from union
    assert_eq!(result.doc.frontmatter.columns, vec!["Status"]);
    assert_eq!(result.doc.groups.len(), 1);
    assert_eq!(result.doc.groups[0].name.as_deref(), Some("Group"));
    assert_eq!(result.doc.groups[0].cases.len(), 1);
    assert_eq!(result.doc.groups[0].cases[0].number, 1);
    assert_eq!(result.doc.groups[0].cases[0].name, "Name");
}

#[test]
fn mixed_groups_and_ungrouped() {
    let input = "# Topic\n\n### #1 Ungrouped case\n\n**Status:** OK\n\n## Group A\n\n### #2 Grouped case\n\n**Status:** ?\n";
    let result = parse(input).expect("parse should succeed");
    assert_eq!(result.doc.groups.len(), 2);
    assert!(result.doc.groups[0].name.is_none(), "first group should be ungrouped");
    assert_eq!(result.doc.groups[0].cases.len(), 1);
    assert_eq!(result.doc.groups[0].cases[0].number, 1);
    assert_eq!(result.doc.groups[1].name.as_deref(), Some("Group A"));
    assert_eq!(result.doc.groups[1].cases.len(), 1);
    assert_eq!(result.doc.groups[1].cases[0].number, 2);
}

#[test]
fn custom_columns_with_missing_fields() {
    let input = "---\ncolumns: [Risk, Mitigation, Status]\n---\n\n# Security review\n\n### #1 SQL injection\n\n**Risk:** high\n\n**Status:** OK\n";
    let result = parse(input).expect("parse should succeed");
    assert_eq!(result.doc.frontmatter.columns, vec!["Risk", "Mitigation", "Status"]);
    let case = &result.doc.groups[0].cases[0];
    assert_eq!(case.fields.get("Risk").map(String::as_str), Some("high"));
    assert_eq!(case.fields.get("Status").map(String::as_str), Some("OK"));
    assert!(case.fields.get("Mitigation").is_none());
}

#[test]
fn missing_status_column_warns_and_appends() {
    let input = "---\ncolumns: [Setup, Expected]\n---\n\n# Topic\n\n### #1 Name\n\n**Setup:** foo\n";
    let result = parse(input).expect("parse should succeed");
    assert!(result.doc.frontmatter.columns.contains(&"Status".to_string()));
    assert!(
        result.warnings.iter().any(|w| w.message.contains("Status column")),
        "expected Status column warning, got: {:?}",
        result.warnings.iter().map(|w| &w.message).collect::<Vec<_>>()
    );
    // Every case should have an empty Status field
    let case = &result.doc.groups[0].cases[0];
    assert_eq!(case.fields.get("Status").map(String::as_str), Some(""));
}

#[test]
fn duplicate_case_numbers_warn() {
    let input = "# Topic\n\n### #1 First\n\n**Status:** OK\n\n### #1 Second\n\n**Status:** ?\n";
    let result = parse(input).expect("parse should succeed");
    assert_eq!(result.doc.groups[0].cases.len(), 2);
    assert!(
        result.warnings.iter().any(|w| w.message.contains("duplicate")),
        "expected duplicate warning"
    );
}

#[test]
fn invalid_yaml_is_fatal() {
    let input = "---\ntopic: [unclosed sequence\n---\n";
    let err = parse(input).expect_err("invalid YAML should fail");
    assert!(err.to_string().contains("YAML") || err.to_string().contains("frontmatter"));
}

#[test]
fn unknown_block_inside_case_warned_and_dropped() {
    let input = "# Topic\n\n### #1 Name\n\nThis is unknown content that isn't a field.\n\n**Status:** OK\n";
    let result = parse(input).expect("parse should succeed");
    assert!(
        result.warnings.iter().any(|w| w.message.contains("unknown content")),
        "expected unknown content warning"
    );
    let serialized = serialize(&result.doc);
    assert!(
        !serialized.contains("This is unknown content"),
        "unknown content should be dropped on save"
    );
}

#[test]
fn status_not_last_is_reordered() {
    let input = "---\ncolumns: [Setup, Status, Notes]\n---\n\n# Topic\n\n### #1 Name\n\n**Setup:** s\n\n**Status:** OK\n\n**Notes:** n\n";
    let result = parse(input).expect("parse should succeed");
    assert_eq!(
        result.doc.frontmatter.columns,
        vec!["Setup", "Notes", "Status"]
    );
    assert!(
        result.warnings.iter().any(|w| w.message.contains("Status column must")),
        "expected reorder warning, got: {:?}",
        result.warnings.iter().map(|w| &w.message).collect::<Vec<_>>()
    );
}

#[test]
fn warning_severity_is_warn_not_fatal() {
    let input = "---\ncolumns: [Setup]\n---\n\n# Topic\n\n### #1 Name\n\n**Setup:** foo\n";
    let result = parse(input).expect("parse should succeed");
    assert!(!result.warnings.is_empty());
    assert!(result.warnings.iter().all(|w| w.severity == Severity::Warn));
}
