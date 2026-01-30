# utf8proj v1.0: Implementation Roadmap

**Version:** 2.0 (Updated)
**Date:** 2026-01-30
**Status:** Phase 1-3 Complete, Phase 4 Partially Complete

---

## CURRENT STATUS

```
┌─────────────────────────────────────────────────────────────────┐
│                     UTF8PROJ V1.0 STATUS                        │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  PHASE 1: Progress-Aware CPM                    ████████ DONE   │
│  - Domain model extensions                      ✓                │
│  - Progress-aware CPM solver                    ✓                │
│  - CLI schedule command with --as-of            ✓                │
│  - Tests and documentation                      ✓                │
│                                                                  │
│  PHASE 2: History System                        ██████░░ 75%    │
│  - Baseline management (RFC-0013)               ✓                │
│  - Save/list/compare baselines                  ✓                │
│  - Playback engine                              ○ NOT STARTED   │
│  - Auto-snapshots                               ○ NOT STARTED   │
│                                                                  │
│  PHASE 3: Export & Compatibility                ████████ DONE   │
│  - Excel export (RFC-0018)                      ✓                │
│  - Resource leveling (RFC-0003, RFC-0014)       ✓                │
│  - TaskJuggler import                           ✓                │
│                                                                  │
│  PHASE 4: Polish & Release                      ██████░░ 75%    │
│  - Documentation                                ✓                │
│  - Example projects                             ✓                │
│  - Performance benchmarks                       ✓                │
│  - WASM playground                              ✓                │
│  - Status dashboard command                     ○ NOT STARTED   │
│  - Progress update command                      ○ NOT STARTED   │
│                                                                  │
│  CURRENT VERSION: 0.15.1                                        │
│  TEST COVERAGE: ~978 tests, ~86% overall                        │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

---

## COMPLETED WORK

### Phase 1: Progress-Aware CPM (100% Complete)

| Task | Status | Implementation |
|------|--------|----------------|
| Task progress fields | Done | `complete`, `actual_start`, `actual_finish`, `remaining` |
| Project status_date | Done | CLI `--as-of`, project attribute |
| Effective duration calculation | Done | Linear: `duration * (1 - complete%)` with explicit override |
| Progress-aware forward pass | Done | Respects actual dates, derives from predecessors |
| Container derivation | Done | Weighted average by child duration |
| Variance calculation | Done | `utf8proj compare --baseline` |
| P005/P006 diagnostics | Done | Progress conflict warnings |

**Key Files:**
- `crates/utf8proj-solver/src/lib.rs` - Progress-aware scheduling
- `crates/utf8proj-solver/tests/complete_task_scheduling.rs` - Edge case tests

### Phase 2: History System (75% Complete)

| Task | Status | Notes |
|------|--------|-------|
| Baseline management | Done | RFC-0013 implemented |
| `baseline save` | Done | Saves schedule snapshot to YAML sidecar |
| `baseline list` | Done | Lists all baselines |
| `baseline show` | Done | Shows baseline details |
| `baseline remove` | Done | Removes baseline |
| `compare --baseline` | Done | Compares current vs baseline |
| Playback engine | **Not Started** | HTML animation of schedule evolution |
| Auto-snapshots | **Not Started** | Automatic snapshot on changes |
| `utf8proj history` | **Not Started** | List all snapshots |

**Key Files:**
- `crates/utf8proj-core/src/baseline.rs` - Baseline types
- `crates/utf8proj-parser/src/baseline.rs` - YAML sidecar I/O

### Phase 3: Export & Compatibility (100% Complete)

| Task | Status | Notes |
|------|--------|-------|
| Excel export | Done | RFC-0018: 4-sheet workbook |
| Progress tracking columns | Done | `--progress columns/visual/full` |
| Conditional formatting | Done | Color-coded timeline |
| Resource leveling | Done | RFC-0003, RFC-0014 |
| Hybrid BDD leveling | Done | 4-5x faster for large projects |
| Optimal solver | Done | Branch-and-bound for small clusters |
| TaskJuggler import | Done | Tier 1 features |
| MS Project companion | Done | `tools/mpp_to_proj/` |

**Key Files:**
- `crates/utf8proj-render/src/excel.rs` - Excel renderer
- `crates/utf8proj-solver/src/leveling.rs` - Resource leveling
- `crates/utf8proj-solver/src/bdd.rs` - BDD conflict analysis

### Phase 4: Polish & Release (75% Complete)

| Task | Status | Notes |
|------|--------|-------|
| User documentation | Done | CLAUDE.md, MS_PROJECT_COMPARISON.md |
| Example projects | Done | `examples/` directory |
| Performance benchmarks | Done | `utf8proj benchmark`, PSPLIB |
| WASM playground | Done | https://alanbld.github.io/utf8proj/ |
| LSP | Done | Diagnostics, hover, go-to-definition |
| `utf8proj status` | **Not Started** | Dashboard view |
| `utf8proj progress` | **Not Started** | CLI progress update |
| Extended TaskStatus | **Not Started** | blocked, on_hold, at_risk |

---

## REMAINING WORK

### Priority 1: Status Dashboard (Recommended Next)

**Command:** `utf8proj status project.proj`

**Why:** Highest value for PMs with lowest implementation cost. All data is already computed during scheduling; this just needs a formatted summary view.

**Implementation:**
```rust
// crates/utf8proj-cli/src/main.rs
Commands::Status {
    file,
    format,  // text, json
    baseline,  // optional: show variance vs baseline
}

