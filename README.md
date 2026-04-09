# enumerate

A Rust TUI + Claude Code skill for **atomizing decision-making**. When you're staring down a design space with dozens of interacting choices, edge cases to classify, or policies to set, `enumerate` lets Claude lay every case out in a structured doc — then walks you through them one at a time.

The repo ships two pieces that work together:

1. **`enumerate` binary** — a Rust TUI (ratatui + crossterm) that opens an enumeration doc in a new tmux window, overlaid on a dimmed snapshot of your Claude Code pane. You annotate each case in a `Decision` column; the file on disk stays the source of truth.
2. **`SKILL.md`** — the `/enumerate` Claude Code skill that orchestrates the round-trip: it picks a template, enumerates the design space, opens the TUI, and walks you through your marked items one by one.

## How it works

```
/enumerate <topic>
        │
        ▼
  Claude picks a template, enumerates every case,
  writes ./.enumerate/<YYYY-MM-DD>-<topic-slug>.md
        │
        ▼
  Claude runs `enumerate <path> --popup`
        │
        ▼
  TUI opens in a new tmux window, overlaid on a
  dimmed snapshot of your Claude Code pane
  → you mark each case with !! / ! / ? / OK
  → autosaves on every keystroke
  → close the window when done
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

> The first time the TUI opens a file inside `.enumerate/` in a git repo, it appends `.enumerate/` to the local `.git/info/exclude` (creating the file if needed). That's git's per-clone ignore mechanism — never tracked, never committed — so the auto-ignore doesn't pollute the shared repository state. Each clone of the repo gets its own. Decision docs are working notes — keep them out of version control by default. If your project explicitly wants to commit decision docs, remove the entry from `.git/info/exclude` (the binary respects the opt-out marker and won't re-add); if you want a team-wide ignore, commit `.enumerate/` to the tracked `.gitignore` and the binary will respect that and skip writing to the local exclude.

## Setup

### Prerequisites

- Rust 2024 edition (stable toolchain via `rustup`)
- tmux (optional, but required for the in-session window flow with Claude Code)
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

Claude will explore the topic, write `./.enumerate/2026-04-08-auth-middleware-rewrite.md` (date-prefixed for chronological sorting), open it in a new tmux window with the chat snapshot dimmed behind it, and then walk you through your marked items once you close the window.

### Standalone

You can also point the binary at any conformant doc:

```sh
enumerate ./docs/sample.md            # full-screen TUI in the current terminal
enumerate ./docs/sample.md --window   # spawn the TUI in a new tmux window (requires $TMUX)
enumerate ./docs/sample.md --popup    # same, with the current pane captured as a dimmed backdrop
```

`--popup` is what the skill calls from inside tmux. It captures the current pane via `tmux capture-pane`, spawns a new window, and the TUI renders the snapshot dimmed behind a centered overlay. Both `--window` and `--popup` block until the user closes the window. Outside tmux they error out — the skill detects `$TMUX` itself and falls back to instructing the user to run plain `enumerate <path>` manually.

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
cargo run -- ./docs/sample.md
```

Project layout:

```
src/
├── main.rs       # clap dispatch
├── doc/          # parser + serializer + types
├── tui/          # ratatui App, view, state
└── tmux.rs       # tmux detection + new-window launcher
tests/
├── parse_roundtrip.rs
└── fixtures/
```
