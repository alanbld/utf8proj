//! Tests for the `utf8proj init` command

use std::fs;
use std::path::PathBuf;
use std::process::Command;
use tempfile::tempdir;

fn utf8proj_binary() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("target/debug/utf8proj")
}

#[test]
fn init_creates_project_file() {
    let dir = tempdir().unwrap();
    let expected_file = dir.path().join("test-project.proj");

    let output = Command::new(utf8proj_binary())
        .args(["init", "test-project", "-o"])
        .arg(dir.path())
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success(), "Command should succeed");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Created:"), "Should show 'Created:'");
    assert!(stdout.contains("test-project.proj"), "Should show filename");
    assert!(expected_file.exists(), "File should be created");

    // Verify content has expected structure
    let content = fs::read_to_string(&expected_file).unwrap();
    assert!(
        content.contains("project \"test-project\""),
        "Should have project declaration"
    );
    assert!(
        content.contains("task planning"),
        "Should have planning task"
    );
    assert!(content.contains("depends:"), "Should have dependencies");
}

#[test]
fn init_refuses_overwrite() {
    let dir = tempdir().unwrap();
    let existing_file = dir.path().join("existing.proj");

    // Create existing file
    fs::write(&existing_file, "# existing").unwrap();

    let output = Command::new(utf8proj_binary())
        .args(["init", "existing", "-o"])
        .arg(dir.path())
        .output()
        .expect("Failed to execute command");

    assert!(!output.status.success(), "Command should fail");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("already exists"),
        "Should say file already exists"
    );
}

#[test]
fn init_sanitizes_filename() {
    let dir = tempdir().unwrap();

    let output = Command::new(utf8proj_binary())
        .args(["init", "My Cool Project!", "-o"])
        .arg(dir.path())
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success(), "Command should succeed");

    // Special chars replaced with underscores
    assert!(
        dir.path().join("My_Cool_Project_.proj").exists(),
        "Filename should be sanitized"
    );
}

#[test]
fn init_generated_file_schedules() {
    let dir = tempdir().unwrap();
    let project_file = dir.path().join("demo.proj");

    // Create project
    let output = Command::new(utf8proj_binary())
        .args(["init", "demo", "-o"])
        .arg(dir.path())
        .output()
        .expect("Failed to execute command");
    assert!(output.status.success(), "init should succeed");

    // Schedule it
    let output = Command::new(utf8proj_binary())
        .args(["schedule"])
        .arg(&project_file)
        .output()
        .expect("Failed to execute schedule");

    assert!(output.status.success(), "schedule should succeed");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Critical Path:"),
        "Should show critical path"
    );
}

#[test]
fn init_default_name() {
    let dir = tempdir().unwrap();

    let output = Command::new(utf8proj_binary())
        .args(["init", "-o"])
        .arg(dir.path())
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success(), "Command should succeed");
    assert!(
        dir.path().join("my-project.proj").exists(),
        "Default name should be my-project"
    );
}
