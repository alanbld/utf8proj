//! Reference CPM Implementation
//! 
//! A minimal, correct implementation of the Critical Path Method
//! that can serve as a foundation for utf8proj's solver rewrite.
//!
//! This code is designed for clarity over performance.
//! 
//! Run with: cargo test --lib

use std::collections::{HashMap, HashSet, VecDeque};

// ============================================================================
// TYPES
// ============================================================================

pub type TaskId = String;
pub type Days = i32;

/// A single schedulable activity (leaf task)
#[derive(Debug, Clone)]
pub struct Activity {
    pub id: TaskId,
    pub name: String,
    pub duration: Days,
    pub predecessors: Vec<TaskId>,
}

/// Result of CPM scheduling for one activity
#[derive(Debug, Clone)]
pub struct ScheduleResult {
    pub id: TaskId,
    pub es: Days,           // Early Start
    pub ef: Days,           // Early Finish  
    pub ls: Days,           // Late Start
    pub lf: Days,           // Late Finish
    pub total_slack: Days,  // Total Float
    pub is_critical: bool,
}

/// Complete schedule output
#[derive(Debug)]
pub struct Schedule {
    pub results: HashMap<TaskId, ScheduleResult>,
    pub critical_path: Vec<TaskId>,
    pub project_duration: Days,
}

/// Errors that can occur during scheduling
#[derive(Debug, PartialEq)]
pub enum CpmError {
    CycleDetected { involved: Vec<TaskId> },
    MissingPredecessor { task: TaskId, missing: TaskId },
    NegativeSlack { task: TaskId, slack: Days },
}

// ============================================================================
// GRAPH OPERATIONS
// ============================================================================

/// Build adjacency lists from activities
fn build_graph(activities: &[Activity]) -> (
    HashMap<TaskId, Vec<TaskId>>,  // successors
    HashMap<TaskId, Vec<TaskId>>,  // predecessors
) {
    let mut successors: HashMap<TaskId, Vec<TaskId>> = HashMap::new();
    let mut predecessors: HashMap<TaskId, Vec<TaskId>> = HashMap::new();
    
    // Initialize empty lists for all tasks
    for act in activities {
        successors.entry(act.id.clone()).or_default();
        predecessors.entry(act.id.clone()).or_default();
    }
    
    // Build edges
    for act in activities {
        for pred_id in &act.predecessors {
            successors.get_mut(pred_id).unwrap().push(act.id.clone());
            predecessors.get_mut(&act.id).unwrap().push(pred_id.clone());
        }
    }
    
    (successors, predecessors)
}

/// Kahn's algorithm for topological sort
/// Returns tasks in order where all predecessors come before successors
fn topological_sort(
    activities: &[Activity],
    successors: &HashMap<TaskId, Vec<TaskId>>,
    predecessors: &HashMap<TaskId, Vec<TaskId>>,
) -> Result<Vec<TaskId>, CpmError> {
    let mut in_degree: HashMap<TaskId, usize> = HashMap::new();
    
    // Calculate in-degrees
    for act in activities {
        in_degree.insert(act.id.clone(), predecessors[&act.id].len());
    }
    
    // Start with tasks that have no predecessors
    let mut queue: VecDeque<TaskId> = in_degree.iter()
        .filter(|(_, &deg)| deg == 0)
        .map(|(id, _)| id.clone())
        .collect();
    
    let mut result: Vec<TaskId> = Vec::new();
    
    while let Some(task_id) = queue.pop_front() {
        result.push(task_id.clone());
        
        for succ in &successors[&task_id] {
            let deg = in_degree.get_mut(succ).unwrap();
            *deg -= 1;
            if *deg == 0 {
                queue.push_back(succ.clone());
            }
        }
    }
    
    // If we didn't process all tasks, there's a cycle
    if result.len() != activities.len() {
        let involved: Vec<TaskId> = activities.iter()
            .filter(|a| !result.contains(&a.id))
            .map(|a| a.id.clone())
            .collect();
        return Err(CpmError::CycleDetected { involved });
    }
    
    Ok(result)
}

// ============================================================================
// CPM ALGORITHM
// ============================================================================

