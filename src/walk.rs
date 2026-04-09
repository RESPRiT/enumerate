use std::path::Path;

use anyhow::{Context, Result};
use indexmap::IndexMap;

use crate::doc::{self, Case, DECISION_COLUMN};

/// A walked item with pre-rendered scaffold and raw field content.
struct WalkItem {
    scaffold: String,
    fields: IndexMap<String, String>,
}

/// Marker priority for walk ordering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum MarkerPriority {
    MustDiscuss,   // !!
    ShouldDiscuss, // !
    Clarify,       // ?
}

/// Parse the marker prefix from a Decision value.
/// Returns (priority, remaining note text) if it's a walkable marker.
fn parse_marker(decision: &str) -> Option<(MarkerPriority, &str)> {
    let trimmed = decision.trim();
    if let Some(rest) = trimmed.strip_prefix("!!") {
        Some((MarkerPriority::MustDiscuss, rest.trim()))
    } else if let Some(rest) = trimmed.strip_prefix('!') {
        Some((MarkerPriority::ShouldDiscuss, rest.trim()))
    } else if let Some(rest) = trimmed.strip_prefix('?') {
        Some((MarkerPriority::Clarify, rest.trim()))
    } else {
        None
    }
}

fn marker_badge(priority: MarkerPriority) -> &'static str {
    match priority {
        MarkerPriority::MustDiscuss => "`(!!)`",
        MarkerPriority::ShouldDiscuss => "`(!)`",
        MarkerPriority::Clarify => "`(?)`",
    }
}

fn marker_tally_badge(priority: MarkerPriority) -> &'static str {
    match priority {
        MarkerPriority::MustDiscuss => "`(!!)`",
        MarkerPriority::ShouldDiscuss => "`(!)`",
        MarkerPriority::Clarify => "`(?)`",
    }
}

const BAR_WIDTH: usize = 60;
const BAR_CHAR: char = '━';

fn render_divider(index: usize, total: usize) -> String {
    let bar: String = std::iter::repeat(BAR_CHAR).take(BAR_WIDTH).collect();
    let counter = format!("[{} of {}]", index + 1, total);
    let padding = BAR_WIDTH.saturating_sub(counter.len());
    let spaces: String = std::iter::repeat(' ').take(padding).collect();
    format!("`{bar}`\n`{spaces}{counter}`")
}

fn render_scaffold(
    case: &Case,
    priority: MarkerPriority,
    note: &str,
    index: usize,
    total: usize,
) -> String {
    let divider = render_divider(index, total);
    let badge = marker_badge(priority);
    let header = format!("{badge} **#{} {}**", case.number, case.name);

    let mut scaffold = format!("{divider}\n{header}");

    if !note.is_empty() {
        scaffold.push_str(&format!("\n\n> Your note: *{note}*"));
    }

    scaffold
}

fn render_orientation(counts: &[(MarkerPriority, usize)], total: usize) -> String {
    let mut parts = Vec::new();
    for &(priority, count) in counts {
        if count > 0 {
            parts.push(format!("{} \u{00d7} {count}", marker_tally_badge(priority)));
        }
    }
    format!("**Walking {total} items:** {}", parts.join(", "))
}

/// Run the walk subcommand: parse the doc, filter+sort by marker, output YAML.
pub fn run(file: &Path) -> Result<()> {
    let input = std::fs::read_to_string(file)
        .with_context(|| format!("failed to read {}", file.display()))?;

    let load_result = doc::parse(&input).context("failed to parse document")?;
    let doc = load_result.doc;

    // Collect walkable cases: (priority, note, case)
    let mut items: Vec<(MarkerPriority, String, Case)> = Vec::new();
    for group in &doc.groups {
        for case in &group.cases {
            let decision = case.fields.get(DECISION_COLUMN).map(|s| s.as_str()).unwrap_or("");
            if let Some((priority, note)) = parse_marker(decision) {
                items.push((priority, note.to_string(), case.clone()));
            }
        }
    }

    // Sort by priority (MustDiscuss < ShouldDiscuss < Clarify, which is the Ord we want)
    items.sort_by_key(|(priority, _, _)| *priority);

    let total = items.len();

    // Count by priority for orientation header
    let mut must = 0usize;
    let mut should = 0usize;
    let mut clarify = 0usize;
    for &(priority, _, _) in &items {
        match priority {
            MarkerPriority::MustDiscuss => must += 1,
            MarkerPriority::ShouldDiscuss => should += 1,
            MarkerPriority::Clarify => clarify += 1,
        }
    }

    let orientation = render_orientation(
        &[
            (MarkerPriority::MustDiscuss, must),
            (MarkerPriority::ShouldDiscuss, should),
            (MarkerPriority::Clarify, clarify),
        ],
        total,
    );

    // Build walk items
    let walk_items: Vec<WalkItem> = items
        .iter()
        .enumerate()
        .map(|(i, (priority, note, case))| {
            let scaffold = render_scaffold(case, *priority, note, i, total);
            // Fields minus Decision column
            let fields: IndexMap<String, String> = case
                .fields
                .iter()
                .filter(|(k, _)| k.as_str() != DECISION_COLUMN)
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect();
            WalkItem { scaffold, fields }
        })
        .collect();

    // Output YAML
    print!("orientation: |\n");
    for line in orientation.lines() {
        print!("  {line}\n");
    }

    print!("items:\n");
    for item in &walk_items {
        print!("  - scaffold: |\n");
        for line in item.scaffold.lines() {
            print!("      {line}\n");
        }
        print!("    fields:\n");
        for (key, value) in &item.fields {
            if value.contains('\n') {
                print!("      {key}: |\n");
                for line in value.lines() {
                    print!("        {line}\n");
                }
            } else {
                // Quote values that could confuse YAML
                let needs_quoting = value.is_empty()
                    || value.starts_with(|c: char| "{}[]&*?|>!%@`#,".contains(c))
                    || value.contains(": ")
                    || value.contains('#');
                if needs_quoting {
                    let escaped = value.replace('"', "\\\"");
                    print!("      {key}: \"{escaped}\"\n");
                } else {
                    print!("      {key}: {value}\n");
                }
            }
        }
    }

    Ok(())
}
