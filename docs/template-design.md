# Column template design

Skeletons for the named templates plus a custom shell. Crib from these; the design constraints are encoded in the structure, not in a separate rules list.

## Built-in skeletons

```yaml
columns: [Status Quo, Proposed Change, Reasoning]   # revision: before / after / why
columns: [Case, Example, Verdict]                   # enumeration: scenario / sample / call
columns: [Suggestion, Example, Reasoning]           # ideation: option / illustration / pros-cons
columns: [Task, State, Notes]                       # status: what / where / context
```

## Custom skeleton

```yaml
columns: [<concrete>, <concrete>, <reasoning-equivalent>]
```

- 3 slots is the sweet spot; 2 collapses concerns, 5+ is hard to scan
- Last slot must be reasoning-equivalent (`Verdict`, `Analysis`, `Tradeoffs`, etc.)
- Column names should make their content obvious without a description
- Adjacent columns must hold visually distinct content
- Decision column is auto-appended by the binary; never include it
