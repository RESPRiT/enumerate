use std::collections::{HashMap, HashSet};

use anyhow::{Context, Result};
use indexmap::{IndexMap, IndexSet};

use super::{
    Case, Doc, Frontmatter, Group, LoadResult, Location, STATUS_COLUMN, Severity, Warning,
};

pub fn parse(input: &str) -> Result<LoadResult> {
    let (frontmatter_str, body) = split_frontmatter(input);
    let frontmatter = parse_frontmatter(frontmatter_str)?;

    let mut parser = BodyParser::new();
    for line in body.lines() {
        parser.process_line(line);
    }
    parser.finalize();

    let mut doc = Doc {
        frontmatter,
        intro: parser.intro_text(),
        groups: parser.groups,
    };
    let mut warnings = parser.warnings;

    post_process(&mut doc, &mut warnings, parser.h1);

    Ok(LoadResult { doc, warnings })
}

fn split_frontmatter(input: &str) -> (Option<&str>, &str) {
    let after = if let Some(rest) = input.strip_prefix("---\n") {
        rest
    } else if let Some(rest) = input.strip_prefix("---\r\n") {
        rest
    } else {
        return (None, input);
    };

    let mut start = 0;
    while start <= after.len() {
        let line_end = after[start..]
            .find('\n')
            .map(|i| start + i)
            .unwrap_or(after.len());
        let line = after[start..line_end].trim_end_matches('\r');
        if line == "---" {
            let body_start = if line_end < after.len() {
                line_end + 1
            } else {
                after.len()
            };
            return (Some(&after[..start]), &after[body_start..]);
        }
        if line_end >= after.len() {
            break;
        }
        start = line_end + 1;
    }
    (None, input)
}

fn parse_frontmatter(s: Option<&str>) -> Result<Frontmatter> {
    let Some(s) = s else {
        return Ok(Frontmatter::default());
    };
    if s.trim().is_empty() {
        return Ok(Frontmatter::default());
    }

    let value: serde_yaml::Value =
        serde_yaml::from_str(s).context("invalid YAML in frontmatter")?;

    let mut frontmatter = Frontmatter::default();

    if let serde_yaml::Value::Mapping(mapping) = value {
        for (k, v) in mapping {
            let Some(key) = k.as_str().map(|s| s.to_string()) else {
                continue;
            };
            match key.as_str() {
                "topic" => {
                    if let Some(s) = v.as_str() {
                        frontmatter.topic = Some(s.to_string());
                    }
                }
                "created" => {
                    if let Some(s) = v.as_str() {
                        frontmatter.created = Some(s.to_string());
                    }
                }
                "columns" => {
                    if let Some(seq) = v.as_sequence() {
                        frontmatter.columns = seq
                            .iter()
                            .filter_map(|v| v.as_str().map(String::from))
                            .collect();
                    }
                }
                _ => {
                    frontmatter
                        .extras
                        .insert(serde_yaml::Value::String(key), v);
                }
            }
        }
    }

    Ok(frontmatter)
}

struct BodyParser {
    h1: Option<String>,
    intro_lines: Vec<String>,
    groups: Vec<Group>,
    current_group: Option<Group>,
    current_case: Option<Case>,
    current_field: Option<String>,
    current_field_lines: Vec<String>,
    warnings: Vec<Warning>,
}

impl BodyParser {
    fn new() -> Self {
        Self {
            h1: None,
            intro_lines: Vec::new(),
            groups: Vec::new(),
            current_group: None,
            current_case: None,
            current_field: None,
            current_field_lines: Vec::new(),
            warnings: Vec::new(),
        }
    }

    fn process_line(&mut self, line: &str) {
        if let Some(text) = line.strip_prefix("# ") {
            self.handle_h1(text.trim());
        } else if let Some(text) = line.strip_prefix("## ") {
            self.handle_h2(text.trim());
        } else if let Some(text) = line.strip_prefix("### ") {
            self.handle_h3(text.trim());
        } else if let Some(marker) = parse_field_marker(line) {
            self.handle_field_marker(marker);
        } else {
            self.handle_content_line(line);
        }
    }

    fn handle_h1(&mut self, text: &str) {
        if self.h1.is_none() {
            self.h1 = Some(text.to_string());
        } else {
            self.warnings.push(Warning {
                location: Location::Frontmatter,
                message: format!("multiple H1 headings; ignoring '{}'", text),
                severity: Severity::Warn,
            });
        }
    }

    fn handle_h2(&mut self, text: &str) {
        self.finalize_current_case();
        self.finalize_current_group();
        self.current_group = Some(Group {
            name: Some(text.to_string()),
            description: String::new(),
            cases: Vec::new(),
        });
    }

    fn handle_h3(&mut self, text: &str) {
        self.finalize_current_case();
        let parsed = parse_h3_heading(text);
        if parsed.number.is_none() {
            self.warnings.push(Warning {
                location: Location::Frontmatter,
                message: format!("case heading '{}' missing #N number", text),
                severity: Severity::Warn,
            });
        }
        self.ensure_group();
        self.current_case = Some(Case {
            number: parsed.number.unwrap_or(0),
            name: parsed.name,
            fields: IndexMap::new(),
        });
    }

    fn handle_field_marker(&mut self, marker: ParsedFieldMarker) {
        if self.current_case.is_none() {
            self.warnings.push(Warning {
                location: Location::Frontmatter,
                message: format!("field marker '{}' outside any case", marker.name),
                severity: Severity::Warn,
            });
            return;
        }
        self.finalize_current_field();
        self.current_field = Some(marker.name);
        if !marker.inline_value.is_empty() {
            self.current_field_lines.push(marker.inline_value);
        }
    }

