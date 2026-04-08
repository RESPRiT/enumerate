# enumerate-tui v1 UI spec

UI/interaction spec for the TUI invoked by `enumerate open <file>`.
Companion to `V1_SPEC.md`, which covers the data model, parser, CLI,
and agent integration.

## Layout

Single bordered view, rendered top-to-bottom:

```
┌─────────────────────────────────────────────────────────────┐
│ Topic title                          ⚠ 0   [████░░░░] 4 / 9 │  ← sticky header
├─────────────────────────────────────────────────────────────┤
│                                                             │
│   Group A                                                   │
│   Group A description prose, free-form, may wrap onto       │
│   multiple lines.                                           │
│                                                             │
│        ┌──────┬─────────┬──────────┬────────┬───────┐       │
│        │ #    │ Setup   │ Expected │ Decision │ Notes │       │  ← per-table header row
│        ├──────┼─────────┼──────────┼────────┼───────┤       │
│        │ #1   │ wrapped │ wrapped  │ ▌ OK   │ wrap… │       │
│        │ Name │ text…   │ text…    │        │       │       │
│        └──────┴─────────┴──────────┴────────┴───────┘       │
│                                                             │
│   Group B                                                   │
│   …                                                         │
│                                                             │
│        ┌──────┬─────────┬──────────┬────────┬───────┐       │
│        │ #    │ Setup   │ Expected │ Decision │ Notes │       │
│        ├──────┼─────────┼──────────┼────────┼───────┤       │
│        │ #5 … │ …       │ …        │ ?      │ …     │       │
│        └──────┴─────────┴──────────┴────────┴───────┘       │
│                                                             │
│              ┌────────────────────────────┐                 │
│              │          Submit            │                 │
│              └────────────────────────────┘                 │
│                                                             │
├─────────────────────────────────────────────────────────────┤
│ ↑↓ navigate · ⏎/⇥ next · ⇧⇥ prev · ⌫ delete · Submit closes │  ← sticky footer
└─────────────────────────────────────────────────────────────┘
```

ASCII is illustrative only — actual rendering uses ratatui block borders.

### Sticky header

- **Left:** topic title (from `frontmatter.topic` or H1)
- **Right:** `⚠ N   [████░░░░] X / N`
  - `⚠ N` = parser warning count (omitted if zero)
  - Progress: `X` = cases with non-empty Decision, `N` = total cases
  - Bar fills proportionally

### Body (scrollable)

Scrolls vertically inside the bordered region between header and footer. Each section, in document order:

1. **Section header** — group name. The implicit "(ungrouped)" group renders without a header.
2. **Section description** — group description prose, wrapped. Skipped if empty.
3. **Enumeration table** — centered horizontally.

After all sections, a centered **Submit** button.

### Tables

- All columns from frontmatter `columns:` rendered in declared order.
- Leftmost column is `#` (showing `#N\nName`), then the columns in order.
- All cells text-wrap; row heights grow to fit the tallest cell.
- Per-table header row stays visible while any of that table's rows are on screen.
- Only the **Decision** column cells are interactive; all other columns are read-only display.

### Cell states (Decision column only)

| State | Border | Content | Cursor |
|---|---|---|---|
| Unselected, empty | Faded | Greyscale (no text) | None |
| Unselected, filled | Faded | Colored | None |
| Selected, empty | Highlighted | Greyscale | Blinking square |
| Selected, filled | Highlighted | Colored | Blinking square (after last char) |

Non-Decision columns render with no per-cell border (or a default subdued border) and never receive selection.

### Submit button

- Wide, centered, below the last table.
- Two visual states: unselected (default) and selected (highlighted border, mirroring cell selection).
- Activated by pressing Enter while selected.

### Sticky footer

Single line:

```
↑↓ navigate · ⏎/⇥ next · ⇧⇥ prev · ⌫ delete · Submit closes
```

## Navigation

Selection moves between Decision cells in document order, top to bottom. Submit comes after the last Decision cell.

| Key | Action |
|---|---|
| Up | Move selection to previous Decision cell. No-op on the first cell. From Submit, moves back to the last Decision cell. |
| Down | Move selection to next Decision cell. From the last cell, moves to Submit. From Submit, no-op. |
| Enter | Alias for Down — *unless* selection is on Submit, in which case Enter closes the TUI. |
| Tab | Alias for Down. Pressing Tab on Submit no-ops. |
| Shift+Tab | Alias for Up. |
| Esc, Ctrl+C | Close the TUI immediately. (Autosave means the file is already current.) |

When the selected cell is outside the visible scroll region, the body scrolls so it's visible. The sticky header, footer, and per-table header rows do not scroll.

## Text entry

While a Decision cell is selected:

- **Printable characters** (letters, digits, punctuation, space) — append to the cell content at the end.
- **Backspace** — delete the last character. No-op on empty cell.
- **No left/right cursor movement** within a cell. Cursor is always at end-of-text.
- **No line breaks.** Decision is single-line input, visually wrapped if it gets long.

## Autosave

The file is saved to disk on every change (every printable keypress and every Backspace that mutates a cell). No explicit save key, no save-on-exit. Submit (and Esc/Ctrl+C) just closes; the file is already up to date.

If no changes are made during the session, the file is never written, and any non-canonical content on disk stays as-is. Files are not precious; canonicalization happens lazily as the user makes edits.

## Edge cases

- **No cases.** Render header + empty body + Submit button. Submit is selected by default.
- **Decision column missing in `columns:`.** Normal for template-written files. Parser silently appends `Decision` to `columns:` (and adds an empty Decision field to each case). The TUI navigates it like any other Decision file.
- **Empty group name.** Render with no group header (treated as the implicit "(ungrouped)" group).
- **Window too narrow.** Tables wrap normally per ratatui's table widget; no special responsive layout.
- **Single very tall cell.** Row grows to fit; the rest of the table follows.

## Warnings

The header shows `⚠ N` when N > 0 as a passive indicator. There is no interactive warnings panel in v1 — to inspect warnings in detail, the user opens the file in a text editor. Most warnings clear themselves on the next save (since save canonicalizes).