// Output structure
struct ProjectStatus {
    project_name: String,
    status_date: NaiveDate,
    overall_progress: u8,           // 0-100
    total_tasks: usize,
    completed_tasks: usize,
    in_progress_tasks: usize,
    critical_path_length: Duration,
    critical_path_remaining: Duration,
    schedule_variance: Option<Duration>,  // vs baseline
    late_tasks: Vec<TaskSummary>,
    upcoming_tasks: Vec<TaskSummary>,     // starting in next 7 days
    recently_completed: Vec<TaskSummary>, // completed in last 7 days
}
```

**Estimated Scope:** ~200 lines of Rust

### Priority 2: Progress Update Command

**Command:** `utf8proj progress project.proj --task=X --complete=75`

**Why:** Reduces manual editing errors, enables CI/CD integration.

**Implementation:**
```rust
Commands::Progress {
    file,
    task: String,
    complete: Option<u8>,
    actual_start: Option<NaiveDate>,
    actual_finish: Option<NaiveDate>,
    import: Option<PathBuf>,  // CSV batch import
}
```

**Challenges:**
- Requires modifying .proj files programmatically
- Need to preserve comments and formatting
- Consider using tree-sitter for precise edits

**Estimated Scope:** ~400 lines of Rust

### Priority 3: Extended TaskStatus

**Current syntax:**
```proj
task api "API" { complete: 75% }
```

**Proposed syntax:**
```proj
task api "API" {
    status: blocked
    status_reason: "Waiting for security review"
    status_since: 2026-01-25
}
```

**Domain model:**
```rust
pub enum TaskStatus {
    NotStarted,
    InProgress,
    Complete,
    Blocked { reason: String, since: Option<NaiveDate> },
    OnHold { reason: String },
    AtRisk { reason: String },
    Cancelled { reason: String },
}
```

**Estimated Scope:** ~300 lines (parser + core + CLI)

### Priority 4: Playback Engine (Deferred)

**Command:** `utf8proj playback project.proj -o evolution.html`

**Why:** Stakeholder communication, retrospectives.

**Dependencies:** Requires multiple baselines saved over time.

**Implementation:**
1. Load all baselines chronologically
2. Schedule each baseline state
3. Generate HTML with timeline slider
4. Highlight changes between frames
5. Calculate impact metrics (duration change, critical path shifts)

**Estimated Scope:** ~800 lines (HTML template + diff algorithm)

---

## SUCCESS METRICS (Updated)

### Technical Metrics
- [x] Test coverage >85% (currently ~86%)
- [x] All integration tests passing (978 tests)
- [x] Performance: <1s for 1000-task schedule
- [x] Binary size: <10MB
- [x] Zero-dependency deployment

### Feature Completeness
- [x] Progress-aware CPM working
- [x] Baseline system functional (save/list/compare)
- [x] Excel export generates correct workbooks
- [x] Resource leveling resolves conflicts
- [x] TJP import covers Tier 1 features
- [ ] Status dashboard command
- [ ] Progress update command
- [ ] Playback animation

### Documentation
- [x] User guide complete (CLAUDE.md)
- [x] API reference (LSP hover docs)
- [x] Example projects (examples/)
- [x] MS Project comparison guide

---

## POST-V1.0 ROADMAP

### v1.1 (Future)
- `utf8proj status` command
- `utf8proj progress` command
- Extended TaskStatus enum
- VS Code extension

### v1.2 (Future)
- EVM (Earned Value Management): CPI, SPI, EAC, VAC
- GitHub Action for CI/CD
- Playback engine

### v2.0 (Future)
- Web UI with real-time collaboration
- Plugin system
- AI-powered forecasting

---

## RECOMMENDATION

**For Project Managers: Implement `utf8proj status` first.**

This provides:
1. **Instant value**: Daily "how are we doing?" in 2 seconds
2. **Low risk**: Read-only command, no file modifications
3. **Quick delivery**: ~200 lines, builds on existing schedule data
4. **Foundation**: Natural stepping stone to progress command

The existing `utf8proj schedule` output is detailed but verbose. PMs need a summary view for standups and status meetings. The status command fills this gap.

---

**Document Version:** 2.0
**Last Updated:** 2026-01-30
