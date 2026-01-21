# utf8proj v1.0: Implementation Roadmap
**Version:** 1.0  
**Date:** 2026-01-04  
**Status:** Ready to Execute  
**Timeline:** 10 weeks to v1.0 MVP

---

## ROADMAP OVERVIEW

```
┌─────────────────────────────────────────────────────────────────┐
│                     UTF8PROJ V1.0 TIMELINE                       │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  PHASE 1: Progress-Aware CPM (Weeks 1-4)                        │
│  ███████████████████                                             │
│                                                                  │
│  PHASE 2: History System (Weeks 5-6)                            │
│                      ████████                                    │
│                                                                  │
│  PHASE 3: Export & Compatibility (Weeks 7-9)                    │
│                              ████████████                        │
│                                                                  │
│  PHASE 4: Polish & Release (Week 10)                            │
│                                          ████                    │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘

CONFIDENCE: 95% (design decisions finalized)
RISK: LOW (incremental development, test-driven)
DEPENDENCIES: Minimal (single binary, few external crates)
```

---

## PHASE 1: PROGRESS-AWARE CPM
**Duration:** 4 weeks  
**Deliverable:** Core scheduling with progress tracking  
**Dependencies:** Design Decisions A1-A5, B6-B8

### Week 1: Domain Model Extensions

**Goal:** Extend utf8proj-core with progress tracking fields

**Tasks:**

1. **Update Task struct** (2 days)
   ```rust
   // utf8proj-core/src/task.rs
   pub struct Task {
       // Existing fields...
       
       // NEW: Progress tracking
       pub percent_complete: Option<u8>,           // 0-100
       pub actual_start: Option<NaiveDate>,
       pub actual_finish: Option<NaiveDate>,
       pub status: Option<TaskStatus>,
       pub remaining_duration: Option<Duration>,   // Explicit override
       pub notes: Option<String>,
   }
   
   pub enum TaskStatus {
       NotStarted,
       InProgress,
       Complete,
       OnHold { reason: String },
       Blocked { reason: String, since: NaiveDate },
       AtRisk { reason: String },
       Cancelled { reason: String },
   }
   ```
   
   **Tests:**
   - Unit tests for field validation
   - Percent_complete bounds (0-100)
   - Actual dates ordering (start < finish)

2. **Update Project struct** (1 day)
   ```rust
   // utf8proj-core/src/project.rs
   pub struct Project {
       // Existing fields...
       
       // NEW: Progress tracking
       pub status_date: Option<NaiveDate>,         // "As of" date
       pub baseline: Option<BaselineRef>,          // Git tag or snapshot
   }
   
   pub enum BaselineRef {
       GitTag(String),       // "v1.0"
       GitCommit(String),    // "abc123"
       Snapshot(String),     // "2026-01-15-baseline"
   }
   ```

3. **Update ScheduledTask struct** (1 day)
   ```rust
   // utf8proj-core/src/schedule.rs
   pub struct ScheduledTask {
       // Existing: planned dates
       pub planned_start: NaiveDate,
       pub planned_finish: NaiveDate,
       
       // NEW: Actual/progress dates
       pub actual_start: Option<NaiveDate>,
       pub actual_finish: Option<NaiveDate>,
       pub percent_complete: u8,
       
       // NEW: Forecast dates (considering progress)
       pub forecast_start: NaiveDate,
       pub forecast_finish: NaiveDate,
       pub forecast_duration: Duration,
       
       // NEW: Variance
       pub start_variance: Duration,
       pub finish_variance: Duration,
   }
   ```

4. **Domain validation** (1 day)
   ```rust
   // utf8proj-core/src/validation.rs
   pub fn validate_task_progress(task: &Task) -> Result<()> {
       // Percent bounds
       if let Some(pct) = task.percent_complete {
           if pct > 100 { return Err(ValidationError::InvalidPercent); }
       }
       
       // Actual dates consistency
       if let (Some(start), Some(finish)) = (task.actual_start, task.actual_finish) {
           if start > finish {
               return Err(ValidationError::ActualStartAfterFinish);
           }
       }
       
       // 100% complete requires actual_finish
       if task.percent_complete == Some(100) && task.actual_finish.is_none() {
           warnings.push(Warning::CompleteTaskMissingActualFinish);
       }
       
       Ok(())
   }
   ```

