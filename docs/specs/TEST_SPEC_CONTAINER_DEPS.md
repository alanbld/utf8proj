# Test Specification: Container Dependency Semantics

**Module:** Container Dependencies & W014 Diagnostic  
**Test Strategy:** TDD (Test-Driven Development)  
**Coverage Target:** 100% for critical paths, 90% overall  
**Status:** Specification (Write tests first)

---

## Test Organization

```
tests/
├── unit/
│   ├── parser/
│   │   └── container_dependencies_test.rs
│   ├── validator/
│   │   └── diagnostic_w014_test.rs
│   └── solver/
│       └── container_scheduling_test.rs
├── integration/
│   ├── msp_import_container_deps_test.rs
│   └── e2e_container_workflow_test.rs
└── fixtures/
    ├── ms_project/
    │   ├── container_deps_simple.mpp
    │   ├── container_deps_complex.mpp
    │   └── container_deps_nested.xml
    └── proj/
        ├── container_deps_explicit.proj
        └── container_deps_implicit.proj
```

---

## Unit Tests

### 1. Parser Tests

**File:** `tests/unit/parser/container_dependencies_test.rs`

```rust
use utf8proj_parser::parse;

#[test]
fn parse_container_with_dependencies() {
    let source = r#"
        task container {
            depends: predecessor
            
            task child {
                duration: 5d
            }
        }
    "#;
    
    let project = parse(source).expect("Should parse");
    let container = project.find_task("container").unwrap();
    
    assert_eq!(container.depends, vec![TaskId::from("predecessor")]);
    assert_eq!(container.children.len(), 1);
    
    let child = &container.children[0];
    assert_eq!(child.depends, vec![]);  // No dependencies
}

#[test]
fn parse_container_with_multiple_deps() {
    let source = r#"
        task container {
            depends: dep1, dep2, dep3
            task child { duration: 5d }
        }
    "#;
    
    let project = parse(source).expect("Should parse");
    let container = project.find_task("container").unwrap();
    
    assert_eq!(container.depends.len(), 3);
    assert!(container.depends.contains(&TaskId::from("dep1")));
    assert!(container.depends.contains(&TaskId::from("dep2")));
    assert!(container.depends.contains(&TaskId::from("dep3")));
}

#[test]
fn parse_nested_containers_with_deps() {
    let source = r#"
        task outer {
            depends: dep_outer
            
            task inner {
                depends: dep_inner
                
                task leaf {
                    duration: 5d
                }
            }
        }
    "#;
    
    let project = parse(source).expect("Should parse");
    
    let outer = project.find_task("outer").unwrap();
    assert_eq!(outer.depends, vec![TaskId::from("dep_outer")]);
    
    let inner = &outer.children[0];
    assert_eq!(inner.depends, vec![TaskId::from("dep_inner")]);
    
    let leaf = &inner.children[0];
    assert_eq!(leaf.depends, vec![]);
}

#[test]
fn parse_child_with_explicit_container_dep() {
    let source = r#"
        task container {
            depends: predecessor
            
            task child {
                depends: predecessor  # Explicit!
                duration: 5d
            }
        }
    "#;
    
    let project = parse(source).expect("Should parse");
    let container = project.find_task("container").unwrap();
    let child = &container.children[0];
    
    assert_eq!(child.depends, vec![TaskId::from("predecessor")]);
}
```

### 2. Validator Tests (W014 Diagnostic)

**File:** `tests/unit/validator/diagnostic_w014_test.rs`

