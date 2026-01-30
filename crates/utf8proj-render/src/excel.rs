//! Excel costing report renderer
//!
//! Generates XLSX files with multiple sheets for corporate project costing:
//! - Profiles and Costs: Resource rates and totals
//! - Summary: Activities × Profiles matrix with effort allocation
//! - Schedule: Week-based Gantt chart with hour distribution formulas
//!
//! ## Dependency Support
//!
//! When `show_dependencies` is enabled, the Schedule sheet includes:
//! - Task ID column for VLOOKUP references
//! - Depends On column showing predecessor task ID
//! - Dependency Type (FS/SS/FF/SF) with dropdown validation
//! - Lag in days (positive = delay, negative = lead)
//! - **Formula-driven Start/End weeks** that cascade when predecessors change
//!
//! ## Dependency Types
//!
//! - **FS (Finish-to-Start)**: Successor starts after predecessor finishes (most common)
//! - **SS (Start-to-Start)**: Successor starts when predecessor starts
//! - **FF (Finish-to-Finish)**: Successor finishes when predecessor finishes
//! - **SF (Start-to-Finish)**: Successor finishes when predecessor starts
//!
//! ## What-If Analysis Support
//!
//! The Schedule sheet uses Excel **conditional formatting** for dynamic Gantt visualization:
//! - Week cells show blue background when value > 0 (task has hours in that week)
//! - Week cells show alternating white/light-blue when value = 0 (no hours)
//! - This enables **live what-if analysis**: change effort/dependencies and colors update
//!
//! Unlike static formatting (baked in at render time), conditional formatting allows
//! the Gantt bar visualization to respond dynamically when users modify:
//! - Task effort (pd column)
//! - Dependencies (Depends On column)
//! - Lag values
//!
//! ## Example Output Structure
//!
//! ```text
//! Sheet: Profiles and Costs
//! | Profile ID | Profile              | Rate €/d | Days | Cost €   |
//! |------------|----------------------|----------|------|----------|
//! | PM         | Project Manager      | 500      | 10   | 5000     |
//! | DEV        | Developer            | 400      | 50   | 20000    |
//!
//! Sheet: Schedule (with dependencies)
//! | ID     | Activity | Profile | Depends | Type | Lag | pd | Start | End | W1 | W2 | ...
//! |--------|----------|---------|---------|------|-----|----| ------|-----|----|----| ...
//! | design | Design   | DEV     |         |      |     | 5  | 1     | =F  | 20 | 20 | ...
//! | impl   | Implement| DEV     | design  | FS   | 0   | 10 | =F    | =F  | 0  | 0  | ...
//! ```
//!
//! The Start Week formula for dependent tasks:
//! ```text
//! =IF(D2="", 1, IF(E2="FS", VLOOKUP(D2,TaskTable,9,0)+1+F2, ...))
//! ```
//! This creates a **live schedule** - change a task's effort and all successors update!

use chrono::{Datelike, NaiveDate, Weekday};
use rust_decimal::prelude::ToPrimitive;
use rust_xlsxwriter::{
    ConditionalFormatFormula, Format, FormatAlign, FormatBorder, Workbook, Worksheet,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use utf8proj_core::{
    status::ProjectStatus, Calendar, Diagnostic, DiagnosticCode, Project, RenderError, Renderer,
    Schedule, ScheduledTask, Severity,
};

/// Schedule time granularity for Excel export
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ScheduleGranularity {
    /// One column per calendar day (shows weekends/holidays)
    Daily,
    /// One column per week (current behavior)
    #[default]
    Weekly,
}

/// Progress visualization mode for Excel export (RFC-0018)
///
/// Controls how task progress information is displayed in the Schedule sheet.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProgressMode {
    /// No progress columns or visualization (default, backwards compatible)
    #[default]
    None,
    /// Add progress columns only (Complete %, Remaining, Actual Start, Actual End)
    Columns,
    /// Add progress columns + visual progress bars in timeline cells
    Visual,
    /// Full progress view with status date marker and variance analysis
    Full,
}

/// Progress data for a task (RFC-0018)
///
/// Used to pass progress information to row writing functions.
#[derive(Clone, Debug, Default)]
pub struct ProgressData {
    /// Completion percentage (0-100)
    pub percent_complete: u8,
    /// Remaining duration in days
    pub remaining_days: f64,
    /// Actual start date (if task has started)
    pub actual_start: Option<NaiveDate>,
    /// Actual finish date (if task is complete)
    pub actual_finish: Option<NaiveDate>,
    /// Scheduled task start date (for visual progress calculation)
    pub task_start: Option<NaiveDate>,
    /// Scheduled task finish date (for visual progress calculation)
    pub task_finish: Option<NaiveDate>,
    /// Schedule variance in days (positive = ahead, negative = behind)
    pub variance_days: i32,
    /// Task status for Full mode
    pub status: TaskStatus,
}

/// Task status for Full progress mode (RFC-0018)
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TaskStatus {
    /// Task is 100% complete
    Complete,
    /// Task is in progress (0% < complete < 100%)
    InProgress,
    /// Task has not started yet (0% complete, start > status_date)
    #[default]
    NotStarted,
    /// Task is behind schedule (incomplete work past status_date)
    Behind,
    /// Task should have started but hasn't (0% complete, start <= status_date)
    Overdue,
}

impl TaskStatus {
    /// Get the icon representation for this status
    pub fn icon(&self) -> &'static str {
        match self {
            Self::Complete => "✓",
            Self::InProgress => "●",
            Self::NotStarted => "○",
            Self::Behind => "⚠",
            Self::Overdue => "⚠",
        }
    }
}

/// Configuration for Excel export (RFC-0009)
///
/// This struct is designed for JSON serialization to support WASM/browser usage.
/// All fields have sensible defaults, so `ExcelConfig::default()` works well.
///
/// # Example
///
/// ```rust,ignore
/// use utf8proj_render::ExcelConfig;
///
/// let config = ExcelConfig {
///     scale: "daily".to_string(),
///     currency: "USD".to_string(),
///     auto_fit: true,
///     ..Default::default()
/// };
///
/// let renderer = config.to_renderer();
/// ```
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExcelConfig {
    /// Scale: "daily" or "weekly" (default: "weekly")
    #[serde(default = "default_scale")]
    pub scale: String,

    /// Currency symbol (default: "EUR")
    #[serde(default = "default_currency")]
    pub currency: String,

    /// Auto-fit timeframe to project duration (default: true)
    #[serde(default = "default_true")]
    pub auto_fit: bool,

    /// Number of weeks (only used if auto_fit=false and scale=weekly)
    #[serde(default)]
    pub weeks: Option<u32>,

    /// Number of days (only used if auto_fit=false and scale=daily)
    #[serde(default)]
    pub days: Option<u32>,

    /// Working hours per day (default: 8.0)
    #[serde(default = "default_hours_per_day")]
    pub hours_per_day: f64,

    /// Include executive summary sheet (default: true)
    #[serde(default = "default_true")]
    pub include_summary: bool,

    /// Show dependency columns for formula-driven scheduling (default: true)
    #[serde(default = "default_true")]
    pub show_dependencies: bool,

    /// Progress visualization mode (RFC-0018): "none", "columns", "visual", "full"
    #[serde(default)]
    pub progress_mode: ProgressMode,

    /// Status date for progress calculations (optional, defaults to project.status_date)
    #[serde(default)]
    pub status_date: Option<NaiveDate>,

    /// Include Status Dashboard sheet (RFC-0019)
    #[serde(default)]
    pub include_status_dashboard: bool,
}

fn default_scale() -> String {
    "weekly".to_string()
}

fn default_currency() -> String {
    "EUR".to_string()
}

fn default_true() -> bool {
    true
}

fn default_hours_per_day() -> f64 {
    8.0
}

impl Default for ExcelConfig {
    fn default() -> Self {
        Self {
            scale: default_scale(),
            currency: default_currency(),
            auto_fit: true,
            weeks: None,
            days: None,
            hours_per_day: default_hours_per_day(),
            include_summary: true,
            show_dependencies: true,
            progress_mode: ProgressMode::None,
            status_date: None,
            include_status_dashboard: false,
        }
    }
}

impl ExcelConfig {
    /// Convert this configuration into an ExcelRenderer
    pub fn to_renderer(&self) -> ExcelRenderer {
        let mut renderer = ExcelRenderer::new()
            .currency(&self.currency)
            .hours_per_day(self.hours_per_day);

        // Apply scale
        if self.scale == "daily" {
            renderer = renderer.daily();
            if !self.auto_fit {
                if let Some(days) = self.days {
                    renderer = renderer.days(days);
                }
            }
        } else if !self.auto_fit {
            if let Some(weeks) = self.weeks {
                renderer = renderer.weeks(weeks);
            }
        }

        // Apply auto-fit setting
        if !self.auto_fit {
            renderer = renderer.no_auto_fit();
        }

        // Apply other settings
        if !self.include_summary {
            renderer = renderer.no_summary();
        }

        if !self.show_dependencies {
            renderer = renderer.no_dependencies();
        }

        // Apply progress settings (RFC-0018)
        renderer = renderer.with_progress_mode(self.progress_mode);
        if let Some(date) = self.status_date {
            renderer = renderer.with_status_date(date);
        }

        // Apply status dashboard setting (RFC-0019)
        if self.include_status_dashboard {
            renderer = renderer.with_status_dashboard();
        }

        renderer
    }
}

/// Excel costing report renderer
#[derive(Clone, Debug)]
pub struct ExcelRenderer {
    /// Currency symbol
    pub currency: String,
    /// Number of weeks to show in schedule
    pub schedule_weeks: u32,
    /// Working hours per day
    pub hours_per_day: f64,
    /// Working hours per week (for duration calculations)
    pub hours_per_week: f64,
    /// Whether to include Executive Summary sheet
    pub include_summary: bool,
    /// Whether to include formulas (vs static values)
    pub use_formulas: bool,
    /// Project start date for schedule calculations
    pub project_start: Option<NaiveDate>,
    /// Default rate for resources without explicit rate
    pub default_rate: f64,
    /// Whether to show dependency columns and use formula-driven scheduling
    pub show_dependencies: bool,
    /// Whether to include Calendar Analysis sheet
    pub include_calendar_analysis: bool,
    /// Whether to include Diagnostics sheet
    pub include_diagnostics: bool,
    /// Diagnostics to include in the export (if include_diagnostics is true)
    diagnostics: Vec<Diagnostic>,
    /// Schedule time granularity (daily or weekly)
    pub granularity: ScheduleGranularity,
    /// Number of days to show in daily schedule (default: 60)
    pub schedule_days: u32,
    /// Calendar for working days/holidays (used in daily mode)
    calendar: Option<Calendar>,
    /// Auto-fit timeframe to project duration (default: true)
    pub auto_fit: bool,
    /// Progress visualization mode (RFC-0018)
    pub progress_mode: ProgressMode,
    /// Status date for progress calculations
    pub status_date: Option<NaiveDate>,
    /// Whether to include Status Dashboard sheet (RFC-0019)
    pub include_status_dashboard: bool,
}

impl Default for ExcelRenderer {
    fn default() -> Self {
        Self {
            currency: "€".into(),
            schedule_weeks: 18,
            hours_per_day: 8.0,
            hours_per_week: 40.0,
            include_summary: true,
            use_formulas: true,
            project_start: None,
            default_rate: 400.0,
            show_dependencies: true, // Enable by default for live scheduling
            include_calendar_analysis: false,
            include_diagnostics: false,
            diagnostics: Vec::new(),
            granularity: ScheduleGranularity::Weekly,
            schedule_days: 60,
            calendar: None,
            auto_fit: true, // Auto-fit to project duration by default
            progress_mode: ProgressMode::None,
            status_date: None,
            include_status_dashboard: false,
        }
    }
}

impl ExcelRenderer {
    pub fn new() -> Self {
        Self::default()
    }

    /// Set currency symbol
    pub fn currency(mut self, currency: impl Into<String>) -> Self {
        self.currency = currency.into();
        self
    }

    /// Set number of weeks in schedule
    pub fn weeks(mut self, weeks: u32) -> Self {
        self.schedule_weeks = weeks;
        self
    }

    /// Set working hours per day
    pub fn hours_per_day(mut self, hours: f64) -> Self {
        self.hours_per_day = hours;
        self
    }

    /// Disable Executive Summary sheet
    pub fn no_summary(mut self) -> Self {
        self.include_summary = false;
        self
    }

    /// Use static values instead of formulas
    pub fn static_values(mut self) -> Self {
        self.use_formulas = false;
        self
    }

    /// Set default rate for resources
    pub fn default_rate(mut self, rate: f64) -> Self {
        self.default_rate = rate;
        self
    }

    /// Disable dependency columns (simpler output, no formula-driven scheduling)
    pub fn no_dependencies(mut self) -> Self {
        self.show_dependencies = false;
        self
    }

    /// Set working hours per week (default 40)
    pub fn hours_per_week(mut self, hours: f64) -> Self {
        self.hours_per_week = hours;
        self
    }

    /// Include Calendar Analysis sheet showing weekend/holiday impact per task
    pub fn with_calendar_analysis(mut self) -> Self {
        self.include_calendar_analysis = true;
        self
    }

    /// Include Diagnostics sheet with all project diagnostics
    pub fn with_diagnostics(mut self, diagnostics: Vec<Diagnostic>) -> Self {
        self.include_diagnostics = true;
        self.diagnostics = diagnostics;
        self
    }

    /// Use daily granularity (one column per calendar day)
    ///
    /// Daily mode shows weekends and holidays with distinct styling,
    /// making it ideal for short-term operational planning (1-3 months).
    pub fn daily(mut self) -> Self {
        self.granularity = ScheduleGranularity::Daily;
        self
    }

    /// Set number of days to show in daily schedule (default: 60)
    ///
    /// Only used when `daily()` is enabled.
    pub fn days(mut self, days: u32) -> Self {
        self.schedule_days = days;
        self
    }

    /// Set calendar for working days and holidays
    ///
    /// Used in daily mode to determine weekend/holiday styling.
    /// If not set, defaults to Mon-Fri working days with no holidays.
    pub fn with_calendar(mut self, calendar: Calendar) -> Self {
        self.calendar = Some(calendar);
        self
    }

    /// Disable auto-fit and use explicit weeks/days values
    ///
    /// By default, the renderer auto-fits the timeframe to cover the project
    /// duration plus a buffer. Call this to use the fixed `schedule_weeks`
    /// or `schedule_days` values instead.
    pub fn no_auto_fit(mut self) -> Self {
        self.auto_fit = false;
        self
    }

    /// Set progress visualization mode (RFC-0018)
    ///
    /// Controls how task progress information is displayed:
    /// - `None`: No progress columns (default, backwards compatible)
    /// - `Columns`: Add Complete%, Remaining, Actual Start/End columns
    /// - `Visual`: Add progress bars in timeline cells
    /// - `Full`: Add status icons, variance, and status date marker
    pub fn with_progress_mode(mut self, mode: ProgressMode) -> Self {
        self.progress_mode = mode;
        self
    }

    /// Set status date for progress calculations (RFC-0018)
    ///
    /// Used to determine which tasks are behind schedule (past status date
    /// but not complete). If not set, defaults to project.status_date or today.
    pub fn with_status_date(mut self, date: NaiveDate) -> Self {
        self.status_date = Some(date);
        self
    }

    /// Include Status Dashboard sheet (RFC-0019)
    ///
    /// Adds a sheet with a project status summary including:
    /// - Overall progress with visual progress bar
    /// - Schedule metrics (start, baseline, forecast, variance)
    /// - Earned value metrics (PV, EV, SPI)
    /// - Task breakdown by status
    pub fn with_status_dashboard(mut self) -> Self {
        self.include_status_dashboard = true;
        self
    }

    /// Calculate auto-fit weeks to cover project duration
    ///
    /// Returns the number of weeks needed to cover the full project
    /// plus a 10% buffer (minimum 1 week).
    ///
    /// Uses the actual max task finish date (not schedule.project_end) to ensure
    /// all tasks are covered, even if there's a discrepancy.
    pub fn calculate_auto_fit_weeks(&self, schedule: &Schedule, project_start: NaiveDate) -> u32 {
        // Find the actual max finish date from all tasks
        let max_finish = schedule
            .tasks
            .values()
            .map(|t| t.finish)
            .max()
            .unwrap_or(project_start);

        // Use the later of schedule.project_end and max task finish
        let effective_end = max_finish.max(schedule.project_end);

        let days = (effective_end - project_start).num_days().max(0) as u32;
        let weeks = (days + 6) / 7; // Round up to complete weeks
        let buffer = (weeks / 10).max(1); // 10% buffer, minimum 1 week
        (weeks + buffer).max(1) // Ensure at least 1 week
    }

