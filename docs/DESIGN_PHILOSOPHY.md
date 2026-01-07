# utf8proj Design Philosophy

**Version:** 1.0  
**Status:** Authoritative  
**Last Updated:** 2026-01-06

---

## Core Principle: Explicit Over Implicit

utf8proj is built on a foundational belief: **project scheduling logic should be visible, version-controllable, and auditable**. This drives every design decision.

### The Zen of utf8proj

```
Explicit is better than implicit.
Text files are better than binary formats.
Recomputation is better than cached state.
Compiler thinking beats spreadsheet thinking.
Dependencies should be visible in git diffs.
Impossible schedules should be unrepresentable.
```

---

## The Great Divide: WBS vs DAG

### What MS Project Gets Wrong

Microsoft Project and most traditional PM tools **conflate two orthogonal concerns:**

| Concept | Purpose | Should Affect Scheduling? |
|---------|---------|---------------------------|
| **WBS** (Work Breakdown Structure) | Organize work for reporting, budgeting, visualization | **NO** |
| **DAG** (Directed Acyclic Graph) | Define precedence relationships for scheduling | **YES** |

**The Problem:** MS Project makes container hierarchy **implicitly create scheduling constraints**. When you make a container depend on something, all children inherit that dependency invisibly.

**The Solution:** utf8proj **separates organization from sequencing**. Containers are for human organization. Dependencies are for CPM scheduling.

### Concrete Example

**MS Project (Implicit Inheritance):**
```xml
<Task>
  <Name>Development</Name>
  <PredecessorLink>
    <PredecessorUID>42</PredecessorUID>  <!-- Design Approval -->
  </PredecessorLink>
  <Task>
    <Name>Feature X</Name>
    <!-- No predecessor declared -->
    <!-- But IMPLICITLY blocked by Design Approval! -->
  </Task>
</Task>
```

**Result:** Feature X cannot start until Design Approval completes, even though this constraint is **nowhere in Feature X's definition**.

**utf8proj (Explicit Dependencies):**
```proj
task development {
  # This is just organizational metadata
  # It does NOT block children
  depends: design_approval
  
  task feature_x {
    # Want it blocked? Say so explicitly:
    depends: design_approval
    duration: 5d
  }
  
  task feature_y {
    # Different task, different logic
    depends: feature_x
    duration: 3d
  }
}
```

**Result:** Feature X's scheduling constraints are **visible in its own definition**. You can reorganize the WBS without breaking the schedule.

---

## Why This Matters

### 1. **Git-Friendly Scheduling**

When dependencies are explicit, git diffs show scheduling changes:

```diff
 task feature_x {
+  depends: design_approval  # New constraint added
   duration: 5d
 }
```

With implicit dependencies, reorganizing containers can **silently change scheduling logic** with no diff.

### 2. **Refactorable Structure**

Want to split "Development" into "Backend Development" and "Frontend Development"?

**MS Project:** Breaking the container breaks inherited dependencies. Every child must be manually updated.

**utf8proj:** Just move the tasks. Their explicit dependencies remain valid:

```proj
task backend_dev {
  task feature_x {
    depends: design_approval  # Still works
  }
}

task frontend_dev {
  task feature_y {
    depends: feature_x  # Still works across containers
  }
}
```

### 3. **Self-Documenting**

A .proj file is **executable documentation**:

```proj
task integration_test {
  depends: backend.api, frontend.ui  # Clear: needs both
  duration: 3d
}
```

No need to open MS Project, expand trees, and inspect UI to understand scheduling logic.

### 4. **Compiler-Like Correctness**

MS Project is like Excel: you can create impossible schedules by moving cells around.

utf8proj is like Rust: the CPM engine is a "borrow checker" for time. Circular dependencies, impossible constraints, and hidden conflicts are **caught at compile time**.

---

## The "Frozen Artifact" vs "Pure Function" Divide

### MS Project: Stateful Spreadsheet

MS Project works like Excel:
- Dates are **mutable cells**
- Manual overrides "stick"
- "Last known good" state is preserved
- **State machine model**: schedule evolves through user edits

### utf8proj: Deterministic Compiler

utf8proj works like a compiler:
- Dates are **computed outputs**
- Every run recomputes from axioms
- No sticky state or cached results
- **Pure function model**: `schedule = f(project, constraints, progress)`

**Example:**

**MS Project:**
```
1. Import project with 100 tasks
2. Manually adjust Task 47 to start on specific date
3. Add new task → Task 47's manual override persists
4. Export to XML → manual date is "frozen" in file
5. Re-import → still has manual override
```

