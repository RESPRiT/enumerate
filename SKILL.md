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

Run `enumerate walk <path>` via Bash. The binary reads the file, sorts items by marker priority (`!!` > `!` > `?`, skips `OK`), and outputs YAML with two top-level keys:

- `orientation` — pre-rendered markdown for the orientation header. Echo verbatim as the first line of the walk.
- `items` — array of walked cases, each with:
  - `scaffold` — pre-rendered markdown block (divider + counter + case header + quoted note if present). Echo verbatim at the top of each walk step.
  - `fields` — map of the case's content columns as raw text. Use these as source material to write context, recommendation, and ask. Do not echo them verbatim.

The agent never constructs dividers, counters, case headers, or quoted notes itself — it echoes the scaffold and writes only the prose (context, recommendation, ask). Walk-step output is constrained by the **Walk discipline** section below.

For `?` items, lead with clarification — the user marked them because they don't understand the proposal yet. Explain, then ask.

### 5. Summarize decisions

After all items are resolved, present a summary table of decisions made. One row per case, action column on the right. Include any cases marked Disagree or Skip so the table covers everything in the doc, not just the walked items.

### 6. Update the doc

After the summary (not during the walk), update all resolved Decision markers in the enumeration doc in a single batch edit. Do not edit the doc once per decision — wait until the walk is complete and apply all changes at once.

## Walk discipline

How to write each walk step:

### Scaffold (echoed from binary)

The `enumerate walk` output provides pre-rendered markdown for all mechanical parts. The agent echoes these verbatim and never constructs them itself:

- **Orientation header**: echoed from `orientation`. Tallies walked items by marker type using inline-code-wrapped badge format: `` `(!!)` × N ``.
- **Divider + counter**: echoed from each item's `scaffold`. Two inline-code-wrapped lines: 60 `━` (U+2501) on line 1, right-aligned `[N of M]` on line 2. Appears on every step including the first.
- **Case header**: included in `scaffold`. Inline-code-wrapped marker badge + bold `#N` and name. No H3.
- **Quoted note**: included in `scaffold` when the user wrote commentary beyond the bare marker. Omitted for bare markers.

### Agent-written prose

The agent writes these parts using the item's `fields` as source material:

1. **Context** (unlabeled): prose immediately after the scaffold. Summarizes what the case is about — current state, constraints, what's at stake. The user reads this before seeing your opinion.
2. **Recommendation:** (labeled `**Recommendation:**`): your pick. For `?` items, replace with unlabeled explanation prose — the user needs to understand the proposal before deciding.
3. **Ask:** (labeled `**Ask:**`): the concrete question. When multiple alternatives exist, label them inline as `(a)`, `(b)`, etc. with inline-code wrapping for color. `(*)` = "something else" (open-ended escape hatch — include whenever "other" is a reasonable response). `(?)` = "tell me more / discuss further." Write choices as flowing prose, not an itemized list: ``**Ask:** `(a)` Do X, or `(b)` do Y? `(*)` Something else``
4. **`?` items still get structured choices.** After the unlabeled explanation, the Ask should offer concrete alternatives — typically `(a)` accept the proposal as explained, `(b)` skip/decline, `(?)` tell me more.

### Behavioral rules

1. **No extra separators.** Do not add `---` or other visual breaks between cases — the scaffold's divider handles all visual separation.
2. **One item per message.** Each agent turn walks exactly one case. The first turn includes the orientation header + scaffold + prose. Subsequent turns include the scaffold + prose.
3. **Confirmation on resolve.** When the user gives a decision, confirm with: the case header (badge + `#N` + name), then `Noted — [summary of decision]. Moving on.` on the next line. Then the next item's scaffold + prose follow. If it's the last item, the confirmation precedes the summary table instead.
4. **Length** is ≤10 lines typical, not strict — going under is fine, going over should be rare.
5. **One decision per step.** Never bundle nested sub-questions; if discovered, defer them as new cases.
6. **No trailing open questions** or "things to consider later." End each step with a single concrete ask.
7. **Defer discovered sub-cases.** If a case spawns new sub-cases mid-walk, note them for a follow-up enumeration after the walk completes. Do not expand the current step.
8. **Track derivations.** When one decision constrains a later case, note the dependency in your reasoning ("this follows from #4b"). Don't ask for redundant ratifications.

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
