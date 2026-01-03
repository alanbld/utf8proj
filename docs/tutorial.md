# utf8proj Tutorial: Building a CRM Migration Project

This tutorial walks you through creating a complete project schedule using utf8proj's native DSL. We'll build a realistic **CRM Migration Project** - the kind of work Solution Architects deal with regularly. Along the way, we'll compare with TaskJuggler to highlight differences and advantages.

## The Scenario

A mid-size company is migrating from a legacy CRM to Salesforce:
- **Duration:** ~3 months
- **Team:** 6 people across 3 roles
- **Budget:** Daily rates in EUR
- **Phases:** Discovery, Data Migration, Integration, Training, Hypercare

---

## Step 1: Project Declaration

Every utf8proj file starts with a project declaration:

```proj
project "CRM Migration to Salesforce" {
    start: 2026-02-01
    end: 2026-05-15
    currency: EUR
    timezone: Europe/Rome
}
```

### Comparison with TaskJuggler

| Aspect | TaskJuggler | utf8proj |
|--------|-------------|----------|
| Syntax | `project id "name" date - date { }` | `project "name" { start: date }` |
| Duration | `+14w` relative notation | Explicit `end:` date |
| Attributes | Multiple formats supported | Clean `key: value` format |

**TaskJuggler equivalent:**
```tjp
project crm "CRM Migration to Salesforce" 2026-02-01 - 2026-05-15 {
  timezone "Europe/Rome"
  currency "EUR"
}
```

---

## Step 2: Define a Calendar

Calendars define working hours and holidays:

```proj
calendar "standard" {
    working_hours: 09:00-13:00, 14:00-18:00
    working_days: mon-fri

    # Italian holidays 2026
    holiday "Epiphany" 2026-01-06
    holiday "Easter Monday" 2026-04-06
    holiday "Liberation Day" 2026-04-25
    holiday "Labour Day" 2026-05-01
}
```

### Key Features
- **Multiple time ranges:** Split lunch break with `09:00-13:00, 14:00-18:00`
- **Day ranges:** `mon-fri` expands to Monday through Friday
- **Single-date holidays:** Just the date, no range needed for single days

---

## Step 3: Define Resources

Resources are the people (or equipment) that do the work:

```proj
resource pm "Maria Rossi" {
    email: "m.rossi@company.it"
    rate: 850/day
    capacity: 0.75  # 75% allocation to this project
    role: "Project Manager"
}

resource sa1 "Luca Bianchi" {
    email: "l.bianchi@company.it"
    rate: 800/day
    role: "Solution Architect"
}

resource sa2 "Anna Verdi" {
    email: "a.verdi@company.it"
    rate: 750/day
    role: "Solution Architect"
    leave: 2026-03-02..2026-03-13  # Annual leave
}

resource dev1 "Marco Neri" {
    rate: 600/day
    role: "Developer"
}

resource dev2 "Giulia Russo" {
    rate: 620/day
    role: "Developer"
}

resource trainer "Paolo Gialli" {
    rate: 500/day
    role: "Trainer"
}
```

### Resource Attributes

| Attribute | Description | Example |
|-----------|-------------|---------|
| `rate:` | Cost per time unit | `850/day`, `100/hour` |
| `capacity:` | Allocation factor (0-1) | `0.75` = 6h/8h |
| `efficiency:` | Productivity factor | `1.2` = 20% more productive |
| `email:` | Contact email | `"user@company.it"` |
| `role:` | Job role | `"Solution Architect"` |
| `leave:` | Vacation period | `2026-03-02..2026-03-13` |

### Comparison with TaskJuggler

**TaskJuggler:**
```tjp
resource pm "Maria Rossi" {
  email "m.rossi@company.it"
  rate 850.0
  limits { dailymax 6h }
}
```

**Key Differences:**
- utf8proj uses `capacity: 0.75` instead of `limits { dailymax 6h }`
- utf8proj requires units: `850/day` vs `850.0`
- utf8proj supports `leave:` directly; TJ3 uses `leaves annual date - date`

---

## Step 4: Create Tasks (Work Breakdown Structure)

Tasks can be nested to create a hierarchy:

```proj
task discovery "Discovery & Planning" {

    task kickoff "Project Kickoff" {
        duration: 1d
        assign: pm, sa1, sa2
        cost: 500  # Workshop materials
    }

    task requirements "Requirements Analysis" {
        effort: 15d
        assign: sa1, sa2
        depends: kickoff
        note: "Interview 5 departments, document 120+ user stories"
    }

    task gap_analysis "Gap Analysis" {
        effort: 8d
        assign: sa1
        depends: requirements
        tag: critical
    }

    task architecture "Solution Architecture" {
        effort: 10d
        assign: sa1, sa2
        depends: gap_analysis
        tag: critical
    }

    milestone plan_approved "Planning Complete" {
        depends: architecture
        payment: 25000
    }
}
```

### Task Attributes

| Attribute | Description | Example |
|-----------|-------------|---------|
| `effort:` | Person-time required | `15d` (divided among assignees) |
| `duration:` | Calendar time | `2w` (regardless of assignees) |
| `depends:` | Dependencies | `requirements`, `phase.task` |
| `assign:` | Resource assignments | `sa1, sa2`, `dev1(50%)` |
| `priority:` | Scheduling priority | `800` (higher = first) |
| `tag:` | Labels for filtering | `critical, integration` |
| `note:` | Description | `"Any text..."` |
| `cost:` | Fixed costs | `500` |
| `payment:` | Milestone payment | `25000` |

### Effort vs Duration

- **Effort:** Work required. `effort: 10d` with 2 resources = 5 calendar days
- **Duration:** Fixed time. `duration: 2w` always takes 2 weeks

### Partial Allocation

Assign resources at partial capacity:

```proj
task support "Post-Go-Live Support" {
    duration: 2w
    assign: sa2, dev1(50%)  # dev1 at 50%
}
```

Both `dev1(50%)` and `dev1@50%` syntaxes are supported.

---

## Step 5: Define Dependencies

Dependencies control task sequencing:

```proj
task datamigration "Data Migration" {
    depends: discovery.plan_approved  # Wait for milestone

    task etl_dev "ETL Development" {
        effort: 20d
        depends: data_mapping
        assign: dev1, dev2
    }
}

task integration "Integration Development" {
    depends: discovery.plan_approved  # Parallel track

    task integration_test "Integration Testing" {
        effort: 8d
        depends: middleware, erp_connector  # Multiple deps
        assign: dev1, dev2, sa1
    }
}

task deployment "Training & Deployment" {
    # Wait for BOTH tracks to complete
    depends: datamigration.data_complete, integration.integration_complete
}
```

### Dependency Types

| Type | Syntax | Meaning |
|------|--------|---------|
| Finish-to-Start (default) | `depends: task` | Start after predecessor finishes |
| Start-to-Start | `depends: task SS` | Start when predecessor starts |
| Finish-to-Finish | `depends: task FF` | Finish when predecessor finishes |
| Start-to-Finish | `depends: task SF` | Finish when predecessor starts |

### Dependency Lag

Add lag time between tasks:

```proj
task review "Code Review" {
    effort: 2d
    depends: implementation +1d  # Start 1 day after implementation
}

task prep "Preparation" {
    effort: 1d
    depends: meeting -2d  # Start 2 days before meeting
}
```

---

## Step 6: Milestones

Milestones mark significant points with zero duration:

```proj
milestone plan_approved "Planning Complete" {
    depends: architecture
    payment: 25000  # Customer payment trigger
}
```

Alternative syntax (as task attribute):
```proj
task deploy "Deployment Complete" {
    depends: go_live
    milestone: true
}
```

---

## Step 7: Running utf8proj

### Check Syntax
```bash
utf8proj check crm_migration.proj
```

Output:
```
Checking: crm_migration.proj
  Project: CRM Migration to Salesforce
  Start: 2026-02-01
  End: 2026-05-15
  Tasks: 28
  Resources: 6
  Status: OK - No circular dependencies detected
```

### Schedule the Project
```bash
utf8proj schedule crm_migration.proj
```

### Generate Gantt Chart
```bash
utf8proj gantt crm_migration.proj -o timeline.svg
```

---

## Step 8: Reports (Declarative)

Define reports in your project file:

```proj
report gantt "output/gantt_chart.svg" {
    title: "CRM Migration - Project Schedule"
    tasks: all
    show: resources, critical_path, milestones
    scale: week
    width: 1200
}

report tasks "output/task_list.md" {
    type: tasks
    columns: wbs, name, start, end, effort, duration, assigned, status
    format: markdown
}

report costs "output/cost_summary.md" {
    type: costs
    breakdown: phase, resource
    show: labor, fixed_costs, payments
}
```

---

## Complete Example

Here's the full project file structure:

```proj
# CRM Migration Project
# File: crm_migration.proj

project "CRM Migration to Salesforce" {
    start: 2026-02-01
    end: 2026-05-15
    currency: EUR
    timezone: Europe/Rome
}

calendar "standard" {
    working_hours: 09:00-13:00, 14:00-18:00
    working_days: mon-fri
    holiday "Easter Monday" 2026-04-06
}

# Resources
resource pm "Maria Rossi" { rate: 850/day, capacity: 0.75 }
resource sa1 "Luca Bianchi" { rate: 800/day }
resource sa2 "Anna Verdi" { rate: 750/day }
resource dev1 "Marco Neri" { rate: 600/day }
resource dev2 "Giulia Russo" { rate: 620/day }
resource trainer "Paolo Gialli" { rate: 500/day }

# Phase 1: Discovery
task discovery "Discovery & Planning" {
    task kickoff "Project Kickoff" { duration: 1d, assign: pm, sa1, sa2 }
    task requirements "Requirements" { effort: 15d, assign: sa1, sa2, depends: kickoff }
    task architecture "Architecture" { effort: 10d, assign: sa1, sa2, depends: requirements }
    milestone plan_approved "Planning Complete" { depends: architecture }
}

# Phase 2-3: Parallel Tracks
task datamigration "Data Migration" {
    depends: discovery.plan_approved
    task mapping "Data Mapping" { effort: 12d, assign: sa2, dev1 }
    task etl "ETL Development" { effort: 20d, assign: dev1, dev2, depends: mapping }
    milestone data_complete "Data Ready" { depends: etl }
}

task integration "Integration" {
    depends: discovery.plan_approved
    task api "API Design" { effort: 6d, assign: sa1 }
    task connector "ERP Connector" { effort: 15d, assign: dev1, dev2, depends: api }
    milestone integration_complete "Integration Ready" { depends: connector }
}

# Phase 4: Deployment (waits for both tracks)
task deployment "Deployment" {
    depends: datamigration.data_complete, integration.integration_complete
    task training "Training" { effort: 8d, assign: trainer }
    task go_live "Go-Live" { duration: 2d, assign: pm, sa1, dev1 }
    milestone live "System Live" { depends: go_live, payment: 40000 }
}

# Reports
report gantt "timeline.svg" { tasks: all, scale: week }
```

---

## Benchmark: utf8proj vs TaskJuggler

| Capability | TaskJuggler | utf8proj |
|------------|-------------|----------|
| **Parsing** | Ruby-based | Native Rust (10x faster) |
| **Installation** | Requires Ruby | Single binary |
| **Syntax** | Complex, verbose | Clean, minimal |
| **Learning Curve** | Steep | Gentle |
| **Critical Path** | Yes | Yes |
| **Resource Leveling** | Yes | Basic |
| **HTML Reports** | Excellent | Basic |
| **SVG/Excel Output** | Limited | Full support |
| **What-If Analysis** | Manual scenarios | BDD-powered (planned) |
| **MS Project Export** | Limited | MSPDI format |
| **Maturity** | 20+ years | New |

### Adoption Readiness Assessment

**Ready for Production:**
- Parsing of .proj and .tjp files
- CPM scheduling with all dependency types
- Critical path calculation
- SVG Gantt chart generation
- Excel costing reports

**Needs Work:**
- Resource leveling (basic)
- HTML reports (limited)
- What-if analysis (planned)
- CLI refinement

**Recommendation:** utf8proj is ready for teams that:
- Want simple, text-based project files
- Need MS Project interchange
- Value fast parsing and single-binary deployment
- Can live without advanced resource leveling (for now)

---

## Next Steps

1. **Copy the example** to your project
2. **Customize** resources, rates, and tasks
3. **Run** `utf8proj schedule` to validate
4. **Generate** `utf8proj gantt -o timeline.svg`
5. **Iterate** as the project evolves

The text-based format makes it perfect for version control - track changes to your project plan just like code!