```rust
use utf8proj_validator::{validate, DiagnosticCode};

#[test]
fn w014_triggers_when_child_missing_container_dep() {
    let project = parse(r#"
        task container {
            depends: predecessor
            task child { duration: 5d }
        }
    "#).unwrap();
    
    let diagnostics = validate(&project);
    
    let w014_count = diagnostics.iter()
        .filter(|d| d.code == DiagnosticCode::W014)
        .count();
    
    assert_eq!(w014_count, 1);
}

#[test]
fn w014_does_not_trigger_when_child_has_dep() {
    let project = parse(r#"
        task container {
            depends: predecessor
            task child { 
                depends: predecessor
                duration: 5d 
            }
        }
    "#).unwrap();
    
    let diagnostics = validate(&project);
    
    assert!(!diagnostics.iter().any(|d| d.code == DiagnosticCode::W014));
}

#[test]
fn w014_triggers_for_each_missing_child() {
    let project = parse(r#"
        task container {
            depends: predecessor
            
            task child1 { duration: 5d }  # Missing dep
            task child2 { depends: predecessor, duration: 3d }  # Has dep
            task child3 { duration: 2d }  # Missing dep
        }
    "#).unwrap();
    
    let diagnostics = validate(&project);
    
    let w014_count = diagnostics.iter()
        .filter(|d| d.code == DiagnosticCode::W014)
        .count();
    
    assert_eq!(w014_count, 2);  // child1 and child3
}

#[test]
fn w014_checks_all_container_deps() {
    let project = parse(r#"
        task container {
            depends: dep1, dep2
            
            task child {
                depends: dep1  # Has dep1 but missing dep2
                duration: 5d
            }
        }
    "#).unwrap();
    
    let diagnostics = validate(&project);
    
    assert!(diagnostics.iter().any(|d| d.code == DiagnosticCode::W014));
}

#[test]
fn w014_does_not_trigger_for_empty_container_deps() {
    let project = parse(r#"
        task container {
            # No dependencies
            task child { duration: 5d }
        }
    "#).unwrap();
    
    let diagnostics = validate(&project);
    
    assert!(!diagnostics.iter().any(|d| d.code == DiagnosticCode::W014));
}

#[test]
fn w014_does_not_trigger_for_leaf_tasks() {
    let project = parse(r#"
        task leaf_task {
            depends: predecessor
            duration: 5d
        }
    "#).unwrap();
    
    let diagnostics = validate(&project);
    
    assert!(!diagnostics.iter().any(|d| d.code == DiagnosticCode::W014));
}

#[test]
fn w014_message_includes_task_names() {
    let project = parse(r#"
        task development {
            depends: design_approval
            task feature_x { duration: 5d }
        }
    "#).unwrap();
    
    let diagnostics = validate(&project);
    let w014 = diagnostics.iter()
        .find(|d| d.code == DiagnosticCode::W014)
        .unwrap();
    
    assert!(w014.message.contains("development"));
    assert!(w014.message.contains("feature_x"));
    assert!(w014.message.contains("design_approval"));
}

#[test]
fn w014_suggestion_includes_fix() {
    let project = parse(r#"
        task container {
            depends: predecessor
            task child { duration: 5d }
        }
    "#).unwrap();
    
    let diagnostics = validate(&project);
    let w014 = diagnostics.iter()
        .find(|d| d.code == DiagnosticCode::W014)
        .unwrap();
    
    assert!(w014.suggestion.is_some());
    assert!(w014.suggestion.as_ref().unwrap().contains("depends: predecessor"));
}

#[test]
fn w014_nested_containers() {
    let project = parse(r#"
        task outer {
            depends: dep_outer
            
            task inner {
                depends: dep_inner
                
                task leaf {
                    # Missing both dep_outer and dep_inner
                    duration: 5d
                }
            }
        }
    "#).unwrap();
    
    let diagnostics = validate(&project);
    
    // Should trigger for leaf missing inner's dep
    assert!(diagnostics.iter().any(|d| 
        d.code == DiagnosticCode::W014 && d.message.contains("inner")
    ));
}
```

### 3. Scheduler Tests

**File:** `tests/unit/solver/container_scheduling_test.rs`