    fn handle_content_line(&mut self, line: &str) {
        if self.current_field.is_some() {
            self.current_field_lines.push(line.to_string());
        } else if let Some(case) = self.current_case.as_ref() {
            if !line.trim().is_empty() {
                self.warnings.push(Warning {
                    location: Location::Case(case.number),
                    message: "unknown content inside case body before any field marker"
                        .to_string(),
                    severity: Severity::Warn,
                });
            }
        } else if let Some(group) = self.current_group.as_mut() {
            if !group.description.is_empty() {
                group.description.push('\n');
            }
            group.description.push_str(line);
        } else {
            self.intro_lines.push(line.to_string());
        }
    }

    fn finalize_current_field(&mut self) {
        if let Some(name) = self.current_field.take() {
            let value = self.current_field_lines.join("\n").trim().to_string();
            self.current_field_lines.clear();
            if let Some(case) = self.current_case.as_mut() {
                case.fields.insert(name, value);
            }
        }
    }

    fn finalize_current_case(&mut self) {
        self.finalize_current_field();
        if let Some(case) = self.current_case.take() {
            self.ensure_group();
            self.current_group.as_mut().unwrap().cases.push(case);
        }
    }

    fn finalize_current_group(&mut self) {
        if let Some(group) = self.current_group.take() {
            self.groups.push(group);
        }
    }

    fn ensure_group(&mut self) {
        if self.current_group.is_none() {
            self.current_group = Some(Group {
                name: None,
                description: String::new(),
                cases: Vec::new(),
            });
        }
    }

    fn finalize(&mut self) {
        self.finalize_current_case();
        self.finalize_current_group();
    }

    fn intro_text(&self) -> String {
        self.intro_lines.join("\n").trim().to_string()
    }
}

struct ParsedH3 {
    number: Option<u32>,
    name: String,
}

fn parse_h3_heading(text: &str) -> ParsedH3 {
    let trimmed = text.trim();
    if let Some(rest) = trimmed.strip_prefix('#') {
        let num_end = rest
            .find(|c: char| !c.is_ascii_digit())
            .unwrap_or(rest.len());
        if num_end > 0
            && let Ok(num) = rest[..num_end].parse::<u32>()
        {
            let after = rest[num_end..].trim_start();
            return ParsedH3 {
                number: Some(num),
                name: after.to_string(),
            };
        }
    }
    ParsedH3 {
        number: None,
        name: trimmed.to_string(),
    }
}

struct ParsedFieldMarker {
    name: String,
    inline_value: String,
}

fn parse_field_marker(line: &str) -> Option<ParsedFieldMarker> {
    let rest = line.strip_prefix("**")?;
    let close = rest.find("**")?;
    let inner = &rest[..close];
    let name = inner.strip_suffix(':')?;
    if name.is_empty() || name.contains(['*', ':', '\n']) {
        return None;
    }
    let after = rest[close + 2..].trim();
    Some(ParsedFieldMarker {
        name: name.to_string(),
        inline_value: after.to_string(),
    })
}

fn post_process(doc: &mut Doc, warnings: &mut Vec<Warning>, h1: Option<String>) {
    if doc.frontmatter.topic.is_none()
        && let Some(h1) = h1
    {
        doc.frontmatter.topic = Some(h1);
    }

    for group in &mut doc.groups {
        group.description = group.description.trim().to_string();
    }

    let mut used: HashSet<u32> = HashSet::new();
    for group in &doc.groups {
        for case in &group.cases {
            if case.number != 0 {
                used.insert(case.number);
            }
        }
    }
    let mut next_num: u32 = 1;
    for group in &mut doc.groups {
        for case in &mut group.cases {
            if case.number == 0 {
                while used.contains(&next_num) {
                    next_num += 1;
                }
                case.number = next_num;
                used.insert(next_num);
            }
        }
    }

    let mut counts: HashMap<u32, u32> = HashMap::new();
    for group in &doc.groups {
        for case in &group.cases {
            *counts.entry(case.number).or_insert(0) += 1;
        }
    }
    let mut dup_nums: Vec<u32> = counts
        .into_iter()
        .filter_map(|(n, c)| if c > 1 { Some(n) } else { None })
        .collect();
    dup_nums.sort();
    for num in dup_nums {
        warnings.push(Warning {
            location: Location::Case(num),
            message: format!("duplicate case number #{}", num),
            severity: Severity::Warn,
        });
    }

    if doc.frontmatter.columns.is_empty() {
        let mut seen: IndexSet<String> = IndexSet::new();
        for group in &doc.groups {
            for case in &group.cases {
                for key in case.fields.keys() {
                    seen.insert(key.clone());
                }
            }
        }
        doc.frontmatter.columns = seen.into_iter().collect();
    }

    let status_idx = doc
        .frontmatter
        .columns
        .iter()
        .position(|c| c.eq_ignore_ascii_case(STATUS_COLUMN));
    match status_idx {
        None => {
            doc.frontmatter.columns.push(STATUS_COLUMN.to_string());
            warnings.push(Warning {
                location: Location::Frontmatter,
                message: "Status column missing from `columns:`; appended".to_string(),
                severity: Severity::Warn,
            });
            for group in &mut doc.groups {
                for case in &mut group.cases {
                    case.fields
                        .entry(STATUS_COLUMN.to_string())
                        .or_default();
                }
            }
        }
        Some(idx) if idx != doc.frontmatter.columns.len() - 1 => {
            let status_name = doc.frontmatter.columns.remove(idx);
            doc.frontmatter.columns.push(status_name);
            warnings.push(Warning {
                location: Location::Frontmatter,
                message: "Status column must be the last column; reordered".to_string(),
                severity: Severity::Warn,
            });
        }
        _ => {}
    }
}
