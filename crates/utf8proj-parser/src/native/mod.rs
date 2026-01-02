//! Native DSL parser for .proj files using pest.

use chrono::NaiveDate;
use pest::iterators::Pair;
use pest::Parser;
use pest_derive::Parser;
use rust_decimal::Decimal;
use std::str::FromStr;

use utf8proj_core::{
    Calendar, Dependency, DependencyType, Duration, Holiday, Money, Project, Resource,
    ResourceRef, Task, TaskConstraint, TimeRange,
};

use crate::ParseError;

#[derive(Parser)]
#[grammar = "native/grammar.pest"]
pub struct ProjectParser;

/// Parse a complete project file
pub fn parse(input: &str) -> Result<Project, ParseError> {
    let mut pairs = ProjectParser::parse(Rule::project_file, input).map_err(|e| {
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

    let mut project = Project::new("");
    project.calendars.clear(); // Remove default calendar, we'll add parsed ones

    // The top-level pair is project_file, we need its inner pairs
    let file_pair = pairs.next().unwrap();
    for pair in file_pair.into_inner() {
        match pair.as_rule() {
            Rule::project_decl => parse_project_decl(pair, &mut project)?,
            Rule::calendar_decl => {
                let calendar = parse_calendar_decl(pair)?;
                project.calendars.push(calendar);
            }
            Rule::resource_decl => {
                let resource = parse_resource_decl(pair)?;
                project.resources.push(resource);
            }
            Rule::task_decl => {
                let task = parse_task_decl(pair)?;
                project.tasks.push(task);
            }
            Rule::report_decl => {
                // Reports are parsed but not stored in Project for now
                // They will be handled separately during rendering
            }
            Rule::EOI => {}
            _ => {}
        }
    }

    // If no calendars were defined, add the default one
    if project.calendars.is_empty() {
        project.calendars.push(Calendar::default());
    }

    Ok(project)
}

// =============================================================================
// Primitive Parsers
// =============================================================================

fn parse_string(pair: Pair<Rule>) -> String {
    let s = pair.as_str();
    // Remove surrounding quotes
    s[1..s.len() - 1].to_string()
}

fn parse_identifier(pair: Pair<Rule>) -> String {
    pair.as_str().to_string()
}

fn parse_date(pair: Pair<Rule>) -> Result<NaiveDate, ParseError> {
    let s = pair.as_str();
    NaiveDate::parse_from_str(s, "%Y-%m-%d")
        .map_err(|_| ParseError::InvalidValue(format!("Invalid date: {}", s)))
}

fn parse_duration(pair: Pair<Rule>) -> Result<Duration, ParseError> {
    let s = pair.as_str();
    let len = s.len();
    let unit = &s[len - 1..];
    let value: f64 = s[..len - 1]
        .parse()
        .map_err(|_| ParseError::InvalidValue(format!("Invalid duration number: {}", s)))?;

    let minutes = match unit {
        "m" => (value * 60.0) as i64,           // months -> assume 20 working days
        "w" => (value * 5.0 * 8.0 * 60.0) as i64, // weeks
        "d" => (value * 8.0 * 60.0) as i64,       // days (8-hour workday)
        "h" => (value * 60.0) as i64,             // hours
        _ => return Err(ParseError::InvalidValue(format!("Unknown duration unit: {}", unit))),
    };

    Ok(Duration { minutes })
}

fn parse_number(pair: Pair<Rule>) -> Result<f64, ParseError> {
    pair.as_str()
        .parse()
        .map_err(|_| ParseError::InvalidValue(format!("Invalid number: {}", pair.as_str())))
}

fn parse_integer(pair: Pair<Rule>) -> Result<i64, ParseError> {
    pair.as_str()
        .parse()
        .map_err(|_| ParseError::InvalidValue(format!("Invalid integer: {}", pair.as_str())))
}

fn parse_percentage(pair: Pair<Rule>) -> Result<f32, ParseError> {
    let s = pair.as_str();
    let value: f32 = s[..s.len() - 1] // Remove the '%'
        .parse()
        .map_err(|_| ParseError::InvalidValue(format!("Invalid percentage: {}", s)))?;
    Ok(value / 100.0)
}

fn parse_boolean(pair: Pair<Rule>) -> bool {
    pair.as_str() == "true"
}

fn parse_time(pair: Pair<Rule>) -> Result<u16, ParseError> {
    let s = pair.as_str();
    let parts: Vec<&str> = s.split(':').collect();
    if parts.len() != 2 {
        return Err(ParseError::InvalidValue(format!("Invalid time: {}", s)));
    }
    let hours: u16 = parts[0]
        .parse()
        .map_err(|_| ParseError::InvalidValue(format!("Invalid time hours: {}", s)))?;
    let minutes: u16 = parts[1]
        .parse()
        .map_err(|_| ParseError::InvalidValue(format!("Invalid time minutes: {}", s)))?;
    Ok(hours * 60 + minutes)
}

fn parse_day(pair: Pair<Rule>) -> u8 {
    match pair.as_str() {
        "sun" => 0,
        "mon" => 1,
        "tue" => 2,
        "wed" => 3,
        "thu" => 4,
        "fri" => 5,
        "sat" => 6,
        _ => 1, // Default to Monday
    }
}

// =============================================================================
// Project Declaration Parser
// =============================================================================

fn parse_project_decl(pair: Pair<Rule>, project: &mut Project) -> Result<(), ParseError> {
    for inner in pair.into_inner() {
        match inner.as_rule() {
            Rule::string => {
                project.name = parse_string(inner);
                project.id = project.name.to_lowercase().replace(' ', "_");
            }
            Rule::project_body => {
                for attr in inner.into_inner() {
                    if attr.as_rule() == Rule::project_attr {
                        parse_project_attr(attr, project)?;
                    }
                }
            }
            _ => {}
        }
    }
    Ok(())
}

fn parse_project_attr(pair: Pair<Rule>, project: &mut Project) -> Result<(), ParseError> {
    let inner = pair.into_inner().next().unwrap();
    match inner.as_rule() {
        Rule::project_start => {
            let date_pair = inner.into_inner().next().unwrap();
            project.start = parse_date(date_pair)?;
        }
        Rule::project_end => {
            let date_pair = inner.into_inner().next().unwrap();
            project.end = Some(parse_date(date_pair)?);
        }
        Rule::project_currency => {
            let id_pair = inner.into_inner().next().unwrap();
            project.currency = parse_identifier(id_pair);
        }
        Rule::project_calendar => {
            let id_pair = inner.into_inner().next().unwrap();
            project.calendar = parse_identifier(id_pair);
        }
        _ => {}
    }
    Ok(())
}

// =============================================================================
// Calendar Declaration Parser
// =============================================================================

fn parse_calendar_decl(pair: Pair<Rule>) -> Result<Calendar, ParseError> {
    let mut calendar = Calendar {
        id: String::new(),
        name: String::new(),
        working_hours: Vec::new(),
        working_days: Vec::new(),
        holidays: Vec::new(),
        exceptions: Vec::new(),
    };

    for inner in pair.into_inner() {
        match inner.as_rule() {
            Rule::string => {
                calendar.name = parse_string(inner);
                calendar.id = calendar.name.to_lowercase().replace(' ', "_");
            }
            Rule::calendar_body => {
                for attr in inner.into_inner() {
                    if attr.as_rule() == Rule::calendar_attr {
                        parse_calendar_attr(attr, &mut calendar)?;
                    }
                }
            }
            _ => {}
        }
    }

    // Default working days if none specified
    if calendar.working_days.is_empty() {
        calendar.working_days = vec![1, 2, 3, 4, 5]; // Mon-Fri
    }

    // Default working hours if none specified
    if calendar.working_hours.is_empty() {
        calendar.working_hours = vec![
            TimeRange { start: 9 * 60, end: 12 * 60 },
            TimeRange { start: 13 * 60, end: 17 * 60 },
        ];
    }

    Ok(calendar)
}

fn parse_calendar_attr(pair: Pair<Rule>, calendar: &mut Calendar) -> Result<(), ParseError> {
    let inner = pair.into_inner().next().unwrap();
    match inner.as_rule() {
        Rule::working_hours => {
            for time_range in inner.into_inner() {
                if time_range.as_rule() == Rule::time_range_list {
                    for tr in time_range.into_inner() {
                        if tr.as_rule() == Rule::time_range {
                            let range = parse_time_range(tr)?;
                            calendar.working_hours.push(range);
                        }
                    }
                }
            }
        }
        Rule::working_days => {
            for day_list in inner.into_inner() {
                if day_list.as_rule() == Rule::day_list {
                    parse_day_list(day_list, &mut calendar.working_days)?;
                }
            }
        }
        Rule::holiday => {
            let holiday = parse_holiday(inner)?;
            calendar.holidays.push(holiday);
        }
        _ => {}
    }
    Ok(())
}

fn parse_time_range(pair: Pair<Rule>) -> Result<TimeRange, ParseError> {
    let mut times = pair.into_inner();
    let start = parse_time(times.next().unwrap())?;
    let end = parse_time(times.next().unwrap())?;
    Ok(TimeRange { start, end })
}

fn parse_day_list(pair: Pair<Rule>, days: &mut Vec<u8>) -> Result<(), ParseError> {
    let mut inner = pair.into_inner().peekable();

    // Check if this is a range (mon-fri) or a list (mon, wed, fri)
    let first_day = inner.next().unwrap();
    let first = parse_day(first_day);

    if let Some(next) = inner.peek() {
        if next.as_rule() == Rule::day {
            // This might be a range or continuation
            let second = inner.next().unwrap();
            let second_day = parse_day(second);

            // Check if there are more days (indicating it's a list, not a range)
            if inner.peek().is_some() {
                // It's a list
                days.push(first);
                days.push(second_day);
                for d in inner {
                    if d.as_rule() == Rule::day {
                        days.push(parse_day(d));
                    }
                }
            } else {
                // It's a range
                for d in first..=second_day {
                    days.push(d);
                }
            }
        } else {
            days.push(first);
        }
    } else {
        days.push(first);
    }

    Ok(())
}

fn parse_holiday(pair: Pair<Rule>) -> Result<Holiday, ParseError> {
    let mut inner = pair.into_inner();
    let name = parse_string(inner.next().unwrap());
    let date_range = inner.next().unwrap();
    let mut dates = date_range.into_inner();
    let start = parse_date(dates.next().unwrap())?;
    let end = parse_date(dates.next().unwrap())?;

    Ok(Holiday { name, start, end })
}

// =============================================================================
// Resource Declaration Parser
// =============================================================================

fn parse_resource_decl(pair: Pair<Rule>) -> Result<Resource, ParseError> {
    let mut inner = pair.into_inner();
    let id = parse_identifier(inner.next().unwrap());
    let name = parse_string(inner.next().unwrap());

    let mut resource = Resource::new(&id).name(&name);

    if let Some(body) = inner.next() {
        if body.as_rule() == Rule::resource_body {
            for attr in body.into_inner() {
                if attr.as_rule() == Rule::resource_attr {
                    parse_resource_attr(attr, &mut resource)?;
                }
            }
        }
    }

    Ok(resource)
}

fn parse_resource_attr(pair: Pair<Rule>, resource: &mut Resource) -> Result<(), ParseError> {
    let inner = pair.into_inner().next().unwrap();
    match inner.as_rule() {
        Rule::resource_rate => {
            let money_pair = inner.into_inner().next().unwrap();
            let money = parse_money(money_pair)?;
            resource.rate = Some(money);
        }
        Rule::resource_capacity => {
            let num_pair = inner.into_inner().next().unwrap();
            resource.capacity = parse_number(num_pair)? as f32;
        }
        Rule::resource_calendar => {
            let id_pair = inner.into_inner().next().unwrap();
            resource.calendar = Some(parse_identifier(id_pair));
        }
        Rule::resource_efficiency => {
            let num_pair = inner.into_inner().next().unwrap();
            resource.efficiency = parse_number(num_pair)? as f32;
        }
        _ => {}
    }
    Ok(())
}

fn parse_money(pair: Pair<Rule>) -> Result<Money, ParseError> {
    let mut inner = pair.into_inner();
    let amount_str = inner.next().unwrap().as_str();
    let amount = Decimal::from_str(amount_str)
        .map_err(|_| ParseError::InvalidValue(format!("Invalid money amount: {}", amount_str)))?;

    let time_unit = inner.next().unwrap().as_str();
    // Currency is derived from time_unit context - we use the time unit as a suffix indicator
    // The actual currency comes from the project

    Ok(Money {
        amount,
        currency: format!("/{}", time_unit), // Store rate type for now
    })
}

// =============================================================================
// Task Declaration Parser
// =============================================================================

fn parse_task_decl(pair: Pair<Rule>) -> Result<Task, ParseError> {
    let mut inner = pair.into_inner();
    let id = parse_identifier(inner.next().unwrap());
    let name = parse_string(inner.next().unwrap());

    let mut task = Task::new(&id).name(&name);

    if let Some(body) = inner.next() {
        if body.as_rule() == Rule::task_body {
            for item in body.into_inner() {
                match item.as_rule() {
                    Rule::task_attr => parse_task_attr(item, &mut task)?,
                    Rule::task_decl => {
                        let child = parse_task_decl(item)?;
                        task.children.push(child);
                    }
                    _ => {}
                }
            }
        }
    }

    Ok(task)
}

fn parse_task_attr(pair: Pair<Rule>, task: &mut Task) -> Result<(), ParseError> {
    let inner = pair.into_inner().next().unwrap();
    match inner.as_rule() {
        Rule::task_effort => {
            let dur_pair = inner.into_inner().next().unwrap();
            task.effort = Some(parse_duration(dur_pair)?);
        }
        Rule::task_duration => {
            let dur_pair = inner.into_inner().next().unwrap();
            task.duration = Some(parse_duration(dur_pair)?);
        }
        Rule::task_depends => {
            for dep_list in inner.into_inner() {
                if dep_list.as_rule() == Rule::dependency_list {
                    for dep in dep_list.into_inner() {
                        if dep.as_rule() == Rule::dependency {
                            let dependency = parse_dependency(dep)?;
                            task.depends.push(dependency);
                        }
                    }
                }
            }
        }
        Rule::task_assign => {
            for ref_list in inner.into_inner() {
                if ref_list.as_rule() == Rule::resource_ref_list {
                    for res_ref in ref_list.into_inner() {
                        if res_ref.as_rule() == Rule::resource_ref {
                            let resource_ref = parse_resource_ref(res_ref)?;
                            task.assigned.push(resource_ref);
                        }
                    }
                }
            }
        }
        Rule::task_priority => {
            let int_pair = inner.into_inner().next().unwrap();
            task.priority = parse_integer(int_pair)? as u32;
        }
        Rule::task_milestone => {
            let bool_pair = inner.into_inner().next().unwrap();
            task.milestone = parse_boolean(bool_pair);
            if task.milestone {
                task.duration = Some(Duration::zero());
            }
        }
        Rule::task_complete => {
            let pct_pair = inner.into_inner().next().unwrap();
            task.complete = Some(parse_percentage(pct_pair)? * 100.0); // Store as 0-100
        }
        Rule::task_constraint => {
            let constraint = parse_task_constraint(inner)?;
            task.constraints.push(constraint);
        }
        _ => {}
    }
    Ok(())
}

fn parse_dependency(pair: Pair<Rule>) -> Result<Dependency, ParseError> {
    let mut inner = pair.into_inner();

    // Parse task reference (may be dotted path like "design.requirements")
    let task_ref = inner.next().unwrap();
    let predecessor: String = task_ref
        .into_inner()
        .map(|p| p.as_str())
        .collect::<Vec<_>>()
        .join(".");

    let mut dependency = Dependency {
        predecessor,
        dep_type: DependencyType::FinishToStart,
        lag: None,
    };

    // Parse optional modifier
    if let Some(modifier) = inner.next() {
        if modifier.as_rule() == Rule::dep_modifier {
            let mod_inner = modifier.into_inner().next().unwrap();
            match mod_inner.as_rule() {
                Rule::dep_lag => {
                    let lag_str = mod_inner.as_str();
                    let is_negative = lag_str.starts_with('-');
                    let dur_part = &lag_str[1..]; // Skip +/-
                    // Create a fake pair for duration parsing
                    let minutes = parse_duration_str(dur_part)?;
                    let lag = if is_negative {
                        Duration { minutes: -minutes.minutes }
                    } else {
                        minutes
                    };
                    dependency.lag = Some(lag);
                }
                Rule::dep_type => {
                    let type_pair = mod_inner.into_inner().next().unwrap();
                    dependency.dep_type = match type_pair.as_str() {
                        "FS" => DependencyType::FinishToStart,
                        "SS" => DependencyType::StartToStart,
                        "FF" => DependencyType::FinishToFinish,
                        "SF" => DependencyType::StartToFinish,
                        _ => DependencyType::FinishToStart,
                    };
                }
                _ => {}
            }
        }
    }

    Ok(dependency)
}

fn parse_duration_str(s: &str) -> Result<Duration, ParseError> {
    let len = s.len();
    let unit = &s[len - 1..];
    let value: f64 = s[..len - 1]
        .parse()
        .map_err(|_| ParseError::InvalidValue(format!("Invalid duration: {}", s)))?;

    let minutes = match unit {
        "m" => (value * 20.0 * 8.0 * 60.0) as i64, // months
        "w" => (value * 5.0 * 8.0 * 60.0) as i64,   // weeks
        "d" => (value * 8.0 * 60.0) as i64,         // days
        "h" => (value * 60.0) as i64,               // hours
        _ => return Err(ParseError::InvalidValue(format!("Unknown duration unit: {}", unit))),
    };

    Ok(Duration { minutes })
}

fn parse_resource_ref(pair: Pair<Rule>) -> Result<ResourceRef, ParseError> {
    let mut inner = pair.into_inner();
    let resource_id = parse_identifier(inner.next().unwrap());

    let units = if let Some(pct) = inner.next() {
        parse_percentage(pct)?
    } else {
        1.0
    };

    Ok(ResourceRef { resource_id, units })
}

fn parse_task_constraint(pair: Pair<Rule>) -> Result<TaskConstraint, ParseError> {
    let mut inner = pair.into_inner();
    let constraint_type = inner.next().unwrap();
    let date = parse_date(inner.next().unwrap())?;

    let type_str = constraint_type.as_str();
    match type_str {
        "must_start_on" => Ok(TaskConstraint::MustStartOn(date)),
        "must_finish_on" => Ok(TaskConstraint::MustFinishOn(date)),
        "start_no_earlier_than" => Ok(TaskConstraint::StartNoEarlierThan(date)),
        "start_no_later_than" => Ok(TaskConstraint::StartNoLaterThan(date)),
        "finish_no_earlier_than" => Ok(TaskConstraint::FinishNoEarlierThan(date)),
        "finish_no_later_than" => Ok(TaskConstraint::FinishNoLaterThan(date)),
        _ => Err(ParseError::InvalidValue(format!("Unknown constraint type: {}", type_str))),
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_empty_project() {
        let input = r#"project "Test" {
    start: 2025-01-01
}"#;
        let result = ProjectParser::parse(Rule::project_file, input);
        assert!(result.is_ok(), "Failed to parse: {:?}", result.err());
    }

    #[test]
    fn parse_simple_project() {
        let input = r#"
project "Hello World" {
    start: 2025-01-01
    currency: USD
}

resource dev "Developer" {
    rate: 800/day
    capacity: 1.0
}

task design "Design" {
    effort: 3d
    assign: dev
}

task implement "Implementation" {
    effort: 5d
    assign: dev
    depends: design
}
"#;
        let project = parse(input).expect("Failed to parse project");

        assert_eq!(project.name, "Hello World");
        assert_eq!(project.currency, "USD");
        assert_eq!(project.resources.len(), 1);
        assert_eq!(project.resources[0].id, "dev");
        assert_eq!(project.resources[0].name, "Developer");
        assert_eq!(project.tasks.len(), 2);
        assert_eq!(project.tasks[0].id, "design");
        assert_eq!(project.tasks[1].id, "implement");
        assert_eq!(project.tasks[1].depends.len(), 1);
        assert_eq!(project.tasks[1].depends[0].predecessor, "design");
    }

    #[test]
    fn parse_duration_values() {
        assert_eq!(parse_duration_str("5d").unwrap().as_days(), 5.0);
        assert_eq!(parse_duration_str("2w").unwrap().as_days(), 10.0);
        assert_eq!(parse_duration_str("8h").unwrap().as_hours(), 8.0);
    }

    #[test]
    fn parse_nested_tasks() {
        let input = r#"
project "Test" {
    start: 2025-01-01
}

task phase1 "Phase 1" {
    task design "Design" {
        effort: 3d
    }
    task implement "Implement" {
        effort: 5d
        depends: design
    }
}
"#;
        let project = parse(input).expect("Failed to parse");

        assert_eq!(project.tasks.len(), 1);
        assert_eq!(project.tasks[0].id, "phase1");
        assert_eq!(project.tasks[0].children.len(), 2);
        assert_eq!(project.tasks[0].children[0].id, "design");
        assert_eq!(project.tasks[0].children[1].id, "implement");
    }

    #[test]
    fn parse_milestone() {
        let input = r#"
project "Test" { start: 2025-01-01 }
task deploy "Deployment" {
    milestone: true
    depends: test
}
"#;
        let project = parse(input).expect("Failed to parse");

        assert!(project.tasks[0].milestone);
        assert_eq!(project.tasks[0].duration, Some(Duration::zero()));
    }

    #[test]
    fn parse_with_comments() {
        // Test that comments at the start are handled correctly
        let input = r#"# A comment
project "Test" {
    start: 2025-01-01
}
"#;
        let project = parse(input).expect("Failed to parse with comments");
        assert_eq!(project.name, "Test");
    }

    #[test]
    fn parse_calendar() {
        let input = r#"
project "Test" { start: 2025-01-01 }

calendar "Work Week" {
    working_days: mon-fri
    working_hours: 09:00-12:00, 13:00-17:00
    holiday "Christmas" 2025-12-25..2025-12-26
}
"#;
        let project = parse(input).expect("Failed to parse calendar");

        assert_eq!(project.calendars.len(), 1);
        let cal = &project.calendars[0];
        assert_eq!(cal.name, "Work Week");
        assert_eq!(cal.working_days, vec![1, 2, 3, 4, 5]); // Mon-Fri
        assert_eq!(cal.working_hours.len(), 2);
        assert_eq!(cal.holidays.len(), 1);
        assert_eq!(cal.holidays[0].name, "Christmas");
    }

    #[test]
    fn parse_project_end_and_calendar() {
        let input = r#"
project "Test" {
    start: 2025-01-01
    end: 2025-12-31
    calendar: work_week
}
"#;
        let project = parse(input).expect("Failed to parse project");
        assert_eq!(project.end, Some(NaiveDate::from_ymd_opt(2025, 12, 31).unwrap()));
        assert_eq!(project.calendar, "work_week");
    }

    #[test]
    fn parse_resource_with_all_attributes() {
        let input = r#"
project "Test" { start: 2025-01-01 }

resource dev "Developer" {
    rate: 100/hour
    capacity: 0.8
    calendar: dev_calendar
    efficiency: 1.2
}
"#;
        let project = parse(input).expect("Failed to parse resource");
        let res = &project.resources[0];
        assert_eq!(res.id, "dev");
        assert!(res.rate.is_some());
        assert_eq!(res.capacity, 0.8);
        assert_eq!(res.calendar, Some("dev_calendar".to_string()));
        assert_eq!(res.efficiency, 1.2);
    }

    #[test]
    fn parse_task_with_priority_and_complete() {
        let input = r#"
project "Test" { start: 2025-01-01 }

task high_priority "High Priority Task" {
    effort: 5d
    priority: 100
    complete: 75%
}
"#;
        let project = parse(input).expect("Failed to parse task");
        let task = &project.tasks[0];
        assert_eq!(task.priority, 100);
        assert_eq!(task.complete, Some(75.0));
    }

    #[test]
    fn parse_task_constraints() {
        let input = r#"
project "Test" { start: 2025-01-01 }

task constrained "Constrained Task" {
    effort: 3d
    must_start_on: 2025-02-01
}
"#;
        let project = parse(input).expect("Failed to parse constraint");
        let task = &project.tasks[0];
        assert_eq!(task.constraints.len(), 1);
        match &task.constraints[0] {
            TaskConstraint::MustStartOn(date) => {
                assert_eq!(*date, NaiveDate::from_ymd_opt(2025, 2, 1).unwrap());
            }
            _ => panic!("Expected MustStartOn constraint"),
        }
    }

    #[test]
    fn parse_dependency_with_lag() {
        let input = r#"
project "Test" { start: 2025-01-01 }

task a "Task A" { effort: 5d }
task b "Task B" {
    effort: 3d
    depends: a +2d
}
"#;
        let project = parse(input).expect("Failed to parse dependency with lag");
        let task_b = &project.tasks[1];
        assert_eq!(task_b.depends.len(), 1);
        assert!(task_b.depends[0].lag.is_some());
        assert_eq!(task_b.depends[0].lag.unwrap().as_days(), 2.0);
    }

    #[test]
    fn parse_dependency_with_type() {
        let input = r#"
project "Test" { start: 2025-01-01 }

task a "Task A" { effort: 5d }
task b "Task B" {
    effort: 3d
    depends: a SS
}
task c "Task C" {
    effort: 2d
    depends: a FF
}
"#;
        let project = parse(input).expect("Failed to parse dependency types");
        assert_eq!(project.tasks[1].depends[0].dep_type, DependencyType::StartToStart);
        assert_eq!(project.tasks[2].depends[0].dep_type, DependencyType::FinishToFinish);
    }

    #[test]
    fn parse_resource_ref_with_percentage() {
        let input = r#"
project "Test" { start: 2025-01-01 }
resource dev "Developer" {}
task work "Work" {
    effort: 5d
    assign: dev@50%
}
"#;
        let project = parse(input).expect("Failed to parse resource ref");
        let task = &project.tasks[0];
        assert_eq!(task.assigned.len(), 1);
        assert_eq!(task.assigned[0].resource_id, "dev");
        assert!((task.assigned[0].units - 0.5).abs() < 0.01);
    }

    #[test]
    fn parse_hours_duration() {
        let input = r#"
project "Test" { start: 2025-01-01 }
task quick "Quick Task" {
    effort: 4h
}
"#;
        let project = parse(input).expect("Failed to parse hours");
        let task = &project.tasks[0];
        assert_eq!(task.effort.unwrap().as_hours(), 4.0);
    }

    #[test]
    fn parse_syntax_error() {
        let input = r#"project "Test" { invalid syntax here }"#;
        let result = parse(input);
        assert!(result.is_err());
        if let Err(ParseError::Syntax { line, column, .. }) = result {
            assert!(line > 0);
            assert!(column > 0);
        } else {
            panic!("Expected Syntax error");
        }
    }

    #[test]
    fn parse_simple_proj_fixture() {
        // Test with inline content that matches the fixture (without leading comments for now)
        let fixture = r#"project "Hello World" {
    start: 2025-01-01
    currency: USD
}

resource dev "Developer" {
    rate: 800/day
    capacity: 1.0
}

task design "Design" {
    effort: 3d
    assign: dev
}

task implement "Implementation" {
    effort: 5d
    assign: dev
    depends: design
}

task test "Testing" {
    effort: 2d
    assign: dev
    depends: implement
}

task deploy "Deployment" {
    duration: 1d
    depends: test
    milestone: true
}
"#;

        let project = parse(fixture).expect("Failed to parse simple.proj fixture");

        // Validate project metadata
        assert_eq!(project.name, "Hello World");
        assert_eq!(project.currency, "USD");
        assert_eq!(
            project.start,
            chrono::NaiveDate::from_ymd_opt(2025, 1, 1).unwrap()
        );

        // Validate resource
        assert_eq!(project.resources.len(), 1);
        assert_eq!(project.resources[0].id, "dev");
        assert_eq!(project.resources[0].name, "Developer");
        assert_eq!(project.resources[0].capacity, 1.0);
        assert!(project.resources[0].rate.is_some());

        // Validate tasks
        assert_eq!(project.tasks.len(), 4);

        // Task 1: design
        assert_eq!(project.tasks[0].id, "design");
        assert_eq!(project.tasks[0].name, "Design");
        assert_eq!(project.tasks[0].effort, Some(Duration::days(3)));
        assert_eq!(project.tasks[0].assigned.len(), 1);
        assert_eq!(project.tasks[0].assigned[0].resource_id, "dev");

        // Task 2: implement (depends on design)
        assert_eq!(project.tasks[1].id, "implement");
        assert_eq!(project.tasks[1].depends.len(), 1);
        assert_eq!(project.tasks[1].depends[0].predecessor, "design");

        // Task 3: test (depends on implement)
        assert_eq!(project.tasks[2].id, "test");
        assert_eq!(project.tasks[2].depends[0].predecessor, "implement");

        // Task 4: deploy (milestone)
        assert_eq!(project.tasks[3].id, "deploy");
        assert!(project.tasks[3].milestone);
        assert_eq!(project.tasks[3].duration, Some(Duration::zero()));
    }
}
