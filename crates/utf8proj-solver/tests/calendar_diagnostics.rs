//! Integration tests for Calendar Diagnostics (C001, C002, C010, C011, C020-C023)
//!
//! These tests verify calendar-related diagnostic checks.

use chrono::{Datelike, Local};
use utf8proj_core::{
    Calendar, CollectingEmitter, DiagnosticCode, Duration, Holiday, Project, Resource, Scheduler,
    Task, TimeRange,
};
use utf8proj_solver::{analyze_project, AnalysisConfig, CpmSolver};

/// Helper to get current date
fn today() -> chrono::NaiveDate {
    Local::now().date_naive()
}

/// Test: C001 - Zero working hours calendar emits error
#[test]
fn c001_zero_working_hours() {
    let mut project = Project::new("C001 Test");
    project.start = today();

    // Create a calendar with no working hours
    let mut no_hours_cal = Calendar::default();
    no_hours_cal.id = "no_hours".to_string();
    no_hours_cal.working_hours = vec![]; // No working hours!
    project.calendars.push(no_hours_cal);

    project
        .tasks
        .push(Task::new("task1").duration(Duration::days(5)));

    let solver = CpmSolver::new();
    let schedule = solver.schedule(&project).expect("Should succeed");

    let mut emitter = CollectingEmitter::new();
    let config = AnalysisConfig::default();
    analyze_project(&project, Some(&schedule), &config, &mut emitter);

    // Should have C001 diagnostic for the no_hours calendar
    assert!(
        emitter
            .diagnostics
            .iter()
            .any(|d| d.code == DiagnosticCode::C001ZeroWorkingHours
                && d.message.contains("no_hours")),
        "Should emit C001ZeroWorkingHours for calendar with no working hours"
    );
}

/// Test: C002 - No working days calendar emits error
#[test]
fn c002_no_working_days() {
    let mut project = Project::new("C002 Test");
    project.start = today();

    // Create a calendar with no working days
    let mut no_days_cal = Calendar::default();
    no_days_cal.id = "no_days".to_string();
    no_days_cal.working_days = vec![]; // No working days!
    project.calendars.push(no_days_cal);

    project
        .tasks
        .push(Task::new("task1").duration(Duration::days(5)));

    let solver = CpmSolver::new();
    let schedule = solver.schedule(&project).expect("Should succeed");

    let mut emitter = CollectingEmitter::new();
    let config = AnalysisConfig::default();
    analyze_project(&project, Some(&schedule), &config, &mut emitter);

    // Should have C002 diagnostic for the no_days calendar
    assert!(
        emitter
            .diagnostics
            .iter()
            .any(|d| d.code == DiagnosticCode::C002NoWorkingDays
                && d.message.contains("no_days")),
        "Should emit C002NoWorkingDays for calendar with no working days"
    );
}

/// Test: C010 - Task scheduled on non-working day emits warning
#[test]
fn c010_non_working_day() {
    let mut project = Project::new("C010 Test");

    // Set project start to a Saturday (find the next Saturday from today)
    let mut start_date = today();
    while start_date.weekday() != chrono::Weekday::Sat {
        start_date = start_date.succ_opt().unwrap();
    }
    project.start = start_date;

    // Default calendar has Mon-Fri working days
    let mut standard_cal = Calendar::default();
    standard_cal.id = "standard".to_string();
    standard_cal.working_days = vec![1, 2, 3, 4, 5]; // Mon-Fri (0=Sun, 6=Sat)
    project.calendars.push(standard_cal.clone());
    project.calendar = "standard".to_string();

    project
        .tasks
        .push(Task::new("task1").duration(Duration::days(5)));

    let solver = CpmSolver::new();
    let schedule = solver.schedule(&project).expect("Should succeed");

    let mut emitter = CollectingEmitter::new();
    let config = AnalysisConfig::default();
    analyze_project(&project, Some(&schedule), &config, &mut emitter);

    // Should have C010 diagnostic since task starts on Saturday
    assert!(
        emitter
            .diagnostics
            .iter()
            .any(|d| d.code == DiagnosticCode::C010NonWorkingDay),
        "Should emit C010NonWorkingDay for task starting on Saturday"
    );
}