**utf8proj:**
```
1. Parse project.proj with 100 tasks
2. Add constraint to Task 47: must_start_on(date)
3. Run schedule() → dates computed from constraints
4. Edit file, add task → run schedule() again
5. All dates recomputed deterministically
```

**Why deterministic is better:**
- **Reproducible:** Same .proj file → same schedule
- **Debuggable:** No hidden state to reverse-engineer
- **Auditable:** Git history shows constraint evolution
- **Testable:** `schedule()` is a pure function

---

## MS Project Compatibility Philosophy

### The Challenge

When importing MS Project files, we face a choice:

1. **Mimic MS Project's implicit behavior** (match user expectations but hide logic)
2. **Require explicit dependencies** (expose logic but differ from MS Project)

### Our Solution: Diagnose and Educate

utf8proj chooses **Option 2 + Education**:

```
✓ Imported 45 tasks, 12 resources
⚠ 3 semantic differences detected (use --explain for details)

W014: Container dependency without child dependencies

  Location: Development Phase → Feature X
  
  MS Project: Feature X implicitly blocked until Design Approval
  utf8proj:   Feature X can start immediately (no explicit dependency)
  
  To match MS Project behavior:
    task feature_x {
      depends: design_approval  # Add explicit dependency
      duration: 5d
    }
  
  Auto-fix: utf8proj import --fix-container-deps project.mpp
```

**Rationale:**
- Respects user intelligence (explains difference)
- Preserves utf8proj philosophy (explicit dependencies)
- Offers pragmatic solution (auto-fix flag)
- Creates teaching moment (educates about design choice)

---

## Design Principles in Detail

### 1. Text-Based Source of Truth

**Principle:** Project schedules should be human-readable text files.

**Why:**
- Version control (git diff, blame, history)
- Code review (pull requests for schedule changes)
- Automation (CI/CD integration, scripting)
- Portability (no proprietary binary formats)
- Longevity (text files survive tool changes)

**Example:**
```proj
# This is source code for a project schedule
task q1_release {
  duration: 12w
  depends: requirements_complete
  
  task development {
    effort: 30d
    assign: dev_team
  }
}
```

### 2. Single Binary, Zero Dependencies

**Principle:** Installing utf8proj should be `curl | sh` simple.

**Why:**
- No Ruby (unlike TaskJuggler)
- No JVM (unlike ProjectLibre)
- No Python environment (unlike most tools)
- Just: Download. Run. Schedule.

**Reality Check:**
```bash
# TaskJuggler
$ gem install taskjuggler
# (Hope Ruby version matches, hope dependencies resolve)

# utf8proj
$ curl -L https://github.com/alanbld/utf8proj/releases/latest/download/utf8proj | sh
# Done. No dependencies, no hassle.
```

### 3. CPM Correctness Above All

**Principle:** Scheduling math must be textbook-correct.

**Why:**
- Critical path is mathematically defined
- Slack/float calculations must be precise
- Progress tracking must respect CPM semantics
- Users trust tools that get math right