    /// Calculate auto-fit days to cover project duration
    ///
    /// Returns the number of days needed to cover the full project
    /// plus a 10% buffer (minimum 5 days).
    ///
    /// Uses the actual max task finish date (not schedule.project_end) to ensure
    /// all tasks are covered, even if there's a discrepancy.
    pub fn calculate_auto_fit_days(&self, schedule: &Schedule, project_start: NaiveDate) -> u32 {
        // Find the actual max finish date from all tasks
        let max_finish = schedule
            .tasks
            .values()
            .map(|t| t.finish)
            .max()
            .unwrap_or(project_start);

        // Use the later of schedule.project_end and max task finish
        let effective_end = max_finish.max(schedule.project_end);

        let days = (effective_end - project_start).num_days().max(0) as u32;
        let buffer = (days / 10).max(5); // 10% buffer, minimum 5 days
        (days + buffer).max(5) // Ensure at least 5 days
    }

    /// Get effective weeks (auto-fit or manual)
    pub fn get_effective_weeks(&self, schedule: &Schedule, project_start: NaiveDate) -> u32 {
        if self.auto_fit {
            self.calculate_auto_fit_weeks(schedule, project_start)
        } else {
            self.schedule_weeks
        }
    }

    /// Get effective days (auto-fit or manual)
    pub fn get_effective_days(&self, schedule: &Schedule, project_start: NaiveDate) -> u32 {
        if self.auto_fit {
            self.calculate_auto_fit_days(schedule, project_start)
        } else {
            self.schedule_days
        }
    }

    /// Get the number of extra columns added by progress mode (RFC-0018)
    ///
    /// Returns 4 for Columns/Visual modes (Complete, Remaining, Act.Start, Act.End)
    /// Returns 6 for Full mode (adds Status, Variance)
    /// Returns 0 for None mode
    fn progress_column_count(&self) -> u16 {
        match self.progress_mode {
            ProgressMode::None => 0,
            ProgressMode::Columns | ProgressMode::Visual => 4,
            ProgressMode::Full => 6,
        }
    }

    /// Generate Excel workbook bytes
    pub fn render_to_bytes(
        &self,
        project: &Project,
        schedule: &Schedule,
    ) -> Result<Vec<u8>, RenderError> {
        let mut workbook = Workbook::new();

        // Create formats
        let formats = self.create_formats();

        // Build resource rate lookup (convert Money to f64)
        let resource_rates: HashMap<String, f64> = project
            .resources
            .iter()
            .map(|r| {
                let rate = r
                    .rate
                    .as_ref()
                    .and_then(|m| m.amount.to_f64())
                    .unwrap_or(self.default_rate);
                (r.id.clone(), rate)
            })
            .collect();

        // Get project start
        let project_start = self.project_start.unwrap_or(project.start);

        // Add sheets
        self.add_profiles_sheet(&mut workbook, project, schedule, &formats, &resource_rates)?;
        self.add_schedule_sheet(&mut workbook, project, schedule, &formats, project_start)?;

        if self.include_summary {
            self.add_executive_summary(
                &mut workbook,
                project,
                schedule,
                &formats,
                &resource_rates,
            )?;
        }

        // Add Calendar Analysis sheet if enabled
        if self.include_calendar_analysis {
            self.add_calendar_analysis_sheet(&mut workbook, project, schedule, &formats)?;
        }

        // Add Diagnostics sheet if enabled
        if self.include_diagnostics {
            self.add_diagnostics_sheet(&mut workbook, project, &formats)?;
        }

        // Add Status Dashboard sheet if enabled (RFC-0019)
        if self.include_status_dashboard {
            self.add_status_dashboard_sheet(&mut workbook, project, schedule, &formats)?;
        }

        // Save to buffer
        let buffer = workbook
            .save_to_buffer()
            .map_err(|e| RenderError::Format(format!("Failed to create Excel: {e}")))?;

        Ok(buffer)
    }

    /// Create reusable formats
    fn create_formats(&self) -> ExcelFormats {
        let header = Format::new()
            .set_bold()
            .set_align(FormatAlign::Center)
            .set_background_color(0x4472C4)
            .set_font_color(0xFFFFFF)
            .set_border(FormatBorder::Thin);

        let currency = Format::new()
            .set_num_format(&format!("#,##0.00 \"{}\"", self.currency))
            .set_border(FormatBorder::Thin);

        let number = Format::new()
            .set_num_format("#,##0.0")
            .set_border(FormatBorder::Thin);

        let integer = Format::new()
            .set_num_format("#,##0")
            .set_border(FormatBorder::Thin);

        let text = Format::new().set_border(FormatBorder::Thin);

        let week_header = Format::new()
            .set_bold()
            .set_align(FormatAlign::Center)
            .set_rotation(90)
            .set_background_color(0x4472C4)
            .set_font_color(0xFFFFFF)
            .set_border(FormatBorder::Thin);

        let total_row = Format::new()
            .set_bold()
            .set_background_color(0xE2EFDA)
            .set_border(FormatBorder::Thin);

        let total_currency = Format::new()
            .set_bold()
            .set_num_format(&format!("#,##0.00 \"{}\"", self.currency))
            .set_background_color(0xE2EFDA)
            .set_border(FormatBorder::Thin);

        // Alternating row colors for Schedule sheet (light blue/white banding per task)
        let row_even_text = Format::new().set_border(FormatBorder::Thin);

        let row_even_number = Format::new()
            .set_num_format("#,##0.0")
            .set_border(FormatBorder::Thin);

        let row_odd_text = Format::new()
            .set_background_color(0xDDEBF7) // Light blue
            .set_border(FormatBorder::Thin);

        let row_odd_number = Format::new()
            .set_num_format("#,##0.0")
            .set_background_color(0xDDEBF7) // Light blue
            .set_border(FormatBorder::Thin);

        // Milestone row formats (light gold tint for semantic distinction)
        let milestone_text = Format::new()
            .set_background_color(0xFFF2CC) // Light gold
            .set_border(FormatBorder::Thin);

        let milestone_number = Format::new()
            .set_num_format("#,##0.0")
            .set_background_color(0xFFF2CC) // Light gold
            .set_border(FormatBorder::Thin);

        // Milestone week cell (gold diamond marker)
        let milestone_week = Format::new()
            .set_align(FormatAlign::Center)
            .set_background_color(0xFFE699) // Slightly darker gold for emphasis
            .set_border(FormatBorder::Thin);

        // Container task formats (bold to distinguish phases from leaf tasks)
        let container_even_text = Format::new().set_bold().set_border(FormatBorder::Thin);

        let container_odd_text = Format::new()
            .set_bold()
            .set_background_color(0xDDEBF7) // Light blue
            .set_border(FormatBorder::Thin);

        // Week column empty formats for alternating row banding
        // Filled color (blue) is applied via conditional formatting for what-if analysis
        let gantt_even_empty = Format::new()
            .set_align(FormatAlign::Center)
            .set_border(FormatBorder::Thin); // White (matches even row)

        let gantt_odd_empty = Format::new()
            .set_background_color(0xDDEBF7) // Light blue (matches odd row)
            .set_align(FormatAlign::Center)
            .set_border(FormatBorder::Thin);

        // Daily schedule: weekend formats (medium gray)
        let weekend_header = Format::new()
            .set_bold()
            .set_align(FormatAlign::Center)
            .set_background_color(0xA6A6A6) // Medium gray
            .set_font_color(0xFFFFFF)
            .set_border(FormatBorder::Thin);

        let weekend_cell = Format::new()
            .set_align(FormatAlign::Center)
            .set_background_color(0xD9D9D9) // Light gray
            .set_border(FormatBorder::Thin);

        // Daily schedule: holiday formats (gold/orange)
        let holiday_header = Format::new()
            .set_bold()
            .set_align(FormatAlign::Center)
            .set_background_color(0xED7D31) // Orange
            .set_font_color(0xFFFFFF)
            .set_border(FormatBorder::Thin);

        let holiday_cell = Format::new()
            .set_align(FormatAlign::Center)
            .set_background_color(0xFCE4D6) // Light orange/peach
            .set_border(FormatBorder::Thin);

        // Progress formats (RFC-0018 Visual mode)
        let progress_complete = Format::new()
            .set_align(FormatAlign::Center)
            .set_background_color(0xC6EFCE) // Light green
            .set_border(FormatBorder::Thin);

        let progress_behind = Format::new()
            .set_align(FormatAlign::Center)
            .set_background_color(0xFFC7CE) // Light red
            .set_border(FormatBorder::Thin);

        let progress_remaining = Format::new()
            .set_align(FormatAlign::Center)
            .set_background_color(0xBDD7EE) // Light blue
            .set_border(FormatBorder::Thin);

        ExcelFormats {
            header,
            currency,
            number,
            integer,
            text,
            week_header,
            total_row,
            total_currency,
            row_even_text,
            row_even_number,
            row_odd_text,
            row_odd_number,
            milestone_text,
            milestone_number,
            milestone_week,
            container_even_text,
            container_odd_text,
            gantt_even_empty,
            gantt_odd_empty,
            weekend_header,
            weekend_cell,
            holiday_header,
            holiday_cell,
            progress_complete,
            progress_behind,
            progress_remaining,
        }
    }

    /// Add Profiles and Costs sheet
    fn add_profiles_sheet(
        &self,
        workbook: &mut Workbook,
        project: &Project,
        schedule: &Schedule,
        formats: &ExcelFormats,
        resource_rates: &HashMap<String, f64>,
    ) -> Result<(), RenderError> {
        let sheet = workbook.add_worksheet();
        sheet
            .set_name("Profiles and Costs")
            .map_err(|e| RenderError::Format(e.to_string()))?;

        // Headers
        let headers = [
            "Profile ID",
            "Profile",
            &format!("Rate {}/d", self.currency),
            "Days (pd)",
            &format!("Cost {}", self.currency),
        ];

        for (col, header) in headers.iter().enumerate() {
            sheet
                .write_with_format(0, col as u16, *header, &formats.header)
                .map_err(|e| RenderError::Format(e.to_string()))?;
        }

        // Column widths
        sheet.set_column_width(0, 12).ok();
        sheet.set_column_width(1, 30).ok();
        sheet.set_column_width(2, 12).ok();
        sheet.set_column_width(3, 10).ok();
        sheet.set_column_width(4, 15).ok();

        // Calculate resource effort totals from schedule
        // Use explicit effort_days if available, otherwise calculate from duration × units
        let mut resource_effort: HashMap<String, f64> = HashMap::new();
        for scheduled in schedule.tasks.values() {
            for assignment in &scheduled.assignments {
                let effort = if let Some(effort_days) = assignment.effort_days {
                    effort_days
                } else {
                    let assignment_days = (assignment.finish - assignment.start).num_days() as f64;
                    assignment_days * assignment.units as f64
                };
                *resource_effort
                    .entry(assignment.resource_id.clone())
                    .or_default() += effort;
            }
        }

        // Write resource rows
        let mut row = 1u32;
        let mut total_cost = 0.0;

        for resource in &project.resources {
            let rate = resource_rates
                .get(&resource.id)
                .copied()
                .unwrap_or(self.default_rate);
            let days = resource_effort.get(&resource.id).copied().unwrap_or(0.0);
            let cost = rate * days;
            total_cost += cost;

            sheet
                .write_with_format(row, 0, &resource.id, &formats.text)
                .map_err(|e| RenderError::Format(e.to_string()))?;
            sheet
                .write_with_format(row, 1, &resource.name, &formats.text)
                .map_err(|e| RenderError::Format(e.to_string()))?;
            sheet
                .write_with_format(row, 2, rate, &formats.currency)
                .map_err(|e| RenderError::Format(e.to_string()))?;
            sheet
                .write_with_format(row, 3, days, &formats.number)
                .map_err(|e| RenderError::Format(e.to_string()))?;

            if self.use_formulas {
                let formula = format!("=C{}*D{}", row + 1, row + 1);
                sheet
                    .write_formula_with_format(row, 4, formula.as_str(), &formats.currency)
                    .map_err(|e| RenderError::Format(e.to_string()))?;
            } else {
                sheet
                    .write_with_format(row, 4, cost, &formats.currency)
                    .map_err(|e| RenderError::Format(e.to_string()))?;
            }

            row += 1;
        }

        // Total row
        sheet
            .write_with_format(row, 0, "TOTAL", &formats.total_row)
            .map_err(|e| RenderError::Format(e.to_string()))?;
        sheet
            .write_with_format(row, 1, "", &formats.total_row)
            .map_err(|e| RenderError::Format(e.to_string()))?;
        sheet
            .write_with_format(row, 2, "", &formats.total_row)
            .map_err(|e| RenderError::Format(e.to_string()))?;

        if self.use_formulas && row > 1 {
            let sum_days = format!("=SUM(D2:D{})", row);
            sheet
                .write_formula_with_format(row, 3, sum_days.as_str(), &formats.total_row)
                .map_err(|e| RenderError::Format(e.to_string()))?;
            let sum_cost = format!("=SUM(E2:E{})", row);
            sheet
                .write_formula_with_format(row, 4, sum_cost.as_str(), &formats.total_currency)
                .map_err(|e| RenderError::Format(e.to_string()))?;
        } else {
            let total_days: f64 = resource_effort.values().sum();
            sheet
                .write_with_format(row, 3, total_days, &formats.total_row)
                .map_err(|e| RenderError::Format(e.to_string()))?;
            sheet
                .write_with_format(row, 4, total_cost, &formats.total_currency)
                .map_err(|e| RenderError::Format(e.to_string()))?;
        }

        Ok(())
    }

