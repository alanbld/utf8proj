//! Fix command integration tests
//!
//! Tests for `utf8proj fix` subcommands, ensuring they preserve all task properties.

use std::fs;
use std::path::PathBuf;
use std::process::Command;
use tempfile::TempDir;

fn utf8proj_binary() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("target/debug/utf8proj")
}

/// Test that fix container-deps preserves effort values when both duration and effort are present
#[test]
fn fix_container_deps_preserves_effort() {
    let temp_dir = TempDir::new().unwrap();
    let input_path = temp_dir.path().join("input.proj");
    let output_path = temp_dir.path().join("output.proj");

    // Create a project file with both duration AND effort on a task
    let input_content = r#"project "Test Project" {
    start: 2026-01-01
}

calendar "standard" {
    working_days: mon-fri
    working_hours: 09:00-17:00
}

task parent "Parent Task" {
    depends: external_task

    task child "Child Task" {
        duration: 5d
        effort: 40d
    }
}
"#;

    fs::write(&input_path, input_content).unwrap();

    // Run fix container-deps
    let status = Command::new(utf8proj_binary())
        .arg("fix")
        .arg("container-deps")
        .arg(&input_path)
        .arg("-o")
        .arg(&output_path)
        .status()
        .expect("failed to execute utf8proj");

    assert!(status.success(), "fix command should succeed");

    // Read the output and verify effort is preserved
    let output_content = fs::read_to_string(&output_path).unwrap();

    // Check that both duration AND effort are present in the output
    assert!(
        output_content.contains("duration: 5d"),
        "duration should be preserved. Output:\n{}",
        output_content
    );
    assert!(
        output_content.contains("effort: 40d"),
        "effort should be preserved when both duration and effort are present. Output:\n{}",
        output_content
    );
}

/// Test that fix container-deps preserves effort when task has only effort (no duration)
#[test]
fn fix_container_deps_preserves_effort_only() {
    let temp_dir = TempDir::new().unwrap();
    let input_path = temp_dir.path().join("input.proj");
    let output_path = temp_dir.path().join("output.proj");

    // Create a project file with only effort (no duration)
    let input_content = r#"project "Test Project" {
    start: 2026-01-01
}

calendar "standard" {
    working_days: mon-fri
}

task parent "Parent Task" {
    depends: external_task

    task child "Child Task" {
        effort: 20d
    }
}
"#;

    fs::write(&input_path, input_content).unwrap();

    // Run fix container-deps
    let status = Command::new(utf8proj_binary())
        .arg("fix")
        .arg("container-deps")
        .arg(&input_path)
        .arg("-o")
        .arg(&output_path)
        .status()
        .expect("failed to execute utf8proj");

    assert!(status.success(), "fix command should succeed");

    // Read the output and verify effort is preserved
    let output_content = fs::read_to_string(&output_path).unwrap();

    assert!(
        output_content.contains("effort: 20d"),
        "effort should be preserved when task has only effort. Output:\n{}",
        output_content
    );
}

/// Test that fix container-deps preserves assignments
#[test]
fn fix_container_deps_preserves_assignments() {
    let temp_dir = TempDir::new().unwrap();
    let input_path = temp_dir.path().join("input.proj");
    let output_path = temp_dir.path().join("output.proj");

    let input_content = r#"project "Test Project" {
    start: 2026-01-01
}

calendar "standard" {
    working_days: mon-fri
}

resource dev "Developer" {
    rate: 500/day
}

task parent "Parent Task" {
    depends: external_task

    task child "Child Task" {
        duration: 5d
        effort: 40d
        assign: dev
    }
}
"#;

    fs::write(&input_path, input_content).unwrap();

    // Run fix container-deps
    let status = Command::new(utf8proj_binary())
        .arg("fix")
        .arg("container-deps")
        .arg(&input_path)
        .arg("-o")
        .arg(&output_path)
        .status()
        .expect("failed to execute utf8proj");

    assert!(status.success(), "fix command should succeed");

    let output_content = fs::read_to_string(&output_path).unwrap();

    assert!(
        output_content.contains("duration: 5d"),
        "duration should be preserved"
    );
    assert!(
        output_content.contains("effort: 40d"),
        "effort should be preserved"
    );
    assert!(
        output_content.contains("assign: dev"),
        "assignments should be preserved. Output:\n{}",
        output_content
    );
}
