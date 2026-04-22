---
topic: enumerate skill rewrite
created: 2026-04-08
columns: [Setup, Expected, Notes, Decision]
---

# enumerate skill rewrite

Updating `~/.claude/skills/enumerate/SKILL.md` to use the new
`enumerate-tui` v1 binary instead of the old markdown-table flow. Each
case below is a discrete change to the skill description that should be
decided independently.

## Storage format

How the agent writes the decision doc on disk.

### #1 Section-per-case format

**Setup:** Current SKILL.md tells Claude to write a markdown table with columns `# | Case | Setup/Structure | Expected outcome | Status`.

**Expected:** Replace with the section-per-case markdown defined in `docs/V1_SPEC.md`: YAML frontmatter, H1 topic, optional H2 groups, H3 cases as `### #N Name`, fields as `**FieldName:**` markers (inline or block).

**Notes:** This is the original motivation for the whole project — markdown tables don't text-wrap. The TUI both parses and serializes the new format.

**Decision:** OK

### #2 Default column set

**Setup:** The current skill hardcodes the four columns into a table header.

**Expected:** Skill writes `columns: [Setup, Expected, Notes, Decision]` in the frontmatter as the default set. `Notes` is the catch-all for back-and-forth discussion that the old format didn't have.

**Notes:** Renames `Setup/Structure` → `Setup` and `Expected outcome` → `Expected` for compactness.

**Decision:** !!

### #3 Custom columns guidance

**Setup:** Nothing in the current skill tells Claude that columns can be specialized per topic.

**Expected:** Add a sentence to step 1: "For specialized topics, declare custom columns in the frontmatter — e.g., a security review might use `[Risk, Mitigation, Severity, Status]`."

**Notes:** This is a v1 feature of the TUI that's worth surfacing so Claude actually uses it.

**Decision:** OK

### #4 Status-last enforcement guidance

**Setup:** The parser silently moves `Status` to the end of `columns:` with a warning if it's not already last.

**Expected:** Skill explicitly tells Claude: "`Status` must be the last column in `columns:`."

**Notes:** Cosmetic: files still work if Status is elsewhere, but generating them correctly avoids the parser's reorder warning on every load.

**Decision:** OK

## Popup invocation

How the agent opens the TUI after writing the file. This is the most
substantial change, since the entire popup mechanism is new.

### #5 In-tmux flow

**Setup:** Current skill has no mention of the popup or TUI.

**Expected:** New step 2: "Run `enumerate popup <path>` via Bash. Inside tmux this opens the TUI as a popup over Claude Code and blocks until the user closes it; re-read the file in the same turn."

**Notes:** Bash naturally blocks on `tmux display-popup -E`, so the in-tmux case is a single-turn flow.

**Decision:** OK

### #6 Out-of-tmux flow

**Setup:** No fallback path in the current skill for when Claude Code isn't running inside tmux.

**Expected:** New step 2 sub-branch: "If not in tmux, `enumerate popup` prints the path to stderr and exits without opening anything. End the turn and ask the user to run `enumerate open <path>` themselves, then reply 'done'. Re-read the file in the next turn."

**Notes:** Two-turn flow. The popup binary handles tmux detection internally, so the skill doesn't need to check `$TMUX` itself.

**Decision:** OK

### #7 Deterministic invocation

**Setup:** As written, the skill could be ambiguous about whether to invoke the popup — Claude might judge that some topics don't warrant it, or forget the step entirely.

**Expected:** Phrase the popup invocation as a hard procedural step (`MUST run`, "every time", no optional branches) rather than a tip or recommendation. Calling `enumerate popup` becomes a deterministic output of the skill, not a judgment call.

**Notes:** User-requested. The intent: once `/enumerate` is invoked, the procedure is fixed — write the file, then call `enumerate popup`. No branches, no Claude deciding "this doesn't need a popup". This eliminates a class of "Claude decided not to" failures.

**Decision:** !

## Marker definitions

The vocabulary the user types into the Status column.

### #8 Define !! alongside other markers

**Setup:** Current skill defines `!`, `?`, `OK` in step 1's marker list, but introduces `!!` only in step 2's priority order.

**Expected:** Define `!!` ("must discuss, highest priority") in step 1's marker list alongside `!`, `?`, `OK`.

**Notes:** Small consistency fix that's been a wart since v0. No semantic change.

**Decision:** OK

## Mode contract

The split between what the agent does and what the TUI does.

### #9 Agent walks, TUI browses

**Setup:** No explicit statement of the contract between agent and TUI in the current skill (because there is no TUI yet).

**Expected:** Add a one-line note: "The TUI is browse-mode — the user navigates and edits Status cells in any order. Walk-mode — going through items one at a time, in priority order — happens in step 3, driven by the agent in chat."

**Notes:** Prevents Claude from trying to walk items inside the popup, or trying to do free-form browsing in chat.

**Decision:** OK

## Procedural cleanup

### #10 Remove async-annotation branch

**Setup:** Current step 1 ends with "If the user wants to annotate asynchronously (in another session), stop here."

**Expected:** Remove this branch. The out-of-tmux flow (#6) covers the equivalent case where the user annotates in their own terminal. No separate "is this async?" judgment.

**Notes:** Simplifies the skill flow and removes another judgment call, in line with #7.

**Decision:** OK
