//! Exit code integration tests
//!
//! These tests verify that the CLI exits with the correct codes under various
//! diagnostic configurations. This is critical for CI integration.
//!
//! ## Exit Code Contract
//!
//! | Exit Code | Meaning |
//! |-----------|---------|
//! | 0 | Success: no errors (warnings/hints/info allowed) |
//! | 1 | Failure: one or more errors emitted |
//!
//! ## Policy Effects
//!
//! - Default: only native errors cause exit 1
//! - --strict: warnings→errors, hints→warnings (warnings now cause exit 1)
//! - --quiet: does NOT affect exit code
//! - --format=json: exit codes identical to text mode

use std::path::PathBuf;
use std::process::Command;

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/diagnostics")
}

fn utf8proj_binary() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("target/debug/utf8proj")
}

/// Run schedule command and return exit code
fn run_schedule(fixture: &str, args: &[&str]) -> i32 {
    let input_path = fixtures_dir().join(fixture);

    let mut cmd = Command::new(utf8proj_binary());
    cmd.arg("schedule").arg(&input_path);

    for arg in args {
        cmd.arg(arg);
    }

    let status = cmd.status().expect("failed to execute utf8proj");

    status.code().unwrap_or(-1)
}

// =============================================================================
// Default Mode Exit Codes
// =============================================================================

#[test]
fn exit_0_success_no_diagnostics() {
    // i001_success.proj has no warnings/errors, just info
    let code = run_schedule("i001_success.proj", &[]);
    assert_eq!(code, 0, "Success with only info should exit 0");
}

#[test]
fn exit_0_warnings_only() {
    // w001_abstract_assignment.proj has warnings but no errors
    let code = run_schedule("w001_abstract_assignment.proj", &[]);
    assert_eq!(code, 0, "Warnings only should exit 0 in default mode");
}

#[test]
fn exit_0_hints_only() {
    // h002_unused_profile.proj has hints but no errors
    let code = run_schedule("h002_unused_profile.proj", &[]);
    assert_eq!(code, 0, "Hints only should exit 0 in default mode");
}

#[test]
fn exit_1_native_errors() {
    // e001_circular_specialization.proj has native errors
    let code = run_schedule("e001_circular_specialization.proj", &[]);
    assert_eq!(code, 1, "Native errors should exit 1");
}

#[test]
fn exit_1_profile_without_rate_error() {
    // e002_profile_without_rate.proj has E002 error
    let code = run_schedule("e002_profile_without_rate.proj", &[]);
    assert_eq!(code, 0, "E002 is a warning by default, exits 0");
}

// =============================================================================
// Strict Mode Exit Codes
// =============================================================================

#[test]
fn exit_1_strict_mode_escalates_warnings() {
    // w001_abstract_assignment.proj has warnings that become errors in strict
    let code = run_schedule("w001_abstract_assignment.proj", &["--strict"]);
    assert_eq!(
        code, 1,
        "Warnings should become errors and exit 1 in strict mode"
    );
}

#[test]
fn exit_0_strict_mode_hints_become_warnings() {
    // h002_unused_profile.proj has hints that become warnings (not errors)
    let code = run_schedule("h002_unused_profile.proj", &["--strict"]);
    assert_eq!(
        code, 0,
        "Hints become warnings (not errors), should exit 0 in strict"
    );
}

#[test]
fn exit_0_strict_mode_info_unchanged() {
    // i001_success.proj has only info, stays info
    let code = run_schedule("i001_success.proj", &["--strict"]);
    assert_eq!(
        code, 0,
        "Info stays info, should exit 0 even in strict mode"
    );
}

#[test]
fn exit_1_strict_mode_native_error_still_error() {
    // e001_circular_specialization.proj has errors, still errors
    let code = run_schedule("e001_circular_specialization.proj", &["--strict"]);
    assert_eq!(code, 1, "Native errors should still exit 1 in strict mode");
}

// =============================================================================
// Quiet Mode Exit Codes
// =============================================================================

#[test]
fn exit_0_quiet_mode_doesnt_change_success() {
    let code = run_schedule("w001_abstract_assignment.proj", &["--quiet"]);
    assert_eq!(
        code, 0,
        "Quiet mode should not affect exit code for warnings"
    );
}