/// Test: C011 - Calendar mismatch between project and resource emits warning
#[test]
fn c011_calendar_mismatch() {
    let mut project = Project::new("C011 Test");
    project.start = today();

    // Create two different calendars
    let mut cal_a = Calendar::default();
    cal_a.id = "cal_a".to_string();
    project.calendars.push(cal_a);

    let mut cal_b = Calendar::default();
    cal_b.id = "cal_b".to_string();
    project.calendars.push(cal_b);

    // Project uses cal_a
    project.calendar = "cal_a".to_string();

    // Resource uses cal_b (different from project)
    let mut resource = Resource::new("dev");
    resource.calendar = Some("cal_b".to_string());
    project.resources.push(resource);

    // Task assigned to resource with different calendar
    let task = Task::new("task1")
        .duration(Duration::days(5))
        .assign("dev");
    project.tasks.push(task);

    let solver = CpmSolver::new();
    let schedule = solver.schedule(&project).expect("Should succeed");

    let mut emitter = CollectingEmitter::new();
    let config = AnalysisConfig::default();
    analyze_project(&project, Some(&schedule), &config, &mut emitter);

    // Should have C011 diagnostic
    assert!(
        emitter
            .diagnostics
            .iter()
            .any(|d| d.code == DiagnosticCode::C011CalendarMismatch),
        "Should emit C011CalendarMismatch for resource with different calendar"
    );
}

/// Test: C020 - Low availability calendar emits hint
#[test]
fn c020_low_availability() {
    let mut project = Project::new("C020 Test");
    project.start = today();

    // Create a calendar with only 2 working days per week
    let mut low_avail_cal = Calendar::default();
    low_avail_cal.id = "low_avail".to_string();
    low_avail_cal.working_days = vec![1, 3]; // Only Mon and Wed
    project.calendars.push(low_avail_cal);

    project
        .tasks
        .push(Task::new("task1").duration(Duration::days(5)));

    let solver = CpmSolver::new();
    let schedule = solver.schedule(&project).expect("Should succeed");

    let mut emitter = CollectingEmitter::new();
    let config = AnalysisConfig::default();
    analyze_project(&project, Some(&schedule), &config, &mut emitter);

    // Should have C020 diagnostic for low availability calendar
    assert!(
        emitter
            .diagnostics
            .iter()
            .any(|d| d.code == DiagnosticCode::C020LowAvailability
                && d.message.contains("low_avail")),
        "Should emit C020LowAvailability for calendar with <50% availability"
    );
}

/// Test: C022 - Suspicious working hours emits hint
#[test]
fn c022_suspicious_hours_high() {
    let mut project = Project::new("C022 Test High");
    project.start = today();

    // Create a calendar with 18 hours per day (suspicious, > 16 threshold)
    let mut suspicious_cal = Calendar::default();
    suspicious_cal.id = "too_long".to_string();
    suspicious_cal.working_hours = vec![TimeRange {
        start: 0,     // 00:00
        end: 18 * 60, // 18:00 (18 hours)
    }];
    project.calendars.push(suspicious_cal);

    project
        .tasks
        .push(Task::new("task1").duration(Duration::days(5)));

    let solver = CpmSolver::new();
    let schedule = solver.schedule(&project).expect("Should succeed");

    let mut emitter = CollectingEmitter::new();
    let config = AnalysisConfig::default();
    analyze_project(&project, Some(&schedule), &config, &mut emitter);

    // Should have C022 diagnostic for suspicious hours
    assert!(
        emitter
            .diagnostics
            .iter()
            .any(|d| d.code == DiagnosticCode::C022SuspiciousHours
                && d.message.contains("too_long")),
        "Should emit C022SuspiciousHours for calendar with >12 hours/day"
    );
}

