# enumerate-tui v1 spec

A TUI + tmux popup that replaces manual markdown editing for the
`enumerate` skill. Claude writes a structured markdown file, then
opens it in a popup over Claude Code for the user to annotate.

## Goals

- Annotate enumeration cases without leaving Claude Code (in tmux)
- Render long fields without markdown table truncation
- Same TUI works standalone outside tmux
- File on disk remains the single source of truth

## Tech stack

- **Language:** Rust 2024
- **TUI:** ratatui + crossterm
- **CLI:** clap (derive)
- **Markdown:** pulldown-cmark
- **Frontmatter:** serde + serde_yaml
- **Ordered fields:** indexmap
- **Errors:** anyhow

Single crate, single binary `enumerate`.

## Binary

Two subcommands:

| Command | Behavior |
|---|---|
| `enumerate open <file>` | Launches the TUI in the current terminal, blocking until quit. Used by the user manually, and by the popup wrapper internally. |
| `enumerate popup <file>` | If `$TMUX` is set, execs `tmux display-popup -E "enumerate open <file>"` and exits with that status. Otherwise prints "Run `enumerate open <file>` to edit" to stderr and exits 0. |

The agent always calls `enumerate popup`. Tmux detection lives in the binary, not in the skill.

## Storage format

Section-per-case markdown with YAML frontmatter.

### Example

````markdown
---
topic: enumerate-tui design
created: 2026-04-07
columns: [Setup, Expected, Decision, Notes]
---

# enumerate-tui design

Free prose intro from the agent describing the topic.

## Storage schema

### #1 Heading levels

**Setup:**
H1 topic, H2 group, H3 case.

**Expected:**
Parser uses heading depth.

**Decision:** ?

**Notes:**
Optional alt: skip H1 if frontmatter has topic.

### #2 Field syntax

**Decision:** OK
````

### Rules

- **Frontmatter:** YAML. Recognized keys: `topic`, `created`, `columns`. Other keys preserved as `extras` for round-trip.
- **`columns`:** array of field names, declares the column set + display order. If missing, inferred from union of fields seen across cases.
- **H1:** topic title. Falls back to frontmatter `topic` if absent.
- **H2:** group name. Optional. Cases before any H2 belong to an implicit "(ungrouped)" group.
- **Group description:** prose between an H2 and the first H3 in that group becomes the group's description. Optional, may be multi-paragraph.
- **H3:** case heading, must be `### #N Name` where N is a positive integer.
- **Field markers:** `**FieldName:**` followed by inline value or block content. Block extends until next field marker, next H3, next H2, or EOF.
- **Numbering:** stable, assigned at creation, gap-allowed. Never auto-renumbered. No reorder.
- **Decision column:** hardcoded as the TUI's input column. Templates and agent-written files do **not** include it; the binary appends it on load if not already present, and the TUI navigates it. Column name match is case-insensitive, canonicalized to `Decision` on save. Must be the last column; parser moves it to the end with a warning if found elsewhere. Free-text values; conventional values `!! / ! / ? / OK` are guidance only — the TUI does not enforce or interpret them.

### Tolerance table

| Input | Action |
|---|---|
| `### #1 Foo` | Canonical, accept silently |
| `### 1. Foo`, `### 1 Foo`, `### #1: Foo` | Warn, normalize on save |
| `### Foo` (no number) | Warn, assign next free number on save |
| `**Setup:**` | Canonical, accept silently |
| `**Setup**:`, `__Setup:__`, `Setup:` | Warn, normalize on save |
| Other content inside a case body | Unknown block, warn, dropped on save |
| Missing frontmatter | Synthesize: today's date, columns inferred |
| Invalid YAML in frontmatter | **Fatal.** Refuse to load. |
| Duplicate `#N` | Warn, load both. User must fix. |
| Field key not in `columns:` | Warn, auto-add to `columns:` on save (appended) |
| Decision column missing from `columns:` | Append `Decision` to `columns:` and add empty Decision field to every case (no warning — this is normal for template-written files) |
| Decision column present but not last | Warn, move Decision to the end of `columns:` |
| Duplicate H2 group name | Warn, merge on display + save |
| Empty group | Keep, no warning |
| Case missing a column field | Empty value, no warning |

Principle: load aggressively, normalize on save, warn whenever you tolerated or changed something. Fatal only on YAML parse failure.

## Parser data model

```rust
struct LoadResult {
    doc: Doc,
    warnings: Vec<Warning>,
}

struct Doc {
    frontmatter: Frontmatter,
    intro: String,                       // prose between H1 and first H2/H3
    groups: Vec<Group>,
}

struct Group {
    name: Option<String>,                // None = implicit "(ungrouped)"
    description: String,                 // prose between H2 and first H3, may be empty
    cases: Vec<Case>,
}

struct Case {
    number: u32,                         // stable ID, from "### #N"
    name: String,
    fields: IndexMap<String, String>,    // arbitrary, preserves insertion order
}

struct Frontmatter {
    topic: Option<String>,
    created: Option<String>,
    columns: Vec<String>,                // empty if not declared
    extras: serde_yaml::Mapping,         // round-trip preserve
}

struct Warning {
    location: Location,                  // Frontmatter | Line(u32) | Case(u32) | Group(String)
    message: String,
    severity: Severity,                  // Warn | Fatal
}
```