**Deliverables:**
- ✅ Updated domain model with progress fields
- ✅ Validation logic for progress data
- ✅ Unit tests (>90% coverage)
- ✅ Documentation updates

---

### Week 2: Progress-Aware CPM Solver

**Goal:** Implement ProgressAwareCpmScheduler

**Tasks:**

1. **Effective duration calculation** (2 days)
   ```rust
   // utf8proj-solver/src/progress_cpm.rs
   impl Task {
       pub fn effective_remaining_duration(&self) -> Duration {
           // Decision A1: Linear with explicit override
           if let Some(explicit) = self.remaining_duration {
               return explicit; // Explicit override
           }
           
           if self.actual_finish.is_some() {
               return Duration::zero(); // Completed
           }
           
           let pct = self.percent_complete.unwrap_or(0) as f64 / 100.0;
           self.duration.mul_f64(1.0 - pct) // Linear
       }
   }
   ```
   
   **Tests:**
   - Linear calculation: 0%, 25%, 50%, 75%, 100%
   - Explicit override precedence
   - Edge cases: negative, >100%, actual_finish set

2. **Progress-aware forward pass** (2 days)
   ```rust
   fn forward_pass_with_progress(
       dag: &SchedulingDag,
       status_date: NaiveDate,
   ) -> HashMap<TaskId, EarlyDates> {
       let mut early_dates = HashMap::new();
       
       for task_id in dag.topological_order() {
           let task = dag.get_task(task_id);
           
           // Actual dates override calculation
           let start = if let Some(actual) = task.actual_start {
               actual // Decision A3: Reality wins
           } else {
               // Calculate from dependencies
               let pred_max = task.dependencies.iter()
                   .map(|dep| early_dates[dep].finish)
                   .max()
                   .unwrap_or(project_start);
               calendar.next_working_day(pred_max)
           };
           
           let remaining = task.effective_remaining_duration();
           let finish = if let Some(actual) = task.actual_finish {
               actual // Completed tasks use actual
           } else {
               calendar.add_working_days(start, remaining)
           };
           
           early_dates.insert(task_id, EarlyDates { start, finish });
       }
       
       early_dates
   }
   ```
   
   **Tests:**
   - Forward pass with 0% (baseline CPM)
   - Forward pass with mixed progress
   - Actual dates override dependencies
   - Warnings for date conflicts

3. **Container derivation** (1 day)
   ```rust
   // Decision A5: Weighted average by duration
   fn derive_container_dates(
       container: &Task,
       child_schedules: &HashMap<TaskId, ScheduledTask>,
   ) -> ScheduledTask {
       let mut total_duration = 0.0;
       let mut weighted_progress = 0.0;
       let mut earliest_start = NaiveDate::MAX;
       let mut latest_finish = NaiveDate::MIN;
       
       for child in &container.children {
           let child_sched = &child_schedules[&child.id];
           
           if let Some(dur) = child.duration_in_days() {
               total_duration += dur;
               weighted_progress += dur * child_sched.percent_complete as f64;
           }
           
           earliest_start = earliest_start.min(child_sched.forecast_start);
           latest_finish = latest_finish.max(child_sched.forecast_finish);
       }
       
       let derived_pct = if total_duration > 0.0 {
           (weighted_progress / total_duration).round() as u8
       } else {
           0
       };
       
       ScheduledTask {
           forecast_start: earliest_start,
           forecast_finish: latest_finish,
           percent_complete: container.percent_complete.unwrap_or(derived_pct),
           ...
       }
   }
   ```
   
   **Tests:**
   - Weighted progress calculation
   - Manual override with warning
   - Empty containers
   - Nested containers (multi-level)

4. **Variance calculation** (1 day)
   ```rust
   fn calculate_variances(
       scheduled: &mut ScheduledTask,
       baseline: &ScheduledTask,
   ) {
       scheduled.start_variance = 
           scheduled.forecast_start - baseline.planned_start;
       scheduled.finish_variance = 
           scheduled.forecast_finish - baseline.planned_finish;
   }
   ```

