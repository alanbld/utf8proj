# TaskJuggler vs utf8proj: Comprehensive Comparison

## Executive Summary

This document provides a detailed comparison between TaskJuggler (TJ3) and utf8proj using the CRM Migration project as a benchmark. The comparison covers parsing, scheduling, output rendering, and identifies areas where utf8proj needs improvement before production use.

**Key Finding**: utf8proj successfully parses and generates valid output, but the CPM scheduler has significant bugs when handling hierarchical projects with cross-container dependencies.

---

## 1. Test Project Overview

Both tools were tested with a CRM Migration project containing:

| Aspect | Value |
|--------|-------|
| Project Duration | 20 weeks (Feb-Jun 2026) |
| Resources | 6 team members |
| Tasks | 28 tasks across 5 phases |
| Dependencies | Mix of FS, parallel paths |
| Milestones | 5 phase completion milestones |

### Project Structure

```
CRM Migration Project
├── Discovery & Planning (5 tasks)
├── Data Migration (4 tasks, depends on Discovery)
├── Integration Development (5 tasks, depends on Discovery)
├── Training & Deployment (6 tasks, depends on Data + Integration)
└── Hypercare Support (3 tasks, depends on Deployment)
```

---

## 2. Parsing Comparison

### 2.1 TaskJuggler (.tjp) Format

**File**: `examples/crm_migration.tjp` (423 lines)

TaskJuggler uses a rich DSL with features like:
```tjp
project crm "CRM Migration to Salesforce" 2026-02-01 +20w {
  timezone "Europe/Rome"
  currency "EUR"
  scenario plan "Baseline Plan" {
    scenario aggressive "Aggressive Timeline"
  }
}

task crm "CRM Migration Project" {
  task discovery "Discovery & Planning" {
    task kickoff "Project Kickoff" {
      duration 1d
      allocate pm
      start ${projectstart}
    }
    task requirements "Requirements Analysis" {
      effort 15d
      depends !kickoff
      allocate sa1, sa2
    }
  }
}
```

**TJ3 Parsing Result**: SUCCESS with warnings
```
Warning: Task crm.discovery.kickoff has resource allocation requested,
but did not get any resources assigned. Either use 'effort' or higher 'priority'.
```

### 2.2 utf8proj Native (.proj) Format

**File**: `examples/crm_simple.proj` (204 lines)

utf8proj uses a cleaner, YAML-inspired syntax:
```
project "CRM Migration to Salesforce" {
    start: 2026-02-01
    end: 2026-06-15
    currency: EUR
}

resource pm "Maria Rossi" {
    rate: 850/day
    capacity: 0.75
}

task discovery "Discovery & Planning" {
    task kickoff "Project Kickoff" {
        duration: 1d
        assign: pm, sa1, sa2
    }
    task requirements "Requirements Analysis" {
        effort: 15d
        assign: sa1, sa2
        depends: kickoff
    }
}
```

**utf8proj Parsing Result**: SUCCESS
- All 28 tasks parsed correctly
- All 6 resources parsed correctly
- Calendar and holidays parsed correctly

---

## 3. Scheduling Comparison

### 3.1 TaskJuggler Schedule Output

TaskJuggler correctly schedules tasks respecting dependencies:

| Phase | Start | End | Duration |
|-------|-------|-----|----------|
| Discovery | 2026-02-02 | 2026-03-06 | 24d |
| Data Migration | 2026-03-09 | 2026-04-24 | 35d |
| Integration | 2026-03-09 | 2026-04-17 | 30d |
| Deployment | 2026-04-27 | 2026-05-15 | 15d |
| Hypercare | 2026-05-18 | 2026-06-05 | 15d |
| **Total** | 2026-02-02 | 2026-06-05 | ~18 weeks |

Key scheduling behaviors:
- Tasks respect FS (Finish-to-Start) dependencies
- Parallel phases (Data Migration, Integration) run concurrently
- Deployment waits for BOTH parallel phases to complete
- Resource leveling prevents over-allocation

### 3.2 utf8proj Schedule Output

**CRITICAL BUG**: utf8proj scheduler produces incorrect results with negative slack values:

```
Task                 Start        Finish       Duration    Slack Critical
----------------------------------------------------------------------------
discovery            2026-02-01   2026-03-02       22d    -23d
deployment           2026-02-01   2026-02-24       18d    -19d   << WRONG!
datamigration        2026-02-01   2026-03-06       26d    -27d   << WRONG!
hypercare            2026-02-01   2026-02-16       12d     14d   << WRONG!
```

**Problems Identified**:

1. **Dependency Violation**: `deployment` starts 2026-02-01 instead of waiting for `datamigration` and `integration` to complete
2. **Negative Slack**: Values like -23d, -27d indicate impossible schedules
3. **Container Date Derivation**: Parent containers get incorrect date ranges
4. **Cross-Container Dependencies**: Dependencies between nested containers not handled correctly

### 3.3 Root Cause Analysis

The utf8proj CPM solver has bugs in:

1. **Topological Sort**: Not correctly ordering tasks across container boundaries
2. **Late Start/Finish Calculation**: Backward pass produces negative values
3. **Dependency Resolution**: Cross-container dependency paths not fully resolved

**Evidence from code** (`crates/utf8proj-solver/src/lib.rs:696`):
```rust
Duration::days(node.slack)  // Overflows when slack is very large negative
```

---

## 4. Excel Output Comparison

### 4.1 TaskJuggler Output

TaskJuggler generates HTML reports with interactive Gantt charts:
- `Overview.html` - Project summary with costs
- `Gantt.html` - Detailed schedule with resource assignments
- `Resources.html` - Resource utilization matrix
- `Milestones.html` - Milestone tracking

**Strengths**:
- Rich interactive HTML with CSS styling
- Multiple scenarios comparison
- Cost tracking with account breakdown
- Export to multiple formats (HTML, CSV, iCal)

### 4.2 utf8proj Excel Output

utf8proj generates XLSX files with three sheets:

**Sheet 1: Resources**
```
Profile ID | Profile       | Rate EUR/d | Days (pd) | Cost EUR
-----------|---------------|------------|-----------|----------
pm         | Maria Rossi   | 850        | 1         | =C2*D2
sa1        | Luca Bianchi  | 800        | 15        | =C3*D3
dev1       | Marco Neri    | 600        | 19        | =C4*D4
trainer    | Paolo Gialli  | 500        | 4         | =C5*D5
```

**Sheet 2: Schedule (Gantt-style)**
```
Task ID     | Activity             | Profile | Depends On | Type | Lag | Effort | Start | End | W1 | W2 | W3...
------------|----------------------|---------|------------|------|-----|--------|-------|-----|----|----|----
kickoff     | Project Kickoff      | pm      |            |      |     | 0      | 1     | 1   | X  |    |
requirements| Requirements Analysis| sa1     | kickoff    | FS   | 0   | 9      | 2     | 3   |    | X  | X
design      | Solution Design      | sa1     | requirements| FS  | 0   | 6      | 4     | 5   |    |    |    | X
```

**Sheet 3: Summary**
```
PROJECT SUMMARY
---------------
Project Name: CRM Migration to Salesforce
Start Date:   2026-02-01
End Date:     2026-03-18
Duration:     34 days
Total Tasks:  8
Critical:     8

COST SUMMARY
------------
Total Effort: 39 pd
Total Cost:   26,000 EUR
```

**Excel Features**:
- Formulas for automatic calculation (start/end weeks, costs)
- Conditional formatting for Gantt bars
- VLOOKUP-based dependency resolution
- Week-by-week hour distribution
- Frozen panes for navigation

**Limitations**:
- Only supports simplified linear projects (workaround for scheduler bug)
- No resource leveling visualization
- No cost account breakdown
- No scenario comparison

---

## 5. Feature Comparison Matrix

