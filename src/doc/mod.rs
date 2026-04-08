pub mod parse;
pub mod serialize;

use indexmap::IndexMap;

pub use parse::parse;
pub use serialize::serialize;

#[derive(Debug, Clone)]
pub struct LoadResult {
    pub doc: Doc,
    pub warnings: Vec<Warning>,
}

#[derive(Debug, Clone, Default)]
pub struct Doc {
    pub frontmatter: Frontmatter,
    pub intro: String,
    pub groups: Vec<Group>,
}

#[derive(Debug, Clone)]
pub struct Group {
    pub name: Option<String>,
    pub description: String,
    pub cases: Vec<Case>,
}

#[derive(Debug, Clone)]
pub struct Case {
    pub number: u32,
    pub name: String,
    pub fields: IndexMap<String, String>,
}

#[derive(Debug, Clone, Default)]
pub struct Frontmatter {
    pub topic: Option<String>,
    pub created: Option<String>,
    pub columns: Vec<String>,
    pub extras: serde_yaml::Mapping,
}

#[derive(Debug, Clone)]
pub struct Warning {
    pub location: Location,
    pub message: String,
    pub severity: Severity,
}

#[derive(Debug, Clone)]
pub enum Location {
    Frontmatter,
    Line(u32),
    Case(u32),
    Group(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Warn,
    Fatal,
}

/// The hardcoded name of the input column the TUI navigates and edits.
pub const STATUS_COLUMN: &str = "Status";

impl Doc {
    /// Total number of cases across all groups.
    pub fn total_cases(&self) -> usize {
        self.groups.iter().map(|g| g.cases.len()).sum()
    }

    /// Number of cases whose Status field is non-empty.
    pub fn filled_cases(&self) -> usize {
        self.groups
            .iter()
            .flat_map(|g| g.cases.iter())
            .filter(|c| {
                c.fields
                    .get(STATUS_COLUMN)
                    .map(|s| !s.trim().is_empty())
                    .unwrap_or(false)
            })
            .count()
    }
}
