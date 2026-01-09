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

## Bugs

### `fix container-deps` strips effort values
**Date:** 2026-01-09
**Severity:** Medium
**Location:** `crates/utf8proj-cli/src/main.rs` (fix command)

**Description:** The `fix container-deps` command rewrites the project file but strips `effort:` properties from tasks. Only `duration:` is preserved.

**Steps to reproduce:**
```bash
# File with effort values
grep -c 'effort:' original.proj  # Returns 41

# After fix
utf8proj fix container-deps original.proj -o fixed.proj
grep -c 'effort:' fixed.proj  # Returns 0
```

**Expected:** All task properties should be preserved, only dependencies added.

**Workaround:** Manually merge effort values back into fixed file.

---