## Round-trip contract

- Canonical file → load → save → byte-equal (modulo trailing newline)
- Tolerated/non-canonical file → load → save → canonicalized
- File with unknown blocks → load → save → unknown blocks gone, warnings cleared
- **Saves happen on every edit, not on exit.** The TUI autosaves on each keystroke that mutates state. If the user opens and quits without editing, the file is untouched on disk and any non-canonical content is preserved as-is.

## TUI

**Mode:** browse-mode only. User navigates and edits Decision cells in any order. Walk-mode (one item at a time, priority order) lives in the agent, not the TUI.

**Input column:** hardcoded to `Decision`. The TUI does not navigate or edit any other column. Other columns render as read-only display for context.

**Decision values:** displayed verbatim. No special icons, coloring, or validation.

**Warnings:** sticky header shows a passive `⚠ N` indicator when N > 0. No interactive warnings panel in v1 — to inspect warnings, open the file in a text editor.

**Autosave:** the file is written on every keystroke that mutates state. No save-on-exit. Any exit path (Submit, Esc, Ctrl+C, signal) just closes the TUI.

> Detailed layout, cell states, navigation, and text-entry behavior are specified separately in `docs/V1_UI_SPEC.md`.

## Agent integration

The `enumerate` skill is rewritten to:

1. **Enumerate.** Pick one of the named templates (`default (revision)`, `exhaustiveness/coverage`, `design/ideation`, `task completion`, or `custom`) based on the topic, and write a section-per-case markdown file at `./.enumerate/<YYYY-MM-DD>-<topic-slug>.md`. Templates contain only **content columns** — they do *not* include the Decision column.
2. **Open the TUI.** Run `enumerate popup <path>` via Bash. The binary auto-appends the Decision column on load. In tmux, the call blocks until the popup closes; outside tmux, it returns immediately after printing the path.
3. **Wait** (out-of-tmux only). End the turn, ask the user to run `enumerate open <path>` and reply when done.
4. **Walk.** Re-read the file. Process items one at a time in priority order driven by Decision markers (`!! > ! > ? > OK`). One item, one decision.
5. **Summarize and implement.** Same as the current skill.

The skill markers `!! / ! / ? / OK` are documented as conventions; the TUI does not enforce them.

The skill rewrite happens after the binary works end-to-end, so the bash flow can be verified before the skill changes.

## Project layout

```
/Users/harrison/dev/enumerate/
├── Cargo.toml
├── src/
│   ├── main.rs          # entry, clap dispatch
│   ├── doc/
│   │   ├── mod.rs       # types
│   │   ├── parse.rs     # markdown → Doc, with warnings
│   │   └── serialize.rs # Doc → markdown
│   ├── tui/
│   │   ├── mod.rs       # ratatui App + event loop
│   │   ├── view.rs      # rendering
│   │   └── state.rs     # selection / edit state
│   └── popup.rs         # tmux detect + display-popup invocation
└── tests/
    ├── parse_roundtrip.rs
    └── fixtures/        # sample .md files
```

## Install

`cargo install --path .` from the project directory. Installs to `~/.cargo/bin/enumerate`.

Verify `~/.cargo/bin` is on Claude Code's Bash PATH before relying on the agent flow; fall back to a `~/.local/bin` symlink if not.

## Tests

Round-trip parser/serializer tests in `tests/parse_roundtrip.rs`:

1. Canonical file round-trips byte-equal
2. Each tolerance rule produces correct warning + canonical save
3. Invalid YAML returns fatal error
4. Empty file → empty Doc, no warnings
5. Frontmatter-only file → empty groups, no warnings
6. Mixed groups + ungrouped cases → correct group structure
7. Custom columns + missing fields → fields shown empty
8. Extra unknown field → warning, column auto-added on save
9. Duplicate `#N` → both loaded, warning emitted

No TUI tests for v1.

## Out of scope for v1

- Config file
- Logging framework
- Shell completions
- CI / GitHub Actions
- README / LICENSE
- TUI snapshot tests
- Live file watching / sync between agent and TUI
- File locking
- Reordering cases
- Auto-renumbering
- Hand-edit-friendly preservation of unknown blocks

## Implementation order

1. `cargo init` + dependencies
2. `doc/` types + parser + serializer + round-trip tests
3. `popup.rs` + `main.rs` CLI scaffolding
4. `tui/` (waits on `docs/V1_UI_SPEC.md`)
5. Skill rewrite (after end-to-end works)
