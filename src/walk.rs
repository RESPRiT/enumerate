use std::collections::HashSet;
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

const BAR_CHAR: char = '━';

/// Divider width used when the pane can't be measured — no tmux, or tmux
/// declined to answer. Matches the width the walk shipped with before the
/// divider became pane-relative, so an unmeasured run looks unremarkable.
const DEFAULT_BAR_WIDTH: usize = 60;

/// Floor and ceiling on the divider width. The floor keeps a very narrow pane
/// from collapsing the bar to a stub shorter than the `[N of M]` counter it
/// right-aligns against; the ceiling keeps a wide terminal from rendering a
/// full-width wall of `━` that reads as noise rather than as a separator.
const MIN_BAR_WIDTH: usize = 20;
const MAX_BAR_WIDTH: usize = 100;

/// Columns of left gutter the agent's rendered message sits behind, subtracted
/// from the pane width so the bar ends inside the window rather than wrapping.
const GUTTER_COLS: usize = 2;

/// Width of the tmux pane this process was launched from, in columns.
///
/// `enumerate walk` runs as a child of the agent, whose stdout is a pipe rather
/// than a terminal — so there is no window size to read from the fd, and
/// `$COLUMNS` arrives as 0. tmux is the only source that knows the real size.
/// `$TMUX_PANE` names the pane the caller occupies; without it tmux answers for
/// the session's active pane, which is the same pane in the common case.
///
/// Returns `None` whenever the measurement isn't trustworthy — outside tmux, if
/// the binary is missing, or if the reply doesn't parse as a number — leaving
/// the caller to fall back rather than act on a guess.
fn tmux_pane_width() -> Option<usize> {
    if std::env::var_os("TMUX").is_none() {
        return None;
    }

    let mut cmd = std::process::Command::new("tmux");
    cmd.arg("display-message").arg("-p");
    if let Some(pane) = std::env::var_os("TMUX_PANE") {
        cmd.arg("-t").arg(pane);
    }
    cmd.arg("#{pane_width}");

    let output = cmd.output().ok()?;
    if !output.status.success() {
        return None;
    }
    std::str::from_utf8(&output.stdout).ok()?.trim().parse().ok()
}

/// Resolve the divider width from an explicit override or a pane measurement.
///
/// `override_width` is the `--width` escape hatch: it replaces the measurement,
/// not the clamp, so a wildly out-of-range value still renders a usable bar.
fn resolve_bar_width(override_width: Option<usize>) -> usize {
    let measured = match override_width {
        Some(width) => width,
        None => tmux_pane_width()
            .map(|cols| cols.saturating_sub(GUTTER_COLS))
            .unwrap_or(DEFAULT_BAR_WIDTH),
    };
    measured.clamp(MIN_BAR_WIDTH, MAX_BAR_WIDTH)
}

fn render_divider(index: usize, total: usize, bar_width: usize) -> String {
    let bar: String = std::iter::repeat(BAR_CHAR).take(bar_width).collect();
    let counter = format!("[{} of {}]", index + 1, total);
    let padding = bar_width.saturating_sub(counter.len());
    let spaces: String = std::iter::repeat(' ').take(padding).collect();
    format!("`{bar}`\n`{spaces}{counter}`")
}

fn render_scaffold(
    case: &Case,
    priority: MarkerPriority,
    note: &str,
    index: usize,
    total: usize,
    bar_width: usize,
) -> String {
    let divider = render_divider(index, total, bar_width);
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
///
/// `exclude` is a set of case numbers to skip — typically cases already walked
/// earlier in the session. Empty slice means include all marked cases.
///
/// `width` overrides the measured divider width; `None` measures the tmux pane.
pub fn run(file: &Path, exclude: &[u32], width: Option<usize>) -> Result<()> {
    let input = std::fs::read_to_string(file)
        .with_context(|| format!("failed to read {}", file.display()))?;

    let load_result = doc::parse(&input).context("failed to parse document")?;
    let doc = load_result.doc;
    let exclude_set: HashSet<u32> = exclude.iter().copied().collect();

    // Collect walkable cases: (priority, note, case)
    let mut items: Vec<(MarkerPriority, String, Case)> = Vec::new();
    for group in &doc.groups {
        for case in &group.cases {
            if exclude_set.contains(&case.number) {
                continue;
            }
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
    let bar_width = resolve_bar_width(width);
    let walk_items: Vec<WalkItem> = items
        .iter()
        .enumerate()
        .map(|(i, (priority, note, case))| {
            let scaffold = render_scaffold(case, *priority, note, i, total, bar_width);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn override_width_is_honored_within_range() {
        assert_eq!(resolve_bar_width(Some(96)), 96);
    }

    #[test]
    fn override_width_is_clamped_at_both_ends() {
        assert_eq!(resolve_bar_width(Some(500)), MAX_BAR_WIDTH);
        assert_eq!(resolve_bar_width(Some(1)), MIN_BAR_WIDTH);
    }

    #[test]
    fn divider_right_aligns_counter_to_bar_width() {
        let divider = render_divider(2, 9, 40);
        let mut lines = divider.lines();

        // Backtick-wrapped, so the rendered bar is the line minus two delimiters.
        let bar = lines.next().unwrap();
        assert_eq!(bar.chars().filter(|&c| c == BAR_CHAR).count(), 40);

        let counter = lines.next().unwrap().trim_matches('`');
        assert_eq!(counter.chars().count(), 40);
        assert!(counter.ends_with("[3 of 9]"));
    }

    #[test]
    fn narrow_bar_still_fits_its_counter() {
        // The floor exists so padding never underflows to zero and shoves the
        // counter left of the bar's end.
        let divider = render_divider(9, 10, MIN_BAR_WIDTH);
        let counter = divider.lines().nth(1).unwrap().trim_matches('`');
        assert_eq!(counter.chars().count(), MIN_BAR_WIDTH);
    }
}
