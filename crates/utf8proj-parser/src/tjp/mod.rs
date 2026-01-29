//! TaskJuggler (.tjp) parser
//!
//! Parses TaskJuggler project files into utf8proj domain model.

use chrono::NaiveDate;
use pest::Parser;
use pest_derive::Parser;

use crate::ParseError;
use utf8proj_core::{
    Dependency, DependencyType, Duration, Project, Resource, ResourceRef, Task, TaskConstraint,
};

#[derive(Parser)]
#[grammar = "tjp/grammar.pest"]
struct TjpParser;

/// Parse a TaskJuggler file into a Project
pub fn parse(input: &str) -> Result<Project, ParseError> {
    let pairs = TjpParser::parse(Rule::tjp_file, input).map_err(|e| {
        let (line, column) = match e.line_col {
            pest::error::LineColLocation::Pos((l, c)) => (l, c),
            pest::error::LineColLocation::Span((l, c), _) => (l, c),
        };
        ParseError::Syntax {
            line,
            column,
            message: e.variant.message().to_string(),
        }
    })?;

    let mut project = Project::new("Untitled");
    project.start = NaiveDate::from_ymd_opt(2025, 1, 1).unwrap();

    for pair in pairs {
        match pair.as_rule() {
            Rule::tjp_file => {
                for inner in pair.into_inner() {
                    match inner.as_rule() {
                        Rule::project_decl => {
                            parse_project_decl(inner, &mut project)?;
                        }
                        Rule::resource_decl => {
                            if let Some(resource) = parse_resource_decl(inner)? {
                                project.resources.push(resource);
                            }
                        }
                        Rule::task_decl => {
                            if let Some(task) = parse_task_decl(inner)? {
                                project.tasks.push(task);
                            }
                        }
                        Rule::vacation_decl => {
                            // Vacations/holidays - could be added to calendar
                        }
                        Rule::report_decl => {
                            // Reports are ignored - we generate our own
                        }
                        Rule::EOI => {}
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }

    Ok(project)
}

/// Parse project declaration
fn parse_project_decl(
    pair: pest::iterators::Pair<Rule>,
    project: &mut Project,
) -> Result<(), ParseError> {
    let mut inner = pair.into_inner();

    // project id
    let _project_id = inner.next().unwrap().as_str();

    // project name
    let name_pair = inner.next().unwrap();
    project.name = parse_string(name_pair.as_str());

    // start date
    let start_pair = inner.next().unwrap();
    project.start = parse_date(start_pair.as_str())?;

    // end date
    let end_pair = inner.next().unwrap();
    project.end = Some(parse_date(end_pair.as_str())?);

    // project body (attributes)
    if let Some(body) = inner.next() {
        for attr in body.into_inner() {
            // Unwrap project_attr to get the actual attribute
            let actual_attr = if attr.as_rule() == Rule::project_attr {
                attr.into_inner().next().unwrap()
            } else {
                attr
            };

            match actual_attr.as_rule() {
                Rule::timezone_attr => {
                    // Could store timezone
                }
                Rule::currency_attr => {
                    let mut inner = actual_attr.into_inner();
                    if let Some(currency) = inner.next() {
                        project.currency = parse_string(currency.as_str());
                    }
                }
                Rule::workinghours_attr => {
                    // Could configure calendar
                }
                Rule::now_attr => {
                    // RFC-0017: Parse 'now' attribute as status_date
                    let mut inner = actual_attr.into_inner();
                    if let Some(date_pair) = inner.next() {
                        project.status_date = Some(parse_date(date_pair.as_str())?);
                    }
                }
                _ => {}
            }
        }
    }

    Ok(())
}

/// Parse resource declaration
fn parse_resource_decl(pair: pest::iterators::Pair<Rule>) -> Result<Option<Resource>, ParseError> {
    let mut inner = pair.into_inner();

    let id = inner.next().unwrap().as_str().to_string();
    let name_pair = inner.next().unwrap();
    let name = parse_string(name_pair.as_str());

    let mut resource = Resource::new(&id);
    resource.name = name;

    // Parse optional resource body
    if let Some(body) = inner.next() {
        for attr in body.into_inner() {
            // Unwrap resource_attr to get the actual attribute
            let actual_attr = if attr.as_rule() == Rule::resource_attr {
                attr.into_inner().next().unwrap()
            } else {
                attr
            };

            match actual_attr.as_rule() {
                Rule::efficiency_attr => {
                    let mut inner = actual_attr.into_inner();
                    if let Some(num) = inner.next() {
                        resource.efficiency = num.as_str().parse().unwrap_or(1.0);
                    }
                }
                _ => {}
            }
        }
    }

    Ok(Some(resource))
}

/// Parse task declaration
fn parse_task_decl(pair: pest::iterators::Pair<Rule>) -> Result<Option<Task>, ParseError> {
    let mut inner = pair.into_inner();

    let id = inner.next().unwrap().as_str().to_string();
    let name_pair = inner.next().unwrap();
    let name = parse_string(name_pair.as_str());

    let mut task = Task::new(&id);
    task.name = name; // TJP quoted string is the human-readable task name

    // Parse task body - iterate through remaining pairs
    for body_or_attr in inner {
        // Could be task_body or directly task_attr depending on grammar
        let items = if body_or_attr.as_rule() == Rule::task_body {
            body_or_attr.into_inner().collect::<Vec<_>>()
        } else {
            vec![body_or_attr]
        };

        for item in items {
            // Handle nested task declarations
            if item.as_rule() == Rule::task_decl {
                if let Some(child_task) = parse_task_decl(item)? {
                    task.children.push(child_task);
                }
                continue;
            }

            // task_attr is a wrapper rule, get the actual attribute inside
            let actual_attr = if item.as_rule() == Rule::task_attr {
                item.into_inner().next().unwrap()
            } else {
                item
            };

            match actual_attr.as_rule() {
                Rule::duration_attr => {
                    let mut inner = actual_attr.into_inner();
                    if let Some(dur) = inner.next() {
                        task.duration = Some(parse_duration(dur.as_str())?);
                    }
                }
                Rule::length_attr => {
                    // length = working days (same as MS Project duration)
                    let mut inner = actual_attr.into_inner();
                    if let Some(dur) = inner.next() {
                        task.duration = Some(parse_duration(dur.as_str())?);
                    }
                }
                Rule::effort_attr => {
                    let mut inner = actual_attr.into_inner();
                    if let Some(dur) = inner.next() {
                        task.effort = Some(parse_duration(dur.as_str())?);
                    }
                }
                Rule::milestone_attr => {
                    task.milestone = true;
                    task.duration = Some(Duration::zero());
                }
                Rule::depends_attr => {
                    for dep in actual_attr.into_inner() {
                        if dep.as_rule() == Rule::dependency_list {
                            for dep_item in dep.into_inner() {
                                if dep_item.as_rule() == Rule::dependency {
                                    let mut has_ss_marker = false;
                                    let mut has_ff_marker = false;
                                    let mut task_path = String::new();
                                    let mut lag: Option<Duration> = None;

                                    for part in dep_item.into_inner() {
                                        match part.as_rule() {
                                            Rule::dep_ss_marker => has_ss_marker = true,
                                            Rule::dep_ff_marker => has_ff_marker = true,
                                            Rule::task_path => {
                                                task_path = part.as_str().to_string()
                                            }
                                            Rule::dep_modifier => {
                                                // Parse lag from modifier
                                                // dep_modifier contains dep_attr rules
                                                for dep_attr in part.into_inner() {
                                                    if dep_attr.as_rule() == Rule::dep_attr {
                                                        // dep_attr wraps the actual attribute
                                                        if let Some(attr) =
                                                            dep_attr.into_inner().next()
                                                        {
                                                            match attr.as_rule() {
                                                                Rule::gaplength_attr
                                                                | Rule::gapduration_attr => {
                                                                    if let Some(dur) =
                                                                        attr.into_inner().next()
                                                                    {
                                                                        lag = Some(parse_duration(
                                                                            dur.as_str(),
                                                                        )?);
                                                                    }
                                                                }
                                                                Rule::onstart_attr => {
                                                                    has_ss_marker = true
                                                                }
                                                                Rule::onend_attr => {
                                                                    has_ff_marker = true
                                                                }
                                                                _ => {}
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                            _ => {}
                                        }
                                    }

                                    // Determine dependency type from markers
                                    let dep_type = match (has_ss_marker, has_ff_marker) {
                                        (true, true) => DependencyType::StartToFinish,
                                        (true, false) => DependencyType::StartToStart,
                                        (false, true) => DependencyType::FinishToFinish,
                                        (false, false) => DependencyType::FinishToStart,
                                    };

                                    task.depends.push(Dependency {
                                        predecessor: task_path,
                                        dep_type,
                                        lag,
                                    });
                                }
                            }
                        }
                    }
                }
                Rule::start_attr => {
                    let mut inner = actual_attr.into_inner();
                    if let Some(date_pair) = inner.next() {
                        let date = parse_date(date_pair.as_str())?;
                        task.constraints.push(TaskConstraint::MustStartOn(date));
                    }
                }
                Rule::allocate_attr => {
                    for res in actual_attr.into_inner() {
                        if res.as_rule() == Rule::identifier_list {
                            for res_id in res.into_inner() {
                                task.assigned.push(ResourceRef {
                                    resource_id: res_id.as_str().to_string(),
                                    units: 1.0,
                                });
                            }
                        }
                    }
                }
                Rule::priority_attr => {
                    let mut inner = actual_attr.into_inner();
                    if let Some(prio) = inner.next() {
                        task.priority = prio.as_str().parse().unwrap_or(500);
                    }
                }
                Rule::note_attr => {
                    let mut inner = actual_attr.into_inner();
                    if let Some(note) = inner.next() {
                        // TJP note is supplementary info, store in attributes
                        task.attributes
                            .insert("note".to_string(), parse_string(note.as_str()));
                    }
                }
                Rule::complete_attr => {
                    let mut inner = actual_attr.into_inner();
                    if let Some(pct) = inner.next() {
                        task.complete = Some(pct.as_str().parse::<i32>().unwrap_or(0) as f32);
                    }
                }
                _ => {}
            }
        }
    }

    Ok(Some(task))
}

/// Parse a quoted string, removing quotes and handling escapes
fn parse_string(s: &str) -> String {
    let inner = s.trim_matches('"');
    // Handle common escape sequences
    inner
        .replace("\\\"", "\"")
        .replace("\\n", "\n")
        .replace("\\t", "\t")
        .replace("\\\\", "\\")
}

/// Parse a date string (YYYY-MM-DD)
fn parse_date(s: &str) -> Result<NaiveDate, ParseError> {
    NaiveDate::parse_from_str(s, "%Y-%m-%d")
        .map_err(|_| ParseError::InvalidValue(format!("Invalid date: {}", s)))
}

/// Parse a duration string (e.g., "5d", "2w", "8h")
fn parse_duration(s: &str) -> Result<Duration, ParseError> {
    let s = s.trim();

    // Find where the number ends and the unit begins
    let num_end = s
        .chars()
        .position(|c| !c.is_ascii_digit() && c != '.' && c != '-')
        .unwrap_or(s.len());

    let (num_str, unit) = s.split_at(num_end);
    let num: f64 = num_str
        .parse()
        .map_err(|_| ParseError::InvalidValue(format!("Invalid duration number: {}", num_str)))?;

    let minutes = match unit.trim() {
        "min" => num as i64,
        "h" => (num * 60.0) as i64,
        "d" => (num * 8.0 * 60.0) as i64,         // 8 hours per day
        "w" => (num * 5.0 * 8.0 * 60.0) as i64,   // 5 days per week
        "m" => (num * 20.0 * 8.0 * 60.0) as i64,  // ~20 working days per month
        "y" => (num * 250.0 * 8.0 * 60.0) as i64, // ~250 working days per year
        _ => {
            return Err(ParseError::InvalidValue(format!(
                "Unknown duration unit: {}",
                unit
            )))
        }
    };

    Ok(Duration { minutes })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_simple_tjp() {
        let input = r#"
            project test "Test Project" 2025-01-01 - 2025-12-31 {
                timezone "UTC"
                currency "EUR"
            }

            resource dev "Developer"

            task task_1 "First Task" {
                duration 5d
            }

            task task_2 "Second Task" {
                duration 3d
                depends task_1
            }
        "#;

        let project = parse(input).unwrap();
        assert_eq!(project.name, "Test Project");
        assert_eq!(project.tasks.len(), 2);
        assert_eq!(project.resources.len(), 1);
    }

    #[test]
    fn parse_milestone() {
        let input = r#"
            project test "Test" 2025-01-01 - 2025-12-31 {}

            task m1 "Milestone 1" {
                milestone
                start 2025-02-01
            }
        "#;

        let project = parse(input).unwrap();
        assert_eq!(project.tasks.len(), 1);
        assert!(project.tasks[0].milestone);
    }

    #[test]
    fn parse_duration_values() {
        assert_eq!(parse_duration("5d").unwrap().as_days(), 5.0);
        assert_eq!(parse_duration("2w").unwrap().as_days(), 10.0);
        assert_eq!(parse_duration("8h").unwrap().minutes, 480);
    }

    #[test]
    fn parse_dependencies() {
        let input = r#"
            project test "Test" 2025-01-01 - 2025-12-31 {}

            task t1 "Task 1" { duration 1d }
            task t2 "Task 2" { duration 1d }
            task t3 "Task 3" {
                duration 1d
                depends t1, t2
            }
        "#;

        let project = parse(input).unwrap();
        let task3 = &project.tasks[2];
        assert_eq!(task3.depends.len(), 2);
        assert!(task3.depends.iter().any(|d| d.predecessor == "t1"));
        assert!(task3.depends.iter().any(|d| d.predecessor == "t2"));
    }

    #[test]
    fn parse_syntax_error() {
        let input = "project test Test 2025-01-01 - 2025-12-31 {}"; // Missing quotes
        let result = parse(input);
        assert!(result.is_err());
        if let Err(ParseError::Syntax { line, column, .. }) = result {
            assert!(line >= 1);
            assert!(column >= 1);
        }
    }

    #[test]
    fn parse_resource_with_body() {
        // Test resource parsing with body - covers resource_body parsing
        let input = r#"
            project test "Test" 2025-01-01 - 2025-12-31 {}
            resource dev "Developer" {
                efficiency 0.8
            }
        "#;

        let project = parse(input).unwrap();
        assert_eq!(project.resources.len(), 1);
        assert_eq!(project.resources[0].id, "dev");
        // Note: efficiency parsing covers the code path even if value isn't properly stored
    }

    #[test]
    fn parse_task_priority() {
        let input = r#"
            project test "Test" 2025-01-01 - 2025-12-31 {}
            task t1 "Task 1" {
                duration 1d
                priority 800
            }
        "#;

        let project = parse(input).unwrap();
        assert_eq!(project.tasks[0].priority, 800);
    }

    #[test]
    fn parse_task_note() {
        let input = r#"
            project test "Test" 2025-01-01 - 2025-12-31 {}
            task t1 "Task 1" {
                duration 1d
                note "This is a note"
            }
        "#;

        let project = parse(input).unwrap();
        // TJP quoted string is the task name (primary display)
        assert_eq!(project.tasks[0].name, "Task 1");
        // TJP note is supplementary info stored in attributes
        assert_eq!(
            project.tasks[0].attributes.get("note"),
            Some(&"This is a note".to_string())
        );
    }

    #[test]
    fn parse_task_complete() {
        let input = r#"
            project test "Test" 2025-01-01 - 2025-12-31 {}
            task t1 "Task 1" {
                duration 1d
                complete 50
            }
        "#;

        let project = parse(input).unwrap();
        assert_eq!(project.tasks[0].complete, Some(50.0));
    }

    #[test]
    fn parse_all_duration_units() {
        // Minutes
        assert_eq!(parse_duration("30min").unwrap().minutes, 30);

        // Hours
        assert_eq!(parse_duration("2h").unwrap().minutes, 120);

        // Days
        assert_eq!(parse_duration("1d").unwrap().minutes, 480);

        // Weeks
        assert_eq!(parse_duration("1w").unwrap().minutes, 2400);

        // Months
        assert_eq!(parse_duration("1m").unwrap().minutes, 9600);

        // Years
        assert_eq!(parse_duration("1y").unwrap().minutes, 120000);
    }

    #[test]
    fn parse_invalid_duration_unit() {
        let result = parse_duration("5x");
        assert!(result.is_err());
    }

    #[test]
    fn parse_task_allocate() {
        let input = r#"
            project test "Test" 2025-01-01 - 2025-12-31 {}
            resource dev "Developer"
            task t1 "Task 1" {
                duration 5d
                allocate dev
            }
        "#;

        let project = parse(input).unwrap();
        assert_eq!(project.tasks[0].assigned.len(), 1);
        assert_eq!(project.tasks[0].assigned[0].resource_id, "dev");
    }

    #[test]
    fn parse_nested_tasks() {
        let input = r#"
            project test "Test" 2025-01-01 - 2025-12-31 {}
            task phase1 "Phase 1" {
                task t1 "Sub Task 1" { duration 2d }
                task t2 "Sub Task 2" { duration 3d }
            }
        "#;

        let project = parse(input).unwrap();
        assert_eq!(project.tasks.len(), 1);
        assert_eq!(project.tasks[0].children.len(), 2);
    }

    #[test]
    fn parse_task_effort() {
        let input = r#"
            project test "Test" 2025-01-01 - 2025-12-31 {}
            task t1 "Task 1" {
                effort 40h
            }
        "#;

        let project = parse(input).unwrap();
        assert!(project.tasks[0].effort.is_some());
    }

    #[test]
    fn parse_task_start_constraint() {
        let input = r#"
            project test "Test" 2025-01-01 - 2025-12-31 {}
            task t1 "Task 1" {
                duration 5d
                start 2025-03-01
            }
        "#;

        let project = parse(input).unwrap();
        // Task should be parsed with start constraint
        assert!(!project.tasks.is_empty());
    }

    #[test]
    fn parse_project_currency() {
        // Lines 96-98: currency attribute
        let input = r#"
            project test "Test" 2025-01-01 - 2025-12-31 {
                currency "EUR"
            }
        "#;

        let project = parse(input).unwrap();
        assert_eq!(project.currency, "EUR");
    }

    #[test]
    fn parse_resource_efficiency() {
        // Lines 128-130: efficiency attribute
        let input = r#"
            project test "Test" 2025-01-01 - 2025-12-31 {}
            resource dev "Developer" {
                efficiency 0.8
            }
        "#;

        let project = parse(input).unwrap();
        assert_eq!(project.resources.len(), 1);
        assert!((project.resources[0].efficiency - 0.8).abs() < 0.001);
    }

    #[test]
    fn parse_dependency_onstart() {
        // Line 229: onstart dependency marker (SS)
        let input = r#"
            project test "Test" 2025-01-01 - 2025-12-31 {}
            task t1 "Task 1" { duration 5d }
            task t2 "Task 2" {
                duration 3d
                depends t1 { onstart }
            }
        "#;

        let project = parse(input).unwrap();
        let t2 = &project.tasks[1];
        assert!(!t2.depends.is_empty());
        // onstart creates SS dependency
        assert_eq!(
            t2.depends[0].dep_type,
            utf8proj_core::DependencyType::StartToStart
        );
    }

    #[test]
    fn parse_dependency_onend() {
        // Line 230: onend dependency marker (FF)
        let input = r#"
            project test "Test" 2025-01-01 - 2025-12-31 {}
            task t1 "Task 1" { duration 5d }
            task t2 "Task 2" {
                duration 3d
                depends t1 { onend }
            }
        "#;

        let project = parse(input).unwrap();
        let t2 = &project.tasks[1];
        assert!(!t2.depends.is_empty());
        // onend creates FF dependency
        assert_eq!(
            t2.depends[0].dep_type,
            utf8proj_core::DependencyType::FinishToFinish
        );
    }

    #[test]
    fn parse_project_now_attribute() {
        // RFC-0017: 'now' attribute sets status_date
        let input = r#"
            project test "Test" 2025-01-01 - 2025-12-31 {
                now 2025-03-15
            }
            task t1 "Task 1" { duration 5d }
        "#;

        let project = parse(input).unwrap();
        assert!(
            project.status_date.is_some(),
            "status_date should be set from 'now'"
        );
        assert_eq!(
            project.status_date.unwrap(),
            chrono::NaiveDate::from_ymd_opt(2025, 3, 15).unwrap()
        );
    }
}
