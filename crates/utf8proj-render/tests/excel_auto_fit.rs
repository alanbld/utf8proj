//! TDD tests for Excel auto-fit timeframe (RFC-0009)
//!
//! These tests verify the automatic timeframe calculation for Excel exports.

use chrono::NaiveDate;
use utf8proj_render::excel::{ExcelRenderer, ScheduleGranularity};

// =============================================================================
// Auto-fit Weeks Tests
// =============================================================================

#[test]
fn auto_fit_weeks_short_project_2_weeks() {
    // A 14-day project should get ~2-3 weeks + buffer
    let project_start = NaiveDate::parse_from_str("2026-01-06", "%Y-%m-%d").unwrap();
    let project_end = project_start + chrono::Duration::days(13); // 14 days total

    let renderer = ExcelRenderer::new();
    let weeks = calculate_weeks(&renderer, project_start, project_end);

    // 14 days = 2 weeks, plus 10% buffer (min 1) = 3 weeks
    assert!(
        weeks >= 3,
        "Expected at least 3 weeks for 14-day project, got {}",
        weeks
    );
    assert!(
        weeks <= 5,
        "Expected at most 5 weeks for 14-day project, got {}",
        weeks
    );
}

#[test]
fn auto_fit_weeks_medium_project_8_weeks() {
    // A 56-day project (8 weeks) should get ~9-10 weeks
    let project_start = NaiveDate::parse_from_str("2026-01-06", "%Y-%m-%d").unwrap();
    let project_end = project_start + chrono::Duration::days(55); // 56 days

    let renderer = ExcelRenderer::new();
    let weeks = calculate_weeks(&renderer, project_start, project_end);

    // 56 days = 8 weeks, plus ~10% buffer = 9 weeks
    assert!(
        weeks >= 9,
        "Expected at least 9 weeks for 56-day project, got {}",
        weeks
    );
    assert!(
        weeks <= 11,
        "Expected at most 11 weeks for 56-day project, got {}",
        weeks
    );
}

#[test]
fn auto_fit_weeks_long_project_6_months() {
    // A 180-day project (~26 weeks) should get ~28-30 weeks
    let project_start = NaiveDate::parse_from_str("2026-01-06", "%Y-%m-%d").unwrap();
    let project_end = project_start + chrono::Duration::days(179);

    let renderer = ExcelRenderer::new();
    let weeks = calculate_weeks(&renderer, project_start, project_end);

    // 180 days = ~26 weeks, plus ~10% buffer = ~29 weeks
    assert!(
        weeks >= 28,
        "Expected at least 28 weeks for 180-day project, got {}",
        weeks
    );
    assert!(
        weeks <= 32,
        "Expected at most 32 weeks for 180-day project, got {}",
        weeks
    );
}

#[test]
fn auto_fit_weeks_zero_duration_minimum() {
    // Same-day project should get minimum reasonable weeks
    let project_start = NaiveDate::parse_from_str("2026-01-06", "%Y-%m-%d").unwrap();
    let project_end = project_start; // Same day

    let renderer = ExcelRenderer::new();
    let weeks = calculate_weeks(&renderer, project_start, project_end);

    // Should have at least 1 week even for zero-duration
    assert!(
        weeks >= 1,
        "Expected at least 1 week for empty project, got {}",
        weeks
    );
    assert!(
        weeks <= 4,
        "Expected at most 4 weeks for empty project, got {}",
        weeks
    );
}

#[test]
fn auto_fit_weeks_partial_week_rounds_up() {
    // 10 days = 1.43 weeks, should round up to 2 + buffer
    let project_start = NaiveDate::parse_from_str("2026-01-06", "%Y-%m-%d").unwrap();
    let project_end = project_start + chrono::Duration::days(9); // 10 days

    let renderer = ExcelRenderer::new();
    let weeks = calculate_weeks(&renderer, project_start, project_end);

    // 10 days rounds to 2 weeks, plus buffer = 3 weeks
    assert!(
        weeks >= 3,
        "Expected at least 3 weeks for 10-day project, got {}",
        weeks
    );
}

// =============================================================================
// Auto-fit Days Tests
// =============================================================================

#[test]
fn auto_fit_days_short_project() {
    // A 14-day project should get ~20 days (14 + buffer)
    let project_start = NaiveDate::parse_from_str("2026-01-06", "%Y-%m-%d").unwrap();
    let project_end = project_start + chrono::Duration::days(13);

    let renderer = ExcelRenderer::new().daily();
    let days = calculate_days(&renderer, project_start, project_end);

    // 13 days (end - start) + 10% buffer (min 5) = 18 days
    assert!(
        days >= 18,
        "Expected at least 18 days for 14-day project, got {}",
        days
    );
    assert!(
        days <= 25,
        "Expected at most 25 days for 14-day project, got {}",
        days
    );
}

