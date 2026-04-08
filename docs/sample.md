---
topic: sample enumeration
created: 2026-04-08
columns: [Status Quo, Proposed Change, Reasoning, Decision]
---

# sample enumeration

A small sample using the `default (revision)` template. Navigate with arrow keys, type into Decision cells, press Enter on Submit (or Esc/Ctrl+C) to close.

## Base cases

The simplest cases the TUI should handle.

### #1 Empty decision

**Status Quo:** A case where the user hasn't decided yet.

**Proposed Change:** Renders as empty (greyed out) until the user types something.

**Reasoning:** Empty Decision cells are the default state when a template is first opened — Claude doesn't populate them.

**Decision:**

### #2 Pre-filled decision

**Status Quo:** A case where Decision already has a value when the file loads.

**Proposed Change:** Renders the value in white (filled), with leading marker tokens like `OK`/`!`/`?`/`!!` highlighted in yellow.

**Reasoning:** Decisions persist across sessions; once you've decided, the value is on disk for the next walk.

**Decision:** OK

## Wrapping cases

Cases that exercise text wrapping in cells.

### #3 Long content

**Status Quo:** This is a very long Status Quo field that should wrap onto multiple lines inside its table cell. Markdown tables would truncate this; the whole point of the TUI is to show all of it.

**Proposed Change:** The row grows tall enough to fit all the wrapped text, and every other row in every other table grows to match (uniform row heights).

**Reasoning:** Wrapping is the original motivation for the project. Compare this to how the same content would render in a plain markdown table.

**Decision:** ?
