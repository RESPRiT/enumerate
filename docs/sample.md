---
topic: sample enumeration
created: 2026-04-07
columns: [Setup, Expected, Notes, Status]
---

# sample enumeration

A small sample to try out the TUI. Navigate with arrow keys, type into Status cells, press Enter on Submit (or Esc/Ctrl+C) to close.

## Base cases

The simplest cases the TUI should handle.

### #1 Empty status

**Setup:** A case where Status hasn't been filled in yet.

**Expected:** Renders as empty (greyed out).

**Notes:** Type something while this row is selected to see autosave.

**Status:**

### #2 Pre-filled status

**Setup:** A case where Status already has a value when the file loads.

**Expected:** Renders the value in green (filled).

**Notes:** This one should look different from #1.

**Status:** OK

## Wrapping cases

Cases that exercise text wrapping in cells.

### #3 Long Setup

**Setup:** This is a very long Setup field that should wrap onto multiple lines inside its table cell. Markdown tables would truncate this; the whole point of the TUI is to show all of it.

**Expected:** The row grows tall enough to fit all the wrapped text.

**Notes:** Compare this to how the same content would look in a markdown table.

**Status:** ?
