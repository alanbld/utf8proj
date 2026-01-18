# RFC 0012 — Temporal Regimes: Explicit Time Semantics for Tasks and Events

**Status:** Draft
**Author:** utf8proj core team
**Created:** 2026-01-18
**Target Version:** ≥ 0.10
**Supersedes:** Implicit calendar-based task semantics
**Related Issues:** SNET/FNET rounding on non-working days, milestone scheduling semantics

---

## Executive Summary

This RFC introduces **Temporal Regimes**: an explicit abstraction that defines *how time behaves* for different kinds of project activities.

By separating **work**, **events**, and **deadlines** at the semantic level, utf8proj resolves long-standing scheduling ambiguities, fixes known bugs (e.g. SNET on weekends), and establishes a foundation for advanced project modeling that traditional tools cannot express.

---

## 1. Motivation: The Calendar Conflation Problem

### 1.1 One Calendar, Too Many Meanings

Today, utf8proj (like most PM tools) relies on calendars to model fundamentally different temporal concepts:

| Real-World Concept | Current Modeling | Issue |
|-------------------|------------------|-------|
| **Work effort** (coding, construction) | Working-day calendar | ✅ Correct |
| **Events** (releases, audits) | Working-day calendar + hacks | ❌ Events aren't work |
| **Deadlines** (contractual) | Working-day calendar | ❌ Calendar mismatch |
| **Milestones** | Zero-duration tasks | ❌ Rounding ambiguity |

Calendars are designed to answer *"when can work be done?"*
They are routinely misused to answer *"when does something happen?"*

### 1.2 The SNET Bug as a Symptom

The recently fixed bug:

```
start_no_earlier_than: 2018-06-03  # Sunday
→ scheduled on 2018-06-01 (Friday)
```

exposed the deeper issue:

> **Events do not follow work calendars.**

Fixing rounding logic alone treats the symptom, not the cause.

### 1.3 Why This Matters Now

* Professional users expect correct event scheduling
* Complex projects mix work, events, and deadlines
* Ad-hoc exceptions increase solver complexity and user confusion

---

## 2. Design Principles

1. **Calendars constrain effort, not reality**
2. **Events exist independently of work availability**
3. **Temporal semantics must be explicit**
4. **Backward compatibility is mandatory**

---

## 3. Proposal: Temporal Regimes

A **Temporal Regime** defines how time behaves for a task:

* which days advance time
* how constraints round (or do not)
* how durations are interpreted

A task operates under exactly **one regime**, intrinsic to its nature.

---

## 4. Canonical Regimes

### 4.1 Work Regime (default)

For effort-bearing tasks.

```
Advances on: working days
Floor constraints: round forward
Ceiling constraints: round backward
Duration meaning: effort days
```

Examples: development, construction, testing.

---

### 4.2 Event Regime (default for milestones)

For factual occurrences.

```
Advances on: all calendar days
Constraint rounding: none (exact dates)
Duration meaning: point-in-time (typically 0)
```

Examples: releases, approvals, audits, go-live dates.

---

### 4.3 Deadline Regime (future)

For legal or contractual constraints.

```
Advances on: calendar days
Constraint rounding: none
Duration meaning: calendar days
```

---

## 5. Implicit vs Explicit Regimes

### Phase 1 (≤ 0.9.x): Implicit Assignment

No syntax changes.

```rust
if task.is_milestone() {
    regime = Event
} else {
    regime = Work
}
```

This alone fixes SNET/FNET weekend behavior correctly.

---

### Phase 2 (≥ 0.10): Explicit Syntax

```proj
task milestone "Release v1.0" {
    regime: event
    start_no_earlier_than: 2024-06-03  # Sunday, honored exactly
}

task "Implementation" {
    regime: work
    duration: 10d  # working days
}
```

Explicit regimes communicate intent and enable validation.

---

## 6. Solver Architecture Impact

### 6.1 Core Abstraction

```rust
enum TemporalRegime {
    Work,
    Event,
    Deadline, // future
}
```

Each regime governs:

* date advancement
* constraint application
* rounding rules

Dependencies transfer **dates**, not regimes.

---

## 7. Mixed-Regime Dependencies

Example:

```proj
task milestone "Approval" {
    regime: event
    start_no_earlier_than: 2024-06-03  # Sunday
}

task "Implementation" {
    regime: work
    depends: "Approval"
    duration: 5d
}
```

**Rule:**

* Approval occurs Sunday
* Implementation advances from Sunday using *work* rules → starts Monday

This mirrors real-world reasoning and avoids semantic leakage.

---

## 8. Benefits

### Immediate

* Fixes known constraint bugs naturally
* Removes milestone special-casing
* Simplifies solver logic

### Medium-Term

* Clearer language semantics
* Better diagnostics and validation
* Fewer calendar hacks

### Long-Term

* Custom temporal regimes
* Industry-specific modeling
* Advanced temporal analysis

---

## 9. Alternatives Rejected

### Calendar Exceptions

❌ Treat symptoms, not cause
❌ Do not generalize

### Task Flags (`ignore_calendar`)

❌ Implicit semantics
❌ Non-composable

### Separate "Event Calendars"

❌ Calendars still model work availability conceptually

---

## 10. Backward Compatibility

**Guarantee:**
Existing projects will behave identically or *more correctly*.

Milestones that previously snapped to working days will now schedule on their actual dates without requiring changes.

---

## 11. Open Questions

* Should regimes be visualized explicitly in reports?
* Should dependencies be validated across incompatible regimes?
* When to expose user-defined regimes?

---

## 12. Conclusion

Temporal Regimes elevate time from an implementation detail to a first-class concept.

They resolve real bugs, clarify semantics, simplify the solver, and position utf8proj as a genuinely next-generation project planning language.

> *Calendars describe when we can work.*
> *Regimes describe what time means.*

---

**Recommendation:**
Approve Phase 1 for immediate inclusion, with Phase 2 targeted for 0.10.
