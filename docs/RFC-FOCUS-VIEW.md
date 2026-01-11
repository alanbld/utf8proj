# RFC: Focus View for Gantt Charts

## Summary

Add a "Focus View" capability to Gantt chart rendering that allows users to view specific task hierarchies in detail while showing the rest of the project as collapsed context bars.

## Motivation

Large projects (like M2C with 200+ tasks) produce unwieldy Gantt charts. Users often need to focus on a specific stream (e.g., "6.3.2 OS Script Migration") while maintaining awareness of how it relates to other project streams.

## Design

### CLI Interface

```bash
utf8proj gantt project.proj --focus="pattern" [--context-depth=N]
```

### Parameters

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `--focus` | String (glob pattern) | None | Tasks matching this pattern are expanded with all descendants |
| `--context-depth` | Integer | 1 | Non-focused tasks collapsed to this depth (0 = hide, 1 = top-level only) |

### Focus Pattern Matching

The focus pattern supports:
- **Task ID prefix**: `6.3.2` matches task IDs starting with "6.3.2"
- **Glob patterns**: `*.3.2.*` matches any task with ".3.2." in its ID
- **Multiple patterns**: `6.3.2,7.5` (comma-separated)

### Rendering Behavior

```
Focused Task Hierarchy:
├── task_A (focused) ─────────────── EXPANDED
│   ├── task_A.1 ════════           (show all children)
│   ├── task_A.2 ═══════════════
│   └── task_A.3 ════════════

Non-Focused Tasks (context-depth=1):
├── task_B ▶─────────────────────── COLLAPSED (single bar)
├── task_C ▶─────────────────────── COLLAPSED (single bar)
```

### Visual Styling

| State | Bar Style | Icon | Color |
|-------|-----------|------|-------|
| Expanded container | Thin bracket | ▼ | Normal |
| Collapsed container | Solid bar | ▶ | Muted/gray |
| Expanded leaf task | Normal bar | None | Normal |
| Focused ancestor | Thin bracket | ▼ | Highlighted border |

### Algorithm

```
function determine_visibility(task, focus_patterns, context_depth):
    if task matches any focus_pattern:
        return EXPANDED
    if task is ancestor of any focused task:
        return EXPANDED (to show path to focused)
    if task is descendant of any focused task:
        return EXPANDED
    if task.depth <= context_depth:
        return COLLAPSED
    return HIDDEN
```

### Data Structures

```rust
/// Configuration for focus view rendering
#[derive(Clone, Debug, Default)]
pub struct FocusConfig {
    /// Patterns to match for focused (expanded) tasks
    pub focus_patterns: Vec<String>,
    /// Depth to show for non-focused tasks (0 = hide, 1 = top-level)
    pub context_depth: usize,
}

/// Visibility state for a task in focus view
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TaskVisibility {
    /// Show task and all descendants
    Expanded,
    /// Show task as collapsed summary bar (no children)
    Collapsed,
    /// Do not show task
    Hidden,
}
```

## Examples

### Example 1: Focus on single stream

```bash
utf8proj gantt m2c.proj --focus="6.3.2" --context-depth=1
```

Output:
```
[5] PIL&FOUND          ▶════════════════════
[6] DEV&TRANSL         ▶════════════════════════════
  [6.3.2] OS Script Migration
    [6.3.2.1] GNU Validation    ════════
    [6.3.2.2] ABL Services      ═══════════════
    [6.3.2.3] ABL Mig Part 1    ════
    [6.3.2.5] ABL Mig Part 2    ═══════════════════
    [6.3.2.6] Shell Migration   ══════════════
    [6.3.2.7] Perl Migration    ══════════
    [6.3.2.8] Platform Cutover  ═════════
[7] ENV                ▶════════════════════════════════
[8] VAL&REH            ▶══════════════════════════════════════
[9] CUTOVER            ▶════════════════════════════════════════
```

### Example 2: Focus on multiple streams

```bash
utf8proj gantt m2c.proj --focus="6.3.2,8.6" --context-depth=1
```

### Example 3: Hide context entirely

```bash
utf8proj gantt m2c.proj --focus="6.3.2" --context-depth=0
```

## Implementation Plan

1. **Phase 1**: Add `FocusConfig` to `utf8proj-render`
2. **Phase 2**: Implement visibility calculation in Gantt renderer
3. **Phase 3**: Add collapsed bar rendering style
4. **Phase 4**: Add CLI options to `utf8proj-cli`
5. **Phase 5**: Integration tests with real projects

## Test Cases

1. No focus (default behavior) - all tasks visible
2. Focus on leaf task - only that task expanded
3. Focus on container - container and all descendants expanded
4. Focus pattern with glob - multiple matches expanded
5. Context depth 0 - non-focused tasks hidden
6. Context depth 1 - only top-level containers shown
7. Context depth 2 - two levels of containers shown
8. Ancestor visibility - path to focused task always visible
9. Multiple focus patterns - union of matches expanded

## Acceptance Criteria

- [ ] `--focus` pattern correctly identifies tasks to expand
- [ ] Non-focused containers render as collapsed bars
- [ ] Collapsed bars span min(start) to max(finish) of children
- [ ] Visual distinction between collapsed and expanded states
- [ ] Ancestors of focused tasks are visible (to show hierarchy context)
- [ ] Works with SVG, HTML, and Mermaid output formats
