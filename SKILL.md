---
name: enumerate
description: "Atomize user decision-making by enumerating a design space into a structured doc, then walking through each item one at a time. Use when facing many interacting decisions, edge cases to classify, or policies to set. Also use when the user says 'enumerate', 'let's itemize', or 'walk through the cases'."
argument-hint: <topic or file path>
---

Enumerate the decision space for `$ARGUMENTS` into a structured doc, then walk through each item for decisions.

If a doc already exists at the path provided, skip to step 3.

## Purpose

`/enumerate` exists to **atomize user decision-making**. The user has many interacting choices to make and a finite amount of attention. The skill's job is to:

1. Surface every distinct case, edge case, and decision that needs an answer.
2. Lay them out in a structured doc the user can navigate and annotate.
3. Walk the user through the items one at a time, in priority order, so each decision is made deliberately and independently.

The TUI binary (`enumerate`) is the user-facing surface. It opens the doc in a tmux window alongside Claude Code and lets the user mark each case in the **Decision** column. The skill orchestrates the round-trip between agent and user.

## Markers

The user types one of these into the **Decision** column for each case:

- `!!` — must discuss, highest priority
- `!` — should discuss
- `?` — don't understand the proposal yet, explain it first
- `OK` — agree with the proposal as written

These are conventions the agent walks by. The TUI does not enforce or interpret them.

## Templates

Pick the template that best fits the topic. If none fits, use `custom`. If the user names a template explicitly (e.g., "use the risks template"), use that one without judgment.

| Template | Content columns | When to use |
|---|---|---|
| `default (revision)` | `[Status Quo, Proposed Change, Reasoning]` | Proposing changes to existing code, configuration, or process. Each case is a discrete delta from the current state. |
| `exhaustiveness, coverage` | `[Case, Example, Verdict]` | Enumerating edge cases or behaviors a system must handle. Each case is a distinct scenario with an expected outcome. |
| `design, ideation` | `[Suggestion, Example, Reasoning]` | Comparing alternative approaches to a single decision. Each case is one option with its rationale. |
| `task completion` | `[Task, State, Notes]` | Status reports on in-progress work. Each case is a work item with its current state and context. |
| `custom` | `[...]` | None of the above fit. Define your own content columns following the column design rules below. |

The **Decision** column is appended automatically by the binary on file load. Templates and agent-written files do **not** include it.

### Column descriptions

**`default (revision)`**

- **Status Quo:** how things currently are. The "before" state.
- **Proposed Change:** the specific change being proposed. The "after" state.
- **Reasoning:** why this change is being proposed; what problem it solves; relevant tradeoffs.

**`exhaustiveness, coverage`**

- **Case:** a description of the edge case or behavior being enumerated.
- **Example:** a concrete sample input or scenario that triggers this case.
- **Verdict:** the agent's call on what should happen (valid/invalid/expected output/etc).

**`design, ideation`**

- **Suggestion:** a named alternative or approach.
- **Example:** a concrete illustration of what the suggestion looks like.
- **Reasoning:** the agent's analysis of pros, cons, and tradeoffs.

**`task completion`**

- **Task:** a short description of the work item.
- **State:** the current factual state of the work (in progress, blocked, done, etc.) — *not* the user's decision.
- **Notes:** additional context, blockers, dependencies.

In every template, the **Decision** column is reserved for the user to fill in during the walk. Never populate it yourself.

## Column design rules

When designing a `custom` template (or proposing changes to existing templates), follow these rules:

1. **Always include a reasoning-equivalent column** (`Reasoning`, `Verdict`, `Analysis`, etc.) so the agent's analysis has its own slot. Without it, downstream confusion is likely.
2. **Avoid semantic overlap between adjacent columns.** If two columns sound like they could hold the same kind of content, rename one or merge them.
3. **The Decision column is always rightmost and is added by the binary.** Do not include it in your template.
4. **Use clear, distinct column names.** A column name should make its content obvious without needing a description.
5. **3–4 content columns is the sweet spot.** Fewer collapses concerns; more is hard to navigate.

