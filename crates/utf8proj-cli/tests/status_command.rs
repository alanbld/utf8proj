//! E2E tests for the status command (RFC-0019)
//!
//! These tests verify the status command output in various formats.

use std::path::PathBuf;
use std::process::Command;

fn examples_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("examples")
}

fn utf8proj_binary() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("target/debug/utf8proj")
}

/// Run status command and return (exit_code, stdout, stderr)
fn run_status(file: &str, args: &[&str]) -> (i32, String, String) {
    let input_path = examples_dir().join(file);

    let mut cmd = Command::new(utf8proj_binary());
    cmd.arg("status").arg(&input_path);

    for arg in args {
        cmd.arg(arg);
    }

    let output = cmd.output().expect("failed to execute utf8proj");

    let exit_code = output.status.code().unwrap_or(-1);
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    (exit_code, stdout, stderr)
}

// =============================================================================
// Text Output Tests
// =============================================================================

#[test]
fn test_status_text_contains_project_name() {
    let (code, stdout, _) = run_status("crm_simple.proj", &[]);
    assert_eq!(code, 0);
    assert!(
        stdout.contains("CRM Migration to Salesforce"),
        "Output should contain project name"
    );
}

#[test]
fn test_status_text_shows_progress_bar() {
    let (code, stdout, _) = run_status("crm_simple.proj", &[]);
    assert_eq!(code, 0);
    assert!(stdout.contains("Progress:"), "Output should show progress bar");
    assert!(stdout.contains("%"), "Output should show percentage");
}

#[test]
fn test_status_text_shows_status_indicator() {
    let (code, stdout, _) = run_status("crm_simple.proj", &[]);
    assert_eq!(code, 0);
    assert!(stdout.contains("Status:"), "Output should show status indicator");
    // Should be one of: ON TRACK, AT RISK, BEHIND
    assert!(
        stdout.contains("ON TRACK") || stdout.contains("AT RISK") || stdout.contains("BEHIND"),
        "Output should contain status indicator"
    );
}

#[test]
fn test_status_text_shows_schedule_dates() {
    let (code, stdout, _) = run_status("crm_simple.proj", &[]);
    assert_eq!(code, 0);
    assert!(stdout.contains("Start:"), "Output should show start date");
    assert!(stdout.contains("Baseline:"), "Output should show baseline finish");
    assert!(stdout.contains("Forecast:"), "Output should show forecast finish");
    assert!(stdout.contains("Variance:"), "Output should show variance");
}

#[test]
fn test_status_text_shows_earned_value() {
    let (code, stdout, _) = run_status("crm_simple.proj", &[]);
    assert_eq!(code, 0);
    assert!(stdout.contains("PV:"), "Output should show Planned Value");
    assert!(stdout.contains("EV:"), "Output should show Earned Value");
    assert!(stdout.contains("SPI:"), "Output should show SPI");
}

#[test]
fn test_status_text_shows_task_breakdown() {
    let (code, stdout, _) = run_status("crm_simple.proj", &[]);
    assert_eq!(code, 0);
    assert!(stdout.contains("Tasks:"), "Output should show task count");
    assert!(stdout.contains("Complete:"), "Output should show completed count");
    assert!(stdout.contains("In Progress:"), "Output should show in-progress count");
    assert!(stdout.contains("Not Started:"), "Output should show not-started count");
    assert!(stdout.contains("Critical Path:"), "Output should show critical path length");
}

#[test]
fn test_status_text_shows_days_remaining() {
    let (code, stdout, _) = run_status("crm_simple.proj", &[]);
    assert_eq!(code, 0);
    assert!(
        stdout.contains("days remaining") || stdout.contains("completes today") || stdout.contains("days past"),
        "Output should show days remaining"
    );
}

// =============================================================================
// JSON Output Tests
// =============================================================================

#[test]
fn test_status_json_valid_structure() {
    let (code, stdout, _) = run_status("crm_simple.proj", &["--format", "json"]);
    assert_eq!(code, 0);

    // Parse JSON to verify it's valid
    let parsed: serde_json::Value =
        serde_json::from_str(&stdout).expect("Output should be valid JSON");

    assert!(parsed.is_object(), "JSON output should be an object");
}

#[test]
fn test_status_json_has_all_required_fields() {
    let (code, stdout, _) = run_status("crm_simple.proj", &["--format", "json"]);
    assert_eq!(code, 0);

    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();

    // Top-level fields
    assert!(
        parsed.get("project_name").is_some(),
        "JSON should have project_name"
    );
    assert!(
        parsed.get("status_date").is_some(),
        "JSON should have status_date"
    );
    assert!(
        parsed.get("progress").is_some(),
        "JSON should have progress"
    );
    assert!(
        parsed.get("schedule").is_some(),
        "JSON should have schedule"
    );
    assert!(
        parsed.get("earned_value").is_some(),
        "JSON should have earned_value"
    );
    assert!(parsed.get("tasks").is_some(), "JSON should have tasks");
}