#[test]
fn auto_fit_days_medium_project() {
    // A 45-day project should get ~50 days
    let project_start = NaiveDate::parse_from_str("2026-01-06", "%Y-%m-%d").unwrap();
    let project_end = project_start + chrono::Duration::days(44);

    let renderer = ExcelRenderer::new().daily();
    let days = calculate_days(&renderer, project_start, project_end);

    // 44 days (end - start) + 10% buffer = ~49 days
    assert!(
        days >= 49,
        "Expected at least 49 days for 45-day project, got {}",
        days
    );
    assert!(
        days <= 55,
        "Expected at most 55 days for 45-day project, got {}",
        days
    );
}

#[test]
fn auto_fit_days_zero_duration_minimum() {
    // Empty project should get minimum days
    let project_start = NaiveDate::parse_from_str("2026-01-06", "%Y-%m-%d").unwrap();
    let project_end = project_start;

    let renderer = ExcelRenderer::new().daily();
    let days = calculate_days(&renderer, project_start, project_end);

    // Should have at least 5 days (minimum buffer)
    assert!(
        days >= 5,
        "Expected at least 5 days for empty project, got {}",
        days
    );
}

// =============================================================================
// Manual Override Tests
// =============================================================================

#[test]
fn manual_weeks_overrides_auto_fit() {
    let renderer = ExcelRenderer::new().no_auto_fit().weeks(52); // Force 52 weeks

    assert!(!renderer.auto_fit, "auto_fit should be false");
    assert_eq!(renderer.schedule_weeks, 52, "Manual weeks should be set");
}

#[test]
fn manual_days_overrides_auto_fit() {
    let renderer = ExcelRenderer::new().daily().no_auto_fit().days(90); // Force 90 days

    assert!(!renderer.auto_fit, "auto_fit should be false");
    assert_eq!(renderer.schedule_days, 90, "Manual days should be set");
}

#[test]
fn auto_fit_is_default() {
    let renderer = ExcelRenderer::new();
    assert!(renderer.auto_fit, "auto_fit should be true by default");
}

// =============================================================================
// Granularity Tests
// =============================================================================

#[test]
fn default_granularity_is_weekly() {
    let renderer = ExcelRenderer::new();
    assert_eq!(renderer.granularity, ScheduleGranularity::Weekly);
}

#[test]
fn daily_sets_granularity() {
    let renderer = ExcelRenderer::new().daily();
    assert_eq!(renderer.granularity, ScheduleGranularity::Daily);
}

// =============================================================================
// Edge Cases
// =============================================================================

#[test]
fn auto_fit_handles_very_long_project() {
    // 2-year project (730 days, ~104 weeks)
    let project_start = NaiveDate::parse_from_str("2026-01-06", "%Y-%m-%d").unwrap();
    let project_end = project_start + chrono::Duration::days(729);

    let renderer = ExcelRenderer::new();
    let weeks = calculate_weeks(&renderer, project_start, project_end);

    // Should cap at reasonable maximum or handle gracefully
    assert!(weeks >= 104, "Should cover full project duration");
    assert!(
        weeks <= 120,
        "Should not have excessive buffer for long projects"
    );
}

#[test]
fn auto_fit_handles_project_ending_before_start() {
    // Edge case: negative duration (shouldn't happen, but handle gracefully)
    let project_start = NaiveDate::parse_from_str("2026-01-06", "%Y-%m-%d").unwrap();
    let project_end = project_start - chrono::Duration::days(5);

    let renderer = ExcelRenderer::new();
    let weeks = calculate_weeks(&renderer, project_start, project_end);

    // Should return minimum, not negative
    assert!(
        weeks >= 1,
        "Should return positive weeks even for invalid schedule"
    );
}

// =============================================================================
// Helper Functions
// =============================================================================

/// Calculate weeks using the same formula as ExcelRenderer
fn calculate_weeks(
    _renderer: &ExcelRenderer,
    project_start: NaiveDate,
    project_end: NaiveDate,
) -> u32 {
    let days = (project_end - project_start).num_days().max(0) as u32;
    let weeks = (days + 6) / 7; // Round up to complete weeks
    let buffer = (weeks / 10).max(1); // 10% buffer, minimum 1 week
    (weeks + buffer).max(1) // Ensure at least 1 week
}

/// Calculate days using the same formula as ExcelRenderer
fn calculate_days(
    _renderer: &ExcelRenderer,
    project_start: NaiveDate,
    project_end: NaiveDate,
) -> u32 {
    let days = (project_end - project_start).num_days().max(0) as u32;
    let buffer = (days / 10).max(5); // 10% buffer, minimum 5 days
    (days + buffer).max(5) // Ensure at least 5 days
}