## Steps

### 1. Pick a template

Read the topic. Pick the template that best fits. Default to `default (revision)` if unsure. If the user named a template explicitly, use that one.

### 2. Enumerate

Explore the topic thoroughly. Identify every case, edge case, and interacting decision. For each, write a short, descriptive name and fill in the template's content columns.

Group related cases under H2 headers (e.g., "Base types", "Promotions", "Edge cases"). Each H2 may have a brief description paragraph.

Write the doc to `./.enumerate/<YYYY-MM-DD>-<topic-slug>.md` in the current project directory, where `<YYYY-MM-DD>` is today's date. Create the `.enumerate/` directory if it doesn't exist. Use this format:

````markdown
---
topic: <human-readable topic>
created: <YYYY-MM-DD>
columns: [Col1, Col2, Col3]
---

# <human-readable topic>

Optional intro paragraph.

## <Group name>

Optional group description.

### #1 <case name>

**Col1:** ...

**Col2:** ...

**Col3:** ...

### #2 <case name>

**Col1:** ...

...
````

Format rules:

- Case headings must be `### #N <name>` where N is a positive integer. Numbers are stable IDs across sessions.
- Field markers are `**FieldName:**` followed by inline value or block content.
- **Do not include the Decision column.** The binary appends it on load.

### 3. Open the TUI

Check whether you're inside tmux by inspecting `$TMUX` (e.g., `echo "${TMUX:-no}"` via Bash):

- **Inside tmux ($TMUX is set):** run `enumerate <path> --popup` via Bash. The binary captures the current pane as a dimmed backdrop, spawns a new tmux window running the TUI as a centered overlay, and blocks until that window closes. When it returns, re-read the file in the same turn and proceed to step 4.
- **Outside tmux ($TMUX is unset):** do **not** invoke the binary yourself — `--popup` errors out non-zero outside tmux, and running it without the flag would take over the agent's terminal. Instead, end your turn and tell the user to run `enumerate <path>` themselves, then reply when they're done. Re-read the file in the next turn and proceed to step 4.

This step is **always** run after writing the doc. There is no condition under which the TUI step is skipped or replaced with another invocation.

### 4. Walk the list

Read the file. Process Decision markers in priority order:

1. `!!` items first (must discuss)
2. `!` items next (should discuss)
3. `?` items last (explain the proposal, get confirmation)
4. Skip `OK` items (already agreed)

For each item:

- State the current proposal clearly.
- Give your recommendation with reasoning.
- Wait for the user's decision before moving to the next item.
- **Do not batch items.** One item, one decision.

For `?` items, lead with clarification — the user marked them because they don't understand the proposal, so explain first, then ask.

The TUI is browse-mode (the user navigates freely, in any order). Walk-mode — going one item at a time, in priority order — happens here in chat, driven by you.

### 5. Summarize decisions

After all items are resolved, present a summary table of decisions made.

### 6. Implement (if applicable)

If the decisions lead to code changes:

- Set a plan (goal, unknowns, steps, done-when).
- Implement all decisions together.
- Write tests covering the decided cases.
- Verify against real data when possible.

Update the doc with:

- Resolved Decision markers.
- Test names linked to each case.
- Open questions narrowed to only truly unresolved items.

## Tips

- **The doc is the agenda.** Everything lives in the doc — no decisions held in heads.
- **Decisions are atomic.** Each item is a single yes/no/clarify. Avoid compound decisions that require backtracking.
- **Prep and decide can be separate sessions.** The enumeration session does the expensive work (finding edge cases). The decision session skips straight to choosing.
- **Explain `?` items before asking.** The user marked them because they don't understand — lead with clarification, not a question.
- **Track derivations.** When one decision constrains another, note it. "This follows from our decision on #4b."
- **Browse vs. walk is a contract.** The TUI is browse-mode; you walk in chat. Don't try to walk inside the TUI, and don't try to do free-form browsing in chat.