**Deliverables:**
- ✅ ProgressAwareCpmScheduler implementation
- ✅ Integration tests (10+ scenarios)
- ✅ Variance reporting
- ✅ Performance validated (1000+ tasks <1s)

---

### Week 3: CLI Commands & Reporting

**Goal:** User-facing commands for progress tracking

**Tasks:**

1. **`utf8proj status` command** (2 days)
   ```rust
   // utf8proj-cli/src/commands/status.rs
   pub fn run_status(project_path: &Path) -> Result<()> {
       let project = parse_project(project_path)?;
       let schedule = schedule_with_progress(&project)?;
       
       // Dashboard output
       println!("╔════════════════════════════════════════════╗");
       println!("║  Project: {}                  ║", project.name);
       println!("║  Status Date: {}              ║", 
                project.status_date.unwrap_or(today()));
       println!("╠════════════════════════════════════════════╣");
       println!("║  Progress:     {}%         ║", overall_progress);
       println!("║  Variance:     {} days      ║", finish_variance);
       println!("║  Critical Path: {} tasks    ║", critical_tasks.len());
       println!("╚════════════════════════════════════════════╝");
       
       // Phase breakdown
       render_phase_table(&schedule);
       
       // Issues & risks
       render_issues(&schedule.warnings);
       
       Ok(())
   }
   ```
   
   **Output example:** See Design Decisions document Part IV, CLI section
   
   **Tests:**
   - Dashboard rendering
   - Various progress states
   - Warning/issue highlighting

2. **`utf8proj progress` command** (2 days)
   ```bash
   # Single task update
   utf8proj progress --task=backend_api --complete=75
   
   # With status
   utf8proj progress --task=frontend --status=blocked --reason="API spec delayed"
   
   # Batch import
   utf8proj progress --import=weekly_update.csv
   ```
   
   **Implementation:**
   ```rust
   pub fn update_progress(
       project_path: &Path,
       task_id: &str,
       percent: u8,
       status: Option<TaskStatus>,
   ) -> Result<()> {
       let mut project = parse_project(project_path)?;
       
       let task = project.find_task_mut(task_id)?;
       task.percent_complete = Some(percent);
       
       if let Some(status) = status {
           task.status = Some(status);
       }
       
       // Auto-snapshot (Decision C11)
       if !options.no_snapshot {
           create_snapshot(&project, "Progress update")?;
       }
       
       save_project(&project, project_path)?;
       
       println!("✓ Updated {} to {}%", task_id, percent);
       Ok(())
   }
   ```

3. **`utf8proj forecast` command** (1 day)
   ```rust
   pub fn run_forecast(
       project_path: &Path,
       baseline_ref: Option<String>,
   ) -> Result<()> {
       let current = parse_project(project_path)?;
       let baseline = if let Some(ref_str) = baseline_ref {
           load_baseline(&current, ref_str)?
       } else {
           // Use first snapshot as baseline
           load_first_snapshot(&current)?
       };
       
       let current_schedule = schedule_with_progress(&current)?;
       let baseline_schedule = schedule(&baseline)?;
       
       // Comparison report
       render_forecast_report(&baseline_schedule, &current_schedule);
       
       Ok(())
   }
   ```

**Deliverables:**
- ✅ Three CLI commands (status, progress, forecast)
- ✅ Dashboard rendering
- ✅ CSV import/export
- ✅ User documentation

---

### Week 4: Testing & Documentation

**Goal:** Comprehensive testing and docs

**Tasks:**

1. **Integration tests** (2 days)
   ```rust
   // tests/integration/progress_scheduling.rs
   
   #[test]
   fn test_progress_delays_successors() {
       let proj = parse(r#"
           project "Test" { start: 2026-01-01, status_date: 2026-02-01 }
           task a "Task A" {
               duration: 10d
               complete: 50%
               actual_start: 2026-01-20  # Started late
           }
           task b "Task B" {
               duration: 5d
               depends: a
           }
       "#).unwrap();
       
       let schedule = schedule_with_progress(&proj).unwrap();
       
       // A: 50% done, 5d remaining from 2026-02-01
       assert_eq!(schedule["a"].forecast_finish, date(2026, 2, 8));
       
       // B: waits for A to finish
       assert!(schedule["b"].forecast_start >= date(2026, 2, 8));
   }
   
   #[test]
   fn test_container_weighted_progress() { /* ... */ }
   
   #[test]
   fn test_actual_dates_override_dependencies() { /* ... */ }
   ```