    /// Add Schedule (Gantt) sheet with optional dependency support
    fn add_schedule_sheet(
        &self,
        workbook: &mut Workbook,
        project: &Project,
        schedule: &Schedule,
        formats: &ExcelFormats,
        project_start: NaiveDate,
    ) -> Result<(), RenderError> {
        // Branch based on granularity
        match self.granularity {
            ScheduleGranularity::Daily => {
                return self.add_daily_schedule_sheet(
                    workbook,
                    project,
                    schedule,
                    formats,
                    project_start,
                );
            }
            ScheduleGranularity::Weekly => {
                // Continue with weekly implementation below
            }
        }

        // Calculate effective weeks (applies auto-fit if enabled)
        let effective_weeks = self.get_effective_weeks(schedule, project_start);

        let sheet = workbook.add_worksheet();
        sheet
            .set_name("Schedule")
            .map_err(|e| RenderError::Format(e.to_string()))?;

        // Column layout depends on show_dependencies
        // With deps: Task ID, Activity, M, Profile, Depends On, Type, Lag, Effort, Start, End, W1...
        // Without:   Activity, M, Profile, pd, Start, End, W1...

        // Column layout with Lvl column added after Activity:
        // With deps (base): Task ID(0), Activity(1), Lvl(2), M(3), Profile(4), Depends(5), Type(6), Lag(7), Effort(8), Start(9), End(10), Weeks(11+)
        // With deps + progress: ... M(3), [Complete(4), Remaining(5), ActStart(6), ActEnd(7)], Profile(8), ... Weeks(15+)
        // Without (base): Activity(0), Lvl(1), M(2), Profile(3), pd(4), Start(5), End(6), Weeks(7+)
        // Without + progress: ... M(2), [Complete(3), Remaining(4), ActStart(5), ActEnd(6)], Profile(7), ... Weeks(11+)
        let progress_offset = self.progress_column_count();
        let (week_start_col, effort_col, start_col, end_col) = if self.show_dependencies {
            self.write_schedule_headers_with_deps(sheet, formats)?;
            (
                11u16 + progress_offset,
                8u16 + progress_offset,
                9u16 + progress_offset,
                10u16 + progress_offset,
            )
        } else {
            self.write_schedule_headers_simple(sheet, formats)?;
            (
                7u16 + progress_offset,
                4u16 + progress_offset,
                5u16 + progress_offset,
                6u16 + progress_offset,
            )
        };

        // Week column headers
        for week in 1..=effective_weeks {
            let col = week_start_col + (week - 1) as u16;
            sheet
                .write_with_format(0, col, week as f64, &formats.week_header)
                .map_err(|e| RenderError::Format(e.to_string()))?;
            sheet.set_column_width(col, 4).ok();
        }

        // Set row height for header (rotated text)
        sheet.set_row_height(0, 50).ok();

        // Collect tasks in WBS order (depth-first traversal of project hierarchy)
        let wbs_order = Self::collect_wbs_order(&project.tasks, 0);

        // Map scheduled tasks to WBS order
        let tasks: Vec<(&ScheduledTask, usize)> = wbs_order
            .iter()
            .filter_map(|(task_id, level)| schedule.tasks.get(task_id).map(|st| (st, *level)))
            .collect();

        // Build set of all full task IDs for predecessor resolution
        let all_full_ids: std::collections::HashSet<String> =
            tasks.iter().map(|(st, _)| st.task_id.clone()).collect();

        // Build mapping from simple task IDs to full path IDs for VLOOKUP resolution
        // e.g., "gnu_analysis" -> "os_migration.gnu_val.gnu_analysis"
        let simple_to_full_id: HashMap<String, String> = tasks
            .iter()
            .map(|(st, _)| {
                let simple = st
                    .task_id
                    .rsplit('.')
                    .next()
                    .unwrap_or(&st.task_id)
                    .to_string();
                (simple, st.task_id.clone())
            })
            .collect();

        // Build task row mapping for VLOOKUP (task_id -> row number)
        let mut task_row_map: HashMap<String, u32> = HashMap::new();
        let mut current_row = 2u32; // Excel rows are 1-indexed, data starts at row 2
        for (scheduled, _level) in &tasks {
            task_row_map.insert(scheduled.task_id.clone(), current_row);
            if scheduled.assignments.is_empty() {
                current_row += 1;
            } else {
                current_row += scheduled.assignments.len() as u32;
            }
        }
        let last_data_row = current_row - 1;

        // Write task rows
        let mut row = 1u32;
        let mut prev_task_id = String::new();
        let mut is_odd = false;
        for (scheduled, level) in &tasks {
            // Toggle alternating row color when task changes
            if scheduled.task_id != prev_task_id {
                is_odd = !is_odd;
                prev_task_id = scheduled.task_id.clone();
            }
            // Extract the simple task ID from the path (e.g., "task_5.task_6.task_7" -> "task_7")
            let simple_id = scheduled
                .task_id
                .rsplit('.')
                .next()
                .unwrap_or(&scheduled.task_id);
            let task = project.get_task(simple_id);

            // Check if this is a container task (has children)
            // Container tasks have derived duration (span of children) which should NOT
            // be counted as effort to avoid double-counting
            let is_container = task.map(|t| !t.children.is_empty()).unwrap_or(false);

            // Check if this is a milestone (explicit milestone: true attribute)
            let is_milestone = task.map(|t| t.milestone).unwrap_or(false);

            // Get base task name and add indentation for hierarchy
            let base_name = task
                .map(|t| t.name.clone())
                .unwrap_or_else(|| simple_id.to_string());
            let indent = "  ".repeat(*level);
            let task_name = format!("{}{}", indent, base_name);

            // Get first predecessor (if any) for dependency column
            // Resolve simple predecessor ID to full path for VLOOKUP compatibility
            let (predecessor, dep_type, lag) = task
                .and_then(|t| t.depends.first())
                .map(|d| {
                    use utf8proj_core::DependencyType;
                    let dep_type = match d.dep_type {
                        DependencyType::StartToStart => "SS",
                        DependencyType::FinishToFinish => "FF",
                        DependencyType::StartToFinish => "SF",
                        DependencyType::FinishToStart => "FS",
                    };
                    let lag_days = d.lag.map(|l| l.as_days() as i32).unwrap_or(0);
                    // Resolve predecessor ID to full path for VLOOKUP compatibility
                    // Handle: simple IDs ("gnu_analysis"), partial paths ("gnu_val.gnu_analysis"),
                    // and full paths ("os_migration.gnu_val.gnu_analysis")
                    let full_predecessor = if all_full_ids.contains(&d.predecessor) {
                        // Already a full path
                        d.predecessor.clone()
                    } else if let Some(full) = simple_to_full_id.get(&d.predecessor) {
                        // Simple ID -> full path
                        full.clone()
                    } else {
                        // Partial path - find full path that ends with this suffix
                        all_full_ids
                            .iter()
                            .find(|full_id| {
                                full_id.ends_with(&format!(".{}", d.predecessor))
                                    || full_id.ends_with(&d.predecessor)
                            })
                            .cloned()
                            .unwrap_or_else(|| d.predecessor.clone())
                    };
                    (full_predecessor, dep_type, lag_days)
                })
                .unwrap_or_default();

            // Calculate week numbers relative to project start
            let start_week = self.date_to_week(scheduled.start, project_start);
            let end_week = self.date_to_week(scheduled.finish, project_start);

            // For container tasks, effort is 0 (their duration is derived from children)
            // Only leaf tasks contribute actual effort
            let duration_days = if is_container {
                0.0
            } else {
                scheduled.duration.as_days()
            };

            // Build progress data if progress mode is enabled (RFC-0018)
            let progress_data = if self.progress_mode != ProgressMode::None {
                // Get status date for variance/status calculation
                let status_date = self
                    .status_date
                    .unwrap_or_else(|| chrono::Local::now().date_naive());

                // Calculate variance: how many days ahead/behind based on expected progress
                // Expected progress = (status_date - start) / duration * 100
                // Variance = (actual_progress - expected_progress) * duration / 100
                let duration_total = (scheduled.finish - scheduled.start).num_days() as f64;
                let variance_days = if duration_total > 0.0 && status_date >= scheduled.start {
                    let days_elapsed =
                        (status_date.min(scheduled.finish) - scheduled.start).num_days() as f64;
                    let expected_pct = (days_elapsed / duration_total * 100.0).min(100.0);
                    let actual_pct = scheduled.percent_complete as f64;
                    // Positive = ahead, negative = behind
                    ((actual_pct - expected_pct) * duration_total / 100.0).round() as i32
                } else {
                    0
                };

                // Determine task status
                let status = if scheduled.percent_complete >= 100 {
                    TaskStatus::Complete
                } else if scheduled.percent_complete > 0 {
                    // In progress - check if behind
                    if variance_days < 0 {
                        TaskStatus::Behind
                    } else {
                        TaskStatus::InProgress
                    }
                } else {
                    // Not started (0%)
                    if scheduled.start <= status_date {
                        TaskStatus::Overdue // Should have started
                    } else {
                        TaskStatus::NotStarted
                    }
                };

                Some(ProgressData {
                    percent_complete: scheduled.percent_complete,
                    remaining_days: scheduled.remaining_duration.as_days(),
                    actual_start: task.and_then(|t| t.actual_start),
                    actual_finish: task.and_then(|t| t.actual_finish),
                    task_start: Some(scheduled.start),
                    task_finish: Some(scheduled.finish),
                    variance_days,
                    status,
                })
            } else {
                None
            };

            // If task has assignments, create a row per assignment
            if scheduled.assignments.is_empty() {
                if self.show_dependencies {
                    self.write_schedule_row_with_deps(
                        sheet,
                        row,
                        &scheduled.task_id,
                        &task_name,
                        *level,
                        "",
                        &predecessor,
                        dep_type,
                        lag,
                        duration_days,
                        start_week,
                        end_week,
                        scheduled.is_critical,
                        is_milestone,
                        is_container,
                        formats,
                        week_start_col,
                        effort_col,
                        start_col,
                        end_col,
                        last_data_row,
                        is_odd,
                        effective_weeks,
                        progress_data.as_ref(),
                        project_start,
                    )?;
                } else {
                    self.write_schedule_row_simple(
                        sheet,
                        row,
                        &task_name,
                        *level,
                        "",
                        duration_days,
                        start_week,
                        end_week,
                        scheduled.is_critical,
                        is_milestone,
                        is_container,
                        formats,
                        week_start_col,
                        effort_col,
                        start_col,
                        end_col,
                        is_odd,
                        effective_weeks,
                        progress_data.as_ref(),
                        project_start,
                    )?;
                }
                row += 1;
            } else {
                // One row per assignment
                let mut first_assignment = true;
                for assignment in &scheduled.assignments {
                    // Use explicit effort_days if available, otherwise calculate from duration × units
                    let effort = if let Some(effort_days) = assignment.effort_days {
                        effort_days
                    } else {
                        let assignment_days =
                            (assignment.finish - assignment.start).num_days() as f64;
                        assignment_days * assignment.units as f64
                    };

                    // Only show dependency info on first row for this task
                    let (pred, dtype, lag_val) = if first_assignment {
                        (predecessor.clone(), dep_type, lag)
                    } else {
                        (String::new(), "", 0)
                    };

                    if self.show_dependencies {
                        self.write_schedule_row_with_deps(
                            sheet,
                            row,
                            &scheduled.task_id,
                            &task_name,
                            *level,
                            &assignment.resource_id,
                            &pred,
                            dtype,
                            lag_val,
                            effort,
                            start_week,
                            end_week,
                            scheduled.is_critical,
                            is_milestone,
                            is_container,
                            formats,
                            week_start_col,
                            effort_col,
                            start_col,
                            end_col,
                            last_data_row,
                            is_odd,
                            effective_weeks,
                            progress_data.as_ref(),
                            project_start,
                        )?;
                    } else {
                        self.write_schedule_row_simple(
                            sheet,
                            row,
                            &task_name,
                            *level,
                            &assignment.resource_id,
                            effort,
                            start_week,
                            end_week,
                            scheduled.is_critical,
                            is_milestone,
                            is_container,
                            formats,
                            week_start_col,
                            effort_col,
                            start_col,
                            end_col,
                            is_odd,
                            effective_weeks,
                            progress_data.as_ref(),
                            project_start,
                        )?;
                    }
                    first_assignment = false;
                    row += 1;
                }
            }
        }

        // Total row for each week column
        self.write_schedule_totals(
            sheet,
            row,
            week_start_col,
            effort_col,
            formats,
            effective_weeks,
        )?;

        // Add conditional formatting for week columns: blue fill when numeric value > 0
        // Uses ISNUMBER check to exclude milestones ("◆") and empty cells ("")
        // This enables dynamic what-if analysis - colors update when effort/dependencies change
        let last_week_col = week_start_col + effective_weeks as u16 - 1;
        let last_data_row_for_cf = row - 1; // Exclude totals row from conditional formatting
        if last_data_row_for_cf >= 1 {
            // Create format for filled cells (blue background for Gantt bar)
            let gantt_filled_format = Format::new()
                .set_background_color(0x5B9BD5) // Standard blue for Gantt bar
                .set_align(FormatAlign::Center)
                .set_border(FormatBorder::Thin);

            // Formula-based conditional format: apply blue fill only when cell is numeric AND > 0
            // This excludes milestones ("◆" text) and empty cells ("") from blue formatting
            let first_week_col_letter = Self::col_to_letter(week_start_col);
            let formula = format!(
                "=AND(ISNUMBER({}2),{}2>0)",
                first_week_col_letter, first_week_col_letter
            );
            let conditional_format = ConditionalFormatFormula::new()
                .set_rule(formula.as_str())
                .set_format(gantt_filled_format);

            // Apply to entire week column range (rows 1 to last_data_row, columns week_start to last_week)
            sheet
                .add_conditional_format(
                    1, // Start row (after header)
                    week_start_col,
                    last_data_row_for_cf,
                    last_week_col,
                    &conditional_format,
                )
                .map_err(|e| RenderError::Format(e.to_string()))?;
        }

        // Freeze first row and fixed columns
        let freeze_cols = if self.show_dependencies { 10 } else { 6 };
        sheet.set_freeze_panes(1, freeze_cols).ok();

        Ok(())
    }

    /// Add Daily Schedule sheet with calendar awareness (weekends/holidays)
    fn add_daily_schedule_sheet(
        &self,
        workbook: &mut Workbook,
        project: &Project,
        schedule: &Schedule,
        formats: &ExcelFormats,
        project_start: NaiveDate,
    ) -> Result<(), RenderError> {
        let sheet = workbook.add_worksheet();
        sheet
            .set_name("Schedule")
            .map_err(|e| RenderError::Format(e.to_string()))?;

        // Get calendar for working day detection
        let calendar = self.calendar.clone().unwrap_or_else(|| {
            // Try to get from project, otherwise use default
            project.calendars.first().cloned().unwrap_or_default()
        });

        // Column layout (same as weekly with dependencies):
        // Task ID(0), Activity(1), Lvl(2), M(3), Profile(4), Depends(5), Type(6), Lag(7), Effort(8), Start(9), End(10), Days(11+)
        let day_start_col = 11u16;
        let effort_col = 8u16;
        let start_col = 9u16;
        let end_col = 10u16;

        // Write base headers (same as weekly with deps)
        self.write_daily_schedule_headers(sheet, formats, project_start, &calendar)?;

        // Collect tasks in WBS order
        let wbs_order = Self::collect_wbs_order(&project.tasks, 0);
        let tasks: Vec<(&ScheduledTask, usize)> = wbs_order
            .iter()
            .filter_map(|(task_id, level)| schedule.tasks.get(task_id).map(|st| (st, *level)))
            .collect();

        // Build predecessor resolution maps
        let all_full_ids: std::collections::HashSet<String> =
            tasks.iter().map(|(st, _)| st.task_id.clone()).collect();

        let simple_to_full_id: HashMap<String, String> = tasks
            .iter()
            .map(|(st, _)| {
                let simple = st
                    .task_id
                    .rsplit('.')
                    .next()
                    .unwrap_or(&st.task_id)
                    .to_string();
                (simple, st.task_id.clone())
            })
            .collect();

        // Track last data row (for future formula-driven mode)
        let _last_data_row: u32 = tasks
            .iter()
            .map(|(st, _)| {
                if st.assignments.is_empty() {
                    1
                } else {
                    st.assignments.len() as u32
                }
            })
            .sum::<u32>()
            + 1;

        // Write task rows
        let mut row = 1u32;
        let mut prev_task_id = String::new();
        let mut is_odd = false;

        for (scheduled, level) in &tasks {
            // Toggle alternating row color when task changes
            if scheduled.task_id != prev_task_id {
                is_odd = !is_odd;
                prev_task_id = scheduled.task_id.clone();
            }

            let simple_id = scheduled
                .task_id
                .rsplit('.')
                .next()
                .unwrap_or(&scheduled.task_id);
            let task = project.get_task(simple_id);
            let is_container = task.map(|t| !t.children.is_empty()).unwrap_or(false);
            let is_milestone = task.map(|t| t.milestone).unwrap_or(false);

            let base_name = task
                .map(|t| t.name.clone())
                .unwrap_or_else(|| simple_id.to_string());
            let indent = "  ".repeat(*level);
            let task_name = format!("{}{}", indent, base_name);

            // Get dependency info
            let (predecessor, dep_type, lag) = task
                .and_then(|t| t.depends.first())
                .map(|d| {
                    use utf8proj_core::DependencyType;
                    let dep_type = match d.dep_type {
                        DependencyType::StartToStart => "SS",
                        DependencyType::FinishToFinish => "FF",
                        DependencyType::StartToFinish => "SF",
                        DependencyType::FinishToStart => "FS",
                    };
                    let lag_days = d.lag.map(|l| l.as_days() as i32).unwrap_or(0);
                    let full_predecessor = if all_full_ids.contains(&d.predecessor) {
                        d.predecessor.clone()
                    } else if let Some(full) = simple_to_full_id.get(&d.predecessor) {
                        full.clone()
                    } else {
                        all_full_ids
                            .iter()
                            .find(|full_id| {
                                full_id.ends_with(&format!(".{}", d.predecessor))
                                    || full_id.ends_with(&d.predecessor)
                            })
                            .cloned()
                            .unwrap_or_else(|| d.predecessor.clone())
                    };
                    (full_predecessor, dep_type, lag_days)
                })
                .unwrap_or_default();

            let duration_days = if is_container {
                0.0
            } else {
                scheduled.duration.as_days()
            };

            if scheduled.assignments.is_empty() {
                self.write_daily_schedule_row(
                    sheet,
                    row,
                    &scheduled.task_id,
                    &task_name,
                    *level,
                    "",
                    &predecessor,
                    dep_type,
                    lag,
                    duration_days,
                    scheduled.start,
                    scheduled.finish,
                    scheduled.is_critical,
                    is_milestone,
                    is_container,
                    formats,
                    day_start_col,
                    effort_col,
                    start_col,
                    end_col,
                    project_start,
                    &calendar,
                    is_odd,
                )?;
                row += 1;
            } else {
                let mut first_assignment = true;
                for assignment in &scheduled.assignments {
                    let effort = assignment.effort_days.unwrap_or_else(|| {
                        let assignment_days =
                            (assignment.finish - assignment.start).num_days() as f64;
                        assignment_days * assignment.units as f64
                    });

                    let (pred, dtype, lag_val) = if first_assignment {
                        (predecessor.clone(), dep_type, lag)
                    } else {
                        (String::new(), "", 0)
                    };

                    self.write_daily_schedule_row(
                        sheet,
                        row,
                        &scheduled.task_id,
                        &task_name,
                        *level,
                        &assignment.resource_id,
                        &pred,
                        dtype,
                        lag_val,
                        effort,
                        scheduled.start,
                        scheduled.finish,
                        scheduled.is_critical,
                        is_milestone,
                        is_container,
                        formats,
                        day_start_col,
                        effort_col,
                        start_col,
                        end_col,
                        project_start,
                        &calendar,
                        is_odd,
                    )?;
                    first_assignment = false;
                    row += 1;
                }
            }
        }

        // Add conditional formatting for day columns
        let last_day_col = day_start_col + self.schedule_days as u16 - 1;
        let last_data_row_for_cf = row - 1;
        if last_data_row_for_cf >= 1 {
            let gantt_filled_format = Format::new()
                .set_background_color(0x5B9BD5)
                .set_align(FormatAlign::Center)
                .set_border(FormatBorder::Thin);

            let first_day_col_letter = Self::col_to_letter(day_start_col);
            let formula = format!(
                "=AND(ISNUMBER({}2),{}2>0)",
                first_day_col_letter, first_day_col_letter
            );
            let conditional_format = ConditionalFormatFormula::new()
                .set_rule(formula.as_str())
                .set_format(gantt_filled_format);

            sheet
                .add_conditional_format(
                    1,
                    day_start_col,
                    last_data_row_for_cf,
                    last_day_col,
                    &conditional_format,
                )
                .map_err(|e| RenderError::Format(e.to_string()))?;
        }

        // Freeze header row and fixed columns
        sheet.set_freeze_panes(1, 10).ok();

        Ok(())
    }