/// Test: C022 - 7-day week with full hours is suspicious
#[test]
fn c022_suspicious_seven_day_week() {
    let mut project = Project::new("C022 Test 7-Day");
    project.start = today();

    // Create a calendar with 7 working days and 8 hours
    let mut seven_day_cal = Calendar::default();
    seven_day_cal.id = "seven_day".to_string();
    seven_day_cal.working_days = vec![0, 1, 2, 3, 4, 5, 6]; // All 7 days
    seven_day_cal.working_hours = vec![TimeRange {
        start: 9 * 60,  // 09:00
        end: 17 * 60,   // 17:00 (8 hours)
    }];
    project.calendars.push(seven_day_cal);

    project
        .tasks
        .push(Task::new("task1").duration(Duration::days(5)));

    let solver = CpmSolver::new();
    let schedule = solver.schedule(&project).expect("Should succeed");

    let mut emitter = CollectingEmitter::new();
    let config = AnalysisConfig::default();
    analyze_project(&project, Some(&schedule), &config, &mut emitter);

    // Should have C022 diagnostic for 7-day week
    assert!(
        emitter
            .diagnostics
            .iter()
            .any(|d| d.code == DiagnosticCode::C022SuspiciousHours
                && d.message.contains("seven_day")
                && d.message.contains("7-day")),
        "Should emit C022SuspiciousHours for 7-day workweek"
    );
}

/// Test: C023 - Redundant holiday emits hint
#[test]
fn c023_redundant_holiday() {
    let mut project = Project::new("C023 Test");
    project.start = today();

    // Find the next Sunday from today
    let mut sunday = today();
    while sunday.weekday() != chrono::Weekday::Sun {
        sunday = sunday.succ_opt().unwrap();
    }

    // Create a calendar with Mon-Fri working days and a holiday on Sunday
    let mut cal = Calendar::default();
    cal.id = "with_redundant".to_string();
    cal.working_days = vec![1, 2, 3, 4, 5]; // Mon-Fri (Sunday is already non-working)
    cal.holidays = vec![Holiday {
        name: "Redundant Holiday".to_string(),
        start: sunday,
        end: sunday,
    }];
    project.calendars.push(cal);

    project
        .tasks
        .push(Task::new("task1").duration(Duration::days(5)));

    let solver = CpmSolver::new();
    let schedule = solver.schedule(&project).expect("Should succeed");

    let mut emitter = CollectingEmitter::new();
    let config = AnalysisConfig::default();
    analyze_project(&project, Some(&schedule), &config, &mut emitter);

    // Should have C023 diagnostic for redundant holiday
    assert!(
        emitter
            .diagnostics
            .iter()
            .any(|d| d.code == DiagnosticCode::C023RedundantHoliday
                && d.message.contains("Redundant Holiday")),
        "Should emit C023RedundantHoliday for holiday on non-working day"
    );
}

/// Test: Valid calendar emits no calendar diagnostics
#[test]
fn valid_calendar_no_diagnostics() {
    let mut project = Project::new("Valid Calendar");
    project.start = today();

    // Standard Mon-Fri, 8-hour calendar
    let mut standard_cal = Calendar::default();
    standard_cal.id = "standard".to_string();
    standard_cal.working_days = vec![1, 2, 3, 4, 5]; // Mon-Fri
    standard_cal.working_hours = vec![TimeRange {
        start: 9 * 60,  // 09:00
        end: 17 * 60,   // 17:00 (8 hours)
    }];
    project.calendars.push(standard_cal);
    project.calendar = "standard".to_string();

    project
        .tasks
        .push(Task::new("task1").duration(Duration::days(5)));

    let solver = CpmSolver::new();
    let schedule = solver.schedule(&project).expect("Should succeed");

    let mut emitter = CollectingEmitter::new();
    let config = AnalysisConfig::default();
    analyze_project(&project, Some(&schedule), &config, &mut emitter);

    // Should have no C* diagnostics
    let calendar_diagnostics: Vec<_> = emitter
        .diagnostics
        .iter()
        .filter(|d| d.code.as_str().starts_with("C"))
        .collect();

    assert!(
        calendar_diagnostics.is_empty(),
        "Valid calendar should not emit calendar diagnostics, got: {:?}",
        calendar_diagnostics
    );
}