/// Schedule activities using the Critical Path Method
/// 
/// This implements the textbook CPM algorithm:
/// 1. Topological sort
/// 2. Forward pass (compute ES, EF)
/// 3. Backward pass (compute LS, LF)
/// 4. Slack calculation
/// 5. Critical path identification
pub fn schedule(activities: &[Activity]) -> Result<Schedule, CpmError> {
    // Validate predecessors exist
    let task_ids: HashSet<_> = activities.iter().map(|a| &a.id).collect();
    for act in activities {
        for pred in &act.predecessors {
            if !task_ids.contains(pred) {
                return Err(CpmError::MissingPredecessor {
                    task: act.id.clone(),
                    missing: pred.clone(),
                });
            }
        }
    }
    
    let (successors, predecessors) = build_graph(activities);
    let topo_order = topological_sort(activities, &successors, &predecessors)?;
    
    // Create lookup for durations
    let duration: HashMap<TaskId, Days> = activities.iter()
        .map(|a| (a.id.clone(), a.duration))
        .collect();
    
    // ========================================================================
    // FORWARD PASS
    // ========================================================================
    // For each task in topological order:
    //   ES = max(EF of all predecessors), or 0 if no predecessors
    //   EF = ES + duration
    
    let mut es: HashMap<TaskId, Days> = HashMap::new();
    let mut ef: HashMap<TaskId, Days> = HashMap::new();
    
    for task_id in &topo_order {
        let preds = &predecessors[task_id];
        
        let early_start = if preds.is_empty() {
            0  // Project start
        } else {
            preds.iter()
                .map(|p| ef[p])
                .max()
                .unwrap()
        };
        
        let early_finish = early_start + duration[task_id];
        
        es.insert(task_id.clone(), early_start);
        ef.insert(task_id.clone(), early_finish);
    }
    
    // Project end is the maximum EF
    let project_duration = ef.values().cloned().max().unwrap_or(0);
    
    // ========================================================================
    // BACKWARD PASS
    // ========================================================================
    // For each task in REVERSE topological order:
    //   LF = min(LS of all successors), or project_end if no successors
    //   LS = LF - duration
    
    let mut ls: HashMap<TaskId, Days> = HashMap::new();
    let mut lf: HashMap<TaskId, Days> = HashMap::new();
    
    for task_id in topo_order.iter().rev() {
        let succs = &successors[task_id];
        
        let late_finish = if succs.is_empty() {
            project_duration  // Project end
        } else {
            succs.iter()
                .map(|s| ls[s])
                .min()
                .unwrap()
        };
        
        let late_start = late_finish - duration[task_id];
        
        lf.insert(task_id.clone(), late_finish);
        ls.insert(task_id.clone(), late_start);
    }
    
    // ========================================================================
    // SLACK & CRITICAL PATH
    // ========================================================================
    // Total Slack = LS - ES (must be >= 0)
    // Critical if Slack == 0
    
    let mut results: HashMap<TaskId, ScheduleResult> = HashMap::new();
    let mut critical_path: Vec<TaskId> = Vec::new();
    
    for task_id in &topo_order {
        let total_slack = ls[task_id] - es[task_id];
        
        // INVARIANT: Slack must never be negative
        if total_slack < 0 {
            return Err(CpmError::NegativeSlack {
                task: task_id.clone(),
                slack: total_slack,
            });
        }
        
        let is_critical = total_slack == 0;
        if is_critical {
            critical_path.push(task_id.clone());
        }
        
        results.insert(task_id.clone(), ScheduleResult {
            id: task_id.clone(),
            es: es[task_id],
            ef: ef[task_id],
            ls: ls[task_id],
            lf: lf[task_id],
            total_slack,
            is_critical,
        });
    }
    
    Ok(Schedule {
        results,
        critical_path,
        project_duration,
    })
}

