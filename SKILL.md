---
name: enumerate
description: "Atomize user decision-making by enumerating a design space into a structured doc, then walking through each item one at a time. Use when facing many interacting decisions, edge cases to classify, or policies to set. Also use when the user says 'enumerate', 'let's itemize', or 'walk through the cases'."
argument-hint: <topic or file path>
---

Enumerate the decision space for `$ARGUMENTS` into a structured doc, then walk through each item for decisions.

If a doc already exists at the path provided, skip to step 3.

> Procedure for `/enumerate`. Read top to bottom. The **Walk discipline** and **Enumeration discipline** sections constrain output shape and are load-bearing.

## Steps

### 1. Pick a template

Pick the named template that best fits the topic from [Reference: Templates](#reference-templates) below. Default to `default (revision)` if unsure. If the user names a template explicitly, use that one. If none fit, define a custom column set per `docs/template-design.md`.

### 2. Enumerate

Explore the topic thoroughly. Identify every distinct case, edge case, and interacting decision. For each, write a short descriptive name and fill in the template's content columns. Group related cases under H2 headers; each group may have a brief description.

Write the doc to `./.enumerate/<YYYY-MM-DD>-<topic-slug>.md` in the current project directory, where `<YYYY-MM-DD>` is today's date. Create the `.enumerate/` directory if it doesn't exist.

Format rules:

- Case headings must be `### #N <name>` where N is a positive integer. Numbers are stable IDs across sessions.
- Field markers are `**FieldName:**` followed by inline value or block content.
- **Do not include the Decision column.** The binary appends it on load.

See `docs/sample.md` for a complete conformant example. The full format spec lives in `docs/V1_SPEC.md`.

Case shape and density are constrained by the **Enumeration discipline** section below.

### 3. Open the TUI

Check whether you're inside tmux by inspecting `$TMUX` (e.g., `echo "${TMUX:-no}"` via Bash):

- **Inside tmux ($TMUX is set):** run `enumerate <path> --popup` via Bash.
- **Outside tmux ($TMUX is unset):** do **not** invoke the binary yourself — `--popup` errors out non-zero outside tmux. End your turn and tell the user to run `enumerate <path>` themselves, then reply when they're done. Re-read the file in the next turn and proceed to step 4.

**Bash backgrounding (inside tmux).** The Bash call to `enumerate <path> --popup` will return immediately with `Command running in background with ID: ...`. **That is expected, not an error.** Do **not** read the doc file until you receive the explicit `<task-notification>` with `status: completed`. The TUI autosaves on every keystroke, so any read during the editing session sees an intermediate snapshot of the user's in-progress decisions, not their final state. Wait for the completion notification, then re-read the file and proceed to step 4. Do not pass a custom timeout to the Bash call — it backgrounds automatically and runs as long as the user needs.

This step is **always** run after writing the doc. There is no condition under which it's skipped or replaced.

### 4. Walk the list

Read the file. Process Decision markers in priority order:

1. `!!` items first (must discuss)
2. `!` items next (should discuss)
3. `?` items last (explain the proposal first, then ask)
4. Skip `OK` items (already agreed)

For `?` items, lead with clarification — the user marked them because they don't understand the proposal yet. Explain, then ask.

The TUI is browse-mode; the walk happens here in chat, driven by you. Walk-step output is constrained by the **Walk discipline** section below.

### 5. Summarize decisions

After all items are resolved, present a summary table of decisions made. One row per case, action column on the right. Include any cases marked Disagree or Skip so the table covers everything in the doc, not just the walked items.

### 6. Update the doc

After the summary (not during the walk), update all resolved Decision markers in the enumeration doc in a single batch edit. Do not edit the doc once per decision — wait until the walk is complete and apply all changes at once.

### 7. Implement

If the decisions lead to code changes, implement them as you would any other coding task. After implementing, link test names to each case in the doc where applicable — that's the one piece of post-walk work that's specific to `/enumerate`.

## Walk discipline

How to write each walk step:

1. **Shape**: marker badge → case header → quoted note → recommendation → why → ask. In that order, every step. For `?` items (user needs clarification), replace recommendation+why with a plain explanation, then ask.
2. **Labels**: `**Recommendation:**`, `**Why:**`, `**Ask:**`. For `?` items, the explanation has no label — just prose between the quoted note and the **Ask:** line.
3. **Case header**: `` `(!!)` **#N Case name** `` — inline-code-wrapped marker badge (one of `(!!)`, `(!)`, `(?)`) followed by bold `#N` and name. No H3 — bold text only.
4. **Quoted note**: `> Your note: *user's commentary*` — strip the marker prefix from the quote (it's already in the badge). Italicize the note text.
5. **Divider + counter** between consecutive walk steps (not before the first step). Two-line block, both inline-code-wrapped:
   - Line 1: 60 `━` characters (U+2501 heavy horizontal)
   - Line 2: right-aligned `[N of M]` counter, padded with leading spaces so its right edge aligns with column 60 (i.e., 60 minus the length of the counter string)
   - No blank line between the divider and the next case header — they sit flush.
6. **Orientation header**: first line of the walk, before the first case: `**Walking N items:** ` `` `!!` `` ` × A, ` `` `!` `` ` × B, ` `` `?` `` ` × C.` — tallies by marker type.
7. **One item per message.** Each agent turn walks exactly one case. The divider+counter appears at the top of every turn except the first.
8. **Confirmation on resolve.** When the user gives a decision, confirm with: the case header (badge + `#N` + name), then `Noted — [summary of decision]. Moving on.` on the next line. Then the divider+counter and next case follow. If it's the last item, the confirmation precedes the summary table instead.
9. **Length** is ≤10 lines typical, not strict — going under is fine, going over should be rare.
10. **One decision per step.** Never bundle nested sub-questions; if discovered, defer them as new cases.
11. **No trailing open questions** or "things to consider later." End each step with a single concrete ask.
12. **Defer discovered sub-cases.** If a case spawns new sub-cases mid-walk, note them for a follow-up enumeration after the walk completes. Do not expand the current step.
13. **Track derivations.** When one decision constrains a later case, note the dependency in your reasoning ("this follows from #4b"). Don't ask for redundant ratifications.

## Enumeration discipline

How to write each case in step 2:

1. **Atomize**, but allow bundling when splitting would create false granularity. Test: would the user ever want to take part of the case but not the rest? If yes, split. If no, bundle.
2. **Field prose ≤3 sentences.** Lead with the concrete fact; cut hedging and qualifiers.
3. **Case count is not a target.** 25 small atomic cases is better than 5 bundled ones. Don't trim to hit an arbitrary count — atomize instead.
4. **Single suggestion is the default** in the **Proposed Change** field. Add alternatives only when each represents a real tradeoff the user might decide differently than you would. A few is fine; more suggests you haven't done the picking work.

## Markers

- `!!` — must discuss, highest priority
- `!` — should discuss
- `?` — don't understand the proposal yet, explain it first
- `OK` — agree with the proposal as written

## Reference: Templates

| Template | Content columns | When to use |
|---|---|---|
| `default (revision)` | `[Status Quo, Proposed Change, Reasoning]` | Proposing changes to existing code, configuration, or process. Each case is a discrete delta from the current state. |
| `exhaustiveness, coverage` | `[Case, Example, Verdict]` | Enumerating edge cases or behaviors a system must handle. Each case is a distinct scenario with an expected outcome. |
| `design, ideation` | `[Suggestion, Example, Reasoning]` | Comparing alternative approaches to a single decision. Each case is one option with its rationale. |
| `task completion` | `[Task, State, Notes]` | Status reports on in-progress work. State is the *factual* state of the work (in progress, blocked, done) — not the user's call, which lives in the auto-appended Decision column. |
| `custom` | `[...]` | None of the above fit. See `docs/template-design.md` for the design constraints and skeletons. |

The **Decision** column is appended automatically by the binary on load. Templates and agent-written files do **not** include it.
