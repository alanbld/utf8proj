# Design Review: W014 Container Dependency Semantics

**Review ID:** DR-2026-01-W014-container-deps
**Date:** 2026-01-07
**Reviewer:** Claude (Opus 4.5)
**RFC:** RFC003_CONTAINER_DEPENDENCY_SEMANTICS.md
**Status:** APPROVED

---

## Summary

This design review formally approves the implementation of W014 (Container Dependency Without Child Dependencies) diagnostic and the underlying architectural decisions for container dependency semantics in utf8proj.

---

## Items Reviewed

### 1. W014 Diagnostic Implementation

**Location:** `crates/utf8proj-solver/src/lib.rs`

**Assessment:** Semantically correct and well-placed.

**Details:**
- Correctly identifies when containers have dependencies but children lack matching dependencies
- Emits one warning per affected child (not one per container)
- Message format matches DIAGNOSTICS.md specification
- Includes both MS Project behavior explanation and utf8proj behavior
- Provides actionable hint for remediation

### 2. Validation-First Architecture

**Assessment:** Sound architectural decision.

**Rationale:**
- utf8proj's role is to validate and schedule, not to convert
- Conversion complexity belongs in external tools (ms2tj, import utilities)
- Clear separation of concerns enables better testing and maintenance
- Diagnostic system provides clear feedback without silent behavior changes

### 3. Separation of WBS and DAG

**Assessment:** Correct design boundary.

**Rationale:**
- Containers (WBS hierarchy) are organizational, not scheduling
- Dependencies (DAG) are the source of truth for scheduling
- This separation enables:
  - Git-diffable schedule changes
  - Refactorable WBS without breaking schedule
  - Explicit, visible dependencies
  - Correct CPM computation

### 4. MS Project Compatibility Strategy

**Assessment:** Pragmatic approach approved.

**Strategy:**
- Detect divergence from MS Project semantics
- Warn user with clear explanation
- Offer auto-fix capability (`--fix-container-deps`)
- Preserve utf8proj's explicit-dependency principle

---

## Test Coverage

**File:** `crates/utf8proj-solver/tests/container_dependency_diagnostics.rs`

**Tests Verified:**
| Test | Purpose | Status |
|------|---------|--------|
| `w014_triggers_when_child_missing_container_dep` | Basic trigger condition | PASS |
| `w014_does_not_trigger_when_child_has_dep` | Explicit dep suppresses warning | PASS |
| `w014_triggers_for_each_missing_child` | Per-child emission | PASS |
| `w014_does_not_trigger_for_empty_container_deps` | Container without deps | PASS |
| `w014_does_not_trigger_for_leaf_tasks` | Leaf task with deps | PASS |
| `w014_nested_containers` | Multi-level hierarchy | PASS |
| `w014_message_includes_task_names` | Message content verification | PASS |

---

## Risks and Mitigations

| Risk | Mitigation |
|------|------------|
| Users confused by MS Project difference | W014 explains both behaviors |
| Over-warning on large projects | Per-child granularity allows targeted fixes |
| Performance on deep hierarchies | Recursive traversal is O(n) |

---

## Approved Next Steps

1. **Implement `utf8proj fix container-deps` command**
   - Auto-propagates container dependencies to children
   - Operates on .proj files (post-conversion)
   - Idempotent and safe

2. **Document in CLI help**
   - Add `--fix-container-deps` to import workflow
   - Update man pages

3. **Integration testing**
   - Test with real MS Project imports
   - Verify W014 count matches expectations

---

## Decision

**APPROVED**

The W014 diagnostic implementation and container dependency semantics are:
- Semantically correct
- Architecturally sound
- Well-tested
- Properly documented

This design review constitutes formal approval to proceed with the `fix container-deps` command implementation.

---

## Sign-Off

| Role | Name | Date |
|------|------|------|
| Design Reviewer | Claude (Opus 4.5) | 2026-01-07 |
| Implementation Author | Alan | 2026-01-06 |

---

## References

- [RFC003: Container Dependency Semantics](../rfcs/RFC003_CONTAINER_DEPENDENCY_SEMANTICS.md)
- [DESIGN_PHILOSOPHY.md](../DESIGN_PHILOSOPHY.md)
- [DIAGNOSTICS.md](../DIAGNOSTICS.md) - W014 specification
- [TEST_SPEC_CONTAINER_DEPS.md](../specs/TEST_SPEC_CONTAINER_DEPS.md)
