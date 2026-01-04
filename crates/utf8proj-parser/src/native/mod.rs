//! Native DSL parser for .proj files using pest.

use chrono::NaiveDate;
use pest::iterators::Pair;
use pest::Parser;
use pest_derive::Parser;
use rust_decimal::Decimal;
use std::str::FromStr;

use utf8proj_core::{
    Calendar, Dependency, DependencyType, Duration, Holiday, Money, Project, Resource,
    ResourceRef, Task, TaskConstraint, TaskStatus, TimeRange,
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
            Rule::milestone_decl => {
                let task = parse_milestone_decl(pair)?;
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

fn parse_status(pair: Pair<Rule>) -> TaskStatus {
    match pair.as_str() {
        "not_started" => TaskStatus::NotStarted,
        "in_progress" => TaskStatus::InProgress,
        "complete" => TaskStatus::Complete,
        "blocked" => TaskStatus::Blocked,
        "at_risk" => TaskStatus::AtRisk,
        "on_hold" => TaskStatus::OnHold,
        _ => TaskStatus::NotStarted, // Default
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
        Rule::project_timezone => {
            // Timezone stored in attributes for now
            let tz_pair = inner.into_inner().next().unwrap();
            project.attributes.insert("timezone".to_string(), tz_pair.as_str().to_string());
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
    let date_or_range = inner.next().unwrap();

    let (start, end) = if date_or_range.as_rule() == Rule::date_range {
        let mut dates = date_or_range.into_inner();
        let start = parse_date(dates.next().unwrap())?;
        let end = parse_date(dates.next().unwrap())?;
        (start, end)
    } else {
        // Single date - start and end are the same
        let date = parse_date(date_or_range)?;
        (date, date)
    };

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
        Rule::resource_email => {
            let str_pair = inner.into_inner().next().unwrap();
            resource.attributes.insert("email".to_string(), parse_string(str_pair));
        }
        Rule::resource_role => {
            let str_pair = inner.into_inner().next().unwrap();
            resource.attributes.insert("role".to_string(), parse_string(str_pair));
        }
        Rule::resource_leave => {
            let date_range = inner.into_inner().next().unwrap();
            let mut dates = date_range.into_inner();
            let start = parse_date(dates.next().unwrap())?;
            let end = parse_date(dates.next().unwrap())?;
            resource.attributes.insert("leave".to_string(), format!("{}..{}", start, end));
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
                    Rule::milestone_decl => {
                        let child = parse_milestone_decl(item)?;
                        task.children.push(child);
                    }
                    _ => {}
                }
            }
        }
    }

    Ok(task)
}

/// Parse a milestone declaration (milestone id "name" { ... })
fn parse_milestone_decl(pair: Pair<Rule>) -> Result<Task, ParseError> {
    let mut inner = pair.into_inner();
    let id = parse_identifier(inner.next().unwrap());
    let name = parse_string(inner.next().unwrap());

    let mut task = Task::new(&id).name(&name);
    task.milestone = true;
    task.duration = Some(Duration::zero());

    if let Some(body) = inner.next() {
        if body.as_rule() == Rule::milestone_body {
            for attr in body.into_inner() {
                if attr.as_rule() == Rule::milestone_attr {
                    parse_milestone_attr(attr, &mut task)?;
                }
            }
        }
    }

    Ok(task)
}

fn parse_milestone_attr(pair: Pair<Rule>, task: &mut Task) -> Result<(), ParseError> {
    let inner = pair.into_inner().next().unwrap();
    match inner.as_rule() {
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
        Rule::task_note => {
            let str_pair = inner.into_inner().next().unwrap();
            task.attributes.insert("note".to_string(), parse_string(str_pair));
        }
        Rule::task_payment => {
            let num_pair = inner.into_inner().next().unwrap();
            task.attributes.insert("payment".to_string(), num_pair.as_str().to_string());
        }
        _ => {}
    }
    Ok(())
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
        Rule::task_note => {
            let str_pair = inner.into_inner().next().unwrap();
            task.attributes.insert("note".to_string(), parse_string(str_pair));
        }
        Rule::task_tag => {
            let mut tags = Vec::new();
            for id in inner.into_inner() {
                if id.as_rule() == Rule::identifier_list {
                    for tag in id.into_inner() {
                        tags.push(parse_identifier(tag));
                    }
                }
            }
            task.attributes.insert("tags".to_string(), tags.join(","));
        }
        Rule::task_cost => {
            let num_pair = inner.into_inner().next().unwrap();
            task.attributes.insert("cost".to_string(), num_pair.as_str().to_string());
        }
        Rule::task_payment => {
            let num_pair = inner.into_inner().next().unwrap();
            task.attributes.insert("payment".to_string(), num_pair.as_str().to_string());
        }
        Rule::task_actual_start => {
            let date_pair = inner.into_inner().next().unwrap();
            task.actual_start = Some(parse_date(date_pair)?);
        }
        Rule::task_actual_finish => {
            let date_pair = inner.into_inner().next().unwrap();
            task.actual_finish = Some(parse_date(date_pair)?);
        }
        Rule::task_status => {
            let status_pair = inner.into_inner().next().unwrap();
            task.status = Some(parse_status(status_pair));
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
    use chrono::Datelike;

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

    #[test]
    fn parse_single_date_holiday() {
        let input = r#"
project "Test" { start: 2025-01-01 }

calendar "Standard" {
    working_days: mon-fri
    holiday "New Year" 2025-01-01
}
"#;
        let project = parse(input).expect("Failed to parse single date holiday");
        let cal = &project.calendars[0];
        assert_eq!(cal.holidays.len(), 1);
        assert_eq!(cal.holidays[0].name, "New Year");
        // Single date means start == end
        assert_eq!(cal.holidays[0].start, cal.holidays[0].end);
    }

    #[test]
    fn parse_working_days_list() {
        let input = r#"
project "Test" { start: 2025-01-01 }

calendar "Part Time" {
    working_days: mon, wed, fri
}
"#;
        let project = parse(input).expect("Failed to parse working days list");
        let cal = &project.calendars[0];
        // Should be Monday (1), Wednesday (3), Friday (5)
        assert_eq!(cal.working_days.len(), 3);
        assert!(cal.working_days.contains(&1)); // Monday
        assert!(cal.working_days.contains(&3)); // Wednesday
        assert!(cal.working_days.contains(&5)); // Friday
    }

    #[test]
    fn parse_single_working_day() {
        let input = r#"
project "Test" { start: 2025-01-01 }

calendar "Minimal" {
    working_days: fri
}
"#;
        let project = parse(input).expect("Failed to parse single working day");
        let cal = &project.calendars[0];
        assert_eq!(cal.working_days.len(), 1);
        assert_eq!(cal.working_days[0], 5); // Friday
    }

    #[test]
    fn parse_milestone_declaration_syntax() {
        let input = r#"
project "Test" { start: 2025-01-01 }

task work "Do Work" { effort: 5d }

milestone phase1_complete "Phase 1 Complete" {
    depends: work
}
"#;
        let project = parse(input).expect("Failed to parse milestone declaration");
        assert_eq!(project.tasks.len(), 2);

        let milestone = &project.tasks[1];
        assert_eq!(milestone.id, "phase1_complete");
        assert_eq!(milestone.name, "Phase 1 Complete");
        assert!(milestone.milestone);
        assert_eq!(milestone.duration, Some(Duration::zero()));
        assert_eq!(milestone.depends.len(), 1);
        assert_eq!(milestone.depends[0].predecessor, "work");
    }

    #[test]
    fn parse_milestone_with_note_and_payment() {
        let input = r#"
project "Test" { start: 2025-01-01 }

milestone payment_due "Payment Due" {
    depends: delivery
    note: "Invoice to be sent upon completion"
    payment: 50000
}
"#;
        let project = parse(input).expect("Failed to parse milestone with attributes");
        let milestone = &project.tasks[0];

        assert!(milestone.milestone);
        assert_eq!(milestone.attributes.get("note").map(|s| s.as_str()),
                   Some("Invoice to be sent upon completion"));
        assert_eq!(milestone.attributes.get("payment").map(|s| s.as_str()),
                   Some("50000"));
    }

    #[test]
    fn parse_all_constraint_types() {
        let input = r#"
project "Test" { start: 2025-01-01 }

task a "Task A" { effort: 1d
    must_start_on: 2025-02-01
}
task b "Task B" { effort: 1d
    must_finish_on: 2025-03-15
}
task c "Task C" { effort: 1d
    start_no_earlier_than: 2025-02-15
}
task d "Task D" { effort: 1d
    start_no_later_than: 2025-04-01
}
task e "Task E" { effort: 1d
    finish_no_earlier_than: 2025-05-01
}
task f "Task F" { effort: 1d
    finish_no_later_than: 2025-06-30
}
"#;
        let project = parse(input).expect("Failed to parse all constraint types");
        assert_eq!(project.tasks.len(), 6);

        // Check each constraint type
        match &project.tasks[0].constraints[0] {
            TaskConstraint::MustStartOn(d) => assert_eq!(d.month(), 2),
            _ => panic!("Expected MustStartOn"),
        }
        match &project.tasks[1].constraints[0] {
            TaskConstraint::MustFinishOn(d) => assert_eq!(d.month(), 3),
            _ => panic!("Expected MustFinishOn"),
        }
        match &project.tasks[2].constraints[0] {
            TaskConstraint::StartNoEarlierThan(d) => assert_eq!(d.day(), 15),
            _ => panic!("Expected StartNoEarlierThan"),
        }
        match &project.tasks[3].constraints[0] {
            TaskConstraint::StartNoLaterThan(d) => assert_eq!(d.month(), 4),
            _ => panic!("Expected StartNoLaterThan"),
        }
        match &project.tasks[4].constraints[0] {
            TaskConstraint::FinishNoEarlierThan(d) => assert_eq!(d.month(), 5),
            _ => panic!("Expected FinishNoEarlierThan"),
        }
        match &project.tasks[5].constraints[0] {
            TaskConstraint::FinishNoLaterThan(d) => assert_eq!(d.month(), 6),
            _ => panic!("Expected FinishNoLaterThan"),
        }
    }

    #[test]
    fn parse_task_with_note_and_tags() {
        let input = r#"
project "Test" { start: 2025-01-01 }

task documented "Documented Task" {
    effort: 3d
    note: "This is an important task"
    tag: critical
    tag: priority
}
"#;
        let project = parse(input).expect("Failed to parse task with note");
        let task = &project.tasks[0];

        assert_eq!(task.attributes.get("note").map(|s| s.as_str()),
                   Some("This is an important task"));
    }

    #[test]
    fn parse_task_with_cost() {
        let input = r#"
project "Test" { start: 2025-01-01 }

task expensive "Expensive Task" {
    effort: 5d
    cost: 10000
}
"#;
        let project = parse(input).expect("Failed to parse task with cost");
        let task = &project.tasks[0];

        assert_eq!(task.attributes.get("cost").map(|s| s.as_str()),
                   Some("10000"));
    }

    #[test]
    fn parse_sf_dependency_type() {
        let input = r#"
project "Test" { start: 2025-01-01 }

task a "Task A" { effort: 5d }
task b "Task B" {
    effort: 3d
    depends: a SF
}
"#;
        let project = parse(input).expect("Failed to parse SF dependency");
        assert_eq!(project.tasks[1].depends[0].dep_type, DependencyType::StartToFinish);
    }

    #[test]
    fn parse_dependency_with_negative_lag() {
        let input = r#"
project "Test" { start: 2025-01-01 }

task a "Task A" { effort: 5d }
task b "Task B" {
    effort: 3d
    depends: a -1d
}
"#;
        let project = parse(input).expect("Failed to parse negative lag");
        let dep = &project.tasks[1].depends[0];
        assert!(dep.lag.is_some());
        assert_eq!(dep.lag.unwrap().as_days(), -1.0);
    }

    #[test]
    fn parse_project_with_timezone() {
        let input = r#"
project "Test" {
    start: 2025-01-01
    timezone: Europe/Rome
}
"#;
        let project = parse(input).expect("Failed to parse timezone");
        assert_eq!(project.attributes.get("timezone").map(|s| s.as_str()),
                   Some("Europe/Rome"));
    }

    #[test]
    fn parse_resource_with_email() {
        let input = r#"
project "Test" { start: 2025-01-01 }

resource pm "Project Manager" {
    rate: 850/day
    email: "pm@example.com"
}
"#;
        let project = parse(input).expect("Failed to parse resource email");
        let res = &project.resources[0];
        assert_eq!(res.attributes.get("email").map(|s| s.as_str()),
                   Some("pm@example.com"));
    }

    #[test]
    fn parse_resource_percentage_parentheses() {
        let input = r#"
project "Test" { start: 2025-01-01 }
resource dev "Developer" {}
task work "Work" {
    effort: 5d
    assign: dev(75%)
}
"#;
        let project = parse(input).expect("Failed to parse percentage with parentheses");
        let task = &project.tasks[0];
        assert!((task.assigned[0].units - 0.75).abs() < 0.01);
    }

    #[test]
    fn parse_resource_with_role() {
        let input = r#"
project "Test" { start: 2025-01-01 }
resource dev "Developer" {
    rate: 500/day
    role: "Senior Engineer"
}
"#;
        let project = parse(input).expect("Failed to parse resource role");
        let res = &project.resources[0];
        assert_eq!(res.attributes.get("role").map(|s| s.as_str()),
                   Some("Senior Engineer"));
    }

    #[test]
    fn parse_resource_with_leave() {
        let input = r#"
project "Test" { start: 2025-01-01 }
resource dev "Developer" {
    rate: 500/day
    leave: 2025-06-01..2025-06-15
}
"#;
        let project = parse(input).expect("Failed to parse resource leave");
        let res = &project.resources[0];
        assert!(res.attributes.contains_key("leave"));
    }

    #[test]
    fn parse_invalid_duration_unit() {
        let input = r#"
project "Test" { start: 2025-01-01 }
task a "Task A" { duration: 5x }
"#;
        let result = parse(input);
        assert!(result.is_err());
    }

    #[test]
    fn parse_saturday_working_day() {
        let input = r#"
project "Test" { start: 2025-01-01 }
calendar "Weekend Work" {
    working_days: mon-sat
}
"#;
        let project = parse(input).expect("Failed to parse saturday working day");
        let cal = &project.calendars[0];
        assert!(cal.working_days.contains(&6)); // Saturday
    }

    #[test]
    fn parse_sunday_working_day() {
        let input = r#"
project "Test" { start: 2025-01-01 }
calendar "Seven Days" {
    working_days: sun-sat
}
"#;
        let project = parse(input).expect("Failed to parse sunday working day");
        let cal = &project.calendars[0];
        assert!(cal.working_days.contains(&0)); // Sunday
        assert_eq!(cal.working_days.len(), 7);
    }

    #[test]
    fn parse_calendar_default_working_days() {
        // Line 253: calendar with no working_days gets default Mon-Fri
        let input = r#"
project "Test" { start: 2025-01-01 }
calendar "Default Hours" {
    working_hours: 08:00-17:00
}
"#;
        let project = parse(input).expect("Failed to parse calendar with default days");
        let cal = &project.calendars[0];
        // Should default to Mon-Fri (1-5)
        assert_eq!(cal.working_days, vec![1, 2, 3, 4, 5]);
    }

    #[test]
    fn parse_nested_milestone_in_task() {
        // Lines 464-465: milestone nested inside a task
        let input = r#"
project "Test" { start: 2025-01-01 }
task phase "Phase" {
    task work "Work" { duration: 5d }
    milestone done "Done" { }
}
"#;
        let project = parse(input).expect("Failed to parse nested milestone");
        let phase = &project.tasks[0];
        assert_eq!(phase.children.len(), 2);
        // The milestone should be the second child
        assert!(phase.children[1].milestone);
    }

    #[test]
    fn parse_task_payment() {
        // Lines 601-602: task payment attribute
        let input = r#"
project "Test" { start: 2025-01-01 }
task a "Task A" {
    duration: 5d
    payment: 5000
}
"#;
        let project = parse(input).expect("Failed to parse task payment");
        let task = &project.tasks[0];
        assert_eq!(task.attributes.get("payment").map(|s| s.as_str()), Some("5000"));
    }

    #[test]
    fn parse_task_with_progress_tracking() {
        let input = r#"
project "Test" { start: 2025-01-01 }

task design "Design Phase" {
    duration: 10d
    complete: 60%
    actual_start: 2026-01-15
    status: in_progress
}

task done "Completed Task" {
    duration: 5d
    complete: 100%
    actual_start: 2026-01-01
    actual_finish: 2026-01-08
    status: complete
}
"#;
        let project = parse(input).expect("Failed to parse progress tracking");

        // Check first task
        let task1 = &project.tasks[0];
        assert!((task1.complete.unwrap() - 60.0).abs() < 0.01);
        assert_eq!(task1.actual_start, Some(NaiveDate::from_ymd_opt(2026, 1, 15).unwrap()));
        assert!(task1.actual_finish.is_none());
        assert_eq!(task1.status, Some(TaskStatus::InProgress));

        // Check second task
        let task2 = &project.tasks[1];
        assert!((task2.complete.unwrap() - 100.0).abs() < 0.01);
        assert_eq!(task2.actual_start, Some(NaiveDate::from_ymd_opt(2026, 1, 1).unwrap()));
        assert_eq!(task2.actual_finish, Some(NaiveDate::from_ymd_opt(2026, 1, 8).unwrap()));
        assert_eq!(task2.status, Some(TaskStatus::Complete));
    }

    #[test]
    fn parse_all_status_values() {
        let input = r#"
project "Test" { start: 2025-01-01 }
task a "Not Started" { duration: 1d status: not_started }
task b "In Progress" { duration: 1d status: in_progress }
task c "Complete" { duration: 1d status: complete }
task d "Blocked" { duration: 1d status: blocked }
task e "At Risk" { duration: 1d status: at_risk }
task f "On Hold" { duration: 1d status: on_hold }
"#;
        let project = parse(input).expect("Failed to parse all status values");

        assert_eq!(project.tasks[0].status, Some(TaskStatus::NotStarted));
        assert_eq!(project.tasks[1].status, Some(TaskStatus::InProgress));
        assert_eq!(project.tasks[2].status, Some(TaskStatus::Complete));
        assert_eq!(project.tasks[3].status, Some(TaskStatus::Blocked));
        assert_eq!(project.tasks[4].status, Some(TaskStatus::AtRisk));
        assert_eq!(project.tasks[5].status, Some(TaskStatus::OnHold));
    }
}