2. **Documentation** (2 days)
   - User guide: Progress tracking workflow
   - Tutorial: Update weekly status
   - Reference: DSL syntax for progress fields
   - Examples: Real project with progress

3. **Performance validation** (1 day)
   - Benchmark: 1000-task project with mixed progress
   - Target: <1s for scheduling
   - Profiling and optimization

**Deliverables:**
- ✅ 20+ integration tests
- ✅ Complete user documentation
- ✅ Performance benchmarks
- ✅ Phase 1 COMPLETE

---

## PHASE 2: HISTORY SYSTEM
**Duration:** 2 weeks  
**Deliverable:** YAML sidecar history with playback  
**Dependencies:** Design Decisions C9-C11, E15-E17

### Week 5: YAML Sidecar Implementation

**Goal:** Snapshot creation and storage

**Tasks:**

1. **History provider trait** (1 day)
   ```rust
   // utf8proj-history/src/traits.rs
   pub trait HistoryProvider {
       fn get_snapshots(&self, project_path: &Path) -> Result<Vec<Snapshot>>;
       fn save_snapshot(&self, snapshot: Snapshot) -> Result<()>;
       fn get_snapshot(&self, id: &str) -> Result<Snapshot>;
   }
   
   pub struct Snapshot {
       pub id: String,
       pub timestamp: DateTime<Utc>,
       pub snapshot_type: SnapshotType,
       pub author: Option<String>,
       pub message: String,
       pub content: ProjectContent,
   }
   
   pub enum SnapshotType {
       Full,
       Diff { parent: String },
   }
   
   pub enum ProjectContent {
       Full(String),              // Full .proj content
       Diff(Vec<DiffOperation>),  // Diff from parent
   }
   ```

2. **YAML sidecar provider** (2 days)
   ```rust
   // utf8proj-history/src/sidecar.rs
   pub struct SidecarHistoryProvider {
       history_path: PathBuf,  // project.proj.history
   }
   
   impl HistoryProvider for SidecarHistoryProvider {
       fn save_snapshot(&self, snapshot: Snapshot) -> Result<()> {
           let mut history = self.load_history()?;
           
           // Decision C10: Hybrid structure
           if should_create_full_snapshot(&history) {
               history.snapshots.push(snapshot);
           } else {
               let diff = create_diff(
                   &history.latest_full_snapshot()?,
                   &snapshot,
               )?;
               history.snapshots.push(Snapshot {
                   snapshot_type: SnapshotType::Diff { parent: ... },
                   content: ProjectContent::Diff(diff),
                   ..snapshot
               });
           }
           
           self.save_history(&history)?;
           Ok(())
       }
   }
   ```

3. **Auto-snapshot integration** (1 day)
   ```rust
   // utf8proj-cli/src/snapshot.rs
   pub fn auto_snapshot_if_enabled(
       project: &Project,
       message: &str,
   ) -> Result<()> {
       if !project.auto_snapshot {
           return Ok(()); // Disabled
       }
       
       let provider = SidecarHistoryProvider::new(&project.path);
       let snapshot = Snapshot::new(project, message);
       provider.save_snapshot(snapshot)?;
       
       Ok(())
   }
   ```

4. **Snapshot CLI command** (1 day)
   ```bash
   utf8proj snapshot --message="Before sprint 3"
   utf8proj history                    # List snapshots
   utf8proj show @snapshot-5           # Show specific version
   ```

**Deliverables:**
- ✅ YAML sidecar history provider
- ✅ Auto-snapshot on state changes
- ✅ Manual snapshot CLI command
- ✅ Tests for storage/retrieval

---

### Week 6: Playback Engine

**Goal:** History animation and diff

**Tasks:**

