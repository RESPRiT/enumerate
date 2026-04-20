---
topic: expansion overlap test
created: 2026-04-17
columns: [Setup, Expected, Reasoning, Decision]
---

# expansion overlap test

Cases with very long text to test expansion overlapping rows below.

## Overlap group

### #1 Tall expansion

**Setup:** This case has a very long setup field that will cause the expansion to extend well past the capped row height. It keeps going and going to make sure the expanded overlay covers the rows below it. We want to verify that the obscured text from row 2 shows through in dark grey underneath the expansion padding area.

**Expected:** The expansion dropdown should show dimmed text from the row below peeking through where the expanded content has unused padding lines.

**Reasoning:** This is the primary test: a tall expansion that overlaps the next row. The underlying row 2 content should be visible but dimmed in the expansion's padding area. Also testing that the yellow bottom border connects to the left table border with the correct T or L intersection character.

**Decision:**

### #2 Row underneath

**Setup:** This is row 2. Its text should appear dimmed inside row 1's expansion overlay, in the padding area below row 1's actual content.

**Expected:** Visible but dark grey when row 1 is expanded above.

**Reasoning:** Short reasoning for row 2.

**Decision:**

### #3 Another row below

**Setup:** Row 3 content. If the expansion from row 1 is tall enough, this row might also be partially obscured.

**Expected:** Should render normally if not covered by the expansion, or dimmed if it is.

**Reasoning:** Tests multi-row overlap.

**Decision:**