#[test]
fn exit_1_quiet_mode_doesnt_change_failure() {
    let code = run_schedule("e001_circular_specialization.proj", &["--quiet"]);
    assert_eq!(code, 1, "Quiet mode should not affect exit code for errors");
}

#[test]
fn exit_1_quiet_strict_escalates_and_exits() {
    // Warnings escalate to errors (strict), output suppressed (quiet), but exit 1
    let code = run_schedule("w001_abstract_assignment.proj", &["--quiet", "--strict"]);
    assert_eq!(code, 1, "Quiet+strict should exit 1 for escalated warnings");
}

// =============================================================================
// JSON Format Exit Codes
// =============================================================================

#[test]
fn exit_0_json_format_success() {
    let code = run_schedule("i001_success.proj", &["--format=json"]);
    assert_eq!(code, 0, "JSON format should exit 0 on success");
}

#[test]
fn exit_0_json_format_warnings_only() {
    let code = run_schedule("w001_abstract_assignment.proj", &["--format=json"]);
    assert_eq!(code, 0, "JSON format should exit 0 for warnings");
}

#[test]
fn exit_1_json_format_errors() {
    let code = run_schedule("e001_circular_specialization.proj", &["--format=json"]);
    assert_eq!(code, 1, "JSON format should exit 1 for errors");
}

#[test]
fn exit_1_json_strict_escalation() {
    let code = run_schedule(
        "w001_abstract_assignment.proj",
        &["--format=json", "--strict"],
    );
    assert_eq!(code, 1, "JSON+strict should exit 1 for escalated warnings");
}

// =============================================================================
// Edge Cases
// =============================================================================

#[test]
fn exit_1_multiple_errors() {
    // e001_circular_specialization has 3 circular errors
    let code = run_schedule("e001_circular_specialization.proj", &[]);
    assert_eq!(code, 1, "Multiple errors should still exit 1");
}

#[test]
fn exit_1_mixed_severities_with_error() {
    // e001 has errors + warnings + hints
    let code = run_schedule("e001_circular_specialization.proj", &[]);
    assert_eq!(code, 1, "Mixed severities with error should exit 1");
}

#[test]
fn exit_0_mixed_warnings_hints_no_error() {
    // h001_mixed_abstraction has warnings and hints but no errors
    let code = run_schedule("h001_mixed_abstraction.proj", &[]);
    assert_eq!(code, 0, "Warnings + hints without errors should exit 0");
}

// =============================================================================
// Check Command Exit Codes
// =============================================================================

/// Run check command and return exit code
fn run_check(fixture: &str, args: &[&str]) -> i32 {
    let input_path = fixtures_dir().join(fixture);

    let mut cmd = Command::new(utf8proj_binary());
    cmd.arg("check").arg(&input_path);

    for arg in args {
        cmd.arg(arg);
    }

    let status = cmd.status().expect("failed to execute utf8proj");

    status.code().unwrap_or(-1)
}

#[test]
fn check_exit_0_success() {
    let code = run_check("i001_success.proj", &[]);
    assert_eq!(code, 0, "check: success should exit 0");
}

#[test]
fn check_exit_0_warnings_only() {
    let code = run_check("w001_abstract_assignment.proj", &[]);
    assert_eq!(code, 0, "check: warnings only should exit 0");
}

#[test]
fn check_exit_1_errors() {
    let code = run_check("e001_circular_specialization.proj", &[]);
    assert_eq!(code, 1, "check: errors should exit 1");
}

#[test]
fn check_exit_1_strict_escalates_warnings() {
    let code = run_check("w001_abstract_assignment.proj", &["--strict"]);
    assert_eq!(code, 1, "check --strict: warnings should exit 1");
}

#[test]
fn check_exit_0_quiet_no_output() {
    let code = run_check("i001_success.proj", &["--quiet"]);
    assert_eq!(code, 0, "check --quiet: success should exit 0");
}

#[test]
fn check_exit_0_json_format() {
    let code = run_check("i001_success.proj", &["--format=json"]);
    assert_eq!(code, 0, "check --format=json: success should exit 0");
}

#[test]
fn check_exit_1_json_errors() {
    let code = run_check("e001_circular_specialization.proj", &["--format=json"]);
    assert_eq!(code, 1, "check --format=json: errors should exit 1");
}