1. **Semantic diff algorithm** (2 days)
   ```rust
   // utf8proj-history/src/diff.rs (Decision E15)
   pub fn schedule_diff(
       old: &Schedule,
       new: &Schedule,
   ) -> ScheduleDiff {
       let mut changes = Vec::new();
       
       // Match tasks by ID
       for (id, new_task) in &new.tasks {
           if let Some(old_task) = old.tasks.get(id) {
               if old_task != new_task {
                   changes.push(Change::Modified {
                       id: id.clone(),
                       fields: diff_fields(old_task, new_task),
                   });
               }
           } else {
               // Check rename (similarity matching)
               if let Some(similar) = find_similar(new_task, &old.tasks) {
                   changes.push(Change::Renamed {
                       from: similar.id,
                       to: id.clone(),
                   });
               } else {
                   changes.push(Change::Added { task: new_task.clone() });
               }
           }
       }
       
       // Deletions
       for id in old.tasks.keys() {
           if !new.tasks.contains_key(id) {
               changes.push(Change::Deleted { id: id.clone() });
           }
       }
       
       ScheduleDiff { changes }
   }
   ```

2. **Impact metrics** (1 day)
   ```rust
   // Decision E16
   pub struct ScheduleImpact {
       pub project_duration_delta: Duration,
       pub critical_path_changed: bool,
       pub tasks_added: usize,
       pub tasks_modified: usize,
       pub significance: u8,  // 0-100
   }
   
   impl ScheduleImpact {
       fn calculate_significance(&self) -> u8 {
           let mut score = 0;
           if self.project_duration_delta.abs() > Duration::days(5) {
               score += 40;
           }
           if self.critical_path_changed { score += 30; }
           if self.tasks_added > 5 { score += 20; }
           score.min(100)
       }
   }
   ```

3. **HTML playback renderer** (2 days)
   ```rust
   // Decision E17: HTML+JS primary
   pub fn render_html_playback(
       snapshots: &[Snapshot],
       output: &Path,
   ) -> Result<()> {
       let mut frames = Vec::new();
       
       for (i, snapshot) in snapshots.iter().enumerate() {
           let project = parse(&snapshot.content)?;
           let schedule = schedule_with_progress(&project)?;
           let gantt_svg = render_gantt(&schedule)?;
           
           let impact = if i > 0 {
               Some(calculate_impact(&snapshots[i-1], snapshot)?)
           } else {
               None
           };
           
           frames.push(PlaybackFrame {
               index: i,
               timestamp: snapshot.timestamp,
               gantt_svg,
               impact,
               changes: snapshot.changes.clone(),
           });
       }
       
       let html = handlebars.render("playback.html", &frames)?;
       std::fs::write(output, html)?;
       
       Ok(())
   }
   ```

**Deliverables:**
- ✅ Semantic diff algorithm
- ✅ Impact metrics calculation
- ✅ HTML playback renderer
- ✅ `utf8proj playback` command
- ✅ Phase 2 COMPLETE

---

## PHASE 3: EXPORT & COMPATIBILITY
**Duration:** 3 weeks  
**Deliverable:** Excel export, resource leveling, TJP import  
**Dependencies:** Design Decisions F18-F20, G21-G23, I27-I29

### Week 7: Excel Export

**Goal:** Professional Excel workbooks

**Tasks:**

1. **rust_xlsxwriter integration** (1 day)
   ```toml
   # utf8proj-render/Cargo.toml
   [dependencies]
   rust_xlsxwriter = "1.1"
   ```