**Non-Negotiable:**
- Early Start = max(predecessors' Early Finish) + lag
- Late Start = Late Finish - duration
- Slack = Late Start - Early Start
- Critical path = tasks with slack = 0

### 4. Progress Tracking is Data, Not Magic

**Principle:** Progress is explicitly recorded, not inferred.

**Why:**
- No "% complete" guessing games
- Actual dates are facts, not estimates
- Forecast finish = f(remaining work, completion rate)
- Variance = actual - planned (simple, auditable)

**Example:**
```proj
task backend_api {
  duration: 20d
  
  # Progress is DATA
  actual_start: 2025-02-01
  complete: 60%        # 12 of 20 days done
  remaining: 10d       # Reestimate: will take 22d total
  
  # Forecast computed from data
  # forecast_finish: 2025-02-23 (2 days late)
}
```

### 5. Constraints Over Overrides

**Principle:** Don't fight the CPM engine. Add constraints instead.

**Why:**
- Manual date overrides create "frozen artifacts"
- Constraints are **declarative** and **composable**
- Constraints can be validated and diagnosed
- Constraints enable "what-if" analysis

**Bad (MS Project style):**
```
Task 47: Start = 2025-03-15  [manually set, sticky]
```

**Good (utf8proj style):**
```proj
task task_47 {
  must_start_on: 2025-03-15
  # Or: start_no_earlier_than: 2025-03-15
  # Or: depends: external_milestone
}
```

### 6. Fail Fast, Fail Loud

**Principle:** Invalid schedules should be compile errors, not runtime surprises.

**Why:**
- Circular dependencies: **Error** (not silently ignored)
- Over-allocated resources: **Warning** (not hidden until report)
- Constraint conflicts: **Error with explanation**
- Impossible progress states: **Error with diagnostic**

**Example:**
```bash
$ utf8proj schedule project.proj

Error: Circular dependency detected
  
  task_a depends on task_b
  task_b depends on task_c
  task_c depends on task_a
  
  Cycle: task_a → task_b → task_c → task_a
  
  Fix: Remove one dependency to break the cycle
```

---

## Philosophical Influences

### From Programming Languages

**Rust:**
- Explicit over implicit (no hidden conversions)
- Compiler catches errors (no runtime surprises)
- Zero-cost abstractions (fast + correct)

**Python (Zen):**
- "Explicit is better than implicit"
- "Simple is better than complex"
- "Readability counts"

**Haskell:**
- Pure functions (deterministic scheduling)
- Type safety (impossible states unrepresentable)
- Declarative constraints (not imperative commands)

### From Software Engineering

**Git:**
- Text-based (diffs, merges, history)
- Deterministic (same input → same output)
- Distributed (no central server required)

**Make:**
- Declarative dependencies
- Recompute when inputs change
- Show what's being done and why

**Compiler Design:**
- Parse → Validate → Optimize → Execute
- Clear error messages with suggestions
- Single-pass determinism

---

## Non-Goals (Things We Explicitly Reject)

### 1. ❌ GUI-First Design

We will **not** design around GUI constraints. The .proj text format is the interface. GUI (if built) is sugar.

### 2. ❌ MS Project Bug-for-Bug Compatibility

We respect MS Project but won't replicate its implicit behaviors. We'll document differences and help users migrate.

### 3. ❌ Enterprise-Only Feature Bloat

utf8proj stays focused on **scheduling correctness**. We won't add Gantt prettiness, resource histogram colors, or "Executive Dashboard" features that dilute the core.

### 4. ❌ Stochastic/Monte Carlo Scheduling

We're a **deterministic CPM engine**. Probabilistic scheduling (PERT, Monte Carlo) is a different paradigm. We won't half-ass it.

### 5. ❌ "Magic" Auto-Scheduling

We won't guess what users mean. If you want a constraint, declare it. If you want a dependency, specify it. No AI inference, no "smart" defaults that hide logic.

---

## The Long Game: Version-Controlled PM

### The Vision

In 10 years, we want project schedules to be treated like source code:

```bash
# Branch for alternative timeline
git checkout -b scenario/aggressive-timeline

# Edit constraints
vim project.proj

# See impact
utf8proj schedule --explain-changes

# Compare scenarios
utf8proj diff scenario/baseline scenario/aggressive-timeline

# Merge accepted changes
git checkout main
git merge scenario/aggressive-timeline
```

### The Cultural Shift

We're not just building a tool. We're arguing that:

1. **Project schedules are code**
2. **Scheduling logic should be reviewed like code**
3. **Schedule changes should be versioned like code**
4. **Impossible schedules should fail like invalid code**

This is **compiler thinking applied to project management**.

---

## For Contributors: Making Design Decisions

When facing a design choice, ask:

1. **Does this make scheduling logic more explicit?**
   - Yes → Good
   - No → Reconsider

2. **Will this appear in git diffs when it changes?**
   - Yes → Good
   - Hidden in binary/UI → Bad

3. **Can I write a test that proves correctness?**
   - Yes → Good
   - Magic/heuristic → Suspect

4. **Does this respect CPM math or fight it?**
   - Respects → Good
   - Fights (manual overrides) → Bad

5. **Would a compiler do this?**
   - Yes (parse, validate, error) → Good
   - No (guess, infer, hide) → Bad

---

## Conclusion

utf8proj is **opinionated software** built on **principled design**:

- **Explicit > Implicit:** Dependencies are visible in text
- **Recompute > Cache:** Every schedule is computed fresh
- **Text > Binary:** .proj files are source code
- **Correct > Convenient:** CPM math is non-negotiable
- **Educate > Mimic:** We explain differences, not hide them

When we differ from MS Project, it's **by design, not by accident**. We're building the tool we wish existed: a **git-native, compiler-correct, developer-friendly** project scheduling engine.

**This document is authoritative.** When in doubt, return to these principles.

---

**See Also:**
- [RFC 001: Architecture](UTF8PROJ_RFC_MASTER.md)
- [RFC 002: CPM Correctness](docs/RFC002_CPM_CORRECTNESS.md)
- [Design Decisions](UTF8PROJ_DESIGN_DECISIONS.md)
- [MS Project Compatibility Guide](docs/MS_PROJECT_COMPATIBILITY.md)

**Questions or Discussion:**
- GitHub Discussions: [Philosophy & Design](https://github.com/alanbld/utf8proj/discussions/categories/philosophy)
- Chat: `#design-philosophy` in Discord