```rust
use utf8proj_solver::CpmScheduler;
use chrono::NaiveDate;

fn date(y: i32, m: u32, d: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(y, m, d).unwrap()
}

#[test]
fn container_dep_does_not_block_children() {
    let project = parse(r#"
        project "Test" { start: 2025-01-01 }
        
        task predecessor { duration: 5d }
        
        task container {
            depends: predecessor
            
            task child {
                # No explicit dependency
                duration: 3d
            }
        }
    "#).unwrap();
    
    let schedule = CpmScheduler::new().schedule(&project).unwrap();
    
    // Child can start immediately
    assert_eq!(schedule.tasks["child"].start, date(2025, 1, 1));
    
    // NOT blocked by predecessor
    assert_ne!(schedule.tasks["child"].start, date(2025, 1, 6));
}

#[test]
fn explicit_dep_blocks_child() {
    let project = parse(r#"
        project "Test" { start: 2025-01-01 }
        
        task predecessor { duration: 5d }
        
        task container {
            depends: predecessor
            
            task child {
                depends: predecessor  # Explicit
                duration: 3d
            }
        }
    "#).unwrap();
    
    let schedule = CpmScheduler::new().schedule(&project).unwrap();
    
    // Child blocked by predecessor
    assert_eq!(schedule.tasks["child"].start, date(2025, 1, 6));
}

#[test]
fn container_dates_computed_from_children() {
    let project = parse(r#"
        project "Test" { start: 2025-01-01 }
        
        task container {
            task child1 { duration: 5d }
            task child2 { duration: 3d, depends: child1 }
        }
    "#).unwrap();
    
    let schedule = CpmScheduler::new().schedule(&project).unwrap();
    
    let container = &schedule.tasks["container"];
    
    // Container start = earliest child start
    assert_eq!(container.start, schedule.tasks["child1"].start);
    
    // Container finish = latest child finish
    assert_eq!(container.finish, schedule.tasks["child2"].finish);
}

#[test]
fn container_with_parallel_children() {
    let project = parse(r#"
        project "Test" { start: 2025-01-01 }
        
        task container {
            task child1 { duration: 10d }
            task child2 { duration: 5d }  # Can run in parallel
        }
    "#).unwrap();
    
    let schedule = CpmScheduler::new().schedule(&project).unwrap();
    
    // Both start at same time
    assert_eq!(
        schedule.tasks["child1"].start, 
        schedule.tasks["child2"].start
    );
    
    // Container finishes when longest child finishes
    assert_eq!(
        schedule.tasks["container"].finish,
        schedule.tasks["child1"].finish
    );
}

#[test]
fn mixed_explicit_and_implicit_deps() {
    let project = parse(r#"
        project "Test" { start: 2025-01-01 }
        
        task predecessor { duration: 5d }
        
        task container {
            depends: predecessor
            
            task child1 {
                depends: predecessor  # Explicit
                duration: 5d
            }
            
            task child2 {
                # No explicit dep, but depends on child1
                depends: child1
                duration: 3d
            }
        }
    "#).unwrap();
    
    let schedule = CpmScheduler::new().schedule(&project).unwrap();
    
    // child1 blocked by predecessor
    assert_eq!(schedule.tasks["child1"].start, date(2025, 1, 6));
    
    // child2 blocked by child1
    assert_eq!(schedule.tasks["child2"].start, date(2025, 1, 11));
}

#[test]
fn critical_path_through_container() {
    let project = parse(r#"
        project "Test" { start: 2025-01-01 }
        
        task container {
            task child1 { duration: 10d }
            task child2 { duration: 5d, depends: child1 }
        }
        
        task final_task {
            depends: container.child2
            duration: 3d
        }
    "#).unwrap();
    
    let schedule = CpmScheduler::new().schedule(&project).unwrap();
    
    // Critical path should include child1, child2, final_task
    assert!(schedule.critical_path.contains(&TaskId::from("child1")));
    assert!(schedule.critical_path.contains(&TaskId::from("child2")));
    assert!(schedule.critical_path.contains(&TaskId::from("final_task")));
}
```

---

## Integration Tests

### 4. MS Project Import Tests

**File:** `tests/integration/msp_import_container_deps_test.rs`