2. **4-sheet workbook generation** (3 days)
   ```rust
   // utf8proj-render/src/excel.rs (Decision F19)
   pub fn export_excel(
       schedule: &Schedule,
       output: &Path,
   ) -> Result<()> {
       let mut workbook = Workbook::new();
       
       // Sheet 1: Dashboard
       let dashboard = workbook.add_worksheet("Dashboard")?;
       render_dashboard_sheet(&mut dashboard, schedule)?;
       
       // Sheet 2: Task List
       let tasks = workbook.add_worksheet("Task List")?;
       render_task_list_sheet(&mut tasks, schedule)?;
       
       // Sheet 3: Timeline
       let timeline = workbook.add_worksheet("Timeline")?;
       render_timeline_sheet(&mut timeline, schedule)?;
       
       // Sheet 4: Resources
       let resources = workbook.add_worksheet("Resources")?;
       render_resource_sheet(&mut resources, schedule)?;
       
       workbook.save(output)?;
       Ok(())
   }
   
   fn render_task_list_sheet(
       sheet: &mut Worksheet,
       schedule: &Schedule,
   ) -> Result<()> {
       // Headers
       sheet.write_row(0, 0, &["ID", "Name", "Planned Start", 
                                 "Planned Finish", "Forecast Finish",
                                 "% Complete", "Variance"])?;
       
       // Data with formulas
       for (row, task) in schedule.tasks.values().enumerate() {
           sheet.write(row+1, 0, &task.id)?;
           sheet.write(row+1, 1, &task.name)?;
           sheet.write_date(row+1, 2, task.planned_start)?;
           sheet.write_date(row+1, 3, task.planned_finish)?;
           sheet.write_date(row+1, 4, task.forecast_finish)?;
           sheet.write_number(row+1, 5, task.percent_complete as f64)?;
           
           // Formula: Variance = Forecast - Planned
           let formula = format!("=E{}-D{}", row+2, row+2);
           sheet.write_formula(row+1, 6, &formula)?;
       }
       
       // Conditional formatting
       let format_late = Format::new()
           .set_bg_color(FormatColor::Red);
       sheet.conditional_format(1, 6, schedule.tasks.len(), 6,
           &ConditionalFormat::new()
               .set_type(ConditionalFormatType::Cell)
               .set_criteria(ConditionalFormatCriteria::GreaterThan)
               .set_value(0)
               .set_format(&format_late))?;
       
       Ok(())
   }
   ```

3. **Chart generation** (1 day - Decision F20)
   ```rust
   fn create_gantt_chart(
       workbook: &mut Workbook,
       schedule: &Schedule,
   ) -> Result<()> {
       let chart = Chart::new(ChartType::Bar);
       chart.add_series(ChartSeries::new()
           .set_categories("'Task List'!$B$2:$B$100")
           .set_values("'Task List'!$C$2:$D$100"));
       chart.set_x_axis(ChartAxis::new().set_date_axis(true));
       chart.set_title("Project Timeline");
       
       workbook.insert_chart("Dashboard", 10, &chart)?;
       Ok(())
   }
   ```

**Deliverables:**
- ✅ Excel export with 4 sheets
- ✅ Formulas and conditional formatting
- ✅ Embedded charts
- ✅ `utf8proj export --format=excel`

---

### Week 8: Resource Leveling

**Goal:** Automatic resource conflict resolution

**Tasks:**

1. **Critical path priority heuristic** (2 days - Decision G21)
   ```rust
   // utf8proj-solver/src/leveling.rs
   pub fn level_resources(
       schedule: &mut Schedule,
       mode: LevelingMode,
   ) -> LevelingResult {
       if mode == LevelingMode::Warn {
           return detect_conflicts(schedule);
       }
       
       let mut iterations = 0;
       while has_conflicts(schedule) && iterations < 100 {
           for day in schedule.date_range() {
               for resource in &schedule.resources {
                   let tasks = tasks_on_day(schedule, resource, day);
                   
                   if tasks.len() > resource.capacity {
                       // Sort: critical first, then priority
                       tasks.sort_by(|a, b| {
                           match (a.is_critical, b.is_critical) {
                               (true, false) => Ordering::Less,
                               (false, true) => Ordering::Greater,
                               _ => b.priority.cmp(&a.priority),
                           }
                       });
                       
                       // Delay excess tasks
                       for task in tasks.iter().skip(resource.capacity) {
                           if can_delay(schedule, task) {
                               delay_by_one_day(schedule, task);
                               break;
                           }
                       }
                   }
               }
           }
           iterations += 1;
       }
       
       if iterations == 100 {
           LevelingResult::Partial { 
               warnings: generate_warnings(schedule) 
           }
       } else {
           LevelingResult::Success
       }
   }
   ```