| Feature | TaskJuggler | utf8proj | Notes |
|---------|-------------|----------|-------|
| **Parsing** |
| Hierarchical tasks | Yes | Yes | Both support nested containers |
| Duration/Effort | Yes | Yes | Both support both modes |
| Dependency types | FS,SS,FF,SF | FS,SS,FF,SF | Same coverage |
| Dependency lag | Yes | Yes | `+2d`, `-1d` syntax |
| Resources | Yes | Yes | Rate, capacity, calendar |
| Calendars | Yes | Yes | Working hours, holidays |
| Scenarios | Yes | No | TJ3 supports what-if analysis |
| Cost accounts | Yes | No | TJ3 has hierarchical cost tracking |
| **Scheduling** |
| CPM algorithm | Yes | Partial | utf8proj has bugs |
| Resource leveling | Yes | No | TJ3 auto-levels resources |
| Critical path | Yes | Partial | utf8proj marks but miscalculates |
| Progress tracking | Yes | No | TJ3 has `complete` attribute |
| **Output** |
| HTML reports | Yes | No | TJ3 generates rich HTML |
| Excel export | No | Yes | utf8proj has native XLSX |
| PlantUML | No | Yes | utf8proj generates diagrams |
| MermaidJS | No | Yes | utf8proj generates diagrams |
| JSON export | No | Yes | utf8proj has structured output |
| TJP export | No | Yes | utf8proj can round-trip |
| **Other** |
| Macros | Yes | No | TJ3 has powerful macro system |
| Custom reports | Yes | No | TJ3 has report DSL |
| Web UI | No | Planned | utf8proj targets WASM |

---

## 6. Code Quality Assessment

### 6.1 Test Coverage

| Module | Coverage | Status |
|--------|----------|--------|
| utf8proj-solver | 96.3% | Excellent |
| utf8proj-render | 91.0% | Excellent |
| utf8proj-parser/native | 91.2% | Excellent |
| utf8proj-parser/tjp | 78.8% | Good |
| utf8proj-core | 74.6% | Needs improvement |
| utf8proj-cli | 0% | Not tested |

**Total**: 77.26% (196 tests passing)

### 6.2 Known Issues

1. **Scheduler Overflow** (`Duration::days` overflow)
   - Location: `crates/utf8proj-solver/src/lib.rs:696`
   - Trigger: Hierarchical projects with cross-container dependencies
   - Severity: Critical

2. **Container Date Derivation**
   - Containers don't correctly derive dates from children
   - Affects project summary calculations

3. **Resource Leveling**
   - Not implemented - resources can be over-allocated

---

## 7. Recommendations

### 7.1 For Production Use

**TaskJuggler** is recommended for:
- Enterprise project management
- Complex resource leveling requirements
- Scenario planning and what-if analysis
- Cost account tracking

**utf8proj** is recommended for:
- Simple linear project schedules
- Excel output generation
- Integration with modern toolchains (WASM, JSON)
- Lightweight CLI usage

### 7.2 utf8proj Improvement Priorities

1. **Critical**: Fix CPM scheduler for hierarchical projects
2. **High**: Implement resource leveling
3. **Medium**: Add scenario support
4. **Medium**: Improve CLI test coverage
5. **Low**: Add cost account support

### 7.3 Migration Path

If migrating from TaskJuggler to utf8proj:
1. Start with simple, linear projects
2. Avoid complex cross-container dependencies
3. Validate schedules against TJ3 output
4. Use Excel export for stakeholder communication

---

## 8. Appendix: Sample Outputs

### A. utf8proj Excel Structure (extracted)

```xml
<!-- Sheet 2: Schedule -->
<sheetData>
  <row r="1">Task ID | Activity | Profile | Depends On | Type | Lag | Effort | Start | End | W1-W20</row>
  <row r="2">kickoff | Project Kickoff | pm | | | | 0 | 1 | 1 | [formula-based bars]</row>
  <row r="3">requirements | Requirements Analysis | sa1 | kickoff | FS | 0 | 9 | 2 | 3 | ...</row>
  ...
</sheetData>
```

### B. TaskJuggler HTML Report (structure)

```
Overview.html
├── Project Summary (dates, duration)
├── WBS Table (tasks with costs)
├── Gantt Chart (SVG-based)
└── Footer (generated timestamp)
```

---

## Document Metadata

- **Generated**: 2026-01-03
- **utf8proj Version**: 0.1.0
- **TaskJuggler Version**: 3.8.4
- **Test Project**: CRM Migration to Salesforce