    /// Write headers for daily schedule
    fn write_daily_schedule_headers(
        &self,
        sheet: &mut Worksheet,
        formats: &ExcelFormats,
        project_start: NaiveDate,
        calendar: &Calendar,
    ) -> Result<(), RenderError> {
        // Fixed column headers
        let headers = [
            "Task ID",
            "Activity",
            "Lvl",
            "M",
            "Profile",
            "Depends\nOn",
            "Type",
            "Lag\n(d)",
            "Effort\n(pd)",
            "Start",
            "End",
        ];
        for (col, header) in headers.iter().enumerate() {
            sheet
                .write_with_format(0, col as u16, *header, &formats.header)
                .map_err(|e| RenderError::Format(e.to_string()))?;
        }

        // Column widths for fixed columns
        sheet.set_column_width(0, 12).ok(); // Task ID
        sheet.set_column_width(1, 25).ok(); // Activity
        sheet.set_column_width(2, 3).ok(); // Lvl
        sheet.set_column_width(3, 3).ok(); // M
        sheet.set_column_width(4, 12).ok(); // Profile
        sheet.set_column_width(5, 10).ok(); // Depends On
        sheet.set_column_width(6, 5).ok(); // Type
        sheet.set_column_width(7, 5).ok(); // Lag
        sheet.set_column_width(8, 6).ok(); // Effort
        sheet.set_column_width(9, 8).ok(); // Start
        sheet.set_column_width(10, 8).ok(); // End

        // Day column headers with date and weekend/holiday styling
        let day_start_col = 11u16;
        for day in 0..self.schedule_days {
            let col = day_start_col + day as u16;
            let date = project_start + chrono::Duration::days(day as i64);

            // Format header: "M 6/1" (weekday + date)
            let weekday_abbrev = match date.weekday() {
                Weekday::Mon => "M",
                Weekday::Tue => "T",
                Weekday::Wed => "W",
                Weekday::Thu => "T",
                Weekday::Fri => "F",
                Weekday::Sat => "S",
                Weekday::Sun => "S",
            };
            let header_text = format!("{}\n{}/{}", weekday_abbrev, date.day(), date.month());

            // Check if it's a holiday
            let holiday = calendar.holidays.iter().find(|h| h.contains(date));
            let is_weekend = !calendar
                .working_days
                .contains(&(date.weekday().num_days_from_sunday() as u8));

            // Choose header format based on day type
            let header_fmt = if holiday.is_some() {
                &formats.holiday_header
            } else if is_weekend {
                &formats.weekend_header
            } else {
                &formats.week_header
            };

            sheet
                .write_with_format(0, col, &header_text, header_fmt)
                .map_err(|e| RenderError::Format(e.to_string()))?;
            sheet.set_column_width(col, 4).ok();
        }

        // Set row height for header
        sheet.set_row_height(0, 40).ok();

        Ok(())
    }

    /// Write a daily schedule row
    #[allow(clippy::too_many_arguments)]
    fn write_daily_schedule_row(
        &self,
        sheet: &mut Worksheet,
        row: u32,
        task_id: &str,
        task_name: &str,
        level: usize,
        profile: &str,
        predecessor: &str,
        dep_type: &str,
        lag: i32,
        person_days: f64,
        task_start: NaiveDate,
        task_finish: NaiveDate,
        _is_critical: bool,
        is_milestone: bool,
        is_container: bool,
        formats: &ExcelFormats,
        day_start_col: u16,
        effort_col: u16,
        start_col: u16,
        end_col: u16,
        project_start: NaiveDate,
        calendar: &Calendar,
        is_odd: bool,
    ) -> Result<(), RenderError> {
        // Select formats based on row type
        let (text_fmt, number_fmt) = if is_milestone {
            (&formats.milestone_text, &formats.milestone_number)
        } else if is_odd {
            (&formats.row_odd_text, &formats.row_odd_number)
        } else {
            (&formats.row_even_text, &formats.row_even_number)
        };

        let activity_fmt = if is_container {
            if is_odd {
                &formats.container_odd_text
            } else {
                &formats.container_even_text
            }
        } else {
            text_fmt
        };

        // Col A: Task ID
        sheet
            .write_with_format(row, 0, task_id, text_fmt)
            .map_err(|e| RenderError::Format(e.to_string()))?;

        // Col B: Activity
        sheet
            .write_with_format(row, 1, task_name, activity_fmt)
            .map_err(|e| RenderError::Format(e.to_string()))?;

        // Col C: Lvl
        sheet
            .write_with_format(row, 2, level as f64, number_fmt)
            .map_err(|e| RenderError::Format(e.to_string()))?;

        // Col D: Milestone marker
        let milestone_marker = if is_milestone { "◆" } else { "" };
        sheet
            .write_with_format(row, 3, milestone_marker, text_fmt)
            .map_err(|e| RenderError::Format(e.to_string()))?;

        // Col E: Profile
        sheet
            .write_with_format(row, 4, profile, text_fmt)
            .map_err(|e| RenderError::Format(e.to_string()))?;

        // Col F: Depends On
        sheet
            .write_with_format(row, 5, predecessor, text_fmt)
            .map_err(|e| RenderError::Format(e.to_string()))?;

        // Col G: Type
        let dep_type_val = if predecessor.is_empty() { "" } else { dep_type };
        sheet
            .write_with_format(row, 6, dep_type_val, text_fmt)
            .map_err(|e| RenderError::Format(e.to_string()))?;

        // Col H: Lag
        if !predecessor.is_empty() {
            sheet
                .write_with_format(row, 7, lag as f64, number_fmt)
                .map_err(|e| RenderError::Format(e.to_string()))?;
        } else {
            sheet
                .write_with_format(row, 7, "", text_fmt)
                .map_err(|e| RenderError::Format(e.to_string()))?;
        }

        // Col I: Effort
        sheet
            .write_with_format(row, effort_col, person_days, number_fmt)
            .map_err(|e| RenderError::Format(e.to_string()))?;

        // Col J: Start (date format)
        let start_str = task_start.format("%d/%m").to_string();
        sheet
            .write_with_format(row, start_col, &start_str, text_fmt)
            .map_err(|e| RenderError::Format(e.to_string()))?;

        // Col K: End (date format)
        let end_str = task_finish.format("%d/%m").to_string();
        sheet
            .write_with_format(row, end_col, &end_str, text_fmt)
            .map_err(|e| RenderError::Format(e.to_string()))?;

        // Day columns
        self.write_daily_columns(
            sheet,
            row,
            task_start,
            task_finish,
            is_milestone,
            is_container,
            is_odd,
            formats,
            day_start_col,
            project_start,
            calendar,
            person_days,
        )?;

        Ok(())
    }

    /// Write day columns for a task row
    ///
    /// Uses smart hour distribution to ensure the sum of displayed hours
    /// exactly matches the expected effort (no rounding errors).
    #[allow(clippy::too_many_arguments)]
    fn write_daily_columns(
        &self,
        sheet: &mut Worksheet,
        row: u32,
        task_start: NaiveDate,
        task_finish: NaiveDate,
        is_milestone: bool,
        is_container: bool,
        is_odd: bool,
        formats: &ExcelFormats,
        day_start_col: u16,
        project_start: NaiveDate,
        calendar: &Calendar,
        person_days: f64,
    ) -> Result<(), RenderError> {
        // Calculate total hours and working days for smart distribution
        let total_hours = (person_days * self.hours_per_day).round() as u32;
        let working_days_count = self.count_working_days(task_start, task_finish, calendar);

        // Smart distribution: base hours per day + remainder distributed across first N days
        // Example: 8h over 5 days -> base=1, remainder=3 -> [2,2,2,1,1] sums to 8
        let (base_hours, remainder) = if working_days_count > 0 {
            let base = total_hours / working_days_count;
            let rem = total_hours % working_days_count;
            (base, rem)
        } else {
            (0, 0)
        };

        // Track which working day we're on (for remainder distribution)
        let mut working_day_index = 0u32;

        for day in 0..self.schedule_days {
            let col = day_start_col + day as u16;
            let date = project_start + chrono::Duration::days(day as i64);

            // Check day type
            let holiday = calendar.holidays.iter().find(|h| h.contains(date));
            let is_weekend = !calendar
                .working_days
                .contains(&(date.weekday().num_days_from_sunday() as u8));
            let in_task_range = date >= task_start && date <= task_finish;

            // Select cell format based on day type
            let cell_fmt = if holiday.is_some() {
                &formats.holiday_cell
            } else if is_weekend {
                &formats.weekend_cell
            } else if is_milestone {
                &formats.milestone_week
            } else if is_odd {
                &formats.gantt_odd_empty
            } else {
                &formats.gantt_even_empty
            };

            // Container tasks: no Gantt bar
            if is_container {
                sheet
                    .write_with_format(row, col, "", cell_fmt)
                    .map_err(|e| RenderError::Format(e.to_string()))?;
                continue;
            }

            // Non-working days: always empty (no hours distributed)
            if holiday.is_some() || is_weekend {
                sheet
                    .write_with_format(row, col, "", cell_fmt)
                    .map_err(|e| RenderError::Format(e.to_string()))?;
                continue;
            }

            // Working day within task range
            if in_task_range {
                if is_milestone {
                    sheet
                        .write_with_format(row, col, "◆", cell_fmt)
                        .map_err(|e| RenderError::Format(e.to_string()))?;
                } else if total_hours > 0 {
                    // Smart distribution: first `remainder` days get base+1, rest get base
                    let hours = if working_day_index < remainder {
                        base_hours + 1
                    } else {
                        base_hours
                    };
                    if hours > 0 {
                        sheet
                            .write_with_format(row, col, hours as f64, cell_fmt)
                            .map_err(|e| RenderError::Format(e.to_string()))?;
                    } else {
                        sheet
                            .write_with_format(row, col, "", cell_fmt)
                            .map_err(|e| RenderError::Format(e.to_string()))?;
                    }
                    working_day_index += 1;
                } else {
                    sheet
                        .write_with_format(row, col, "", cell_fmt)
                        .map_err(|e| RenderError::Format(e.to_string()))?;
                }
            } else {
                sheet
                    .write_with_format(row, col, "", cell_fmt)
                    .map_err(|e| RenderError::Format(e.to_string()))?;
            }
        }

        Ok(())
    }

    /// Count working days between two dates (inclusive)
    fn count_working_days(&self, start: NaiveDate, end: NaiveDate, calendar: &Calendar) -> u32 {
        let mut count = 0;
        let mut date = start;
        while date <= end {
            if calendar.is_working_day(date) {
                count += 1;
            }
            date += chrono::Duration::days(1);
        }
        count
    }

    /// Write headers for simple schedule (no dependencies)
    fn write_schedule_headers_simple(
        &self,
        sheet: &mut Worksheet,
        formats: &ExcelFormats,
    ) -> Result<(), RenderError> {
        let progress_offset = self.progress_column_count();

        // Base headers (before progress columns)
        let base_headers = ["Activity", "Lvl", "M"];

        // Progress headers (RFC-0018) - Full mode adds Status and Variance
        let progress_headers: Vec<&str> = if self.progress_mode == ProgressMode::Full {
            vec![
                "Status",
                "Complete",
                "Remaining",
                "Variance",
                "Act.\nStart",
                "Act.\nEnd",
            ]
        } else {
            vec!["Complete", "Remaining", "Act.\nStart", "Act.\nEnd"]
        };

        // Remaining headers (after progress columns)
        let remaining_headers = ["Profile", "pd", "Start\nweek", "End\nweek"];

        // Write base headers (0-2)
        for (col, header) in base_headers.iter().enumerate() {
            sheet
                .write_with_format(0, col as u16, *header, &formats.header)
                .map_err(|e| RenderError::Format(e.to_string()))?;
        }

        // Write progress headers if enabled
        if progress_offset > 0 {
            for (i, header) in progress_headers.iter().enumerate() {
                sheet
                    .write_with_format(0, (3 + i) as u16, *header, &formats.header)
                    .map_err(|e| RenderError::Format(e.to_string()))?;
            }
        }

        // Write remaining headers (offset by progress columns)
        for (i, header) in remaining_headers.iter().enumerate() {
            sheet
                .write_with_format(0, 3 + progress_offset + i as u16, *header, &formats.header)
                .map_err(|e| RenderError::Format(e.to_string()))?;
        }

        // Column widths
        sheet.set_column_width(0, 25).ok(); // Activity
        sheet.set_column_width(1, 3).ok(); // Lvl (nesting level)
        sheet.set_column_width(2, 3).ok(); // M (milestone marker)

        if self.progress_mode == ProgressMode::Full {
            sheet.set_column_width(3, 6).ok(); // Status
            sheet.set_column_width(4, 8).ok(); // Complete
            sheet.set_column_width(5, 9).ok(); // Remaining
            sheet.set_column_width(6, 8).ok(); // Variance
            sheet.set_column_width(7, 10).ok(); // Act. Start
            sheet.set_column_width(8, 10).ok(); // Act. End
        } else if progress_offset > 0 {
            sheet.set_column_width(3, 8).ok(); // Complete
            sheet.set_column_width(4, 9).ok(); // Remaining
            sheet.set_column_width(5, 10).ok(); // Act. Start
            sheet.set_column_width(6, 10).ok(); // Act. End
        }

        let base = 3 + progress_offset;
        sheet.set_column_width(base, 15).ok(); // Profile
        sheet.set_column_width(base + 1, 6).ok(); // pd
        sheet.set_column_width(base + 2, 6).ok(); // Start
        sheet.set_column_width(base + 3, 6).ok(); // End

        Ok(())
    }

    /// Write headers for schedule with dependencies
    fn write_schedule_headers_with_deps(
        &self,
        sheet: &mut Worksheet,
        formats: &ExcelFormats,
    ) -> Result<(), RenderError> {
        let progress_offset = self.progress_column_count();

        // Base headers (before progress columns)
        let base_headers = ["Task ID", "Activity", "Lvl", "M"];

        // Progress headers (RFC-0018) - Full mode adds Status and Variance
        let progress_headers: Vec<&str> = if self.progress_mode == ProgressMode::Full {
            vec![
                "Status",
                "Complete",
                "Remaining",
                "Variance",
                "Act.\nStart",
                "Act.\nEnd",
            ]
        } else {
            vec!["Complete", "Remaining", "Act.\nStart", "Act.\nEnd"]
        };

        // Remaining headers (after progress columns)
        let remaining_headers = [
            "Profile",
            "Depends\nOn",
            "Type",
            "Lag\n(d)",
            "Effort\n(pd)",
            "Start\nweek",
            "End\nweek",
        ];

        // Write base headers (0-3)
        for (col, header) in base_headers.iter().enumerate() {
            sheet
                .write_with_format(0, col as u16, *header, &formats.header)
                .map_err(|e| RenderError::Format(e.to_string()))?;
        }

        // Write progress headers if enabled
        if progress_offset > 0 {
            for (i, header) in progress_headers.iter().enumerate() {
                sheet
                    .write_with_format(0, (4 + i) as u16, *header, &formats.header)
                    .map_err(|e| RenderError::Format(e.to_string()))?;
            }
        }

        // Write remaining headers (offset by progress columns)
        for (i, header) in remaining_headers.iter().enumerate() {
            sheet
                .write_with_format(0, 4 + progress_offset + i as u16, *header, &formats.header)
                .map_err(|e| RenderError::Format(e.to_string()))?;
        }

        // Column widths
        sheet.set_column_width(0, 12).ok(); // Task ID
        sheet.set_column_width(1, 25).ok(); // Activity
        sheet.set_column_width(2, 3).ok(); // Lvl (nesting level)
        sheet.set_column_width(3, 3).ok(); // M (milestone marker)

        if self.progress_mode == ProgressMode::Full {
            sheet.set_column_width(4, 6).ok(); // Status
            sheet.set_column_width(5, 8).ok(); // Complete
            sheet.set_column_width(6, 9).ok(); // Remaining
            sheet.set_column_width(7, 8).ok(); // Variance
            sheet.set_column_width(8, 10).ok(); // Act. Start
            sheet.set_column_width(9, 10).ok(); // Act. End
        } else if progress_offset > 0 {
            sheet.set_column_width(4, 8).ok(); // Complete
            sheet.set_column_width(5, 9).ok(); // Remaining
            sheet.set_column_width(6, 10).ok(); // Act. Start
            sheet.set_column_width(7, 10).ok(); // Act. End
        }

        let base = 4 + progress_offset;
        sheet.set_column_width(base, 12).ok(); // Profile
        sheet.set_column_width(base + 1, 10).ok(); // Depends On
        sheet.set_column_width(base + 2, 5).ok(); // Type
        sheet.set_column_width(base + 3, 5).ok(); // Lag
        sheet.set_column_width(base + 4, 7).ok(); // Effort
        sheet.set_column_width(base + 5, 6).ok(); // Start
        sheet.set_column_width(base + 6, 6).ok(); // End

        Ok(())
    }