// ============================================================================
// TESTS - CPM CORRECTNESS SUITE
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    
    /// Helper to create activities quickly
    fn act(id: &str, duration: Days, preds: &[&str]) -> Activity {
        Activity {
            id: id.to_string(),
            name: id.to_string(),
            duration,
            predecessors: preds.iter().map(|s| s.to_string()).collect(),
        }
    }
    
    // ------------------------------------------------------------------------
    // Basic functionality
    // ------------------------------------------------------------------------
    
    #[test]
    fn single_task() {
        let activities = vec![act("A", 5, &[])];
        let schedule = schedule(&activities).unwrap();
        
        let a = &schedule.results["A"];
        assert_eq!(a.es, 0);
        assert_eq!(a.ef, 5);
        assert_eq!(a.ls, 0);
        assert_eq!(a.lf, 5);
        assert_eq!(a.total_slack, 0);
        assert!(a.is_critical);
        assert_eq!(schedule.project_duration, 5);
    }
    
    #[test]
    fn sequential_tasks() {
        // A(5) -> B(3) -> C(2)
        let activities = vec![
            act("A", 5, &[]),
            act("B", 3, &["A"]),
            act("C", 2, &["B"]),
        ];
        let schedule = schedule(&activities).unwrap();
        
        assert_eq!(schedule.project_duration, 10);
        
        let a = &schedule.results["A"];
        assert_eq!((a.es, a.ef), (0, 5));
        assert!(a.is_critical);
        
        let b = &schedule.results["B"];
        assert_eq!((b.es, b.ef), (5, 8));
        assert!(b.is_critical);
        
        let c = &schedule.results["C"];
        assert_eq!((c.es, c.ef), (8, 10));
        assert!(c.is_critical);
    }
    
    #[test]
    fn parallel_paths_with_slack() {
        // Classic CPM example:
        //     A(5) -----> C(2)
        //                  |
        //     B(3) --------+
        //
        // Critical path: A -> C (duration 7)
        // B has slack of 4 (can start anytime in [0,4])
        
        let activities = vec![
            act("A", 5, &[]),
            act("B", 3, &[]),
            act("C", 2, &["A", "B"]),
        ];
        let schedule = schedule(&activities).unwrap();
        
        assert_eq!(schedule.project_duration, 7);
        
        let a = &schedule.results["A"];
        assert_eq!(a.total_slack, 0);
        assert!(a.is_critical);
        
        let b = &schedule.results["B"];
        assert_eq!(b.es, 0);
        assert_eq!(b.ef, 3);
        assert_eq!(b.ls, 2);  // Can start as late as day 2
        assert_eq!(b.lf, 5);  // And finish by day 5
        assert_eq!(b.total_slack, 2);  // 2 days of float
        assert!(!b.is_critical);
        
        let c = &schedule.results["C"];
        assert_eq!((c.es, c.ef), (5, 7));
        assert!(c.is_critical);
    }
    
    // ------------------------------------------------------------------------
    // CPM Invariants
    // ------------------------------------------------------------------------
    
    #[test]
    fn invariant_slack_never_negative() {
        // This is a fundamental CPM property
        // Create a complex network and verify
        let activities = vec![
            act("START", 0, &[]),
            act("A", 5, &["START"]),
            act("B", 8, &["START"]),
            act("C", 3, &["A"]),
            act("D", 4, &["B"]),
            act("E", 6, &["C", "D"]),
            act("F", 2, &["A"]),
            act("END", 0, &["E", "F"]),
        ];
        
        let schedule = schedule(&activities).unwrap();
        
        for (id, result) in &schedule.results {
            assert!(
                result.total_slack >= 0,
                "Task {} has negative slack: {}",
                id,
                result.total_slack
            );
        }
    }
    
    #[test]
    fn invariant_es_respects_predecessors() {
        let activities = vec![
            act("A", 5, &[]),
            act("B", 3, &["A"]),
            act("C", 4, &["A"]),
            act("D", 2, &["B", "C"]),
        ];
        
        let schedule = schedule(&activities).unwrap();
        let (_, predecessors) = build_graph(&activities);
        
        for act in &activities {
            let result = &schedule.results[&act.id];
            for pred_id in &predecessors[&act.id] {
                let pred_result = &schedule.results[pred_id];
                assert!(
                    result.es >= pred_result.ef,
                    "Task {} starts ({}) before predecessor {} finishes ({})",
                    act.id, result.es, pred_id, pred_result.ef
                );
            }
        }
    }
    
    #[test]
    fn invariant_lf_respects_successors() {
        let activities = vec![
            act("A", 5, &[]),
            act("B", 3, &["A"]),
            act("C", 4, &["A"]),
            act("D", 2, &["B", "C"]),
        ];
        
        let schedule = schedule(&activities).unwrap();
        let (successors, _) = build_graph(&activities);
        
        for act in &activities {
            let result = &schedule.results[&act.id];
            for succ_id in &successors[&act.id] {
                let succ_result = &schedule.results[succ_id];
                assert!(
                    result.lf <= succ_result.ls,
                    "Task {} late finish ({}) exceeds successor {} late start ({})",
                    act.id, result.lf, succ_id, succ_result.ls
                );
            }
        }
    }
    
    #[test]
    fn invariant_critical_path_zero_slack() {
        let activities = vec![
            act("A", 5, &[]),
            act("B", 3, &[]),
            act("C", 2, &["A", "B"]),
        ];
        
        let schedule = schedule(&activities).unwrap();
        
        for task_id in &schedule.critical_path {
            let result = &schedule.results[task_id];
            assert_eq!(
                result.total_slack, 0,
                "Critical task {} has non-zero slack: {}",
                task_id, result.total_slack
            );
        }
    }
    
    // ------------------------------------------------------------------------
    // Error handling
    // ------------------------------------------------------------------------
    
    #[test]
    fn detect_cycle() {
        // A -> B -> C -> A (cycle!)
        let activities = vec![
            act("A", 1, &["C"]),
            act("B", 1, &["A"]),
            act("C", 1, &["B"]),
        ];
        
        let result = schedule(&activities);
        assert!(matches!(result, Err(CpmError::CycleDetected { .. })));
    }
    
    #[test]
    fn detect_missing_predecessor() {
        let activities = vec![
            act("A", 5, &[]),
            act("B", 3, &["NONEXISTENT"]),
        ];
        
        let result = schedule(&activities);
        assert!(matches!(
            result, 
            Err(CpmError::MissingPredecessor { task, missing }) 
            if task == "B" && missing == "NONEXISTENT"
        ));
    }
    
    // ------------------------------------------------------------------------
    // Real-world scenario: CRM Migration
    // ------------------------------------------------------------------------
    
    #[test]
    fn crm_migration_simplified() {
        // Simplified version of the CRM migration project
        let activities = vec![
            // Phase 1: Discovery
            act("kickoff", 1, &[]),
            act("requirements", 8, &["kickoff"]),
            act("gap_analysis", 4, &["requirements"]),
            act("architecture", 5, &["gap_analysis"]),
            
            // Phase 2: Data Migration (depends on architecture)
            act("data_mapping", 6, &["architecture"]),
            act("etl_dev", 10, &["data_mapping"]),
            act("test_migration", 5, &["etl_dev"]),
            
            // Phase 3: Integration (depends on architecture, parallel to data)
            act("api_design", 3, &["architecture"]),
            act("middleware", 4, &["api_design"]),
            act("erp_connector", 8, &["api_design"]),
            act("integration_test", 4, &["middleware", "erp_connector"]),
            
            // Phase 4: Deployment (depends on BOTH data AND integration)
            act("training", 4, &["test_migration", "integration_test"]),
            act("go_live", 2, &["training"]),
        ];
        
        let schedule = schedule(&activities).unwrap();
        
        // Verify cross-phase dependencies work
        let arch = &schedule.results["architecture"];
        let data_mapping = &schedule.results["data_mapping"];
        let api_design = &schedule.results["api_design"];
        
        // Both data_mapping and api_design start after architecture
        assert!(data_mapping.es >= arch.ef);
        assert!(api_design.es >= arch.ef);
        
        // go_live depends on both test_migration AND integration_test
        let go_live = &schedule.results["go_live"];
        let test_migration = &schedule.results["test_migration"];
        let integration_test = &schedule.results["integration_test"];
        
        // Find training which is the direct predecessor
        let training = &schedule.results["training"];
        assert!(training.es >= test_migration.ef);
        assert!(training.es >= integration_test.ef);
        assert!(go_live.es >= training.ef);
        
        // The critical path should be the longest sequence
        // kickoff -> req -> gap -> arch -> data_mapping -> etl -> test -> training -> go_live
        // = 1 + 8 + 4 + 5 + 6 + 10 + 5 + 4 + 2 = 45 days
        assert_eq!(schedule.project_duration, 45);
        
        // Verify critical tasks
        assert!(schedule.results["kickoff"].is_critical);
        assert!(schedule.results["etl_dev"].is_critical);
        assert!(schedule.results["go_live"].is_critical);
        
        // Integration path should have slack (it's shorter)
        assert!(schedule.results["api_design"].total_slack > 0);
        assert!(schedule.results["middleware"].total_slack > 0);
    }
}