```rust
use utf8proj_import::import_mspdi;

#[test]
fn import_msp_container_with_deps() {
    let msp_xml = r#"
        <Project>
          <Tasks>
            <Task><UID>1</UID><n>Predecessor</n><Duration>PT40H</Duration></Task>
            <Task>
              <UID>2</UID>
              <n>Container</n>
              <PredecessorLink>
                <PredecessorUID>1</PredecessorUID>
                <Type>1</Type>  <!-- FS -->
              </PredecessorLink>
            </Task>
            <Task><UID>3</UID><n>Child</n><OutlineLevel>2</OutlineLevel><Duration>PT24H</Duration></Task>
          </Tasks>
        </Project>
    "#;
    
    let project = import_mspdi(msp_xml).expect("Should import");
    
    let container = project.find_task("Container").unwrap();
    assert_eq!(container.depends, vec![TaskId::from("Predecessor")]);
    
    let child = &container.children[0];
    assert_eq!(child.depends, vec![]);  // No dependency in XML
    
    // Should trigger W014
    let diagnostics = validate(&project);
    assert!(diagnostics.iter().any(|d| d.code == DiagnosticCode::W014));
}

#[test]
fn import_with_auto_fix() {
    let msp_xml = /* same as above */;
    
    let project = import_mspdi_with_fix(msp_xml, ImportOptions {
        fix_container_deps: true,
    }).expect("Should import");
    
    let container = project.find_task("Container").unwrap();
    let child = &container.children[0];
    
    // Auto-fix should have added dependency
    assert_eq!(child.depends, vec![TaskId::from("Predecessor")]);
    
    // Should NOT trigger W014
    let diagnostics = validate(&project);
    assert!(!diagnostics.iter().any(|d| d.code == DiagnosticCode::W014));
}

#[test]
fn import_complex_hierarchy() {
    let msp_xml = r#"
        <Project>
          <Tasks>
            <Task><UID>1</UID><n>Kickoff</n><Duration>PT8H</Duration></Task>
            <Task>
              <UID>2</UID><n>Phase 1</n>
              <PredecessorLink><PredecessorUID>1</PredecessorUID></PredecessorLink>
            </Task>
            <Task><UID>3</UID><n>Task A</n><OutlineLevel>2</OutlineLevel><Duration>PT40H</Duration></Task>
            <Task><UID>4</UID><n>Task B</n><OutlineLevel>2</OutlineLevel><Duration>PT24H</Duration></Task>
            <Task>
              <UID>5</UID><n>Phase 2</n>
              <PredecessorLink><PredecessorUID>2</PredecessorUID></PredecessorLink>
            </Task>
            <Task><UID>6</UID><n>Task C</n><OutlineLevel>2</OutlineLevel><Duration>PT32H</Duration></Task>
          </Tasks>
        </Project>
    "#;
    
    let project = import_mspdi(msp_xml).expect("Should import");
    
    // Should detect multiple W014 instances
    let diagnostics = validate(&project);
    let w014_count = diagnostics.iter()
        .filter(|d| d.code == DiagnosticCode::W014)
        .count();
    
    assert_eq!(w014_count, 3);  // Task A, B, C all missing deps
}

#[test]
fn import_preserves_explicit_child_deps() {
    let msp_xml = r#"
        <Project>
          <Tasks>
            <Task><UID>1</UID><n>Predecessor</n><Duration>PT40H</Duration></Task>
            <Task>
              <UID>2</UID><n>Container</n>
              <PredecessorLink><PredecessorUID>1</PredecessorUID></PredecessorLink>
            </Task>
            <Task>
              <UID>3</UID><n>Child</n><OutlineLevel>2</OutlineLevel><Duration>PT24H</Duration>
              <PredecessorLink><PredecessorUID>1</PredecessorUID></PredecessorLink>
            </Task>
          </Tasks>
        </Project>
    "#;
    
    let project = import_mspdi(msp_xml).expect("Should import");
    
    let child = project.find_task("Child").unwrap();
    assert_eq!(child.depends, vec![TaskId::from("Predecessor")]);
    
    // Should NOT trigger W014 (child has explicit dep)
    let diagnostics = validate(&project);
    assert!(!diagnostics.iter().any(|d| d.code == DiagnosticCode::W014));
}
```

### 5. End-to-End Workflow Tests

**File:** `tests/integration/e2e_container_workflow_test.rs`

```rust
#[test]
fn e2e_import_validate_schedule() {
    // Import MS Project file
    let project = import_mspdi_file("tests/fixtures/ms_project/container_deps_simple.mpp")
        .expect("Import failed");
    
    // Should get W014 warnings
    let diagnostics = validate(&project);
    assert!(diagnostics.iter().any(|d| d.code == DiagnosticCode::W014));
    
    // Schedule should still work
    let schedule = CpmScheduler::new().schedule(&project).expect("Schedule failed");
    
    // Verify children can start immediately (not blocked by container dep)
    let child = &schedule.tasks["child"];
    assert_eq!(child.start, project.start);
}

#[test]
fn e2e_import_fix_validate_schedule() {
    // Import with auto-fix
    let project = import_mspdi_file_with_fix(
        "tests/fixtures/ms_project/container_deps_simple.mpp",
        ImportOptions { fix_container_deps: true }
    ).expect("Import failed");
    
    // No W014 warnings after fix
    let diagnostics = validate(&project);
    assert!(!diagnostics.iter().any(|d| d.code == DiagnosticCode::W014));
    
    // Schedule with children properly blocked
    let schedule = CpmScheduler::new().schedule(&project).expect("Schedule failed");
    
    let predecessor = &schedule.tasks["predecessor"];
    let child = &schedule.tasks["child"];
    
    // Child should start after predecessor finishes
    assert!(child.start >= predecessor.finish);
}

#[test]
fn e2e_cli_import_workflow() {
    // Simulate CLI: utf8proj import project.mpp -o project.proj
    let output = Command::new("utf8proj")
        .args(["import", "tests/fixtures/ms_project/container_deps_simple.mpp", "-o", "/tmp/test.proj"])
        .output()
        .expect("Failed to run CLI");
    
    let stderr = String::from_utf8_lossy(&output.stderr);
    
    // Should warn about W014
    assert!(stderr.contains("W014"));
    assert!(stderr.contains("Container dependency without child dependencies"));
    
    // Should suggest auto-fix
    assert!(stderr.contains("--fix-container-deps"));
}

#[test]
fn e2e_cli_import_with_auto_fix() {
    // Simulate CLI: utf8proj import --fix-container-deps project.mpp -o project.proj
    let output = Command::new("utf8proj")
        .args([
            "import",
            "--fix-container-deps",
            "tests/fixtures/ms_project/container_deps_simple.mpp",
            "-o",
            "/tmp/test_fixed.proj"
        ])
        .output()
        .expect("Failed to run CLI");
    
    let stderr = String::from_utf8_lossy(&output.stderr);
    
    // Should report auto-fixes
    assert!(stderr.contains("Auto-fixed"));
    assert!(!stderr.contains("W014"));
    
    // Verify output file has explicit dependencies
    let proj_content = std::fs::read_to_string("/tmp/test_fixed.proj").unwrap();
    assert!(proj_content.contains("depends: predecessor"));
}
```

