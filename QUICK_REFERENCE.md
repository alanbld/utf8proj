# utf8proj Quick Reference Card

## DSL Syntax (.proj files)

### Project
```
project "Name" {
    start: 2025-02-01
    end: 2025-06-30
    currency: EUR
}
```

### Calendar
```
calendar "standard" {
    working_hours: 09:00-12:00, 13:00-17:00
    working_days: mon-fri
    holiday "Easter" 2025-04-18..2025-04-21
}
```

### Resource
```
resource dev "Developer" {
    rate: 850/day
    capacity: 1.0
    efficiency: 1.0
}
```

### Task
```
task impl "Implementation" {
    effort: 10d           # Person-time
    duration: 5d          # Calendar time (overrides effort)
    assign: dev, qa       # Resources
    depends: design       # Dependencies
    priority: 700         # Higher = first
    milestone: true       # Zero duration
}
```

### Nested Tasks (WBS)
```
task phase1 "Phase 1" {
    task design "Design" { effort: 5d }
    task impl "Implement" { effort: 10d, depends: design }
}
```

---

## Duration Units

| Unit | Meaning | Example |
|------|---------|---------|
| `h` | Hours | `8h` |
| `d` | Days (8h) | `5d` |
| `w` | Weeks (5d) | `2w` |
| `m` | Months (20d) | `3m` |

---

## Dependencies

| Syntax | Type | Meaning |
|--------|------|---------|
| `depends: a` | FS | Start after A finishes |
| `depends: a SS` | SS | Start when A starts |
| `depends: a FF` | FF | Finish when A finishes |
| `depends: a SF` | SF | Finish when A starts |
| `depends: a + 2d` | Lag | Wait 2 days after A |
| `depends: a - 3d` | Lead | Start 3 days before A ends |
| `depends: a.50%` | Partial | Start when A is 50% done |

---

## Constraints

```
must_start_on: 2025-03-01
must_finish_on: 2025-03-15
start_no_earlier_than: 2025-02-15
start_no_later_than: 2025-02-28
finish_no_earlier_than: 2025-03-10
finish_no_later_than: 2025-03-31
```

---

## CLI Commands

```bash
# Schedule
utf8proj schedule project.proj -o schedule.json

# Render
utf8proj render project.proj --format svg -o timeline.svg
utf8proj render project.proj --format mermaid -o timeline.mmd

# What-if
utf8proj what-if project.proj --constraint "resource.dev.capacity = 0.5"

# Validate
utf8proj validate project.proj

# MS Project
utf8proj import project.xml -o project.proj
utf8proj export project.proj --format mspdi -o project.xml
```

---

## Feature Flags

```bash
cargo build                          # heuristic only
cargo build --features bdd           # + BDD solver
cargo build --features sat           # + SAT solver
cargo build --features lua           # + Lua scripting
cargo build --features tjp           # + TaskJuggler compat
cargo build --features full          # all features
```

---

## Key Formulas

**Effort-based duration:**
```
duration = effort / (∑ resource_capacity × resource_units × efficiency)
```

**Slack/Float:**
```
slack = late_start - early_start = late_finish - early_finish
```

**Critical path:** Tasks where slack = 0

---

## Report Types

```
report gantt "timeline.svg" {
    tasks: all
    resources: show
    critical_path: highlight
}

report table "tasks.md" {
    type: tasks
    columns: id, name, start, finish, duration
}

report costs "budget.md" {
    type: costs
    columns: task, resource, effort, cost
}
```

---

## Scheduling Tiers

| Tier | Algorithm | Use Case | Feature |
|------|-----------|----------|---------|
| 1 | Heuristic | 95% of projects | default |
| 2 | BDD | What-if analysis | `bdd` |
| 3 | SAT | Complex constraints | `sat` |

---

## Priority Rules

| Rule | Strategy |
|------|----------|
| `LongestPath` | Critical path first |
| `MostSuccessors` | Most dependents first |
| `ShortestDuration` | Quick wins first |
| `HighestPriority` | User-defined |
| `EarliestDeadline` | Constrained first |

---

## utf8dok Integration

```asciidoc
[[fig-timeline]]
.Project Schedule
[schedule, report=gantt]
----
include::project.proj[]
----
```

---

## License

MIT OR Apache-2.0 (your choice)
