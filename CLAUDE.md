# CLAUDE.md

Guidance for Claude Code sessions working on this repo. Assumes you've read `README.md` for user-facing setup. This file focuses on what's needed to edit the repo safely.

## Orientation

This repo ships two coupled artifacts: a Rust TUI binary (`enumerate`) and a Claude Code skill (`SKILL.md`). They are designed together — most features touch both.

## Live-effect gotchas

**`SKILL.md` is symlinked.** The `SKILL.md` at the repo root is typically symlinked to `~/.claude/skills/enumerate/SKILL.md`. Edits here are live immediately for the user's next `/enumerate` invocation — no reload, no safety net. A broken `SKILL.md` breaks the skill.

**Reinstall the binary after `src/` changes.** `/enumerate` shells out to the installed binary at `~/.cargo/bin/enumerate`, not `target/debug/enumerate`. Run `cargo install --path .` after changes or the live skill will keep running stale code even though `cargo build` and `cargo run` succeed.

## Invariants — do not break

**Stable case numbering.** Case `#N` numbers are stable IDs. Never renumber, sort, or fill gaps on save. Gaps are allowed. The walk protocol and user references depend on this.

**Decision column is hardcoded.** Defined as `DECISION_COLUMN` in `src/doc/mod.rs`. The binary appends it on load if missing, must be last, canonicalized case-insensitively to `Decision`. Templates and agent-written files must NOT include it. Values are free text — the TUI does not interpret `!! / ! / ? / OK`.

**`enumerate walk` output is a user-visible rendering surface.** `src/walk.rs` produces YAML that the agent pastes verbatim into its walk responses. Dividers, counters, case headers, and quoted notes are rendered here, not by the agent. Treat it as a template engine whose output the user sees directly, not as opaque data.

## Dev workflow

```
cargo build                     # debug build
cargo test                      # parser round-trip tests
cargo run -- ./docs/sample.md   # run TUI on a sample doc
cargo install --path .          # install to ~/.cargo/bin (needed for /enumerate)
```

**Testing:** parser and serializer have round-trip tests at `tests/fixtures/`. The TUI and tmux launcher have no automated tests — exercise UI changes manually with `cargo run -- <file>`, and for popup-specific changes `cargo run -- <file> --popup` inside tmux.

**Testing with the user in-session:** after `cargo install --path .`, launch the updated TUI in-session via `enumerate <file> --popup` so the user can interact with it directly in the tmux popup — no need to drop to a separate terminal. Background the Bash call and wait for the completion notification before reading the file.

## Cross-cutting

**Binary and `SKILL.md` usually change together.** The skill is the binary's primary consumer, and the binary is the skill's primary tool. New CLI flags, new walk output fields, changed scaffold formatting — all have a corresponding skill-side instruction. When editing one, check the other. Drift between them is the top source of regressions.

**`.enumerate/` is auto-gitignored per-clone.** On first TUI open, the binary appends `.enumerate/` to `.git/info/exclude` (per-clone, not tracked). Decision docs stay local by default and won't show in `git status`. If a project wants to commit decision docs, add `.enumerate/` to the tracked `.gitignore` — the binary respects that and skips writing to the local exclude.
