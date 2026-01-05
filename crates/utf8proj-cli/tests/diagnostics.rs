//! Diagnostic snapshot tests
//!
//! These tests verify that diagnostics are emitted correctly by comparing
//! actual CLI output against expected `.stderr` files.
//!
//! # Adding a new diagnostic test
//!
//! 1. Create `{code}_{description}.proj` with a minimal reproduction
//! 2. Create `{code}_{description}.stderr` with expected output
//! 3. Optionally create `{code}_{description}.strict.stderr` for --strict mode
//! 4. Optionally create `{code}_{description}.json` for JSON output
//! 5. Add a test case below

use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/diagnostics")
}

fn utf8proj_binary() -> PathBuf {
    // Use debug build for testing
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("target/debug/utf8proj")
}

/// Run utf8proj schedule on a fixture and return stderr
fn run_schedule(fixture: &str, strict: bool) -> String {
    let input_path = fixtures_dir().join(fixture);

    let mut cmd = Command::new(utf8proj_binary());
    cmd.arg("schedule").arg(&input_path);

    if strict {
        cmd.arg("--strict");
    }

    let output = cmd.output().expect("failed to execute utf8proj");

    // Normalize paths in output for reproducibility
    let stderr = String::from_utf8_lossy(&output.stderr)
        .replace(input_path.to_str().unwrap(), fixture);

    stderr.to_string()
}

/// Run utf8proj schedule with JSON output
fn run_schedule_json(fixture: &str) -> String {
    let input_path = fixtures_dir().join(fixture);

    let output = Command::new(utf8proj_binary())
        .arg("schedule")
        .arg("--format=json")
        .arg(&input_path)
        .output()
        .expect("failed to execute utf8proj");

    String::from_utf8_lossy(&output.stdout).to_string()
}

/// Compare actual output against expected fixture
fn assert_stderr_matches(fixture_base: &str, strict: bool) {
    let proj_file = format!("{}.proj", fixture_base);
    let stderr_file = if strict {
        format!("{}.strict.stderr", fixture_base)
    } else {
        format!("{}.stderr", fixture_base)
    };

    let expected_path = fixtures_dir().join(&stderr_file);
    let expected = fs::read_to_string(&expected_path)
        .unwrap_or_else(|_| panic!("Missing expected output: {}", stderr_file));

    let actual = run_schedule(&proj_file, strict);

    if actual != expected {
        // Pretty diff for debugging
        eprintln!("=== Expected ({}) ===", stderr_file);
        eprintln!("{}", expected);
        eprintln!("=== Actual ===");
        eprintln!("{}", actual);
        eprintln!("=== End ===");
        panic!("Diagnostic output mismatch for {}", fixture_base);
    }
}

// =============================================================================
// Warning Tests
// =============================================================================

#[test]
fn w001_abstract_assignment() {
    assert_stderr_matches("w001_abstract_assignment", false);
}

#[test]
fn w001_abstract_assignment_strict() {
    assert_stderr_matches("w001_abstract_assignment", true);
}

#[test]
fn w002_wide_cost_range() {
    assert_stderr_matches("w002_wide_cost_range", false);
}

#[test]
fn w003_unknown_trait() {
    assert_stderr_matches("w003_unknown_trait", false);
}

#[test]
fn w004_approximate_leveling() {
    assert_stderr_matches("w004_approximate_leveling", false);
}

// =============================================================================
// Hint Tests
// =============================================================================

#[test]
fn h001_mixed_abstraction() {
    assert_stderr_matches("h001_mixed_abstraction", false);
}

#[test]
fn h002_unused_profile() {
    assert_stderr_matches("h002_unused_profile", false);
}

#[test]
fn h003_unused_trait() {
    assert_stderr_matches("h003_unused_trait", false);
}

// =============================================================================
// Error Tests
// =============================================================================

#[test]
fn e001_circular_specialization() {
    assert_stderr_matches("e001_circular_specialization", false);
}

#[test]
fn e002_profile_without_rate() {
    assert_stderr_matches("e002_profile_without_rate", false);
}

#[test]
fn e002_profile_without_rate_strict() {
    assert_stderr_matches("e002_profile_without_rate", true);
}

// =============================================================================
// Info Tests
// =============================================================================

#[test]
fn i001_success() {
    assert_stderr_matches("i001_success", false);
}

// =============================================================================
// JSON Output Tests
// =============================================================================

#[test]
fn w001_json_output() {
    let expected_path = fixtures_dir().join("w001_abstract_assignment.json");
    let expected: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(&expected_path).unwrap()
    ).unwrap();

    let actual_str = run_schedule_json("w001_abstract_assignment.proj");
    let actual: serde_json::Value = serde_json::from_str(&actual_str)
        .expect("CLI output is not valid JSON");

    // Compare diagnostics array (order matters per spec)
    assert_eq!(
        expected["diagnostics"],
        actual["diagnostics"],
        "Diagnostic JSON mismatch"
    );
}

// =============================================================================
// Regression Tests
// =============================================================================

/// Ensure all fixtures have corresponding expected output files
#[test]
fn all_fixtures_have_expected_output() {
    let fixtures = fixtures_dir();

    for entry in fs::read_dir(&fixtures).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();

        if path.extension().map_or(false, |e| e == "proj") {
            let stem = path.file_stem().unwrap().to_str().unwrap();
            let stderr_path = fixtures.join(format!("{}.stderr", stem));

            assert!(
                stderr_path.exists(),
                "Missing .stderr file for {}",
                path.display()
            );
        }
    }
}