---

## Test Fixtures

### MS Project Fixtures

**File:** `tests/fixtures/ms_project/container_deps_simple.xml`

```xml
<?xml version="1.0" encoding="UTF-8"?>
<Project xmlns="http://schemas.microsoft.com/project">
  <SaveVersion>14</SaveVersion>
  <StartDate>2025-01-01T08:00:00</StartDate>
  <Tasks>
    <Task>
      <UID>1</UID>
      <ID>1</ID>
      <n>Design Approval</n>
      <Start>2025-01-01T08:00:00</Start>
      <Finish>2025-01-05T17:00:00</Finish>
      <Duration>PT40H</Duration>
    </Task>
    <Task>
      <UID>2</UID>
      <ID>2</ID>
      <n>Development Phase</n>
      <Summary>1</Summary>
      <PredecessorLink>
        <PredecessorUID>1</PredecessorUID>
        <Type>1</Type>  <!-- FS -->
      </PredecessorLink>
    </Task>
    <Task>
      <UID>3</UID>
      <ID>3</ID>
      <n>Feature X Implementation</n>
      <OutlineLevel>2</OutlineLevel>
      <Start>2025-01-06T08:00:00</Start>
      <Finish>2025-01-10T17:00:00</Finish>
      <Duration>PT40H</Duration>
    </Task>
  </Tasks>
</Project>
```

---

## Performance Tests

```rust
#[test]
fn perf_validate_1000_tasks_with_containers() {
    let mut project = Project::new("Perf Test");
    
    // Create 100 containers with 10 children each
    for i in 0..100 {
        let mut container = Task::new(format!("container_{}", i));
        container.depends.push(TaskId::from("predecessor"));
        
        for j in 0..10 {
            let child = Task::new(format!("child_{}_{}", i, j));
            container.children.push(child);
        }
        
        project.add_task(container);
    }
    
    let start = Instant::now();
    let diagnostics = validate(&project);
    let elapsed = start.elapsed();
    
    // Should complete in < 100ms for 1000 tasks
    assert!(elapsed < Duration::from_millis(100));
    
    // Should find 1000 W014 instances (all children missing deps)
    let w014_count = diagnostics.iter()
        .filter(|d| d.code == DiagnosticCode::W014)
        .count();
    assert_eq!(w014_count, 1000);
}
```

---

## Test Execution

### Run All Tests

```bash
# All tests
cargo test

# Unit tests only
cargo test --lib

# Integration tests only
cargo test --test '*'

# Specific module
cargo test container_dependencies

# With coverage
cargo tarpaulin --out Html
```

### Test Coverage Goals

| Module | Target | Current |
|--------|--------|---------|
| Parser | 100% | TBD |
| Validator (W014) | 100% | TBD |
| Scheduler | 95% | TBD |
| Import | 90% | TBD |
| CLI | 85% | TBD |

---

## TDD Workflow

1. **Red:** Write failing test
2. **Green:** Write minimal code to pass
3. **Refactor:** Clean up implementation
4. **Repeat**

**Example:**
```bash
# 1. Write test
vim tests/unit/validator/diagnostic_w014_test.rs

# 2. Run test (should fail)
cargo test w014_triggers_when_child_missing_container_dep
# ❌ Test failed: W014 not implemented

# 3. Implement
vim src/validator/diagnostics.rs

# 4. Run test again
cargo test w014_triggers_when_child_missing_container_dep
# ✅ Test passed

# 5. Refactor if needed
# 6. Move to next test
```

---

**Status:** Ready for Implementation  
**Priority:** High (Required for MS Project import)  
**Estimated Effort:** 2-3 days with TDD approach
