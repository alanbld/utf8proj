# RFC-0005: Resource Leveling

**RFC Number:** 0005
**Status:** Phase 1 Complete, Phase 2 Deferred
**Last Updated:** 2026-01-09
**Related:** RFC-0002 (CPM Correctness)

---

## Phase 1: Complete

Phase 1 resource leveling is fully implemented and stable:

| Component | Status |
|-----------|--------|
| `LevelingOptions` | Implemented |
| `LevelingStrategy::CriticalPathFirst` | Implemented |
| `LevelingReason` enum | Implemented |
| `LevelingMetrics` | Implemented |
| `LevelingResult` with original/leveled schedules | Implemented |
| Deterministic conflict sorting | Implemented |
| L001-L004 diagnostics | Implemented |
| CLI `--max-delay-factor` | Implemented |
| LSP hover for L001-L004 | Implemented |
| Test coverage | 93.2% |

### What Phase 1 Solves

1. **"Why is my schedule impossible?"**
   - Overallocation detected with exact resources, dates, peaks
   - Deterministic explanation via L001/L002

2. **"What happens if I allow leveling?"**
   - Explicit opt-in (`-l` flag)
   - Minimal, explainable shifts
   - Clear trade-offs (L003/L004)

3. **"Can I trust the result?"**
   - Deterministic (same input = same output)
   - Original schedule preserved for audit
   - No hidden optimization
   - Full explainability chain (solver → CLI → LSP)

---

## Phase 2: Deferred

Phase 2 features are **not scheduled** and will only be considered if real user demand emerges.

### Candidate Features (Not Implemented)

| Feature | User Pain | Decision |
|---------|-----------|----------|
| Alternative strategies | Rare | Deferred |
| Manual override points | Medium | Out of scope (editor concern) |
| What-if comparisons | Medium | Out of scope (workflow concern) |
| Utilization expansion | Low | Deferred |
| Task splitting | High | Rejected (breaks explainability) |

### Why Phase 2 Is Deferred

1. **Phase 1 is complete, not incomplete v0**
   - All essential user questions are answered
   - Coverage is strong, diagnostics are clear
   - No semantic gaps

2. **Most Phase 2 items are workflow features, not model features**
   - utf8proj is a language + solver, not a project management IDE
   - Editor/dashboard concerns belong outside the solver

3. **Risk of semantic drift**
   - Additional strategies could introduce nondeterminism
   - Optimization creep violates core philosophy

---

## Decision Framework

Resource leveling evolves **only in response to user-authored `.proj` pressure** — never speculative capability.

### Triggers That Would Justify Phase 2

Implement Phase 2 only if at least one appears:

1. Multiple real projects request: *"I accept delay X but want to protect milestone Y"*
2. Users encode priorities that Phase 1 cannot respect without nondeterminism
3. Third-party tools demand richer metrics without solver changes
4. A second algorithm can be proven:
   - Deterministic
   - Explainable
   - Non-optimizing
   - Monotonic (never improves one thing by hiding another)

Until then: **do not move**.

---

## Core Principle

> utf8proj does not optimize your project.
> It tells you exactly *why* it cannot be done as written.

This is not a limitation. This is the value proposition.
