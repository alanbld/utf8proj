# utf8proj Wishlist

Feature requests and enhancements for future consideration.

## Rendering

### SVG Gantt: Support for wider layouts / full task names
**Date:** 2026-01-09
**Context:** When task names are long (e.g., 128 chars with WBS codes), SVG renderer truncates them. The `-w` parameter helps but very long names still get cut off.

**Possible solutions:**
1. Dynamic SVG width based on longest task name
2. Horizontal scrolling support (wrap SVG in scrollable container)
3. HTML Gantt export via CLI (already implemented in `utf8proj-render`, just not exposed)
4. Tooltip on hover showing full name (requires JS/HTML wrapper)

**Workaround:** Use Excel export for full task names.

---