    /// Write a schedule row without dependency formulas
    #[allow(clippy::too_many_arguments)]
    fn write_schedule_row_simple(
        &self,
        sheet: &mut Worksheet,
        row: u32,
        task_name: &str,
        level: usize,
        profile: &str,
        person_days: f64,
        start_week: u32,
        end_week: u32,
        is_critical: bool,
        is_milestone: bool,
        is_container: bool,
        formats: &ExcelFormats,
        week_start_col: u16,
        effort_col: u16,
        start_col: u16,
        end_col: u16,
        is_odd: bool,
        schedule_weeks: u32,
        progress_data: Option<&ProgressData>,
        project_start: NaiveDate,
    ) -> Result<(), RenderError> {
        let progress_offset = self.progress_column_count();

        // Select formats: milestones use gold, otherwise alternate white/blue per task
        let (text_fmt, number_fmt) = if is_milestone {
            (&formats.milestone_text, &formats.milestone_number)
        } else if is_odd {
            (&formats.row_odd_text, &formats.row_odd_number)
        } else {
            (&formats.row_even_text, &formats.row_even_number)
        };

        // Container tasks use bold text for Activity to distinguish phases
        let activity_fmt = if is_container {
            if is_odd {
                &formats.container_odd_text
            } else {
                &formats.container_even_text
            }
        } else {
            text_fmt
        };

        // Col A: Activity (bold for containers)
        sheet
            .write_with_format(row, 0, task_name, activity_fmt)
            .map_err(|e| RenderError::Format(e.to_string()))?;

        // Col B: Lvl (nesting level for hierarchy filtering/grouping)
        sheet
            .write_with_format(row, 1, level as f64, number_fmt)
            .map_err(|e| RenderError::Format(e.to_string()))?;

        // Col C: Milestone marker (◆ for milestones, empty otherwise)
        let milestone_marker = if is_milestone { "◆" } else { "" };
        sheet
            .write_with_format(row, 2, milestone_marker, text_fmt)
            .map_err(|e| RenderError::Format(e.to_string()))?;

        // Progress columns (RFC-0018) - after M column
        if let Some(progress) = progress_data {
            if self.progress_mode == ProgressMode::Full {
                // Full mode: Status, Complete, Remaining, Variance, Act.Start, Act.End
                // Col 3: Status icon
                sheet
                    .write_with_format(row, 3, progress.status.icon(), text_fmt)
                    .map_err(|e| RenderError::Format(e.to_string()))?;

                // Col 4: Complete %
                let complete_str = format!("{}%", progress.percent_complete);
                sheet
                    .write_with_format(row, 4, &complete_str, text_fmt)
                    .map_err(|e| RenderError::Format(e.to_string()))?;

                // Col 5: Remaining
                let remaining_str = format!("{:.0}d", progress.remaining_days);
                sheet
                    .write_with_format(row, 5, &remaining_str, text_fmt)
                    .map_err(|e| RenderError::Format(e.to_string()))?;

                // Col 6: Variance (positive = ahead, negative = behind)
                let variance_str = if progress.variance_days >= 0 {
                    format!("+{}d", progress.variance_days)
                } else {
                    format!("{}d", progress.variance_days)
                };
                sheet
                    .write_with_format(row, 6, &variance_str, text_fmt)
                    .map_err(|e| RenderError::Format(e.to_string()))?;

                // Col 7: Actual Start
                let actual_start_str = progress
                    .actual_start
                    .map(|d| d.format("%Y-%m-%d").to_string())
                    .unwrap_or_default();
                sheet
                    .write_with_format(row, 7, &actual_start_str, text_fmt)
                    .map_err(|e| RenderError::Format(e.to_string()))?;

                // Col 8: Actual End
                let actual_end_str = progress
                    .actual_finish
                    .map(|d| d.format("%Y-%m-%d").to_string())
                    .unwrap_or_default();
                sheet
                    .write_with_format(row, 8, &actual_end_str, text_fmt)
                    .map_err(|e| RenderError::Format(e.to_string()))?;
            } else {
                // Columns/Visual mode: Complete, Remaining, Act.Start, Act.End
                // Col 3: Complete %
                let complete_str = format!("{}%", progress.percent_complete);
                sheet
                    .write_with_format(row, 3, &complete_str, text_fmt)
                    .map_err(|e| RenderError::Format(e.to_string()))?;

                // Col 4: Remaining
                let remaining_str = format!("{:.0}d", progress.remaining_days);
                sheet
                    .write_with_format(row, 4, &remaining_str, text_fmt)
                    .map_err(|e| RenderError::Format(e.to_string()))?;

                // Col 5: Actual Start
                let actual_start_str = progress
                    .actual_start
                    .map(|d| d.format("%Y-%m-%d").to_string())
                    .unwrap_or_default();
                sheet
                    .write_with_format(row, 5, &actual_start_str, text_fmt)
                    .map_err(|e| RenderError::Format(e.to_string()))?;

                // Col 6: Actual End
                let actual_end_str = progress
                    .actual_finish
                    .map(|d| d.format("%Y-%m-%d").to_string())
                    .unwrap_or_default();
                sheet
                    .write_with_format(row, 6, &actual_end_str, text_fmt)
                    .map_err(|e| RenderError::Format(e.to_string()))?;
            }
        }

        // Profile column (offset by progress columns)
        let profile_col = 3 + progress_offset;
        sheet
            .write_with_format(row, profile_col, profile, text_fmt)
            .map_err(|e| RenderError::Format(e.to_string()))?;

        // pd (effort)
        sheet
            .write_with_format(row, effort_col, person_days, number_fmt)
            .map_err(|e| RenderError::Format(e.to_string()))?;

        // Start week
        sheet
            .write_with_format(row, start_col, start_week as f64, number_fmt)
            .map_err(|e| RenderError::Format(e.to_string()))?;

        // End week
        sheet
            .write_with_format(row, end_col, end_week as f64, number_fmt)
            .map_err(|e| RenderError::Format(e.to_string()))?;

        // Week columns (M column is at index 2 for simple layout)
        let milestone_col = 2u16;
        self.write_week_columns(
            sheet,
            row,
            start_week,
            end_week,
            is_critical,
            is_milestone,
            is_container,
            is_odd,
            formats,
            week_start_col,
            milestone_col,
            effort_col,
            start_col,
            end_col,
            person_days,
            schedule_weeks,
            project_start,
            progress_data,
        )?;

        Ok(())
    }

    /// Write a schedule row with dependency formulas
    #[allow(clippy::too_many_arguments)]
    fn write_schedule_row_with_deps(
        &self,
        sheet: &mut Worksheet,
        row: u32,
        task_id: &str,
        task_name: &str,
        level: usize,
        profile: &str,
        predecessor: &str,
        dep_type: &str,
        lag: i32,
        person_days: f64,
        start_week: u32,
        end_week: u32,
        is_critical: bool,
        is_milestone: bool,
        is_container: bool,
        formats: &ExcelFormats,
        week_start_col: u16,
        effort_col: u16,
        start_col: u16,
        end_col: u16,
        last_data_row: u32,
        is_odd: bool,
        schedule_weeks: u32,
        progress_data: Option<&ProgressData>,
        project_start: NaiveDate,
    ) -> Result<(), RenderError> {
        let excel_row = row + 1; // Excel is 1-indexed
        let progress_offset = self.progress_column_count();

        // Select formats: milestones use gold, otherwise alternate white/blue per task
        let (text_fmt, number_fmt) = if is_milestone {
            (&formats.milestone_text, &formats.milestone_number)
        } else if is_odd {
            (&formats.row_odd_text, &formats.row_odd_number)
        } else {
            (&formats.row_even_text, &formats.row_even_number)
        };

        // Container tasks use bold text for Activity to distinguish phases
        let activity_fmt = if is_container {
            if is_odd {
                &formats.container_odd_text
            } else {
                &formats.container_even_text
            }
        } else {
            text_fmt
        };

        // Col A: Task ID
        sheet
            .write_with_format(row, 0, task_id, text_fmt)
            .map_err(|e| RenderError::Format(e.to_string()))?;

        // Col B: Activity (bold for containers)
        sheet
            .write_with_format(row, 1, task_name, activity_fmt)
            .map_err(|e| RenderError::Format(e.to_string()))?;

        // Col C: Lvl (nesting level for hierarchy filtering/grouping)
        sheet
            .write_with_format(row, 2, level as f64, number_fmt)
            .map_err(|e| RenderError::Format(e.to_string()))?;

        // Col D: Milestone marker (◆ for milestones, empty otherwise)
        let milestone_marker = if is_milestone { "◆" } else { "" };
        sheet
            .write_with_format(row, 3, milestone_marker, text_fmt)
            .map_err(|e| RenderError::Format(e.to_string()))?;

        // Progress columns (RFC-0018) - after M column
        if let Some(progress) = progress_data {
            if self.progress_mode == ProgressMode::Full {
                // Full mode: Status, Complete, Remaining, Variance, Act.Start, Act.End
                // Col 4: Status icon
                sheet
                    .write_with_format(row, 4, progress.status.icon(), text_fmt)
                    .map_err(|e| RenderError::Format(e.to_string()))?;

                // Col 5: Complete %
                let complete_str = format!("{}%", progress.percent_complete);
                sheet
                    .write_with_format(row, 5, &complete_str, text_fmt)
                    .map_err(|e| RenderError::Format(e.to_string()))?;

                // Col 6: Remaining
                let remaining_str = format!("{:.0}d", progress.remaining_days);
                sheet
                    .write_with_format(row, 6, &remaining_str, text_fmt)
                    .map_err(|e| RenderError::Format(e.to_string()))?;

                // Col 7: Variance (positive = ahead, negative = behind)
                let variance_str = if progress.variance_days >= 0 {
                    format!("+{}d", progress.variance_days)
                } else {
                    format!("{}d", progress.variance_days)
                };
                sheet
                    .write_with_format(row, 7, &variance_str, text_fmt)
                    .map_err(|e| RenderError::Format(e.to_string()))?;

                // Col 8: Actual Start
                let actual_start_str = progress
                    .actual_start
                    .map(|d| d.format("%Y-%m-%d").to_string())
                    .unwrap_or_default();
                sheet
                    .write_with_format(row, 8, &actual_start_str, text_fmt)
                    .map_err(|e| RenderError::Format(e.to_string()))?;

                // Col 9: Actual End
                let actual_end_str = progress
                    .actual_finish
                    .map(|d| d.format("%Y-%m-%d").to_string())
                    .unwrap_or_default();
                sheet
                    .write_with_format(row, 9, &actual_end_str, text_fmt)
                    .map_err(|e| RenderError::Format(e.to_string()))?;
            } else {
                // Columns/Visual mode: Complete, Remaining, Act.Start, Act.End
                // Col 4: Complete %
                let complete_str = format!("{}%", progress.percent_complete);
                sheet
                    .write_with_format(row, 4, &complete_str, text_fmt)
                    .map_err(|e| RenderError::Format(e.to_string()))?;

                // Col 5: Remaining
                let remaining_str = format!("{:.0}d", progress.remaining_days);
                sheet
                    .write_with_format(row, 5, &remaining_str, text_fmt)
                    .map_err(|e| RenderError::Format(e.to_string()))?;

                // Col 6: Actual Start
                let actual_start_str = progress
                    .actual_start
                    .map(|d| d.format("%Y-%m-%d").to_string())
                    .unwrap_or_default();
                sheet
                    .write_with_format(row, 6, &actual_start_str, text_fmt)
                    .map_err(|e| RenderError::Format(e.to_string()))?;

                // Col 7: Actual End
                let actual_end_str = progress
                    .actual_finish
                    .map(|d| d.format("%Y-%m-%d").to_string())
                    .unwrap_or_default();
                sheet
                    .write_with_format(row, 7, &actual_end_str, text_fmt)
                    .map_err(|e| RenderError::Format(e.to_string()))?;
            }
        }

        // Calculate column positions with progress offset
        let profile_col = 4 + progress_offset;
        let depends_col = 5 + progress_offset;
        let type_col = 6 + progress_offset;
        let lag_col = 7 + progress_offset;

        // Profile
        sheet
            .write_with_format(row, profile_col, profile, text_fmt)
            .map_err(|e| RenderError::Format(e.to_string()))?;

        // Depends On
        sheet
            .write_with_format(row, depends_col, predecessor, text_fmt)
            .map_err(|e| RenderError::Format(e.to_string()))?;

        // Type (FS/SS/FF/SF)
        let dep_type_val = if predecessor.is_empty() { "" } else { dep_type };
        sheet
            .write_with_format(row, type_col, dep_type_val, text_fmt)
            .map_err(|e| RenderError::Format(e.to_string()))?;

        // Lag
        if !predecessor.is_empty() {
            sheet
                .write_with_format(row, lag_col, lag as f64, number_fmt)
                .map_err(|e| RenderError::Format(e.to_string()))?;
        } else {
            sheet
                .write_with_format(row, lag_col, "", text_fmt)
                .map_err(|e| RenderError::Format(e.to_string()))?;
        }

        // Effort (pd)
        sheet
            .write_with_format(row, effort_col, person_days, number_fmt)
            .map_err(|e| RenderError::Format(e.to_string()))?;

        // Start Week - Formula-driven if has predecessor
        // Use dynamic column letters based on progress offset
        let depends_letter = Self::col_to_letter(depends_col);
        let type_letter = Self::col_to_letter(type_col);
        let lag_letter = Self::col_to_letter(lag_col);
        let effort_letter = Self::col_to_letter(effort_col);
        let end_col_num = end_col + 1; // 1-indexed for VLOOKUP
        let start_col_num = start_col + 1;
        let end_letter = Self::col_to_letter(end_col);

        if self.use_formulas && !predecessor.is_empty() {
            let formula = format!(
                "=IF({d}{}=\"\",{},IF({t}{}=\"FS\",VLOOKUP({d}{},$A$2:${e}${},{},0)+1+{l}{},\
                IF({t}{}=\"SS\",VLOOKUP({d}{},$A$2:${e}${},{},0)+{l}{},\
                IF({t}{}=\"FF\",VLOOKUP({d}{},$A$2:${e}${},{},0)-CEILING({ef}{}*{}/{},1)+1+{l}{},\
                IF({t}{}=\"SF\",VLOOKUP({d}{},$A$2:${e}${},{},0)-CEILING({ef}{}*{}/{},1)+1+{l}{},\
                {})))))",
                excel_row,
                start_week,
                excel_row,
                excel_row,
                last_data_row,
                end_col_num,
                excel_row,
                excel_row,
                excel_row,
                last_data_row,
                start_col_num,
                excel_row,
                excel_row,
                excel_row,
                last_data_row,
                end_col_num,
                excel_row,
                self.hours_per_day,
                self.hours_per_week,
                excel_row,
                excel_row,
                excel_row,
                last_data_row,
                start_col_num,
                excel_row,
                self.hours_per_day,
                self.hours_per_week,
                excel_row,
                start_week,
                d = depends_letter,
                t = type_letter,
                l = lag_letter,
                ef = effort_letter,
                e = end_letter,
            );
            sheet
                .write_formula_with_format(row, start_col, formula.as_str(), number_fmt)
                .map_err(|e| RenderError::Format(e.to_string()))?;
        } else {
            sheet
                .write_with_format(row, start_col, start_week as f64, number_fmt)
                .map_err(|e| RenderError::Format(e.to_string()))?;
        }

        // End Week - Formula: Start + CEILING(effort * hours_per_day / hours_per_week) - 1
        if self.use_formulas {
            let start_col_letter = Self::col_to_letter(start_col);
            let formula = format!(
                "={}{}+MAX(CEILING({}{}*{}/{},1)-1,0)",
                start_col_letter,
                excel_row,
                effort_letter,
                excel_row,
                self.hours_per_day,
                self.hours_per_week
            );
            sheet
                .write_formula_with_format(row, end_col, formula.as_str(), number_fmt)
                .map_err(|e| RenderError::Format(e.to_string()))?;
        } else {
            sheet
                .write_with_format(row, end_col, end_week as f64, number_fmt)
                .map_err(|e| RenderError::Format(e.to_string()))?;
        }

        // Week columns (M column is at index 3 for deps layout with Lvl)
        let milestone_col = 3u16;
        self.write_week_columns(
            sheet,
            row,
            start_week,
            end_week,
            is_critical,
            is_milestone,
            is_container,
            is_odd,
            formats,
            week_start_col,
            milestone_col,
            effort_col,
            start_col,
            end_col,
            person_days,
            schedule_weeks,
            project_start,
            progress_data,
        )?;

        Ok(())
    }