2. **Leveling modes** (1 day - Decision G22)
   ```rust
   pub enum LevelingMode {
       Warn,   // Detect only
       Auto,   // Automatic leveling
       Error,  // Fail on conflicts
   }
   
   impl LevelingMode {
       fn from_cli(args: &Args) -> Self {
           args.leveling.unwrap_or(LevelingMode::Warn)
       }
       
       fn from_project(project: &Project) -> Self {
           project.leveling_mode.unwrap_or(LevelingMode::Warn)
       }
   }
   ```

3. **Constraint validation** (1 day - Decision G23)
   ```rust
   fn validate_constraints_after_leveling(
       schedule: &Schedule,
   ) -> Result<()> {
       for task in &schedule.tasks {
           for constraint in &task.constraints {
               if !satisfies(task, constraint) {
                   return Err(ScheduleError::ConstraintViolated {
                       task: task.id.clone(),
                       constraint: constraint.clone(),
                       suggestions: vec![
                           "Add resources",
                           "Relax constraint",
                           "Reduce scope",
                       ],
                   });
               }
           }
       }
       Ok(())
   }
   ```

**Deliverables:**
- ✅ Resource leveling algorithm
- ✅ Three leveling modes
- ✅ Constraint enforcement
- ✅ `utf8proj schedule --leveling=auto`

---

### Week 9: TaskJuggler Import

**Goal:** TJP file compatibility

**Tasks:**

1. **TJP parser (Tier 1 features)** (3 days - Decision I27, I28)
   ```rust
   // utf8proj-parser/src/tjp/parser.rs
   pub fn parse_tjp(content: &str) -> Result<(Project, Vec<Warning>)> {
       let mut warnings = Vec::new();
       let mut project = Project::default();
       
       let statements = pest_parse(content)?;
       
       for stmt in statements {
           match stmt {
               TjpStmt::Project(p) => project.name = p.name,
               TjpStmt::Task(t) => {
                   let task = convert_task(t)?;
                   project.tasks.push(task);
               }
               TjpStmt::Resource(r) => {
                   let resource = convert_resource(r)?;
                   project.resources.push(resource);
               }
               
               // Tier 2: Warn
               TjpStmt::Shift(_) => {
                   warnings.push(Warning::UnsupportedFeature {
                       feature: "shift",
                       alternative: "Use calendar instead",
                   });
               }
               
               // Tier 3: Ignore
               TjpStmt::Journal(_) => {
                   warnings.push(Warning::IgnoredFeature {
                       feature: "journal",
                   });
               }
           }
       }
       
       Ok((project, warnings))
   }
   ```

2. **TJP to .proj conversion** (1 day - Decision I29)
   ```rust
   pub fn convert_tjp_to_proj(tjp_path: &Path) -> Result<String> {
       let content = std::fs::read_to_string(tjp_path)?;
       let (project, warnings) = parse_tjp(&content)?;
       
       // Serialize to .proj format
       let proj_content = serialize_project(&project)?;
       
       // Add warnings as comments
       let mut output = String::new();
       if !warnings.is_empty() {
           output.push_str("# Conversion warnings:\n");
           for warning in warnings {
               output.push_str(&format!("# - {}\n", warning));
           }
           output.push_str("\n");
       }
       output.push_str(&proj_content);
       
       Ok(output)
   }
   ```

3. **Import CLI command** (1 day)
   ```bash
   utf8proj import legacy.tjp --output=modern.proj
   utf8proj import legacy.tjp --validate
   ```

**Deliverables:**
- ✅ TJP parser (Tier 1 features)
- ✅ Conversion tool
- ✅ Validation report
- ✅ `utf8proj import` command
- ✅ Phase 3 COMPLETE

---

## PHASE 4: POLISH & RELEASE
**Duration:** 1 week  
**Deliverable:** v1.0 production release

### Week 10: Final Polish

**Tasks:**

1. **Documentation** (2 days)
   - User guide (comprehensive)
   - Tutorial: End-to-end workflow
   - API reference
   - Migration guide (TJP → utf8proj)

2. **Example projects** (1 day)
   - Simple project (minimal.proj)
   - Software release (complex.proj)
   - Construction project (construction.proj)
   - Progress tracking showcase (progress.proj)
   - TJP compatibility demo (legacy.tjp + modern.proj)

