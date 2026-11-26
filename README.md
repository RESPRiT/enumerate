# enumerate

A Rust TUI + Claude Code skill for **atomizing decision-making**. When you're staring down a design space with dozens of interacting choices, edge cases to classify, or policies to set, `enumerate` lets Claude lay every case out in a structured doc — then walks you through them one at a time.

The repo ships two pieces that work together:

1. **`enumerate` binary** — a Rust TUI (ratatui + crossterm) that opens an enumeration doc as a modal over your Claude Code pane — a floating layer in Rex, or a new tmux window overlaid on a dimmed snapshot of the pane. You annotate each case in a `Decision` column; the file on disk stays the source of truth.
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
  Claude runs `enumerate <path> --popup`.
  The binary detects the multiplexer itself:

   Inside Rex or tmux           Outside both
   ──────────────────           ────────────
   Rex: TUI opens in a          Binary prints a
   floating layer over your     NO_MULTIPLEXER handoff
   Claude Code pane.            line and exits 0.
   tmux: TUI opens in a new     Claude ends its turn
   window, overlaid on a        and asks you to run
   dimmed snapshot of the       `enumerate <path>`
   pane. Claude waits for       yourself in another
   the TUI to close.            terminal. Reply when
                                you're done so Claude
                                can re-read the file.
        │                              │
        └──────────────┬───────────────┘
                       ▼
  → mark each case with !! / ! / ? / OK
  → autosaves on every keystroke
  → close the window (or the editor) when done
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
- Rex or tmux (optional, but one is required for the in-session flow with Claude Code)
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

Claude explores the topic and writes `./.enumerate/2026-04-08-auth-middleware-rewrite.md` (date-prefixed for chronological sorting). It then runs `enumerate <path> --popup` via the Bash tool unconditionally — Claude never inspects the environment itself. The binary decides what happens next based on which multiplexer the Claude Code session is in:

- **Inside Rex.** The binary opens a floating layer over your Claude Code pane (inset within it, so the live pane frames the editor) and runs the TUI there. The Bash call backgrounds and Claude waits for the TUI to close; then the layer disappears and focus returns to Claude.
- **Inside tmux.** The binary spawns a new tmux window and the TUI renders over a dimmed snapshot of your Claude Code pane. The Bash call backgrounds and Claude waits for the window-close notification before re-reading the file. This is the seamless in-session flow.
- **Outside both.** The binary can't open a TUI from a non-interactive shell, so it prints a `NO_MULTIPLEXER:` handoff line and exits 0 without opening anything. Claude sees that line, ends its turn, and tells you to run `enumerate <path>` yourself in another terminal (full-screen TUI, no overlay). Reply in the chat when you've closed the editor — Claude re-reads the file and continues with the walk.

Either way, once the file is annotated, Claude walks you through your marked items one at a time.

### Standalone

You can also point the binary at any conformant doc:

```sh
enumerate ./tests/fixtures/sample.md            # full-screen TUI in the current terminal
enumerate ./tests/fixtures/sample.md --window   # spawn the TUI in a new Rex or tmux window (requires a multiplexer)
enumerate ./tests/fixtures/sample.md --popup    # open the TUI as a modal over the current pane; outside a multiplexer, prints a NO_MULTIPLEXER handoff line and exits 0
```

`--popup` is what the skill calls, in every environment. Detection picks the innermost multiplexer: Rex when `$TERM_PROGRAM` is `rex`, `$REX_BLOCK` is set and the `rex` binary is on PATH (or in `$REX_BIN_DIR`); otherwise tmux when `$TERM_PROGRAM` isn't `rex`, `$TMUX` is set and `tmux` is on PATH. Because each multiplexer sets `$TERM_PROGRAM` in its own panes, tmux nested in a Rex pane (or the reverse) picks the inner one even though the outer one's variables leak through.

- **Rex:** creates a floating layer over the caller's pane (`session.new_layer`) running the TUI and blocks on `rex wait`. If the TUI fails at startup, its output is printed and the layer is closed; if the `--popup` process is killed, it closes the layer on the way out. Closing the layer by hand counts as finishing, like closing the tmux window. Under Rex the TUI turns on the kitty keyboard protocol so Shift/Ctrl+Enter reach it.
- **tmux:** captures the current pane via `tmux capture-pane`, spawns a new window, and the TUI renders the snapshot dimmed behind a centered overlay; it blocks until the user closes the window.
- **Neither:** prints `NO_MULTIPLEXER: ... Ask the user to run: enumerate <path>` to stdout and exits 0 — the skill branches on that prefix and hands off to the user, so the agent never has to probe the environment.

`--window` is the strict variant: a new Rex or tmux window, and a non-zero error outside a multiplexer.

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
cargo run -- ./tests/fixtures/sample.md
```

Project layout:

```
src/
├── main.rs       # clap dispatch
├── doc/          # parser + serializer + types
├── tui/          # ratatui App, view, state
└── mux/          # multiplexer detection + Rex/tmux launchers
tests/
├── parse_roundtrip.rs
└── fixtures/
```