    /// Write week columns with Gantt bar formulas
    ///
    /// Formula-driven rendering for what-if analysis:
    /// - Milestones: derived from M column ("◆"), shows "◆" in milestone week, "" elsewhere
    /// - Tasks: shows hours when > 0, "" when zero (no hidden zeros)
    /// - Container tasks: always empty (no effort to display)
    ///
    /// Visual mode (RFC-0018): Uses progress-aware formatting:
    /// - Completed weeks: green fill
    /// - Remaining weeks (behind schedule): red fill
    /// - Remaining weeks (on schedule): blue fill
    #[allow(clippy::too_many_arguments)]
    fn write_week_columns(
        &self,
        sheet: &mut Worksheet,
        row: u32,
        start_week: u32,
        end_week: u32,
        _is_critical: bool, // Reserved for future conditional formatting of critical path
        is_milestone: bool,
        is_container: bool,
        is_odd: bool,
        formats: &ExcelFormats,
        week_start_col: u16,
        milestone_col: u16, // M column position for formula reference
        effort_col: u16,
        start_col: u16,
        end_col: u16,
        person_days: f64,
        schedule_weeks: u32,
        project_start: NaiveDate,
        progress_data: Option<&ProgressData>,
    ) -> Result<(), RenderError> {
        let excel_row = row + 1;
        let weeks_span = (end_week.saturating_sub(start_week) + 1).max(1);
        let hours_per_week_val = (person_days * self.hours_per_day) / weeks_span as f64;

        let milestone_col_letter = Self::col_to_letter(milestone_col);
        let effort_col_letter = Self::col_to_letter(effort_col);
        let start_col_letter = Self::col_to_letter(start_col);
        let end_col_letter = Self::col_to_letter(end_col);

        // Get status date for progress calculation
        let status_date = self
            .status_date
            .unwrap_or_else(|| chrono::Local::now().date_naive());

        // Check if we should use visual progress rendering
        let use_visual_progress = matches!(
            self.progress_mode,
            ProgressMode::Visual | ProgressMode::Full
        ) && progress_data.is_some()
            && !is_milestone
            && !is_container;

        // Select format based on milestone status and row alternation
        // Milestones get gold background, others get alternating white/blue
        let cell_fmt = if is_milestone {
            &formats.milestone_week
        } else if is_odd {
            &formats.gantt_odd_empty
        } else {
            &formats.gantt_even_empty
        };

        for week in 1..=schedule_weeks {
            let col = week_start_col + (week - 1) as u16;
            let in_range = week >= start_week && week <= end_week;
            let col_letter = Self::col_to_letter(col);

            // Container tasks: no Gantt bar (effort is 0, dates are derived from children)
            if is_container {
                sheet
                    .write_with_format(row, col, "", cell_fmt)
                    .map_err(|e| RenderError::Format(e.to_string()))?;
                continue;
            }

            // Visual progress mode: use progress-aware formatting
            if use_visual_progress && in_range {
                let progress = progress_data.unwrap();
                let percent = progress.percent_complete as f64;

                // Calculate the week's date range
                let week_start_date =
                    project_start + chrono::Duration::days(((week - 1) * 7) as i64);

                // Determine if this week is complete, remaining-behind, or remaining-ontrack
                // Simple linear progress model: if we're X% complete, assume first X% of weeks are done
                let week_position_pct = ((week - start_week) as f64 / weeks_span as f64) * 100.0;
                let is_completed_week = week_position_pct < percent;
                let is_behind = week_start_date <= status_date && !is_completed_week;

                let fmt = if is_completed_week {
                    &formats.progress_complete
                } else if is_behind {
                    &formats.progress_behind
                } else {
                    &formats.progress_remaining
                };

                let hours = hours_per_week_val.round();
                if hours > 0.0 {
                    sheet
                        .write_with_format(row, col, hours, fmt)
                        .map_err(|e| RenderError::Format(e.to_string()))?;
                } else {
                    sheet
                        .write_with_format(row, col, "", fmt)
                        .map_err(|e| RenderError::Format(e.to_string()))?;
                }
            } else if self.use_formulas && !use_visual_progress {
                // Unified formula checking M column for milestone status:
                // =IF($M2="◆",
                //     IF(AND(week>=Start, week<=End), "◆", ""),
                //     IF(AND(week>=Start, week<=End, hours>0), hours, ""))
                //
                // - Milestones: "◆" if in range, "" otherwise
                // - Tasks: hours if in range AND > 0, "" otherwise
                let hours_formula = format!(
                    "({}{}*{})/(${}{}-${}{}+1)",
                    effort_col_letter,
                    excel_row,
                    self.hours_per_day,
                    end_col_letter,
                    excel_row,
                    start_col_letter,
                    excel_row
                );
                let in_range_condition = format!(
                    "{}$1>=${}{},{}$1<=${}{}",
                    col_letter, start_col_letter, excel_row, col_letter, end_col_letter, excel_row
                );
                let formula = format!(
                    "=IF(${}{}=\"◆\",\
                        IF(AND({}),\"◆\",\"\"),\
                        IF(AND({},{}>0),ROUND({},0),\"\"))",
                    milestone_col_letter,
                    excel_row,
                    in_range_condition,
                    in_range_condition,
                    hours_formula,
                    hours_formula
                );
                sheet
                    .write_formula_with_format(row, col, formula.as_str(), cell_fmt)
                    .map_err(|e| RenderError::Format(e.to_string()))?;
            } else {
                // Static mode: compute value directly
                if is_milestone {
                    let value = if in_range { "◆" } else { "" };
                    sheet
                        .write_with_format(row, col, value, cell_fmt)
                        .map_err(|e| RenderError::Format(e.to_string()))?;
                } else {
                    let hours = if in_range {
                        hours_per_week_val.round()
                    } else {
                        0.0
                    };
                    if hours > 0.0 {
                        sheet
                            .write_with_format(row, col, hours, cell_fmt)
                            .map_err(|e| RenderError::Format(e.to_string()))?;
                    } else {
                        sheet
                            .write_with_format(row, col, "", cell_fmt)
                            .map_err(|e| RenderError::Format(e.to_string()))?;
                    }
                }
            }
        }

        Ok(())
    }

    /// Write total row for schedule
    fn write_schedule_totals(
        &self,
        sheet: &mut Worksheet,
        row: u32,
        week_start_col: u16,
        effort_col: u16,
        formats: &ExcelFormats,
        schedule_weeks: u32, // Effective weeks (auto-fit applied)
    ) -> Result<(), RenderError> {
        if row <= 1 {
            return Ok(());
        }

        // Write TOTAL label in first column
        sheet
            .write_with_format(row, 0, "TOTAL", &formats.total_row)
            .map_err(|e| RenderError::Format(e.to_string()))?;

        // Fill empty cells up to effort column
        for col_idx in 1..effort_col {
            sheet
                .write_with_format(row, col_idx, "", &formats.total_row)
                .map_err(|e| RenderError::Format(e.to_string()))?;
        }

        // SUM formula for effort (pd) column
        if self.use_formulas {
            let effort_letter = Self::col_to_letter(effort_col);
            let formula = format!("=SUM({}2:{}{})", effort_letter, effort_letter, row);
            sheet
                .write_formula_with_format(row, effort_col, formula.as_str(), &formats.total_row)
                .map_err(|e| RenderError::Format(e.to_string()))?;
        } else {
            sheet
                .write_with_format(row, effort_col, 0.0, &formats.total_row)
                .map_err(|e| RenderError::Format(e.to_string()))?;
        }

        // Fill empty cells from effort+1 to week columns
        for col_idx in (effort_col + 1)..week_start_col {
            sheet
                .write_with_format(row, col_idx, "", &formats.total_row)
                .map_err(|e| RenderError::Format(e.to_string()))?;
        }

        // Sum formulas for each week column
        for week in 0..schedule_weeks {
            let week_col = week_start_col + week as u16;
            if self.use_formulas {
                let col_letter = Self::col_to_letter(week_col);
                let formula = format!("=SUM({}2:{}{})", col_letter, col_letter, row);
                sheet
                    .write_formula_with_format(row, week_col, formula.as_str(), &formats.total_row)
                    .map_err(|e| RenderError::Format(e.to_string()))?;
            } else {
                sheet
                    .write_with_format(row, week_col, 0.0, &formats.total_row)
                    .map_err(|e| RenderError::Format(e.to_string()))?;
            }
        }

        Ok(())
    }

    /// Add Executive Summary sheet
    fn add_executive_summary(
        &self,
        workbook: &mut Workbook,
        project: &Project,
        schedule: &Schedule,
        formats: &ExcelFormats,
        resource_rates: &HashMap<String, f64>,
    ) -> Result<(), RenderError> {
        let sheet = workbook.add_worksheet();
        sheet
            .set_name("Executive Summary")
            .map_err(|e| RenderError::Format(e.to_string()))?;

        // Project info section
        sheet
            .write_with_format(0, 0, "PROJECT SUMMARY", &formats.header)
            .map_err(|e| RenderError::Format(e.to_string()))?;
        sheet
            .merge_range(0, 0, 0, 1, "PROJECT SUMMARY", &formats.header)
            .ok();

        sheet
            .write_with_format(2, 0, "Project Name:", &formats.text)
            .map_err(|e| RenderError::Format(e.to_string()))?;
        sheet
            .write_with_format(2, 1, &project.name, &formats.text)
            .map_err(|e| RenderError::Format(e.to_string()))?;

        sheet
            .write_with_format(3, 0, "Start Date:", &formats.text)
            .map_err(|e| RenderError::Format(e.to_string()))?;
        sheet
            .write_with_format(
                3,
                1,
                project.start.format("%Y-%m-%d").to_string(),
                &formats.text,
            )
            .map_err(|e| RenderError::Format(e.to_string()))?;

        sheet
            .write_with_format(4, 0, "End Date:", &formats.text)
            .map_err(|e| RenderError::Format(e.to_string()))?;
        sheet
            .write_with_format(
                4,
                1,
                schedule.project_end.format("%Y-%m-%d").to_string(),
                &formats.text,
            )
            .map_err(|e| RenderError::Format(e.to_string()))?;

        sheet
            .write_with_format(5, 0, "Duration (days):", &formats.text)
            .map_err(|e| RenderError::Format(e.to_string()))?;
        sheet
            .write_with_format(5, 1, schedule.project_duration.as_days(), &formats.number)
            .map_err(|e| RenderError::Format(e.to_string()))?;

        sheet
            .write_with_format(6, 0, "Total Tasks:", &formats.text)
            .map_err(|e| RenderError::Format(e.to_string()))?;
        sheet
            .write_with_format(6, 1, schedule.tasks.len() as f64, &formats.integer)
            .map_err(|e| RenderError::Format(e.to_string()))?;

        sheet
            .write_with_format(7, 0, "Critical Tasks:", &formats.text)
            .map_err(|e| RenderError::Format(e.to_string()))?;
        sheet
            .write_with_format(7, 1, schedule.critical_path.len() as f64, &formats.integer)
            .map_err(|e| RenderError::Format(e.to_string()))?;

        // Cost summary
        sheet
            .write_with_format(9, 0, "COST SUMMARY", &formats.header)
            .map_err(|e| RenderError::Format(e.to_string()))?;
        sheet
            .merge_range(9, 0, 9, 1, "COST SUMMARY", &formats.header)
            .ok();

        // Calculate totals
        // Use explicit effort_days if available, otherwise calculate from duration × units
        let mut resource_effort: HashMap<String, f64> = HashMap::new();
        for scheduled in schedule.tasks.values() {
            for assignment in &scheduled.assignments {
                let effort = if let Some(effort_days) = assignment.effort_days {
                    effort_days
                } else {
                    let assignment_days = (assignment.finish - assignment.start).num_days() as f64;
                    assignment_days * assignment.units as f64
                };
                *resource_effort
                    .entry(assignment.resource_id.clone())
                    .or_default() += effort;
            }
        }

        let total_effort: f64 = resource_effort.values().sum();
        let total_cost: f64 = resource_effort
            .iter()
            .map(|(id, effort)| {
                resource_rates.get(id).copied().unwrap_or(self.default_rate) * effort
            })
            .sum();

        sheet
            .write_with_format(11, 0, "Total Effort (pd):", &formats.text)
            .map_err(|e| RenderError::Format(e.to_string()))?;
        sheet
            .write_with_format(11, 1, total_effort, &formats.number)
            .map_err(|e| RenderError::Format(e.to_string()))?;

        sheet
            .write_with_format(
                12,
                0,
                &format!("Total Cost ({}):", self.currency),
                &formats.text,
            )
            .map_err(|e| RenderError::Format(e.to_string()))?;
        sheet
            .write_with_format(12, 1, total_cost, &formats.currency)
            .map_err(|e| RenderError::Format(e.to_string()))?;

        // Column widths
        sheet.set_column_width(0, 20).ok();
        sheet.set_column_width(1, 25).ok();

        Ok(())
    }

    /// Add Calendar Analysis sheet showing weekend/holiday impact per task
    fn add_calendar_analysis_sheet(
        &self,
        workbook: &mut Workbook,
        project: &Project,
        schedule: &Schedule,
        formats: &ExcelFormats,
    ) -> Result<(), RenderError> {
        let sheet = workbook.add_worksheet();
        sheet
            .set_name("Calendar Analysis")
            .map_err(|e| RenderError::Format(e.to_string()))?;

        // Headers
        let headers = [
            "Task ID",
            "Task Name",
            "Calendar",
            "Duration (days)",
            "Working Days",
            "Weekends",
            "Holidays",
            "Non-Working %",
            "Diagnostics",
        ];

        for (col, header) in headers.iter().enumerate() {
            sheet
                .write_with_format(0, col as u16, *header, &formats.header)
                .map_err(|e| RenderError::Format(e.to_string()))?;
        }

        // Column widths
        sheet.set_column_width(0, 15).ok(); // Task ID
        sheet.set_column_width(1, 25).ok(); // Task Name
        sheet.set_column_width(2, 12).ok(); // Calendar
        sheet.set_column_width(3, 12).ok(); // Duration
        sheet.set_column_width(4, 12).ok(); // Working Days
        sheet.set_column_width(5, 10).ok(); // Weekends
        sheet.set_column_width(6, 10).ok(); // Holidays
        sheet.set_column_width(7, 12).ok(); // Non-Working %
        sheet.set_column_width(8, 30).ok(); // Diagnostics

        // Get project calendar for fallback
        let project_calendar = project
            .calendars
            .iter()
            .find(|c| c.id == project.calendar)
            .cloned()
            .unwrap_or_else(Calendar::default);

        // Collect tasks in WBS order
        let wbs_order = Self::collect_wbs_order(&project.tasks, 0);

        let mut row = 1u32;
        for (task_path, _level) in &wbs_order {
            // Get task info from schedule
            if let Some(scheduled) = schedule.tasks.get(task_path) {
                let simple_id = task_path.rsplit('.').next().unwrap_or(task_path);
                let task = project.get_task(simple_id);
                let task_name = task.map(|t| t.name.as_str()).unwrap_or(simple_id);

                // Get the calendar for this task (use project calendar as fallback)
                let calendar = &project_calendar;

                // Calculate calendar impact
                let (working_days, weekend_days, holiday_days) =
                    self.calculate_calendar_impact_for_task(scheduled, calendar);

                let total_span = (scheduled.finish - scheduled.start).num_days().max(1) as f64;
                let non_working_pct = ((weekend_days + holiday_days) as f64 / total_span) * 100.0;

                // Get diagnostics for this task
                let task_diags = self.filter_task_diagnostics(task_path);
                let diag_str = task_diags
                    .iter()
                    .map(|d| d.as_str())
                    .collect::<Vec<_>>()
                    .join(", ");

                // Write row
                sheet
                    .write_with_format(row, 0, task_path, &formats.text)
                    .map_err(|e| RenderError::Format(e.to_string()))?;
                sheet
                    .write_with_format(row, 1, task_name, &formats.text)
                    .map_err(|e| RenderError::Format(e.to_string()))?;
                sheet
                    .write_with_format(row, 2, &calendar.id, &formats.text)
                    .map_err(|e| RenderError::Format(e.to_string()))?;
                sheet
                    .write_with_format(row, 3, scheduled.duration.as_days(), &formats.number)
                    .map_err(|e| RenderError::Format(e.to_string()))?;
                sheet
                    .write_with_format(row, 4, working_days as f64, &formats.integer)
                    .map_err(|e| RenderError::Format(e.to_string()))?;
                sheet
                    .write_with_format(row, 5, weekend_days as f64, &formats.integer)
                    .map_err(|e| RenderError::Format(e.to_string()))?;
                sheet
                    .write_with_format(row, 6, holiday_days as f64, &formats.integer)
                    .map_err(|e| RenderError::Format(e.to_string()))?;

                // Non-working percentage with conditional formatting color
                let pct_format = if non_working_pct > 30.0 {
                    Format::new()
                        .set_num_format("0.0%")
                        .set_background_color(0xFFCCCC)
                        .set_border(FormatBorder::Thin)
                } else if non_working_pct > 15.0 {
                    Format::new()
                        .set_num_format("0.0%")
                        .set_background_color(0xFFFFCC)
                        .set_border(FormatBorder::Thin)
                } else {
                    Format::new()
                        .set_num_format("0.0%")
                        .set_background_color(0xCCFFCC)
                        .set_border(FormatBorder::Thin)
                };
                sheet
                    .write_with_format(row, 7, non_working_pct / 100.0, &pct_format)
                    .map_err(|e| RenderError::Format(e.to_string()))?;

                // Diagnostics column
                let diag_format = if !task_diags.is_empty() {
                    Format::new()
                        .set_background_color(0xFFEEDD)
                        .set_border(FormatBorder::Thin)
                } else {
                    formats.text.clone()
                };
                sheet
                    .write_with_format(row, 8, &diag_str, &diag_format)
                    .map_err(|e| RenderError::Format(e.to_string()))?;

                row += 1;
            }
        }

        // Freeze header row
        sheet.set_freeze_panes(1, 0).ok();

        Ok(())
    }