// ============================================================================
// MAIN (for demonstration)
// ============================================================================

fn main() {
    println!("CPM Reference Implementation");
    println!("=============================\n");
    
    // Example: Simple project
    let activities = vec![
        Activity { id: "A".into(), name: "Design".into(), duration: 5, predecessors: vec![] },
        Activity { id: "B".into(), name: "Develop".into(), duration: 10, predecessors: vec!["A".into()] },
        Activity { id: "C".into(), name: "Test".into(), duration: 3, predecessors: vec!["B".into()] },
        Activity { id: "D".into(), name: "Docs".into(), duration: 4, predecessors: vec!["A".into()] },
        Activity { id: "E".into(), name: "Release".into(), duration: 1, predecessors: vec!["C".into(), "D".into()] },
    ];
    
    match schedule(&activities) {
        Ok(s) => {
            println!("Project Duration: {} days\n", s.project_duration);
            
            println!("{:<10} {:>4} {:>4} {:>4} {:>4} {:>6} {:>10}",
                     "Task", "ES", "EF", "LS", "LF", "Slack", "Critical");
            println!("{}", "-".repeat(55));
            
            // Print in topological order
            for id in ["A", "B", "C", "D", "E"] {
                let r = &s.results[id];
                println!("{:<10} {:>4} {:>4} {:>4} {:>4} {:>6} {:>10}",
                         id, r.es, r.ef, r.ls, r.lf, r.total_slack,
                         if r.is_critical { "***" } else { "" });
            }
            
            println!("\nCritical Path: {:?}", s.critical_path);
        }
        Err(e) => println!("Error: {:?}", e),
    }
}
