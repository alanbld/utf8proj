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

## Bugs (Fixed)

### ~~`fix container-deps` strips effort values~~ ✅ FIXED
**Date:** 2026-01-09
**Fixed:** 2026-01-09
**Location:** `crates/utf8proj-cli/src/main.rs` (serialize_task function)

**Root cause:** The `serialize_task` function used `else if` for effort, so effort was only written if duration was absent. Changed to separate `if` statements.

**Fix:** Changed lines 1183-1190 from `else if` to two separate `if` blocks.

**Tests added:** `crates/utf8proj-cli/tests/fix_command.rs`
- `fix_container_deps_preserves_effort` - both duration and effort present
- `fix_container_deps_preserves_effort_only` - only effort present
- `fix_container_deps_preserves_assignments` - assignments preserved

---