    /// Calculate calendar impact for a scheduled task
    fn calculate_calendar_impact_for_task(
        &self,
        scheduled: &ScheduledTask,
        calendar: &Calendar,
    ) -> (u32, u32, u32) {
        let mut working_days = 0u32;
        let mut weekend_days = 0u32;
        let mut holiday_days = 0u32;

        let mut current = scheduled.start;
        while current <= scheduled.finish {
            let weekday = current.weekday().num_days_from_sunday() as u8;

            // Check if it's a holiday
            let is_holiday = calendar
                .holidays
                .iter()
                .any(|h| current >= h.start && current <= h.end);

            if is_holiday {
                holiday_days += 1;
            } else if !calendar.working_days.contains(&weekday) {
                weekend_days += 1;
            } else {
                working_days += 1;
            }

            current = current.succ_opt().unwrap_or(current);
            if current == scheduled.finish && current == scheduled.start {
                break; // Avoid infinite loop for zero-duration tasks
            }
        }

        (working_days, weekend_days, holiday_days)
    }

    /// Filter diagnostics relevant to a specific task
    fn filter_task_diagnostics(&self, task_id: &str) -> Vec<DiagnosticCode> {
        self.diagnostics
            .iter()
            .filter(|d| Self::is_diagnostic_for_task(d, task_id))
            .map(|d| d.code.clone())
            .collect()
    }

    /// Check if a diagnostic is relevant to a specific task
    fn is_diagnostic_for_task(diagnostic: &Diagnostic, task_id: &str) -> bool {
        let quoted_id = format!("'{}'", task_id);
        match diagnostic.code {
            DiagnosticCode::C010NonWorkingDay | DiagnosticCode::C011CalendarMismatch => {
                diagnostic.message.contains(&quoted_id)
            }
            DiagnosticCode::H004TaskUnconstrained => diagnostic.message.contains(&quoted_id),
            DiagnosticCode::W001AbstractAssignment | DiagnosticCode::H001MixedAbstraction => {
                diagnostic.message.contains(&quoted_id)
            }
            DiagnosticCode::W014ContainerDependency => diagnostic.message.contains(&quoted_id),
            _ => false,
        }
    }

    /// Add Diagnostics sheet with all project diagnostics
    fn add_diagnostics_sheet(
        &self,
        workbook: &mut Workbook,
        _project: &Project,
        formats: &ExcelFormats,
    ) -> Result<(), RenderError> {
        let sheet = workbook.add_worksheet();
        sheet
            .set_name("Diagnostics")
            .map_err(|e| RenderError::Format(e.to_string()))?;

        // Headers
        let headers = ["Code", "Severity", "Message", "Hint"];
        for (col, header) in headers.iter().enumerate() {
            sheet
                .write_with_format(0, col as u16, *header, &formats.header)
                .map_err(|e| RenderError::Format(e.to_string()))?;
        }

        // Column widths
        sheet.set_column_width(0, 8).ok(); // Code
        sheet.set_column_width(1, 10).ok(); // Severity
        sheet.set_column_width(2, 60).ok(); // Message
        sheet.set_column_width(3, 40).ok(); // Hint

        // Sort diagnostics by severity (Error first, then Warning, Hint, Info)
        let mut sorted_diags: Vec<&Diagnostic> = self.diagnostics.iter().collect();
        sorted_diags.sort_by_key(|d| match d.severity {
            Severity::Error => 0,
            Severity::Warning => 1,
            Severity::Hint => 2,
            Severity::Info => 3,
        });

        // Write diagnostic rows
        for (i, diag) in sorted_diags.iter().enumerate() {
            let row = (i + 1) as u32;

            // Color code by severity
            let severity_format = match diag.severity {
                Severity::Error => Format::new()
                    .set_background_color(0xFFCCCC)
                    .set_border(FormatBorder::Thin),
                Severity::Warning => Format::new()
                    .set_background_color(0xFFFFCC)
                    .set_border(FormatBorder::Thin),
                Severity::Hint => Format::new()
                    .set_background_color(0xCCFFFF)
                    .set_border(FormatBorder::Thin),
                Severity::Info => Format::new()
                    .set_background_color(0xCCCCFF)
                    .set_border(FormatBorder::Thin),
            };

            let severity_str = match diag.severity {
                Severity::Error => "Error",
                Severity::Warning => "Warning",
                Severity::Hint => "Hint",
                Severity::Info => "Info",
            };

            sheet
                .write_with_format(row, 0, diag.code.as_str(), &severity_format)
                .map_err(|e| RenderError::Format(e.to_string()))?;
            sheet
                .write_with_format(row, 1, severity_str, &severity_format)
                .map_err(|e| RenderError::Format(e.to_string()))?;
            sheet
                .write_with_format(row, 2, &diag.message, &formats.text)
                .map_err(|e| RenderError::Format(e.to_string()))?;
            let hint_str = diag.hints.first().map(|s| s.as_str()).unwrap_or("");
            sheet
                .write_with_format(row, 3, hint_str, &formats.text)
                .map_err(|e| RenderError::Format(e.to_string()))?;
        }

        // Add summary section at the bottom
        let summary_row = (sorted_diags.len() + 3) as u32;
        sheet
            .write_with_format(summary_row, 0, "SUMMARY", &formats.header)
            .map_err(|e| RenderError::Format(e.to_string()))?;
        sheet
            .merge_range(summary_row, 0, summary_row, 1, "SUMMARY", &formats.header)
            .ok();

        let error_count = self
            .diagnostics
            .iter()
            .filter(|d| matches!(d.severity, Severity::Error))
            .count();
        let warning_count = self
            .diagnostics
            .iter()
            .filter(|d| matches!(d.severity, Severity::Warning))
            .count();
        let hint_count = self
            .diagnostics
            .iter()
            .filter(|d| matches!(d.severity, Severity::Hint))
            .count();
        let calendar_count = self
            .diagnostics
            .iter()
            .filter(|d| d.code.as_str().starts_with("C"))
            .count();

        sheet
            .write_with_format(summary_row + 1, 0, "Errors:", &formats.text)
            .map_err(|e| RenderError::Format(e.to_string()))?;
        sheet
            .write_with_format(summary_row + 1, 1, error_count as f64, &formats.integer)
            .map_err(|e| RenderError::Format(e.to_string()))?;

        sheet
            .write_with_format(summary_row + 2, 0, "Warnings:", &formats.text)
            .map_err(|e| RenderError::Format(e.to_string()))?;
        sheet
            .write_with_format(summary_row + 2, 1, warning_count as f64, &formats.integer)
            .map_err(|e| RenderError::Format(e.to_string()))?;

        sheet
            .write_with_format(summary_row + 3, 0, "Hints:", &formats.text)
            .map_err(|e| RenderError::Format(e.to_string()))?;
        sheet
            .write_with_format(summary_row + 3, 1, hint_count as f64, &formats.integer)
            .map_err(|e| RenderError::Format(e.to_string()))?;

        sheet
            .write_with_format(summary_row + 4, 0, "Calendar Issues:", &formats.text)
            .map_err(|e| RenderError::Format(e.to_string()))?;
        sheet
            .write_with_format(summary_row + 4, 1, calendar_count as f64, &formats.integer)
            .map_err(|e| RenderError::Format(e.to_string()))?;

        // Freeze header row
        sheet.set_freeze_panes(1, 0).ok();

        Ok(())
    }

    /// Convert date to week number relative to project start
    fn date_to_week(&self, date: NaiveDate, project_start: NaiveDate) -> u32 {
        let days = (date - project_start).num_days().max(0) as u32;
        (days / 7) + 1
    }

    /// Convert column number to Excel letter (0 -> A, 25 -> Z, 26 -> AA)
    fn col_to_letter(col: u16) -> String {
        let mut result = String::new();
        let mut n = col as u32;
        loop {
            result.insert(0, (b'A' + (n % 26) as u8) as char);
            if n < 26 {
                break;
            }
            n = n / 26 - 1;
        }
        result
    }

    /// Collect task IDs in WBS (Work Breakdown Structure) order
    ///
    /// Performs depth-first traversal of the task hierarchy, returning
    /// full path task IDs with their nesting level for indentation.
    fn collect_wbs_order(tasks: &[utf8proj_core::Task], level: usize) -> Vec<(String, usize)> {
        Self::collect_wbs_order_with_prefix(tasks, "", level)
    }

    /// Helper for collect_wbs_order that tracks the parent path
    fn collect_wbs_order_with_prefix(
        tasks: &[utf8proj_core::Task],
        parent_path: &str,
        level: usize,
    ) -> Vec<(String, usize)> {
        let mut result = Vec::new();
        for task in tasks {
            // Build the full path ID
            let full_id = if parent_path.is_empty() {
                task.id.clone()
            } else {
                format!("{}.{}", parent_path, task.id)
            };

            // Add this task
            result.push((full_id.clone(), level));

            // Recursively add children
            if !task.children.is_empty() {
                result.extend(Self::collect_wbs_order_with_prefix(
                    &task.children,
                    &full_id,
                    level + 1,
                ));
            }
        }
        result
    }

    /// Add Status Dashboard sheet (RFC-0019)
    ///
    /// Creates a project status summary sheet with:
    /// - Header with project name and status indicator
    /// - Progress bar visualization
    /// - Schedule metrics table
    /// - Earned value metrics
    /// - Task breakdown by status
    fn add_status_dashboard_sheet(
        &self,
        workbook: &mut Workbook,
        project: &Project,
        schedule: &Schedule,
        formats: &ExcelFormats,
    ) -> Result<(), RenderError> {
        let sheet = workbook.add_worksheet();
        sheet
            .set_name("Status Dashboard")
            .map_err(|e| RenderError::Format(e.to_string()))?;

        // Determine status date
        let status_date = self
            .status_date
            .or(project.status_date)
            .unwrap_or_else(|| chrono::Local::now().date_naive());

        // Build ProjectStatus
        let status = ProjectStatus::from_schedule(project, schedule, status_date);

        // Create formats specific to this sheet
        let title_format = Format::new()
            .set_bold()
            .set_font_size(16.0)
            .set_align(FormatAlign::Left);

        let section_header = Format::new()
            .set_bold()
            .set_font_size(12.0)
            .set_background_color(0xDDDDDD)
            .set_border(FormatBorder::Thin);

        let label_format = Format::new()
            .set_align(FormatAlign::Left)
            .set_border(FormatBorder::Thin);

        let value_format = Format::new()
            .set_align(FormatAlign::Right)
            .set_border(FormatBorder::Thin);

        let progress_green = Format::new()
            .set_background_color(0x90EE90)
            .set_align(FormatAlign::Center)
            .set_border(FormatBorder::Thin);

        let progress_empty = Format::new()
            .set_background_color(0xEEEEEE)
            .set_align(FormatAlign::Center)
            .set_border(FormatBorder::Thin);

        let status_on_track = Format::new()
            .set_bold()
            .set_font_color(0x008800)
            .set_align(FormatAlign::Left);

        let status_at_risk = Format::new()
            .set_bold()
            .set_font_color(0xCC8800)
            .set_align(FormatAlign::Left);

        let status_behind = Format::new()
            .set_bold()
            .set_font_color(0xCC0000)
            .set_align(FormatAlign::Left);

        // Set column widths
        sheet.set_column_width(0, 20).ok(); // Label column
        sheet.set_column_width(1, 20).ok(); // Value column
        sheet.set_column_width(2, 5).ok();  // Spacer
        sheet.set_column_width(3, 20).ok(); // Label column 2
        sheet.set_column_width(4, 20).ok(); // Value column 2

        let mut row: u32 = 0;

        // =====================================================================
        // Project Title and Status
        // =====================================================================
        sheet
            .write_with_format(row, 0, &status.project_name, &title_format)
            .map_err(|e| RenderError::Format(e.to_string()))?;
        row += 1;

        // Status indicator
        let indicator_text = match status.status_indicator() {
            utf8proj_core::status::StatusIndicator::OnTrack => "ON TRACK",
            utf8proj_core::status::StatusIndicator::AtRisk => "AT RISK",
            utf8proj_core::status::StatusIndicator::Behind => "BEHIND",
        };
        let indicator_format = match status.status_indicator() {
            utf8proj_core::status::StatusIndicator::OnTrack => &status_on_track,
            utf8proj_core::status::StatusIndicator::AtRisk => &status_at_risk,
            utf8proj_core::status::StatusIndicator::Behind => &status_behind,
        };
        sheet
            .write_with_format(row, 0, format!("Status: {}", indicator_text), indicator_format)
            .map_err(|e| RenderError::Format(e.to_string()))?;
        sheet
            .write_with_format(row, 1, status.variance_string(), &value_format)
            .map_err(|e| RenderError::Format(e.to_string()))?;
        row += 2;

        // =====================================================================
        // Progress Bar
        // =====================================================================
        sheet
            .write_with_format(row, 0, "Overall Progress", &section_header)
            .map_err(|e| RenderError::Format(e.to_string()))?;
        sheet
            .merge_range(row, 0, row, 4, "Overall Progress", &section_header)
            .ok();
        row += 1;

        // Draw progress bar (20 cells)
        let filled_cells = (status.overall_progress as usize * 20 / 100).min(20);
        for i in 0..20 {
            let col = i as u16;
            let fmt = if i < filled_cells {
                &progress_green
            } else {
                &progress_empty
            };
            sheet.write_with_format(row, col, "", fmt).ok();
        }
        // Write percentage at the end
        sheet
            .write_with_format(
                row,
                20,
                format!("{}%", status.overall_progress),
                &value_format,
            )
            .map_err(|e| RenderError::Format(e.to_string()))?;
        row += 2;

        // =====================================================================
        // Schedule Metrics
        // =====================================================================
        sheet
            .write_with_format(row, 0, "Schedule", &section_header)
            .map_err(|e| RenderError::Format(e.to_string()))?;
        sheet
            .write_with_format(row, 3, "Earned Value", &section_header)
            .map_err(|e| RenderError::Format(e.to_string()))?;
        row += 1;

        // Schedule column
        sheet
            .write_with_format(row, 0, "Start", &label_format)
            .map_err(|e| RenderError::Format(e.to_string()))?;
        sheet
            .write_with_format(row, 1, status.start_date.to_string(), &value_format)
            .map_err(|e| RenderError::Format(e.to_string()))?;
        // Earned Value column
        sheet
            .write_with_format(row, 3, "Planned Value (PV)", &label_format)
            .map_err(|e| RenderError::Format(e.to_string()))?;
        sheet
            .write_with_format(row, 4, format!("{}%", status.planned_value), &value_format)
            .map_err(|e| RenderError::Format(e.to_string()))?;
        row += 1;

        sheet
            .write_with_format(row, 0, "Baseline Finish", &label_format)
            .map_err(|e| RenderError::Format(e.to_string()))?;
        sheet
            .write_with_format(row, 1, status.baseline_finish.to_string(), &value_format)
            .map_err(|e| RenderError::Format(e.to_string()))?;
        sheet
            .write_with_format(row, 3, "Earned Value (EV)", &label_format)
            .map_err(|e| RenderError::Format(e.to_string()))?;
        sheet
            .write_with_format(row, 4, format!("{}%", status.earned_value), &value_format)
            .map_err(|e| RenderError::Format(e.to_string()))?;
        row += 1;

        sheet
            .write_with_format(row, 0, "Forecast Finish", &label_format)
            .map_err(|e| RenderError::Format(e.to_string()))?;
        sheet
            .write_with_format(row, 1, status.forecast_finish.to_string(), &value_format)
            .map_err(|e| RenderError::Format(e.to_string()))?;
        sheet
            .write_with_format(row, 3, "SPI", &label_format)
            .map_err(|e| RenderError::Format(e.to_string()))?;
        sheet
            .write_with_format(row, 4, format!("{:.2}", status.spi), &value_format)
            .map_err(|e| RenderError::Format(e.to_string()))?;
        row += 1;

        sheet
            .write_with_format(row, 0, "Variance", &label_format)
            .map_err(|e| RenderError::Format(e.to_string()))?;
        sheet
            .write_with_format(row, 1, status.variance_string(), &value_format)
            .map_err(|e| RenderError::Format(e.to_string()))?;
        row += 1;

        sheet
            .write_with_format(row, 0, "Status Date", &label_format)
            .map_err(|e| RenderError::Format(e.to_string()))?;
        sheet
            .write_with_format(row, 1, status.status_date.to_string(), &value_format)
            .map_err(|e| RenderError::Format(e.to_string()))?;
        row += 2;

        // =====================================================================
        // Task Breakdown
        // =====================================================================
        sheet
            .write_with_format(row, 0, "Task Breakdown", &section_header)
            .map_err(|e| RenderError::Format(e.to_string()))?;
        sheet.merge_range(row, 0, row, 1, "Task Breakdown", &section_header).ok();
        row += 1;

        sheet
            .write_with_format(row, 0, "Total Tasks", &label_format)
            .map_err(|e| RenderError::Format(e.to_string()))?;
        sheet
            .write_with_format(row, 1, status.total_tasks as f64, &formats.integer)
            .map_err(|e| RenderError::Format(e.to_string()))?;
        row += 1;

        sheet
            .write_with_format(row, 0, "✓ Complete", &label_format)
            .map_err(|e| RenderError::Format(e.to_string()))?;
        sheet
            .write_with_format(row, 1, status.completed_tasks as f64, &formats.integer)
            .map_err(|e| RenderError::Format(e.to_string()))?;
        row += 1;

        sheet
            .write_with_format(row, 0, "● In Progress", &label_format)
            .map_err(|e| RenderError::Format(e.to_string()))?;
        sheet
            .write_with_format(row, 1, status.in_progress_tasks as f64, &formats.integer)
            .map_err(|e| RenderError::Format(e.to_string()))?;
        row += 1;

        sheet
            .write_with_format(row, 0, "○ Not Started", &label_format)
            .map_err(|e| RenderError::Format(e.to_string()))?;
        sheet
            .write_with_format(row, 1, status.not_started_tasks as f64, &formats.integer)
            .map_err(|e| RenderError::Format(e.to_string()))?;
        row += 1;

        if status.behind_tasks > 0 {
            sheet
                .write_with_format(row, 0, "⚠ Behind Schedule", &label_format)
                .map_err(|e| RenderError::Format(e.to_string()))?;
            sheet
                .write_with_format(row, 1, status.behind_tasks as f64, &formats.integer)
                .map_err(|e| RenderError::Format(e.to_string()))?;
            row += 1;
        }

        sheet
            .write_with_format(row, 0, "Critical Path Tasks", &label_format)
            .map_err(|e| RenderError::Format(e.to_string()))?;
        sheet
            .write_with_format(row, 1, status.critical_path_length as f64, &formats.integer)
            .map_err(|e| RenderError::Format(e.to_string()))?;
        row += 2;

        // Days remaining
        let days_text = if status.days_remaining > 0 {
            format!("{} days remaining until forecast completion", status.days_remaining)
        } else if status.days_remaining == 0 {
            "Project completes today (forecast)".to_string()
        } else {
            format!(
                "{} days past forecast completion",
                status.days_remaining.abs()
            )
        };
        sheet
            .write_with_format(row, 0, &days_text, &label_format)
            .map_err(|e| RenderError::Format(e.to_string()))?;

        Ok(())
    }
}

