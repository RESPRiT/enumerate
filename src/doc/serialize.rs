use super::{Doc, Frontmatter};

pub fn serialize(doc: &Doc) -> String {
    let mut out = String::new();

    let fm = serialize_frontmatter(&doc.frontmatter);
    let had_frontmatter = !fm.is_empty();
    out.push_str(&fm);
    if had_frontmatter {
        out.push('\n');
    }

    if let Some(topic) = &doc.frontmatter.topic {
        out.push_str(&format!("# {}\n\n", topic));
    }

    if !doc.intro.is_empty() {
        out.push_str(&doc.intro);
        out.push_str("\n\n");
    }

    for group in &doc.groups {
        if let Some(name) = &group.name {
            out.push_str(&format!("## {}\n\n", name));
        }
        if !group.description.is_empty() {
            out.push_str(&group.description);
            out.push_str("\n\n");
        }
        for case in &group.cases {
            out.push_str(&format!("### #{} {}\n\n", case.number, case.name));
            for col in &doc.frontmatter.columns {
                let Some(value) = case.fields.get(col) else {
                    continue;
                };
                if value.is_empty() {
                    out.push_str(&format!("**{}:**\n\n", col));
                } else if value.contains('\n') {
                    out.push_str(&format!("**{}:**\n{}\n\n", col, value));
                } else {
                    out.push_str(&format!("**{}:** {}\n\n", col, value));
                }
            }
        }
    }

    let trimmed = out.trim_end();
    let mut result = trimmed.to_string();
    result.push('\n');
    result
}

fn serialize_frontmatter(fm: &Frontmatter) -> String {
    let has_any = fm.topic.is_some()
        || fm.created.is_some()
        || !fm.columns.is_empty()
        || !fm.extras.is_empty();
    if !has_any {
        return String::new();
    }

    let mut out = String::from("---\n");

    if let Some(topic) = &fm.topic {
        out.push_str(&format!("topic: {}\n", yaml_string(topic)));
    }
    if let Some(created) = &fm.created {
        out.push_str(&format!("created: {}\n", yaml_string(created)));
    }
    if !fm.columns.is_empty() {
        let cols: Vec<String> = fm.columns.iter().map(|c| yaml_string(c)).collect();
        out.push_str(&format!("columns: [{}]\n", cols.join(", ")));
    }
    if !fm.extras.is_empty() {
        let mapping = serde_yaml::Value::Mapping(fm.extras.clone());
        let yaml = serde_yaml::to_string(&mapping).unwrap_or_default();
        out.push_str(&yaml);
    }

    out.push_str("---\n");
    out
}

fn yaml_string(s: &str) -> String {
    let needs_quotes = s.is_empty()
        || s.starts_with(' ')
        || s.ends_with(' ')
        || s.contains([
            ':', '#', '[', ']', '{', '}', ',', '&', '*', '!', '|', '>', '\'', '"', '%', '@', '`',
            '\n',
        ]);
    if needs_quotes {
        format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
    } else {
        s.to_string()
    }
}
