# RFC-0011: Classifier Abstraction

**Status:** Implemented
**Created:** 2026-01-16
**Author:** Claude + Human collaboration

## Summary

Add a `Classifier` trait that discretizes continuous task attributes into categorical labels, enabling multiple view patterns (Kanban, risk matrix, budget tracking) through a single abstraction.

## Motivation

1. **Avoid hardcoding views**: Kanban is just one way to categorize tasks by progress
2. **Reuse infrastructure**: Same grouping logic works for risk, budget, timeline views
3. **Extensibility**: New classifiers without core changes
4. **Read-only guarantee**: Classifiers observe state, never mutate it

### Use Cases

| Classifier | Input | Categories | Order |
|------------|-------|------------|-------|
| StatusClassifier | complete % | Backlog, Ready, Doing, Review, Done | 0-4 |
| RiskClassifier | float/slack | Critical, High, Medium, Low | 0-3 |
| BudgetClassifier | cost variance | Over, At Risk, On Track, Under | 0-3 |

## Design

### Core Trait

```rust
pub trait Classifier: Send + Sync {
    /// Human-readable name for this classification scheme
    fn name(&self) -> &'static str;

    /// Classify a task into a category with ordering
    /// Returns (label, order) where order determines column position
    fn classify(&self, task: &Task, schedule: &Schedule) -> (String, usize);
}
```

### Group-By Function

```rust
pub fn group_by<'a>(
    project: &'a Project,
    schedule: &'a Schedule,
    classifier: &dyn Classifier,
) -> Vec<(String, Vec<&'a Task>)>
```

Returns tasks grouped by classifier category, sorted by the classifier's natural order.

### StatusClassifier Implementation

```rust
pub struct StatusClassifier;

impl Classifier for StatusClassifier {
    fn name(&self) -> &'static str { "Progress Status" }

    fn classify(&self, task: &Task, _schedule: &Schedule) -> (String, usize) {
        let pct = task.percent_complete.unwrap_or(0.0);

        match pct {
            0.0 => ("Backlog".into(), 0),
            0.01..=25.0 => ("Ready".into(), 1),
            25.01..=75.0 => ("Doing".into(), 2),
            75.01..=99.99 => ("Review".into(), 3),
            100.0 => ("Done".into(), 4),
            _ => ("Invalid".into(), 5),
        }
    }
}
```

### CLI Integration

```bash
# Classify tasks by progress status
utf8proj classify project.proj --by status

# Output:
# Progress Status:
#   Backlog: task1, task2
#   Ready: task3
#   Doing: task4, task5
#   Review: task6
#   Done: task7, task8
```

## File Structure

```
crates/utf8proj-core/src/
├── lib.rs                    # Add: pub mod classifier;
└── classifier.rs             # Classifier trait + StatusClassifier + group_by

crates/utf8proj-cli/src/
└── main.rs                   # Add: classify subcommand
```

## Test Plan

| Test | Purpose |
|------|---------|
| test_classifier_trait_exists | Trait compiles |
| test_group_by_empty_project | Empty input returns empty |
| test_group_by_single_task | Single task grouped correctly |
| test_group_by_multiple_tasks | Multiple tasks in multiple groups |
| test_status_classifier_ranges | All progress ranges covered |
| test_status_classifier_boundaries | Edge cases (0%, 25%, 75%, 100%) |
| test_status_classifier_invalid | >100% handled as Invalid |
| test_group_by_ordering | Groups sorted by order value |
| test_classifier_no_mutation | Project unchanged after classification |
| test_custom_classifier | User-defined classifier works |
| test_cli_classify_command | CLI outputs correct format |
| test_cli_classify_with_file | CLI parses and classifies file |

**Target: 12-15 tests**

## Implementation Plan

### Day 1
- [x] Create RFC document
- [x] Add `classifier.rs` to utf8proj-core (inline in lib.rs)
- [x] Implement `Classifier` trait
- [x] Implement `group_by` function
- [x] Write 10+ unit tests

### Day 2
- [x] Implement `StatusClassifier`
- [x] Add CLI `classify` command
- [x] Integration tests with example files

### Day 3
- [x] Documentation
- [x] Performance verification
- [x] Mark RFC as Implemented

## Success Criteria

- [x] 12+ tests passing (18 tests)
- [x] No regression in existing tests
- [x] CLI command works with example projects
- [x] New classifier implementable in <30 minutes

## Future Extensions (Explicitly Deferred)

- RiskClassifier (by float/slack)
- BudgetClassifier (by cost variance)
- TimelineClassifier (by deadline proximity)
- Interactive Kanban UI
- Drag-and-drop status changes
- WIP limits
- Swimlanes

These are separate RFCs, not scope creep for RFC-0011.

## References

- [RFC-0008: Progress-Aware CPM](RFC-0008-PROGRESS-AWARE-CPM.md) - Source of complete %
- [utf8proj Design Principles](../EXPLAINABILITY.md) - Read-only views