/// Reusable Excel formats
struct ExcelFormats {
    header: Format,
    currency: Format,
    number: Format,
    integer: Format,
    text: Format,
    week_header: Format,
    total_row: Format,
    total_currency: Format,
    // Alternating row colors for Schedule sheet (per-task banding)
    row_even_text: Format,
    row_even_number: Format,
    row_odd_text: Format,
    row_odd_number: Format,
    // Milestone formats (gold tint for semantic distinction)
    milestone_text: Format,
    milestone_number: Format,
    milestone_week: Format,
    // Container task formats (bold to distinguish phases from leaf tasks)
    container_even_text: Format,
    container_odd_text: Format,
    // Week column empty formats for alternating row banding (filled via conditional formatting)
    gantt_even_empty: Format,
    gantt_odd_empty: Format,
    // Daily schedule: weekend formats (gray background)
    weekend_header: Format,
    weekend_cell: Format,
    // Daily schedule: holiday formats (gold/orange background)
    holiday_header: Format,
    holiday_cell: Format,
    // Progress formats (RFC-0018 Visual mode)
    progress_complete: Format,  // Green for completed work
    progress_behind: Format,    // Red for remaining work past status date
    progress_remaining: Format, // Blue for remaining work on schedule
}

/// Renderer implementation that saves to file path
impl Renderer for ExcelRenderer {
    type Output = Vec<u8>;

    fn render(&self, project: &Project, schedule: &Schedule) -> Result<Vec<u8>, RenderError> {
        if schedule.tasks.is_empty() {
            return Err(RenderError::InvalidData("No tasks to render".into()));
        }
        self.render_to_bytes(project, schedule)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;
    use rust_decimal_macros::dec;
    use utf8proj_core::{Assignment, Duration, Money, Resource, ScheduledTask, Task, TaskStatus};

    fn create_test_project() -> Project {
        let mut project = Project::new("Test Project");
        project.start = NaiveDate::from_ymd_opt(2025, 1, 6).unwrap();

        // Add resources with Money rates
        project.resources.push(
            Resource::new("PM")
                .name("Project Manager")
                .rate(Money::new(dec!(500), "EUR")),
        );
        project.resources.push(
            Resource::new("DEV")
                .name("Developer")
                .rate(Money::new(dec!(400), "EUR")),
        );
        project.resources.push(
            Resource::new("TEST")
                .name("Tester")
                .rate(Money::new(dec!(350), "EUR")),
        );

        // Add tasks
        project.tasks.push(
            Task::new("design")
                .name("Design Phase")
                .effort(Duration::days(5))
                .assign("PM"),
        );
        project.tasks.push(
            Task::new("implement")
                .name("Implementation")
                .effort(Duration::days(20))
                .assign("DEV")
                .depends_on("design"),
        );
        project.tasks.push(
            Task::new("test")
                .name("Testing")
                .effort(Duration::days(10))
                .assign("TEST")
                .depends_on("implement"),
        );

        project
    }

    fn create_test_schedule() -> Schedule {
        let mut tasks = HashMap::new();

        let start1 = NaiveDate::from_ymd_opt(2025, 1, 6).unwrap();
        let finish1 = NaiveDate::from_ymd_opt(2025, 1, 10).unwrap();
        tasks.insert(
            "design".to_string(),
            ScheduledTask {
                task_id: "design".to_string(),
                start: start1,
                finish: finish1,
                duration: Duration::days(5),
                assignments: vec![Assignment {
                    resource_id: "PM".to_string(),
                    start: start1,
                    finish: finish1,
                    units: 1.0,
                    cost: None,
                    cost_range: None,
                    is_abstract: false,
                    effort_days: None,
                }],
                slack: Duration::zero(),
                is_critical: true,
                early_start: start1,
                early_finish: finish1,
                late_start: start1,
                late_finish: finish1,
                forecast_start: start1,
                forecast_finish: finish1,
                remaining_duration: Duration::days(5),
                percent_complete: 0,
                status: TaskStatus::NotStarted,
                cost_range: None,
                has_abstract_assignments: false,
                baseline_start: start1,
                baseline_finish: finish1,
                start_variance_days: 0,
                finish_variance_days: 0,
            },
        );

        let start2 = NaiveDate::from_ymd_opt(2025, 1, 13).unwrap();
        let finish2 = NaiveDate::from_ymd_opt(2025, 1, 31).unwrap();
        tasks.insert(
            "implement".to_string(),
            ScheduledTask {
                task_id: "implement".to_string(),
                start: start2,
                finish: finish2,
                duration: Duration::days(20),
                assignments: vec![Assignment {
                    resource_id: "DEV".to_string(),
                    start: start2,
                    finish: finish2,
                    units: 1.0,
                    cost: None,
                    cost_range: None,
                    is_abstract: false,
                    effort_days: None,
                }],
                slack: Duration::zero(),
                is_critical: true,
                early_start: start2,
                early_finish: finish2,
                late_start: start2,
                late_finish: finish2,
                forecast_start: start2,
                forecast_finish: finish2,
                remaining_duration: Duration::days(20),
                percent_complete: 0,
                status: TaskStatus::NotStarted,
                cost_range: None,
                has_abstract_assignments: false,
                baseline_start: start2,
                baseline_finish: finish2,
                start_variance_days: 0,
                finish_variance_days: 0,
            },
        );

        let start3 = NaiveDate::from_ymd_opt(2025, 2, 3).unwrap();
        let finish3 = NaiveDate::from_ymd_opt(2025, 2, 14).unwrap();
        tasks.insert(
            "test".to_string(),
            ScheduledTask {
                task_id: "test".to_string(),
                start: start3,
                finish: finish3,
                duration: Duration::days(10),
                assignments: vec![Assignment {
                    resource_id: "TEST".to_string(),
                    start: start3,
                    finish: finish3,
                    units: 1.0,
                    cost: None,
                    cost_range: None,
                    is_abstract: false,
                    effort_days: None,
                }],
                slack: Duration::zero(),
                is_critical: true,
                early_start: start3,
                early_finish: finish3,
                late_start: start3,
                late_finish: finish3,
                forecast_start: start3,
                forecast_finish: finish3,
                remaining_duration: Duration::days(10),
                percent_complete: 0,
                status: TaskStatus::NotStarted,
                cost_range: None,
                has_abstract_assignments: false,
                baseline_start: start3,
                baseline_finish: finish3,
                start_variance_days: 0,
                finish_variance_days: 0,
            },
        );

        let project_end = NaiveDate::from_ymd_opt(2025, 2, 14).unwrap();
        Schedule {
            tasks,
            critical_path: vec![
                "design".to_string(),
                "implement".to_string(),
                "test".to_string(),
            ],
            project_duration: Duration::days(35),
            project_end,
            total_cost: None,
            total_cost_range: None,
            project_progress: 0,
            project_baseline_finish: project_end,
            project_forecast_finish: project_end,
            project_variance_days: 0,
            planned_value: 0,
            earned_value: 0,
            spi: 1.0,
        }
    }

    #[test]
    fn excel_renderer_creation() {
        let renderer = ExcelRenderer::new();
        assert_eq!(renderer.currency, "€");
        assert_eq!(renderer.schedule_weeks, 18);
        assert!(renderer.use_formulas);
    }

    #[test]
    fn excel_renderer_with_options() {
        let renderer = ExcelRenderer::new()
            .currency("$")
            .weeks(24)
            .hours_per_day(7.5)
            .no_summary()
            .static_values();

        assert_eq!(renderer.currency, "$");
        assert_eq!(renderer.schedule_weeks, 24);
        assert_eq!(renderer.hours_per_day, 7.5);
        assert!(!renderer.include_summary);
        assert!(!renderer.use_formulas);
    }

    #[test]
    fn excel_produces_valid_output() {
        let renderer = ExcelRenderer::new();
        let project = create_test_project();
        let schedule = create_test_schedule();

        let result = renderer.render(&project, &schedule);
        assert!(result.is_ok());

        let bytes = result.unwrap();
        // XLSX files start with PK (ZIP header)
        assert!(bytes.len() > 100);
        assert_eq!(&bytes[0..2], b"PK");
    }

    #[test]
    fn excel_empty_schedule_fails() {
        let renderer = ExcelRenderer::new();
        let project = Project::new("Empty");
        let project_end = NaiveDate::from_ymd_opt(2025, 1, 1).unwrap();
        let schedule = Schedule {
            tasks: HashMap::new(),
            critical_path: vec![],
            project_duration: Duration::zero(),
            project_end,
            total_cost: None,
            total_cost_range: None,
            project_progress: 0,
            project_baseline_finish: project_end,
            project_forecast_finish: project_end,
            project_variance_days: 0,
            planned_value: 0,
            earned_value: 0,
            spi: 1.0,
        };

        let result = renderer.render(&project, &schedule);
        assert!(result.is_err());
    }

    #[test]
    fn col_to_letter_works() {
        assert_eq!(ExcelRenderer::col_to_letter(0), "A");
        assert_eq!(ExcelRenderer::col_to_letter(25), "Z");
        assert_eq!(ExcelRenderer::col_to_letter(26), "AA");
        assert_eq!(ExcelRenderer::col_to_letter(27), "AB");
        assert_eq!(ExcelRenderer::col_to_letter(51), "AZ");
        assert_eq!(ExcelRenderer::col_to_letter(52), "BA");
    }

    #[test]
    fn date_to_week_calculation() {
        let renderer = ExcelRenderer::new();
        let start = NaiveDate::from_ymd_opt(2025, 1, 6).unwrap();

        // Same day = week 1
        assert_eq!(renderer.date_to_week(start, start), 1);

        // 6 days later = still week 1
        let day6 = NaiveDate::from_ymd_opt(2025, 1, 12).unwrap();
        assert_eq!(renderer.date_to_week(day6, start), 1);

        // 7 days later = week 2
        let day7 = NaiveDate::from_ymd_opt(2025, 1, 13).unwrap();
        assert_eq!(renderer.date_to_week(day7, start), 2);

        // 14 days later = week 3
        let day14 = NaiveDate::from_ymd_opt(2025, 1, 20).unwrap();
        assert_eq!(renderer.date_to_week(day14, start), 3);
    }

    #[test]
    fn excel_with_static_values() {
        let renderer = ExcelRenderer::new().static_values();
        let project = create_test_project();
        let schedule = create_test_schedule();

        let result = renderer.render(&project, &schedule);
        assert!(result.is_ok());
    }

    #[test]
    fn excel_without_summary() {
        let renderer = ExcelRenderer::new().no_summary();
        let project = create_test_project();
        let schedule = create_test_schedule();

        let result = renderer.render(&project, &schedule);
        assert!(result.is_ok());
    }

    #[test]
    fn excel_with_different_currency() {
        let renderer = ExcelRenderer::new().currency("USD");
        let project = create_test_project();
        let schedule = create_test_schedule();

        let result = renderer.render(&project, &schedule);
        assert!(result.is_ok());
    }

    #[test]
    fn excel_with_dependencies_enabled() {
        // Dependencies are enabled by default
        let renderer = ExcelRenderer::new();
        assert!(renderer.show_dependencies);

        let project = create_test_project();
        let schedule = create_test_schedule();

        let result = renderer.render(&project, &schedule);
        assert!(result.is_ok());

        let bytes = result.unwrap();
        // Should produce valid XLSX
        assert!(bytes.len() > 100);
        assert_eq!(&bytes[0..2], b"PK");
    }

    #[test]
    fn excel_with_dependencies_disabled() {
        let renderer = ExcelRenderer::new().no_dependencies();
        assert!(!renderer.show_dependencies);

        let project = create_test_project();
        let schedule = create_test_schedule();

        let result = renderer.render(&project, &schedule);
        assert!(result.is_ok());

        let bytes = result.unwrap();
        // Should produce valid XLSX
        assert!(bytes.len() > 100);
        assert_eq!(&bytes[0..2], b"PK");
    }

    #[test]
    fn excel_hours_per_week_setting() {
        let renderer = ExcelRenderer::new().hours_per_day(8.0).hours_per_week(35.0); // Part-time work week

        assert_eq!(renderer.hours_per_week, 35.0);

        let project = create_test_project();
        let schedule = create_test_schedule();

        let result = renderer.render(&project, &schedule);
        assert!(result.is_ok());
    }

    #[test]
    fn excel_dependency_formulas_cascade() {
        // Test that with dependencies, changing predecessor would cascade
        // (We can't test actual Excel formula evaluation, but we can verify structure)
        let renderer = ExcelRenderer::new();
        let project = create_test_project();
        let schedule = create_test_schedule();

        let result = renderer.render(&project, &schedule);
        assert!(result.is_ok());
    }

    #[test]
    fn task_status_icons() {
        // RFC-0018: Test TaskStatus icons for Full mode
        use super::TaskStatus as ExcelTaskStatus;

        assert_eq!(ExcelTaskStatus::Complete.icon(), "✓");
        assert_eq!(ExcelTaskStatus::InProgress.icon(), "●");
        assert_eq!(ExcelTaskStatus::NotStarted.icon(), "○");
        assert_eq!(ExcelTaskStatus::Behind.icon(), "⚠");
        assert_eq!(ExcelTaskStatus::Overdue.icon(), "⚠");
    }

    #[test]
    fn progress_column_count_by_mode() {
        // RFC-0018: Test column counts for different progress modes
        let renderer_none = ExcelRenderer::new().with_progress_mode(ProgressMode::None);
        let renderer_columns = ExcelRenderer::new().with_progress_mode(ProgressMode::Columns);
        let renderer_visual = ExcelRenderer::new().with_progress_mode(ProgressMode::Visual);
        let renderer_full = ExcelRenderer::new().with_progress_mode(ProgressMode::Full);

        assert_eq!(renderer_none.progress_column_count(), 0);
        assert_eq!(renderer_columns.progress_column_count(), 4);
        assert_eq!(renderer_visual.progress_column_count(), 4);
        assert_eq!(renderer_full.progress_column_count(), 6); // +Status, +Variance
    }

    #[test]
    fn excel_full_mode_produces_valid_output() {
        // RFC-0018: Test Full progress mode produces valid Excel
        let renderer = ExcelRenderer::new()
            .with_progress_mode(ProgressMode::Full)
            .with_status_date(NaiveDate::from_ymd_opt(2025, 1, 20).unwrap());

        let project = create_test_project();
        let schedule = create_test_schedule();

        let result = renderer.render(&project, &schedule);
        assert!(result.is_ok());

        let bytes = result.unwrap();
        assert!(bytes.len() > 100);
        assert_eq!(&bytes[0..2], b"PK"); // Valid XLSX
    }
}