3. **Performance optimization** (1 day)
   - Profile 1000-task project
   - Optimize hot paths
   - Benchmark suite
   - CI/CD integration

4. **Release preparation** (1 day)
   - Version tagging
   - Changelog
   - Release notes
   - Binary distribution (GitHub releases)

**Deliverables:**
- ✅ Complete documentation
- ✅ 5+ example projects
- ✅ Performance benchmarks
- ✅ v1.0 RELEASE

---

## SUCCESS METRICS

### Technical Metrics
- [ ] Test coverage >85%
- [ ] All integration tests passing
- [ ] Performance: <1s for 1000-task schedule
- [ ] Binary size: <10MB
- [ ] Zero-dependency deployment

### Feature Completeness
- [ ] Progress-aware CPM working
- [ ] History system functional
- [ ] Excel export generates correct workbooks
- [ ] Resource leveling resolves conflicts
- [ ] TJP import covers 70% of files

### Documentation
- [ ] User guide complete
- [ ] 5+ tutorials
- [ ] API reference
- [ ] 5+ example projects

### Community
- [ ] GitHub repository public
- [ ] README with quick start
- [ ] Contributing guidelines
- [ ] Issue templates

---

## RISK MITIGATION

### High-Risk Areas
1. **Excel chart rendering** - Mitigation: Manual testing, user feedback
2. **TJP semantic differences** - Mitigation: Clear documentation, validation reports
3. **Performance at scale** - Mitigation: Benchmarking, optimization pass

### Contingency Plans
- If Excel charts don't work well → Defer to v1.1, generate static images
- If TJP import too complex → Focus on Tier 1 only, mark Tier 2 experimental
- If performance issues → Add caching, lazy evaluation, or async processing

---

## POST-v1.0 ROADMAP

### v1.1 (Months 2-3)
- BDD/SAT integration (if user demand)
- Enhanced TJP compatibility (Tier 2 features)
- WASM build for browser
- Playground baseline support (save/load/compare in browser)
- VS Code extension

### v1.2 (Months 4-6)
- EVM (Earned Value Management)
- Advanced resource management
- GitHub Action
- Plugin system

### v2.0 (Months 7-12)
- Web UI (browser-based)
- Real-time collaboration
- AI-powered forecasting
- Cloud synchronization

---

## DEVELOPMENT WORKFLOW

### Daily Standup (5 min)
- What did I accomplish yesterday?
- What will I work on today?
- Any blockers?

### Weekly Review (30 min)
- Demo completed features
- Update roadmap
- Adjust priorities

### Test-Driven Development
```bash
# 1. Write failing test
touch tests/integration/test_progress.rs

# 2. Implement minimum code to pass
vim crates/utf8proj-solver/src/progress_cpm.rs

# 3. Refactor
# 4. Document
# 5. Commit

git add .
git commit -m "feat: implement progress-aware CPM"
```

### Git Workflow
```bash
# Feature branches
git checkout -b feat/progress-cpm
# Work...
git commit -am "feat: add effective duration calculation"
git push origin feat/progress-cpm
# PR review
git checkout main
git merge feat/progress-cpm
```

---

## RESOURCE ALLOCATION

### Development Time
- **Core development:** 160 hours (4 weeks × 40 hrs)
- **Testing:** 40 hours
- **Documentation:** 20 hours
- **Refinement:** 20 hours
- **Total:** 240 hours (6 person-weeks)

### External Dependencies
- Zero (all Rust, no services)
- Optional: GitHub for hosting
- Optional: crates.io for distribution

---

## CONCLUSION

This roadmap provides a clear, achievable path to utf8proj v1.0. With 95% design confidence and a test-driven approach, the project is ready for implementation.

**Next Steps:**
1. Set up development environment
2. Begin Week 1: Domain Model Extensions
3. Daily commits with tests
4. Weekly progress reviews
5. Ship v1.0 in 10 weeks

**Let's build it! 🚀**

---

**Document Version:** 1.0  
**Last Updated:** 2026-01-04  
**Status:** Ready to Execute