#[test]
fn test_status_json_progress_fields() {
    let (code, stdout, _) = run_status("crm_simple.proj", &["--format", "json"]);
    assert_eq!(code, 0);

    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let progress = parsed.get("progress").unwrap();

    assert!(
        progress.get("percent").is_some(),
        "Progress should have percent"
    );
    assert!(
        progress.get("status").is_some(),
        "Progress should have status"
    );
    assert!(
        progress.get("variance_days").is_some(),
        "Progress should have variance_days"
    );
}

#[test]
fn test_status_json_schedule_fields() {
    let (code, stdout, _) = run_status("crm_simple.proj", &["--format", "json"]);
    assert_eq!(code, 0);

    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let schedule = parsed.get("schedule").unwrap();

    assert!(schedule.get("start").is_some(), "Schedule should have start");
    assert!(
        schedule.get("baseline_finish").is_some(),
        "Schedule should have baseline_finish"
    );
    assert!(
        schedule.get("forecast_finish").is_some(),
        "Schedule should have forecast_finish"
    );
    assert!(
        schedule.get("days_remaining").is_some(),
        "Schedule should have days_remaining"
    );
}

#[test]
fn test_status_json_earned_value_fields() {
    let (code, stdout, _) = run_status("crm_simple.proj", &["--format", "json"]);
    assert_eq!(code, 0);

    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let ev = parsed.get("earned_value").unwrap();

    assert!(ev.get("pv").is_some(), "Earned value should have pv");
    assert!(ev.get("ev").is_some(), "Earned value should have ev");
    assert!(ev.get("spi").is_some(), "Earned value should have spi");
}

#[test]
fn test_status_json_tasks_fields() {
    let (code, stdout, _) = run_status("crm_simple.proj", &["--format", "json"]);
    assert_eq!(code, 0);

    let parsed: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let tasks = parsed.get("tasks").unwrap();

    assert!(tasks.get("total").is_some(), "Tasks should have total");
    assert!(
        tasks.get("completed").is_some(),
        "Tasks should have completed"
    );
    assert!(
        tasks.get("in_progress").is_some(),
        "Tasks should have in_progress"
    );
    assert!(
        tasks.get("not_started").is_some(),
        "Tasks should have not_started"
    );
    assert!(tasks.get("behind").is_some(), "Tasks should have behind");
    assert!(
        tasks.get("critical_path_length").is_some(),
        "Tasks should have critical_path_length"
    );
}

// =============================================================================
// Custom Status Date Tests
// =============================================================================

#[test]
fn test_status_custom_date_overrides_project() {
    let (code1, stdout1, _) = run_status("crm_simple.proj", &["--format", "json"]);
    let (code2, stdout2, _) = run_status(
        "crm_simple.proj",
        &["--format", "json", "--as-of", "2026-02-15"],
    );

    assert_eq!(code1, 0);
    assert_eq!(code2, 0);

    let parsed1: serde_json::Value = serde_json::from_str(&stdout1).unwrap();
    let parsed2: serde_json::Value = serde_json::from_str(&stdout2).unwrap();

    // Status dates should be different
    let date1 = parsed1.get("status_date").unwrap().as_str().unwrap();
    let date2 = parsed2.get("status_date").unwrap().as_str().unwrap();

    assert_eq!(date2, "2026-02-15", "Custom status date should be used");
    assert_ne!(date1, date2, "Dates should be different");
}

#[test]
fn test_status_custom_date_affects_days_remaining() {
    let (code1, stdout1, _) = run_status(
        "crm_simple.proj",
        &["--format", "json", "--as-of", "2026-02-01"],
    );
    let (code2, stdout2, _) = run_status(
        "crm_simple.proj",
        &["--format", "json", "--as-of", "2026-02-20"],
    );

    assert_eq!(code1, 0);
    assert_eq!(code2, 0);

    let parsed1: serde_json::Value = serde_json::from_str(&stdout1).unwrap();
    let parsed2: serde_json::Value = serde_json::from_str(&stdout2).unwrap();

    let days1 = parsed1["schedule"]["days_remaining"].as_i64().unwrap();
    let days2 = parsed2["schedule"]["days_remaining"].as_i64().unwrap();

    // Feb 20 is 19 days after Feb 1, so days_remaining should differ by ~19
    assert!(
        days1 > days2,
        "Earlier status date should have more days remaining"
    );
    assert!(
        (days1 - days2).abs() >= 15,
        "Days difference should be significant"
    );
}

// =============================================================================
// Exit Code Tests
// =============================================================================

#[test]
fn test_status_exit_0_on_success() {
    let (code, _, _) = run_status("crm_simple.proj", &[]);
    assert_eq!(code, 0, "Status command should exit 0 on success");
}

#[test]
fn test_status_exit_0_json_format() {
    let (code, _, _) = run_status("crm_simple.proj", &["--format", "json"]);
    assert_eq!(code, 0, "Status command with JSON should exit 0");
}
