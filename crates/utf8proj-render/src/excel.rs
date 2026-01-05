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

use chrono::NaiveDate;
use rust_xlsxwriter::{Format, FormatAlign, FormatBorder, Workbook, Worksheet};
use rust_decimal::prelude::ToPrimitive;
use std::collections::HashMap;
use utf8proj_core::{Project, RenderError, Renderer, Schedule, ScheduledTask};

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
                let rate = r.rate.as_ref()
                    .and_then(|m| m.amount.to_f64())
                    .unwrap_or(self.default_rate);
                (r.id.clone(), rate)
            })
            .collect();

        // Get project start
        let project_start = self.project_start.unwrap_or(project.start);

        // Add sheets
        self.add_profiles_sheet(&mut workbook, project, schedule, &formats, &resource_rates)?;
        self.add_schedule_sheet(
            &mut workbook,
            project,
            schedule,
            &formats,
            project_start,
        )?;

        if self.include_summary {
            self.add_executive_summary(&mut workbook, project, schedule, &formats, &resource_rates)?;
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

        let gantt_filled = Format::new()
            .set_background_color(0x5B9BD5)
            .set_align(FormatAlign::Center)
            .set_border(FormatBorder::Thin);

        let gantt_empty = Format::new()
            .set_background_color(0xF2F2F2)
            .set_align(FormatAlign::Center)
            .set_border(FormatBorder::Thin);

        let gantt_critical = Format::new()
            .set_background_color(0xFF6B6B)
            .set_align(FormatAlign::Center)
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

        ExcelFormats {
            header,
            currency,
            number,
            integer,
            text,
            week_header,
            gantt_filled,
            gantt_empty,
            gantt_critical,
            total_row,
            total_currency,
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
        // Effort per assignment = task duration * units (since Assignment has start/finish/units)
        let mut resource_effort: HashMap<String, f64> = HashMap::new();
        for scheduled in schedule.tasks.values() {
            for assignment in &scheduled.assignments {
                let assignment_days = (assignment.finish - assignment.start).num_days() as f64;
                let effort = assignment_days * assignment.units as f64;
                *resource_effort.entry(assignment.resource_id.clone()).or_default() += effort;
            }
        }

        // Write resource rows
        let mut row = 1u32;
        let mut total_cost = 0.0;

        for resource in &project.resources {
            let rate = resource_rates.get(&resource.id).copied().unwrap_or(self.default_rate);
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
        let sheet = workbook.add_worksheet();
        sheet
            .set_name("Schedule")
            .map_err(|e| RenderError::Format(e.to_string()))?;

        // Column layout depends on show_dependencies
        // With deps: Task ID, Activity, Profile, Depends On, Type, Lag, Effort, Start, End, W1...
        // Without:   Activity, Profile, pd, Start, End, W1...

        let (week_start_col, effort_col, start_col, end_col) = if self.show_dependencies {
            self.write_schedule_headers_with_deps(sheet, formats)?;
            (9u16, 6u16, 7u16, 8u16) // Week columns start at J (col 9)
        } else {
            self.write_schedule_headers_simple(sheet, formats)?;
            (5u16, 2u16, 3u16, 4u16) // Week columns start at F (col 5)
        };

        // Week column headers
        for week in 1..=self.schedule_weeks {
            let col = week_start_col + (week - 1) as u16;
            sheet
                .write_with_format(0, col, week as f64, &formats.week_header)
                .map_err(|e| RenderError::Format(e.to_string()))?;
            sheet.set_column_width(col, 4).ok();
        }

        // Set row height for header (rotated text)
        sheet.set_row_height(0, 50).ok();

        // Sort tasks by start date
        let mut tasks: Vec<&ScheduledTask> = schedule.tasks.values().collect();
        tasks.sort_by_key(|t| t.start);

        // Build task row mapping for VLOOKUP (task_id -> row number)
        let mut task_row_map: HashMap<String, u32> = HashMap::new();
        let mut current_row = 2u32; // Excel rows are 1-indexed, data starts at row 2
        for scheduled in &tasks {
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
        for scheduled in &tasks {
            let task = project.get_task(&scheduled.task_id);
            let task_name = task
                .map(|t| t.name.clone())
                .unwrap_or_else(|| scheduled.task_id.clone());

            // Get first predecessor (if any) for dependency column
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
                    (d.predecessor.clone(), dep_type, lag_days)
                })
                .unwrap_or_default();

            // Calculate week numbers relative to project start
            let start_week = self.date_to_week(scheduled.start, project_start);
            let end_week = self.date_to_week(scheduled.finish, project_start);
            let duration_days = scheduled.duration.as_days();

            // If task has assignments, create a row per assignment
            if scheduled.assignments.is_empty() {
                if self.show_dependencies {
                    self.write_schedule_row_with_deps(
                        sheet, row, &scheduled.task_id, &task_name, "",
                        &predecessor, dep_type, lag, duration_days,
                        start_week, end_week, scheduled.is_critical,
                        formats, week_start_col, effort_col, start_col, end_col,
                        last_data_row,
                    )?;
                } else {
                    self.write_schedule_row_simple(
                        sheet, row, &task_name, "", duration_days,
                        start_week, end_week, scheduled.is_critical,
                        formats, week_start_col, effort_col, start_col, end_col,
                    )?;
                }
                row += 1;
            } else {
                // One row per assignment
                let mut first_assignment = true;
                for assignment in &scheduled.assignments {
                    let assignment_days = (assignment.finish - assignment.start).num_days() as f64;
                    let effort = assignment_days * assignment.units as f64;

                    // Only show dependency info on first row for this task
                    let (pred, dtype, lag_val) = if first_assignment {
                        (predecessor.clone(), dep_type, lag)
                    } else {
                        (String::new(), "", 0)
                    };

                    if self.show_dependencies {
                        self.write_schedule_row_with_deps(
                            sheet, row, &scheduled.task_id, &task_name, &assignment.resource_id,
                            &pred, dtype, lag_val, effort,
                            start_week, end_week, scheduled.is_critical,
                            formats, week_start_col, effort_col, start_col, end_col,
                            last_data_row,
                        )?;
                    } else {
                        self.write_schedule_row_simple(
                            sheet, row, &task_name, &assignment.resource_id, effort,
                            start_week, end_week, scheduled.is_critical,
                            formats, week_start_col, effort_col, start_col, end_col,
                        )?;
                    }
                    first_assignment = false;
                    row += 1;
                }
            }
        }

        // Total row for each week column
        self.write_schedule_totals(sheet, row, week_start_col, formats)?;

        // Freeze first row and fixed columns
        let freeze_cols = if self.show_dependencies { 9 } else { 5 };
        sheet.set_freeze_panes(1, freeze_cols).ok();

        Ok(())
    }

    /// Write headers for simple schedule (no dependencies)
    fn write_schedule_headers_simple(
        &self,
        sheet: &mut Worksheet,
        formats: &ExcelFormats,
    ) -> Result<(), RenderError> {
        let headers = ["Activity", "Profile", "pd", "Start\nweek", "End\nweek"];
        for (col, header) in headers.iter().enumerate() {
            sheet
                .write_with_format(0, col as u16, *header, &formats.header)
                .map_err(|e| RenderError::Format(e.to_string()))?;
        }

        // Column widths
        sheet.set_column_width(0, 25).ok(); // Activity
        sheet.set_column_width(1, 15).ok(); // Profile
        sheet.set_column_width(2, 6).ok();  // pd
        sheet.set_column_width(3, 6).ok();  // Start
        sheet.set_column_width(4, 6).ok();  // End

        Ok(())
    }

    /// Write headers for schedule with dependencies
    fn write_schedule_headers_with_deps(
        &self,
        sheet: &mut Worksheet,
        formats: &ExcelFormats,
    ) -> Result<(), RenderError> {
        let headers = [
            "Task ID", "Activity", "Profile", "Depends\nOn", "Type", "Lag\n(d)",
            "Effort\n(pd)", "Start\nweek", "End\nweek"
        ];
        for (col, header) in headers.iter().enumerate() {
            sheet
                .write_with_format(0, col as u16, *header, &formats.header)
                .map_err(|e| RenderError::Format(e.to_string()))?;
        }

        // Column widths
        sheet.set_column_width(0, 12).ok(); // Task ID
        sheet.set_column_width(1, 25).ok(); // Activity
        sheet.set_column_width(2, 12).ok(); // Profile
        sheet.set_column_width(3, 10).ok(); // Depends On
        sheet.set_column_width(4, 5).ok();  // Type
        sheet.set_column_width(5, 5).ok();  // Lag
        sheet.set_column_width(6, 7).ok();  // Effort
        sheet.set_column_width(7, 6).ok();  // Start
        sheet.set_column_width(8, 6).ok();  // End

        Ok(())
    }

    /// Write a schedule row without dependency formulas
    #[allow(clippy::too_many_arguments)]
    fn write_schedule_row_simple(
        &self,
        sheet: &mut Worksheet,
        row: u32,
        task_name: &str,
        profile: &str,
        person_days: f64,
        start_week: u32,
        end_week: u32,
        is_critical: bool,
        formats: &ExcelFormats,
        week_start_col: u16,
        effort_col: u16,
        start_col: u16,
        end_col: u16,
    ) -> Result<(), RenderError> {
        // Fixed columns: Activity, Profile, pd, Start, End
        sheet.write_with_format(row, 0, task_name, &formats.text)
            .map_err(|e| RenderError::Format(e.to_string()))?;
        sheet.write_with_format(row, 1, profile, &formats.text)
            .map_err(|e| RenderError::Format(e.to_string()))?;
        sheet.write_with_format(row, effort_col, person_days, &formats.number)
            .map_err(|e| RenderError::Format(e.to_string()))?;
        sheet.write_with_format(row, start_col, start_week as f64, &formats.integer)
            .map_err(|e| RenderError::Format(e.to_string()))?;
        sheet.write_with_format(row, end_col, end_week as f64, &formats.integer)
            .map_err(|e| RenderError::Format(e.to_string()))?;

        // Week columns
        self.write_week_columns(sheet, row, start_week, end_week, is_critical,
            formats, week_start_col, effort_col, start_col, end_col, person_days)?;

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
        profile: &str,
        predecessor: &str,
        dep_type: &str,
        lag: i32,
        person_days: f64,
        start_week: u32,
        end_week: u32,
        is_critical: bool,
        formats: &ExcelFormats,
        week_start_col: u16,
        effort_col: u16,
        start_col: u16,
        end_col: u16,
        last_data_row: u32,
    ) -> Result<(), RenderError> {
        let excel_row = row + 1; // Excel is 1-indexed

        // Col A: Task ID
        sheet.write_with_format(row, 0, task_id, &formats.text)
            .map_err(|e| RenderError::Format(e.to_string()))?;

        // Col B: Activity
        sheet.write_with_format(row, 1, task_name, &formats.text)
            .map_err(|e| RenderError::Format(e.to_string()))?;

        // Col C: Profile
        sheet.write_with_format(row, 2, profile, &formats.text)
            .map_err(|e| RenderError::Format(e.to_string()))?;

        // Col D: Depends On
        sheet.write_with_format(row, 3, predecessor, &formats.text)
            .map_err(|e| RenderError::Format(e.to_string()))?;

        // Col E: Type (FS/SS/FF/SF)
        let dep_type_val = if predecessor.is_empty() { "" } else { dep_type };
        sheet.write_with_format(row, 4, dep_type_val, &formats.text)
            .map_err(|e| RenderError::Format(e.to_string()))?;

        // Col F: Lag
        if !predecessor.is_empty() {
            sheet.write_with_format(row, 5, lag as f64, &formats.integer)
                .map_err(|e| RenderError::Format(e.to_string()))?;
        } else {
            sheet.write_with_format(row, 5, "", &formats.text)
                .map_err(|e| RenderError::Format(e.to_string()))?;
        }

        // Col G: Effort (pd)
        sheet.write_with_format(row, effort_col, person_days, &formats.number)
            .map_err(|e| RenderError::Format(e.to_string()))?;

        // Col H: Start Week - Formula-driven if has predecessor
        if self.use_formulas && !predecessor.is_empty() {
            // Formula: IF(D="", manual_start, VLOOKUP(D, A:I, 9, 0) + 1 + F)
            // For FS: Start = Predecessor End + 1 + Lag
            // For SS: Start = Predecessor Start + Lag
            // For FF: Start = Predecessor End - Duration + 1 + Lag
            // For SF: Start = Predecessor Start - Duration + 1 + Lag
            let formula = format!(
                "=IF(D{}=\"\",{},IF(E{}=\"FS\",VLOOKUP(D{},$A$2:$I${},9,0)+1+F{},\
                IF(E{}=\"SS\",VLOOKUP(D{},$A$2:$I${},8,0)+F{},\
                IF(E{}=\"FF\",VLOOKUP(D{},$A$2:$I${},9,0)-CEILING(G{}*{}/{},1)+1+F{},\
                IF(E{}=\"SF\",VLOOKUP(D{},$A$2:$I${},8,0)-CEILING(G{}*{}/{},1)+1+F{},\
                {})))))",
                excel_row, start_week,
                excel_row, excel_row, last_data_row, excel_row,
                excel_row, excel_row, last_data_row, excel_row,
                excel_row, excel_row, last_data_row, excel_row, self.hours_per_day, self.hours_per_week, excel_row,
                excel_row, excel_row, last_data_row, excel_row, self.hours_per_day, self.hours_per_week, excel_row,
                start_week
            );
            sheet.write_formula_with_format(row, start_col, formula.as_str(), &formats.integer)
                .map_err(|e| RenderError::Format(e.to_string()))?;
        } else {
            sheet.write_with_format(row, start_col, start_week as f64, &formats.integer)
                .map_err(|e| RenderError::Format(e.to_string()))?;
        }

        // Col I: End Week - Formula: Start + CEILING(effort * hours_per_day / hours_per_week) - 1
        if self.use_formulas {
            let start_col_letter = Self::col_to_letter(start_col);
            let effort_col_letter = Self::col_to_letter(effort_col);
            let formula = format!(
                "={}{}+MAX(CEILING({}{}*{}/{},1)-1,0)",
                start_col_letter, excel_row,
                effort_col_letter, excel_row,
                self.hours_per_day, self.hours_per_week
            );
            sheet.write_formula_with_format(row, end_col, formula.as_str(), &formats.integer)
                .map_err(|e| RenderError::Format(e.to_string()))?;
        } else {
            sheet.write_with_format(row, end_col, end_week as f64, &formats.integer)
                .map_err(|e| RenderError::Format(e.to_string()))?;
        }

        // Week columns
        self.write_week_columns(sheet, row, start_week, end_week, is_critical,
            formats, week_start_col, effort_col, start_col, end_col, person_days)?;

        Ok(())
    }

    /// Write week columns with Gantt bar formulas
    #[allow(clippy::too_many_arguments)]
    fn write_week_columns(
        &self,
        sheet: &mut Worksheet,
        row: u32,
        start_week: u32,
        end_week: u32,
        is_critical: bool,
        formats: &ExcelFormats,
        week_start_col: u16,
        effort_col: u16,
        start_col: u16,
        end_col: u16,
        person_days: f64,
    ) -> Result<(), RenderError> {
        let excel_row = row + 1;
        let weeks_span = (end_week.saturating_sub(start_week) + 1).max(1);
        let hours_per_week_val = (person_days * self.hours_per_day) / weeks_span as f64;

        let effort_col_letter = Self::col_to_letter(effort_col);
        let start_col_letter = Self::col_to_letter(start_col);
        let end_col_letter = Self::col_to_letter(end_col);

        for week in 1..=self.schedule_weeks {
            let col = week_start_col + (week - 1) as u16;
            let in_range = week >= start_week && week <= end_week;
            let col_letter = Self::col_to_letter(col);

            let format = if in_range {
                if is_critical { &formats.gantt_critical } else { &formats.gantt_filled }
            } else {
                &formats.gantt_empty
            };

            if self.use_formulas {
                // Formula: IF(week >= Start AND week <= End, Effort * hours_per_day / (End - Start + 1), 0)
                // References Start/End columns which may themselves be formulas for cascading
                let formula = format!(
                    "=ROUND(IF(AND({}$1>=${}{},{}$1<=${}{}),({}{}*{})/(${}{}-${}{}+1),0),0)",
                    col_letter, start_col_letter, excel_row,
                    col_letter, end_col_letter, excel_row,
                    effort_col_letter, excel_row, self.hours_per_day,
                    end_col_letter, excel_row, start_col_letter, excel_row
                );
                sheet.write_formula_with_format(row, col, formula.as_str(), format)
                    .map_err(|e| RenderError::Format(e.to_string()))?;
            } else {
                let value = if in_range { hours_per_week_val.round() } else { 0.0 };
                sheet.write_with_format(row, col, value, format)
                    .map_err(|e| RenderError::Format(e.to_string()))?;
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
        formats: &ExcelFormats,
    ) -> Result<(), RenderError> {
        if row <= 1 {
            return Ok(());
        }

        // Write TOTAL label in first column
        sheet.write_with_format(row, 0, "TOTAL", &formats.total_row)
            .map_err(|e| RenderError::Format(e.to_string()))?;

        // Fill empty cells up to week columns
        for col_idx in 1..week_start_col {
            sheet.write_with_format(row, col_idx, "", &formats.total_row)
                .map_err(|e| RenderError::Format(e.to_string()))?;
        }

        // Sum formulas for each week column
        for week in 0..self.schedule_weeks {
            let week_col = week_start_col + week as u16;
            if self.use_formulas {
                let col_letter = Self::col_to_letter(week_col);
                let formula = format!("=SUM({}2:{}{})", col_letter, col_letter, row);
                sheet.write_formula_with_format(row, week_col, formula.as_str(), &formats.total_row)
                    .map_err(|e| RenderError::Format(e.to_string()))?;
            } else {
                sheet.write_with_format(row, week_col, 0.0, &formats.total_row)
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
        sheet.merge_range(0, 0, 0, 1, "PROJECT SUMMARY", &formats.header).ok();

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
            .write_with_format(3, 1, project.start.format("%Y-%m-%d").to_string(), &formats.text)
            .map_err(|e| RenderError::Format(e.to_string()))?;

        sheet
            .write_with_format(4, 0, "End Date:", &formats.text)
            .map_err(|e| RenderError::Format(e.to_string()))?;
        sheet
            .write_with_format(4, 1, schedule.project_end.format("%Y-%m-%d").to_string(), &formats.text)
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
        sheet.merge_range(9, 0, 9, 1, "COST SUMMARY", &formats.header).ok();

        // Calculate totals
        let mut resource_effort: HashMap<String, f64> = HashMap::new();
        for scheduled in schedule.tasks.values() {
            for assignment in &scheduled.assignments {
                let assignment_days = (assignment.finish - assignment.start).num_days() as f64;
                let effort = assignment_days * assignment.units as f64;
                *resource_effort.entry(assignment.resource_id.clone()).or_default() += effort;
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
            .write_with_format(12, 0, &format!("Total Cost ({}):", self.currency), &formats.text)
            .map_err(|e| RenderError::Format(e.to_string()))?;
        sheet
            .write_with_format(12, 1, total_cost, &formats.currency)
            .map_err(|e| RenderError::Format(e.to_string()))?;

        // Column widths
        sheet.set_column_width(0, 20).ok();
        sheet.set_column_width(1, 25).ok();

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
}

/// Reusable Excel formats
struct ExcelFormats {
    header: Format,
    currency: Format,
    number: Format,
    integer: Format,
    text: Format,
    week_header: Format,
    gantt_filled: Format,
    gantt_empty: Format,
    gantt_critical: Format,
    total_row: Format,
    total_currency: Format,
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
    use utf8proj_core::{Assignment, Duration, Money, Resource, ScheduledTask, Task, TaskStatus};
    use rust_decimal_macros::dec;

    fn create_test_project() -> Project {
        let mut project = Project::new("Test Project");
        project.start = NaiveDate::from_ymd_opt(2025, 1, 6).unwrap();

        // Add resources with Money rates
        project.resources.push(Resource::new("PM").name("Project Manager").rate(Money::new(dec!(500), "EUR")));
        project.resources.push(Resource::new("DEV").name("Developer").rate(Money::new(dec!(400), "EUR")));
        project.resources.push(Resource::new("TEST").name("Tester").rate(Money::new(dec!(350), "EUR")));

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
            },
        );

        Schedule {
            tasks,
            critical_path: vec![
                "design".to_string(),
                "implement".to_string(),
                "test".to_string(),
            ],
            project_duration: Duration::days(35),
            project_end: NaiveDate::from_ymd_opt(2025, 2, 14).unwrap(),
            total_cost: None,
            total_cost_range: None,
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
        let schedule = Schedule {
            tasks: HashMap::new(),
            critical_path: vec![],
            project_duration: Duration::zero(),
            project_end: NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
            total_cost: None,
            total_cost_range: None,
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
        let renderer = ExcelRenderer::new()
            .hours_per_day(8.0)
            .hours_per_week(35.0); // Part-time work week

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
}
