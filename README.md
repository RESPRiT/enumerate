# enumerate

A Rust TUI + Claude Code skill for **atomizing decision-making**. When you're staring down a design space with dozens of interacting choices, edge cases to classify, or policies to set, `enumerate` lets Claude lay every case out in a structured doc — then walks you through them one at a time.

The repo ships two pieces that work together:

1. **`enumerate` binary** — a Rust TUI (ratatui + crossterm) that opens an enumeration doc as a tmux popup over Claude Code. You annotate each case in a `Decision` column; the file on disk stays the source of truth.
2. **`SKILL.md`** — the `/enumerate` Claude Code skill that orchestrates the round-trip: it picks a template, enumerates the design space, opens the popup, and walks you through your marked items one by one.

## How it works

```
/enumerate <topic>
        │
        ▼
  Claude picks a template, enumerates every case,
  writes ./.enumerate/<YYYY-MM-DD>-<topic-slug>.md
        │
        ▼
  Claude runs `enumerate popup <path>`
        │
        ▼
  TUI opens in a tmux popup over Claude Code
  → you mark each case with !! / ! / ? / OK
  → autosaves on every keystroke
        │
        ▼
  Claude re-reads the file and walks you through
  marked items in priority order, one at a time
```

The four markers are conventions the agent walks by — the TUI just stores them as text:

- `!!` — must discuss, highest priority
- `!` — should discuss
- `?` — explain the proposal first
- `OK` — agree as written

See [`SKILL.md`](SKILL.md) for the full skill spec and [`docs/V1_SPEC.md`](docs/V1_SPEC.md) for the binary's storage format, parser rules, and TUI contract.

> The first time the TUI opens a file inside `.enumerate/` in a git repo, it appends `.enumerate/` to the repo's `.gitignore` (creating the file if needed). Decision docs are working notes — keep them out of version control by default. If you want to commit them, remove the entry and the binary won't re-add it.

## Setup

### Prerequisites

- Rust 2024 edition (stable toolchain via `rustup`)
- tmux (optional, but required for the popup-over-Claude-Code flow)
- Claude Code (to use the `/enumerate` skill)

### Install the binary

From the repo root:

```sh
cargo install --path .
```

This builds and installs `enumerate` to `~/.cargo/bin/enumerate`. Make sure `~/.cargo/bin` is on the `PATH` that Claude Code's Bash tool sees — if not, symlink it into `~/.local/bin` or wherever your shell picks up.

Verify:

```sh
enumerate --help
```

### Install the skill

The skill lives at `~/.claude/skills/enumerate/SKILL.md`. The canonical copy is in this repo; symlink it so edits stay in sync:

```sh
mkdir -p ~/.claude/skills/enumerate
ln -sf "$(pwd)/SKILL.md" ~/.claude/skills/enumerate/SKILL.md
```

Restart Claude Code (or start a new session) and `/enumerate` should be available.

## Usage

### Via Claude Code (the intended flow)

```
/enumerate the auth middleware rewrite
```

Claude will explore the topic, write `./.enumerate/2026-04-08-auth-middleware-rewrite.md` (date-prefixed for chronological sorting), open it in a tmux popup, and then walk you through your marked items once you close the popup.

### Standalone

You can also point the binary at any conformant doc:

```sh
enumerate open ./docs/sample.md     # full-screen TUI in the current terminal
enumerate popup ./docs/sample.md    # tmux popup if $TMUX is set, else prints the path
```

`enumerate popup` is what the skill calls. Inside tmux it blocks until the popup closes; outside tmux it prints `Run enumerate open <path> to edit` to stderr and exits 0.

## Doc format

Section-per-case markdown with YAML frontmatter. Example:

````markdown
---
topic: auth middleware rewrite
created: 2026-04-08
columns: [Status Quo, Proposed Change, Reasoning]
---

# auth middleware rewrite

## Session handling

### #1 Token storage location

**Status Quo:** sessions in localStorage

**Proposed Change:** httpOnly cookies

**Reasoning:** legal flagged localStorage for compliance
````

The `Decision` column is appended automatically by the binary on load — templates and agent-written files do **not** include it. Case numbers (`#N`) are stable IDs and never auto-renumbered. Full rules, including the tolerance table for non-canonical input, are in [`docs/V1_SPEC.md`](docs/V1_SPEC.md).

## Development

```sh
cargo build              # debug build
cargo test               # run round-trip parser tests
cargo run -- open ./docs/sample.md
```

Project layout:

```
src/
├── main.rs       # clap dispatch
├── doc/          # parser + serializer + types
├── tui/          # ratatui App, view, state
└── popup.rs      # tmux detection + display-popup
tests/
├── parse_roundtrip.rs
└── fixtures/
```
