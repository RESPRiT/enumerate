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

How to write each walk step. The example walkthrough below is the primary reference — the rules that follow it handle edge cases.

### Example walkthrough

The walk output has two kinds of turns: **presenting an item** (scaffold from binary + agent prose) and **confirming a decision** (short acknowledgment + next item). Here is a complete walkthrough:

---

**Agent turn 1** — orientation + first item:

```
**Walking 3 items:** `(!!)` x 1 `(!)` x 1 `(?)` x 1

`━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━`
`                                                    [1 of 3]`
`(!!)` **#4 Cache invalidation on deploy**

> Your note: *we got bitten by stale caches last release*

The deploy script doesn't bust the CDN cache. Static assets keep serving the old bundle until TTL expires, which can be 24h.

**Recommendation:** Add a `cache-bust` step to the deploy pipeline that purges the CDN after the new assets are live.

**Ask:** `(a)` Add CDN purge step, or `(b)` switch to content-hashed filenames so caching is never stale? `(*)` Something else
```

**User replies:** `(a)`

**Agent turn 2** — confirmation + next item:

```
Noted — adding CDN purge step to deploy pipeline. Moving on.

`━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━`
`                                                    [2 of 3]`
`(!)` **#7 Retry budget for webhook delivery**

Webhooks currently retry forever on 5xx. No backoff, no cap.

**Recommendation:** Exponential backoff, 5 retries max, dead-letter after that.

**Ask:** `(a)` Accept 5-retry cap with dead-letter, or `(b)` keep unlimited retries but add exponential backoff? `(?)` Tell me more
```

**User replies:** what's the dead-letter behavior?

**Agent turn 3** — answer + new ask (same item, no scaffold):

```
After 5 failed attempts, the payload goes to a dead-letter queue. You can inspect and manually replay from there. The webhook endpoint gets marked degraded but not disabled — future events still attempt delivery normally.

**Ask:** `(a)` 5-retry cap with dead-letter queue as described, `(b)` same but also disable the endpoint after 100 consecutive failures, or `(c)` unlimited retries with exponential backoff, no dead-letter? `(*)` Something else
```

**User replies:** `(b)`

**Agent turn 4** — confirmation + next item:

```
Noted — 5-retry cap, dead-letter queue, auto-disable after 100 consecutive failures. Moving on.

`━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━`
`                                                    [3 of 3]`
`(?)` **#12 Rate limit headers**

Rate-limited responses currently return a bare 429. The proposal adds `Retry-After`, `X-RateLimit-Limit`, and `X-RateLimit-Remaining` headers.

These are standard headers most HTTP clients and API wrappers understand. `Retry-After` tells the caller exactly when to retry. The `X-RateLimit-*` headers let callers track their budget proactively.

**Ask:** `(a)` Accept — add all three headers, `(b)` just `Retry-After` and skip the `X-RateLimit-*` pair, or `(?)` tell me more?
```

---

Key patterns to notice:

- The **orientation header** appears once, at the top of the first turn.
- The **scaffold** (divider + counter + case header) appears once per item, when presenting it — not on follow-up turns for the same item.
- The **confirmation** is just "Noted — [summary]. Moving on." — no repeated case header.
- When the user asks `(?)`, the agent answers and offers **new concrete choices** — no scaffold, no repeated context.
- `(?)` items (turn 4) lead with explanation instead of recommendation — the user needs to understand before deciding.
- Agent-written prose is: unlabeled **context**, then **Recommendation:**, then **Ask:**.

### Scaffold (echoed from binary)

The `enumerate walk` binary outputs pre-rendered markdown for all mechanical parts. The agent echoes these verbatim and never constructs them itself:

- **Orientation header**: echoed from `orientation`.
- **Divider + counter + case header + quoted note**: echoed from each item's `scaffold`.

### Agent-written prose

The agent writes these parts using the item's `fields` as source material:

1. **Context** (unlabeled): what the case is about — current state, constraints, what's at stake.
2. **Recommendation:** your pick. For `?` items, replace with unlabeled explanation — the user needs to understand the proposal before deciding.
3. **Ask:** the concrete question. Label alternatives `(a)`, `(b)`, etc. with inline-code wrapping. `(*)` = something else. `(?)` = tell me more. Write choices as flowing prose, not a list. When re-asking after a follow-up, **relabel from `(a)`** — the user's `(a)` always refers to the most recent set of choices, not the original.
4. **`?` items still get structured choices.** After explanation, offer `(a)` accept, `(b)` skip/decline, `(?)` tell me more.

### Behavioral rules

1. **No extra separators.** The scaffold's divider handles all visual separation.
2. **One item per message.** Each agent turn walks exactly one case.
3. **Confirmation on resolve.** See the example above — inline case reference + "Noted" summary, then the next item's full scaffold + prose in the same turn.
4. **Length** is ≤10 lines typical, not strict.
5. **One decision per step.** Never bundle nested sub-questions; defer them as new cases.
6. **No trailing open questions.** End each step with a single concrete ask.
7. **Defer discovered sub-cases.** Note them for a follow-up enumeration after the walk.
8. **Track derivations.** When one decision constrains a later case, note the dependency ("this follows from #4b").

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
