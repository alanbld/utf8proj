//! Hover information provider for utf8proj LSP
//!
//! Provides contextual information when hovering over:
//! - Profile identifiers: shows rate range, specialization chain, traits
//! - Resource identifiers: shows rate, capacity, efficiency
//! - Task identifiers: shows dates, slack, criticality, dependencies, assignments,
//!   calendar impact, and related diagnostics

use chrono::Datelike;
use tower_lsp::lsp_types::{Hover, HoverContents, MarkupContent, MarkupKind, Position};

use utf8proj_core::{
    Calendar, Diagnostic, DiagnosticCode, Project, ResourceProfile, ResourceRate, Schedule, Task,
    TaskConstraint,
};

/// Get hover information for a position in the document
pub fn get_hover_info(
    project: &Project,
    schedule: Option<&Schedule>,
    diagnostics: &[Diagnostic],
    text: &str,
    position: Position,
) -> Option<Hover> {
    // Find the word at the cursor position
    let word = get_word_at_position(text, position)?;

    // Try to match against known identifiers
    if let Some(profile) = project.get_profile(&word) {
        return Some(hover_for_profile(profile, project));
    }

    if let Some(resource) = project.get_resource(&word) {
        return Some(hover_for_resource(resource));
    }

    if let Some(task) = find_task_by_id(&project.tasks, &word) {
        return Some(hover_for_task(task, &word, project, schedule, diagnostics));
    }

    if let Some(t) = project.get_trait(&word) {
        return Some(hover_for_trait(t));
    }

    None
}

/// Extract the word at a given position
fn get_word_at_position(text: &str, position: Position) -> Option<String> {
    let lines: Vec<&str> = text.lines().collect();
    let line = lines.get(position.line as usize)?;

    let col = position.character as usize;
    if col > line.len() {
        return None;
    }

    // Find word boundaries
    let chars: Vec<char> = line.chars().collect();

    // Find start of word
    let mut start = col;
    while start > 0 && is_identifier_char(chars.get(start.saturating_sub(1)).copied()?) {
        start -= 1;
    }

    // Find end of word
    let mut end = col;
    while end < chars.len() && is_identifier_char(chars[end]) {
        end += 1;
    }

    if start == end {
        return None;
    }

    Some(chars[start..end].iter().collect())
}

fn is_identifier_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || c == '-'
}

/// Build hover content for a profile
fn hover_for_profile(profile: &ResourceProfile, project: &Project) -> Hover {
    let mut lines = vec![format!("**Profile: {}**", profile.id)];

    // Specialization chain
    if let Some(ref parent) = profile.specializes {
        let mut chain = vec![profile.id.clone()];
        let mut current = project.get_profile(parent);
        while let Some(p) = current {
            chain.push(p.id.clone());
            current = p.specializes.as_ref().and_then(|s| project.get_profile(s));
        }
        lines.push(format!("Specializes: {}", chain.join(" → ")));
    }

    // Rate
    if let Some(ref rate) = profile.rate {
        match rate {
            ResourceRate::Fixed(money) => {
                lines.push(format!("Rate: ${}/day", money.amount));
            }
            ResourceRate::Range(range) => {
                lines.push(format!(
                    "Rate: ${} - ${}/day (expected: ${})",
                    range.min,
                    range.max,
                    range.expected()
                ));
            }
        }
    } else if let Some(ref parent_id) = profile.specializes {
        // Try to show inherited rate
        if let Some(rate) = get_inherited_rate(parent_id, project) {
            lines.push(format!("Rate (inherited): {}", rate));
        } else {
            lines.push("Rate: *not defined*".to_string());
        }
    } else {
        lines.push("Rate: *not defined*".to_string());
    }

    // Traits
    if !profile.traits.is_empty() {
        let trait_info: Vec<String> = profile
            .traits
            .iter()
            .map(|t| {
                if let Some(trait_def) = project.get_trait(t) {
                    format!("{} ({}x)", t, trait_def.rate_multiplier)
                } else {
                    format!("{} (unknown)", t)
                }
            })
            .collect();
        lines.push(format!("Traits: {}", trait_info.join(", ")));
    }

    // Skills
    if !profile.skills.is_empty() {
        lines.push(format!("Skills: {}", profile.skills.join(", ")));
    }

    Hover {
        contents: HoverContents::Markup(MarkupContent {
            kind: MarkupKind::Markdown,
            value: lines.join("\n\n"),
        }),
        range: None,
    }
}

/// Get inherited rate as a display string
fn get_inherited_rate(profile_id: &str, project: &Project) -> Option<String> {
    let mut current = project.get_profile(profile_id);
    while let Some(p) = current {
        if let Some(ref rate) = p.rate {
            return match rate {
                ResourceRate::Fixed(money) => Some(format!("${}/day from {}", money.amount, p.id)),
                ResourceRate::Range(range) => {
                    Some(format!("${} - ${}/day from {}", range.min, range.max, p.id))
                }
            };
        }
        current = p.specializes.as_ref().and_then(|s| project.get_profile(s));
    }
    None
}

/// Build hover content for a resource
fn hover_for_resource(resource: &utf8proj_core::Resource) -> Hover {
    let mut lines = vec![format!("**Resource: {}**", resource.id)];

    if resource.name != resource.id {
        lines.push(format!("Name: {}", resource.name));
    }

    if let Some(ref rate) = resource.rate {
        lines.push(format!("Rate: ${}/day", rate.amount));
    }

    if resource.capacity != 1.0 {
        lines.push(format!("Capacity: {}%", (resource.capacity * 100.0) as i32));
    }

    if resource.efficiency != 1.0 {
        lines.push(format!(
            "Efficiency: {}%",
            (resource.efficiency * 100.0) as i32
        ));
    }

    Hover {
        contents: HoverContents::Markup(MarkupContent {
            kind: MarkupKind::Markdown,
            value: lines.join("\n\n"),
        }),
        range: None,
    }
}

/// Build hover content for a task
fn hover_for_task(
    task: &Task,
    task_id: &str,
    project: &Project,
    schedule: Option<&Schedule>,
    diagnostics: &[Diagnostic],
) -> Hover {
    let mut lines = vec![format!("**Task: {}**", task.id)];

    if task.name != task.id {
        lines.push(format!("Name: {}", task.name));
    }

    if task.milestone {
        lines.push("Type: Milestone".to_string());
    } else if !task.children.is_empty() {
        lines.push(format!(
            "Type: Container ({} children)",
            task.children.len()
        ));
    }

    if let Some(dur) = task.duration {
        lines.push(format!("Duration: {} days", dur.as_days() as i64));
    }

    if let Some(effort) = task.effort {
        lines.push(format!("Effort: {} days", effort.as_days() as i64));
    }

    if !task.assigned.is_empty() {
        let assignments: Vec<String> = task
            .assigned
            .iter()
            .map(|a| {
                if (a.units - 1.0).abs() < 0.01 {
                    a.resource_id.clone()
                } else {
                    format!("{}@{}%", a.resource_id, (a.units * 100.0) as i32)
                }
            })
            .collect();
        lines.push(format!("Assigned: {}", assignments.join(", ")));
    }

    if !task.depends.is_empty() {
        let deps: Vec<&str> = task
            .depends
            .iter()
            .map(|d| d.predecessor.as_str())
            .collect();
        lines.push(format!("Depends on: {}", deps.join(", ")));
    }

    // Show temporal constraints
    if !task.constraints.is_empty() {
        let constraint_strs: Vec<String> = task.constraints.iter().map(format_constraint).collect();
        lines.push(format!("Constraints: {}", constraint_strs.join(", ")));
    }

    if let Some(complete) = task.complete {
        if complete > 0.0 {
            lines.push(format!("Progress: {}%", (complete * 100.0) as i32));
        }
    }

    // Add schedule information if available
    if let Some(sched) = schedule {
        // Try to find this task in the schedule (may be qualified ID like "phase.task")
        let scheduled = sched
            .tasks
            .get(task_id)
            .or_else(|| sched.tasks.get(&task.id));

        if let Some(st) = scheduled {
            lines.push("---".to_string()); // Separator

            // Dates
            lines.push(format!(
                "📅 **Schedule:** {} → {}",
                st.start.format("%Y-%m-%d"),
                st.finish.format("%Y-%m-%d")
            ));

            // Duration (from schedule, may differ from effort)
            lines.push(format!(
                "⏱️ Duration: {} days",
                st.duration.as_days() as i64
            ));

            // Slack and criticality
            let slack_days = st.slack.as_days() as i64;
            if st.is_critical {
                lines.push("🔴 **Critical Path** (0 days slack)".to_string());
            } else if slack_days > 0 {
                lines.push(format!("🟢 Slack: {} days", slack_days));
            }

            // Calendar Impact section
            let project_calendar = project
                .calendars
                .iter()
                .find(|c| c.id == project.calendar)
                .cloned()
                .unwrap_or_else(Calendar::default);

            let (working_days, weekend_days, holiday_days) =
                calculate_calendar_impact(st.start, st.finish, &project_calendar);

            if weekend_days > 0 || holiday_days > 0 {
                lines.push("---".to_string());
                lines.push("**📆 Calendar Impact:**".to_string());
                let mut impact_parts = Vec::new();
                if weekend_days > 0 {
                    impact_parts.push(format!("{} weekend days", weekend_days));
                }
                if holiday_days > 0 {
                    impact_parts.push(format!("{} holidays", holiday_days));
                }
                lines.push(format!(
                    "• {} working days, {}",
                    working_days,
                    impact_parts.join(", ")
                ));
            }

            // Show constraint effects if any
            if !task.constraints.is_empty() {
                let effects = analyze_constraint_effects(task, st);
                if !effects.is_empty() {
                    lines.push("---".to_string());
                    lines.push("**Constraint Effects:**".to_string());
                    for effect in effects {
                        lines.push(effect);
                    }
                }
            }
        }
    }

    // Related Diagnostics section
    let task_diags = filter_task_diagnostics(task_id, diagnostics);
    if !task_diags.is_empty() {
        lines.push("---".to_string());
        lines.push("**⚠️ Diagnostics:**".to_string());
        for code in &task_diags {
            let severity_icon = match code.as_str().chars().next() {
                Some('E') => "🔴",
                Some('W') => "🟡",
                Some('H') => "💡",
                Some('L') => "⚖️", // Leveling
                Some('C') => "📆", // Calendar
                _ => "ℹ️",
            };
            lines.push(format!("• {} `{}`", severity_icon, code.as_str()));
        }
    }

    Hover {
        contents: HoverContents::Markup(MarkupContent {
            kind: MarkupKind::Markdown,
            value: lines.join("\n\n"),
        }),
        range: None,
    }
}

/// Calculate calendar impact for a date range
fn calculate_calendar_impact(
    start: chrono::NaiveDate,
    finish: chrono::NaiveDate,
    calendar: &Calendar,
) -> (u32, u32, u32) {
    let mut working_days = 0u32;
    let mut weekend_days = 0u32;
    let mut holiday_days = 0u32;

    let mut current = start;
    while current <= finish {
        let weekday = current.weekday().num_days_from_sunday() as u8;

        // Check if it's a holiday
        let is_holiday = calendar
            .holidays
            .iter()
            .any(|h| current >= h.start && current <= h.end);

        if is_holiday {
            holiday_days += 1;
        } else if !calendar.working_days.contains(&weekday) {
            weekend_days += 1;
        } else {
            working_days += 1;
        }

        current = current.succ_opt().unwrap_or(current);
        if current == finish && current == start {
            break; // Avoid infinite loop for zero-duration tasks
        }
    }

    (working_days, weekend_days, holiday_days)
}

/// Filter diagnostics relevant to a specific task
fn filter_task_diagnostics(task_id: &str, diagnostics: &[Diagnostic]) -> Vec<DiagnosticCode> {
    diagnostics
        .iter()
        .filter(|d| is_diagnostic_for_task(d, task_id))
        .map(|d| d.code.clone())
        .collect()
}

/// Check if a diagnostic is relevant to a specific task
fn is_diagnostic_for_task(diagnostic: &Diagnostic, task_id: &str) -> bool {
    let quoted_id = format!("'{}'", task_id);
    match diagnostic.code {
        DiagnosticCode::C010NonWorkingDay | DiagnosticCode::C011CalendarMismatch => {
            diagnostic.message.contains(&quoted_id)
        }
        DiagnosticCode::H004TaskUnconstrained => diagnostic.message.contains(&quoted_id),
        DiagnosticCode::W001AbstractAssignment | DiagnosticCode::H001MixedAbstraction => {
            diagnostic.message.contains(&quoted_id)
        }
        DiagnosticCode::W014ContainerDependency => diagnostic.message.contains(&quoted_id),
        // Leveling diagnostics (L001-L004)
        DiagnosticCode::L001OverallocationResolved
        | DiagnosticCode::L003DurationIncreased
        | DiagnosticCode::L004MilestoneDelayed => diagnostic.message.contains(&quoted_id),
        DiagnosticCode::L002UnresolvableConflict => {
            // L002 mentions resource, not task - check notes for task references
            diagnostic.notes.iter().any(|n| n.contains(&quoted_id))
                || diagnostic.message.contains(&quoted_id)
        }
        _ => false,
    }
}

/// Build hover content for a trait
fn hover_for_trait(t: &utf8proj_core::Trait) -> Hover {
    let mut lines = vec![format!("**Trait: {}**", t.id)];

    if let Some(ref desc) = t.description {
        lines.push(desc.clone());
    }

    lines.push(format!("Rate multiplier: {}x", t.rate_multiplier));

    Hover {
        contents: HoverContents::Markup(MarkupContent {
            kind: MarkupKind::Markdown,
            value: lines.join("\n\n"),
        }),
        range: None,
    }
}

/// Find a task by ID in a task tree
fn find_task_by_id<'a>(tasks: &'a [Task], id: &str) -> Option<&'a Task> {
    for task in tasks {
        if task.id == id {
            return Some(task);
        }
        if let Some(found) = find_task_by_id(&task.children, id) {
            return Some(found);
        }
    }
    None
}

/// Format a constraint for display
fn format_constraint(constraint: &TaskConstraint) -> String {
    match constraint {
        TaskConstraint::MustStartOn(d) => format!("must_start_on: {}", d),
        TaskConstraint::MustFinishOn(d) => format!("must_finish_on: {}", d),
        TaskConstraint::StartNoEarlierThan(d) => format!("start_no_earlier_than: {}", d),
        TaskConstraint::StartNoLaterThan(d) => format!("start_no_later_than: {}", d),
        TaskConstraint::FinishNoEarlierThan(d) => format!("finish_no_earlier_than: {}", d),
        TaskConstraint::FinishNoLaterThan(d) => format!("finish_no_later_than: {}", d),
    }
}

/// Analyze constraint effects on a scheduled task
fn analyze_constraint_effects(
    task: &Task,
    scheduled: &utf8proj_core::ScheduledTask,
) -> Vec<String> {
    use utf8proj_core::Duration;

    let mut effects = Vec::new();
    let es = scheduled.start;
    let ef = scheduled.finish;
    let ls = scheduled.late_start;
    let lf = scheduled.late_finish;
    let zero_slack = Duration::zero();

    for constraint in &task.constraints {
        let effect = match constraint {
            TaskConstraint::MustStartOn(date) => {
                if es == *date && ls == *date {
                    format!("📌 `must_start_on: {}` — Task pinned to this date", date)
                } else if es == *date {
                    format!("✓ `must_start_on: {}` — Pushed early start", date)
                } else if es > *date {
                    format!(
                        "⚠️ `must_start_on: {}` — Superseded by dependencies (ES={})",
                        date, es
                    )
                } else {
                    format!("✓ `must_start_on: {}` — Capped late start", date)
                }
            }
            TaskConstraint::MustFinishOn(date) => {
                if ef == *date && lf == *date {
                    format!("📌 `must_finish_on: {}` — Task pinned to this date", date)
                } else if ef == *date {
                    format!("✓ `must_finish_on: {}` — Pushed early finish", date)
                } else if ef > *date {
                    format!(
                        "⚠️ `must_finish_on: {}` — Superseded by dependencies (EF={})",
                        date, ef
                    )
                } else {
                    format!("✓ `must_finish_on: {}` — Capped late finish", date)
                }
            }
            TaskConstraint::StartNoEarlierThan(date) => {
                if es == *date {
                    format!(
                        "✓ `start_no_earlier_than: {}` — Task starts on boundary",
                        date
                    )
                } else if es > *date {
                    format!(
                        "○ `start_no_earlier_than: {}` — Redundant (ES={})",
                        date, es
                    )
                } else {
                    format!("✓ `start_no_earlier_than: {}` — Pushed early start", date)
                }
            }
            TaskConstraint::StartNoLaterThan(date) => {
                if ls == *date {
                    if scheduled.slack == zero_slack {
                        format!("🔴 `start_no_later_than: {}` — Made task critical", date)
                    } else {
                        format!("✓ `start_no_later_than: {}` — Capped late start", date)
                    }
                } else if ls < *date {
                    format!("○ `start_no_later_than: {}` — Redundant (LS={})", date, ls)
                } else {
                    format!("✓ `start_no_later_than: {}` — Caps late start", date)
                }
            }
            TaskConstraint::FinishNoEarlierThan(date) => {
                if ef == *date {
                    format!(
                        "✓ `finish_no_earlier_than: {}` — Task finishes on boundary",
                        date
                    )
                } else if ef > *date {
                    format!(
                        "○ `finish_no_earlier_than: {}` — Redundant (EF={})",
                        date, ef
                    )
                } else {
                    format!("✓ `finish_no_earlier_than: {}` — Pushed early finish", date)
                }
            }
            TaskConstraint::FinishNoLaterThan(date) => {
                if lf == *date {
                    if scheduled.slack == zero_slack {
                        format!("🔴 `finish_no_later_than: {}` — Made task critical", date)
                    } else {
                        format!("✓ `finish_no_later_than: {}` — Capped late finish", date)
                    }
                } else if lf < *date {
                    format!("○ `finish_no_later_than: {}` — Redundant (LF={})", date, lf)
                } else {
                    format!("✓ `finish_no_later_than: {}` — Caps late finish", date)
                }
            }
        };
        effects.push(effect);
    }

    effects
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;
    use rust_decimal_macros::dec;
    use std::collections::HashMap;
    use utf8proj_core::{
        Dependency, DependencyType, Duration, Money, RateRange, Resource, ResourceProfile,
        ResourceRate, ResourceRef, ScheduledTask, TaskStatus, Trait,
    };

    /// Helper to create a ScheduledTask for tests
    fn make_scheduled_task(
        task_id: &str,
        start: NaiveDate,
        finish: NaiveDate,
        duration: Duration,
        slack: Duration,
        is_critical: bool,
    ) -> ScheduledTask {
        ScheduledTask {
            task_id: task_id.to_string(),
            start,
            finish,
            duration,
            assignments: vec![],
            slack,
            is_critical,
            early_start: start,
            early_finish: finish,
            late_start: start,
            late_finish: finish,
            forecast_start: start,
            forecast_finish: finish,
            remaining_duration: duration,
            percent_complete: 0,
            status: TaskStatus::NotStarted,
            cost_range: None,
            has_abstract_assignments: false,
            baseline_start: start,
            baseline_finish: finish,
            start_variance_days: 0,
            finish_variance_days: 0,
        }
    }

    // =========================================================================
    // get_word_at_position tests
    // =========================================================================

    #[test]
    fn get_word_at_position_basic() {
        let text = "assign: developer";
        let pos = Position::new(0, 10); // In "developer"

        let word = get_word_at_position(text, pos);
        assert_eq!(word, Some("developer".to_string()));
    }

    #[test]
    fn get_word_at_position_start() {
        let text = "developer_senior";
        let pos = Position::new(0, 0);

        let word = get_word_at_position(text, pos);
        assert_eq!(word, Some("developer_senior".to_string()));
    }

    #[test]
    fn get_word_at_position_with_hyphen() {
        let text = "senior-developer";
        let pos = Position::new(0, 8);

        let word = get_word_at_position(text, pos);
        assert_eq!(word, Some("senior-developer".to_string()));
    }

    #[test]
    fn get_word_at_position_empty() {
        let text = "   ";
        let pos = Position::new(0, 1);

        let word = get_word_at_position(text, pos);
        assert_eq!(word, None);
    }

    #[test]
    fn get_word_at_position_multiline() {
        let text = "line one\nline two\nline three";
        let pos = Position::new(1, 5); // "two"

        let word = get_word_at_position(text, pos);
        assert_eq!(word, Some("two".to_string()));
    }

    #[test]
    fn get_word_at_position_out_of_bounds_line() {
        let text = "single line";
        let pos = Position::new(5, 0); // Line doesn't exist

        let word = get_word_at_position(text, pos);
        assert_eq!(word, None);
    }

    #[test]
    fn get_word_at_position_out_of_bounds_column() {
        let text = "short";
        let pos = Position::new(0, 100); // Column beyond line length

        let word = get_word_at_position(text, pos);
        assert_eq!(word, None);
    }

    #[test]
    fn get_word_at_position_at_space() {
        let text = "hello world";
        let pos = Position::new(0, 5); // At space between words

        // When at a space, the word finder looks back and finds "hello"
        let word = get_word_at_position(text, pos);
        assert_eq!(word, Some("hello".to_string()));
    }

    #[test]
    fn get_word_at_position_multiple_spaces() {
        let text = "hello   world";
        let pos = Position::new(0, 7); // In the middle of spaces

        // Multiple spaces - should return None since no word at position
        let word = get_word_at_position(text, pos);
        assert_eq!(word, None);
    }

    // =========================================================================
    // Helper to create test projects
    // =========================================================================

    fn make_test_project() -> Project {
        let mut project = Project::new("Test Project");

        // Add a trait
        project.traits.push(
            Trait::new("senior")
                .description("Senior level expertise")
                .rate_multiplier(1.5),
        );

        // Add profiles with various configurations
        let mut developer = ResourceProfile::new("developer");
        developer.rate = Some(ResourceRate::Range(RateRange {
            min: dec!(100),
            max: dec!(200),
            currency: None,
        }));
        project.profiles.push(developer);

        let mut senior_dev = ResourceProfile::new("senior_dev");
        senior_dev.specializes = Some("developer".to_string());
        senior_dev.traits = vec!["senior".to_string()];
        senior_dev.skills = vec!["rust".to_string(), "python".to_string()];
        project.profiles.push(senior_dev);

        let mut fixed_rate_dev = ResourceProfile::new("fixed_rate_dev");
        fixed_rate_dev.rate = Some(ResourceRate::Fixed(Money::new(dec!(150), "USD")));
        project.profiles.push(fixed_rate_dev);

        project
            .profiles
            .push(ResourceProfile::new("no_rate_profile"));

        let mut unknown_trait_profile = ResourceProfile::new("unknown_trait_profile");
        unknown_trait_profile.traits = vec!["nonexistent".to_string()];
        project.profiles.push(unknown_trait_profile);

        // Add resources
        let mut alice = Resource::new("alice");
        alice.name = "Alice Smith".to_string();
        alice.rate = Some(Money::new(dec!(120), "USD"));
        alice.capacity = 0.8;
        alice.efficiency = 1.2;
        project.resources.push(alice);

        project.resources.push(Resource::new("bob"));

        // Add tasks
        let mut task1 = Task::new("task1");
        task1.name = "First Task".to_string();
        task1.duration = Some(Duration::days(5));
        task1.effort = Some(Duration::days(10));
        task1.assigned = vec![
            ResourceRef {
                resource_id: "alice".to_string(),
                units: 1.0,
            },
            ResourceRef {
                resource_id: "bob".to_string(),
                units: 0.5,
            },
        ];
        task1.depends = vec![Dependency {
            predecessor: "task0".to_string(),
            dep_type: DependencyType::FinishToStart,
            lag: None,
        }];
        task1.complete = Some(0.5);
        project.tasks.push(task1);

        let mut milestone1 = Task::new("milestone1");
        milestone1.milestone = true;
        project.tasks.push(milestone1);

        let mut container = Task::new("container");
        container.name = "Container Task".to_string();
        let mut child1 = Task::new("child1");
        child1.duration = Some(Duration::days(3));
        container.children = vec![child1, Task::new("child2")];
        project.tasks.push(container);

        project.tasks.push(Task::new("simple"));

        project
    }

    // =========================================================================
    // hover_for_profile tests
    // =========================================================================

    #[test]
    fn hover_profile_with_range_rate() {
        let project = make_test_project();
        let profile = project.get_profile("developer").unwrap();

        let hover = hover_for_profile(profile, &project);
        let content = extract_hover_content(&hover);

        assert!(content.contains("**Profile: developer**"));
        assert!(content.contains("Rate: $100 - $200/day"));
        assert!(content.contains("expected:"));
    }

    #[test]
    fn hover_profile_with_fixed_rate() {
        let project = make_test_project();
        let profile = project.get_profile("fixed_rate_dev").unwrap();

        let hover = hover_for_profile(profile, &project);
        let content = extract_hover_content(&hover);

        assert!(content.contains("**Profile: fixed_rate_dev**"));
        assert!(content.contains("Rate: $150/day"));
    }

    #[test]
    fn hover_profile_with_specialization() {
        let project = make_test_project();
        let profile = project.get_profile("senior_dev").unwrap();

        let hover = hover_for_profile(profile, &project);
        let content = extract_hover_content(&hover);

        assert!(content.contains("**Profile: senior_dev**"));
        assert!(content.contains("Specializes:"));
        assert!(content.contains("senior_dev"));
        assert!(content.contains("developer"));
    }

    #[test]
    fn hover_profile_with_traits() {
        let project = make_test_project();
        let profile = project.get_profile("senior_dev").unwrap();

        let hover = hover_for_profile(profile, &project);
        let content = extract_hover_content(&hover);

        assert!(content.contains("Traits: senior (1.5x)"));
    }

    #[test]
    fn hover_profile_with_unknown_trait() {
        let project = make_test_project();
        let profile = project.get_profile("unknown_trait_profile").unwrap();

        let hover = hover_for_profile(profile, &project);
        let content = extract_hover_content(&hover);

        assert!(content.contains("nonexistent (unknown)"));
    }

    #[test]
    fn hover_profile_with_skills() {
        let project = make_test_project();
        let profile = project.get_profile("senior_dev").unwrap();

        let hover = hover_for_profile(profile, &project);
        let content = extract_hover_content(&hover);

        assert!(content.contains("Skills: rust, python"));
    }

    #[test]
    fn hover_profile_no_rate() {
        let project = make_test_project();
        let profile = project.get_profile("no_rate_profile").unwrap();

        let hover = hover_for_profile(profile, &project);
        let content = extract_hover_content(&hover);

        assert!(content.contains("Rate: *not defined*"));
    }

    #[test]
    fn hover_profile_inherited_rate() {
        let project = make_test_project();
        let profile = project.get_profile("senior_dev").unwrap();

        let hover = hover_for_profile(profile, &project);
        let content = extract_hover_content(&hover);

        // senior_dev inherits from developer which has a range rate
        assert!(content.contains("Rate (inherited):"));
        assert!(content.contains("from developer"));
    }

    #[test]
    fn hover_profile_specializes_no_inherited_rate() {
        let mut project = Project::new("Test");

        // Parent with no rate
        project.profiles.push(ResourceProfile::new("base_profile"));

        // Child specializing from parent with no rate
        let mut child = ResourceProfile::new("child_profile");
        child.specializes = Some("base_profile".to_string());
        project.profiles.push(child);

        let profile = project.get_profile("child_profile").unwrap();
        let hover = hover_for_profile(profile, &project);
        let content = extract_hover_content(&hover);

        // Should show "Rate: *not defined*" since parent has no rate
        assert!(content.contains("Rate: *not defined*"));
    }

    // =========================================================================
    // hover_for_resource tests
    // =========================================================================

    #[test]
    fn hover_resource_full() {
        let project = make_test_project();
        let resource = project.get_resource("alice").unwrap();

        let hover = hover_for_resource(resource);
        let content = extract_hover_content(&hover);

        assert!(content.contains("**Resource: alice**"));
        assert!(content.contains("Name: Alice Smith"));
        assert!(content.contains("Rate: $120/day"));
        assert!(content.contains("Capacity: 80%"));
        assert!(content.contains("Efficiency: 120%"));
    }

    #[test]
    fn hover_resource_minimal() {
        let project = make_test_project();
        let resource = project.get_resource("bob").unwrap();

        let hover = hover_for_resource(resource);
        let content = extract_hover_content(&hover);

        assert!(content.contains("**Resource: bob**"));
        // Name same as ID - should not show Name line
        assert!(!content.contains("Name:"));
        // Default capacity/efficiency - should not show
        assert!(!content.contains("Capacity:"));
        assert!(!content.contains("Efficiency:"));
    }

    // =========================================================================
    // hover_for_task tests
    // =========================================================================

    #[test]
    fn hover_task_full() {
        let project = make_test_project();
        let task = find_task_by_id(&project.tasks, "task1").unwrap();

        let hover = hover_for_task(task, "task1", &project, None, &[]);
        let content = extract_hover_content(&hover);

        assert!(content.contains("**Task: task1**"));
        assert!(content.contains("Name: First Task"));
        assert!(content.contains("Duration: 5 days"));
        assert!(content.contains("Effort: 10 days"));
        assert!(content.contains("Assigned: alice, bob@50%"));
        assert!(content.contains("Depends on: task0"));
        assert!(content.contains("Progress: 50%"));
    }

    #[test]
    fn hover_task_milestone() {
        let project = make_test_project();
        let task = find_task_by_id(&project.tasks, "milestone1").unwrap();

        let hover = hover_for_task(task, "milestone1", &project, None, &[]);
        let content = extract_hover_content(&hover);

        assert!(content.contains("**Task: milestone1**"));
        assert!(content.contains("Type: Milestone"));
    }

    #[test]
    fn hover_task_container() {
        let project = make_test_project();
        let task = find_task_by_id(&project.tasks, "container").unwrap();

        let hover = hover_for_task(task, "container", &project, None, &[]);
        let content = extract_hover_content(&hover);

        assert!(content.contains("**Task: container**"));
        assert!(content.contains("Name: Container Task"));
        assert!(content.contains("Type: Container (2 children)"));
    }

    #[test]
    fn hover_task_minimal() {
        let project = make_test_project();
        let task = find_task_by_id(&project.tasks, "simple").unwrap();

        let hover = hover_for_task(task, "simple", &project, None, &[]);
        let content = extract_hover_content(&hover);

        assert!(content.contains("**Task: simple**"));
        // Should not contain optional fields
        assert!(!content.contains("Name:"));
        assert!(!content.contains("Duration:"));
        assert!(!content.contains("Effort:"));
        assert!(!content.contains("Assigned:"));
        assert!(!content.contains("Depends on:"));
        assert!(!content.contains("Progress:"));
    }

    #[test]
    fn hover_task_zero_progress_not_shown() {
        let mut project = make_test_project();
        let mut zero_task = Task::new("zero_progress");
        zero_task.complete = Some(0.0);
        project.tasks.push(zero_task);

        let task = find_task_by_id(&project.tasks, "zero_progress").unwrap();
        let hover = hover_for_task(task, "zero_progress", &project, None, &[]);
        let content = extract_hover_content(&hover);

        // 0% progress should not be shown
        assert!(!content.contains("Progress:"));
    }

    #[test]
    fn hover_task_with_schedule_critical_path() {
        let project = make_test_project();
        let task = find_task_by_id(&project.tasks, "task1").unwrap();

        // Create a schedule with this task on the critical path
        let start = NaiveDate::from_ymd_opt(2025, 1, 6).unwrap();
        let finish = NaiveDate::from_ymd_opt(2025, 1, 10).unwrap();
        let scheduled = make_scheduled_task(
            "task1",
            start,
            finish,
            Duration::days(5),
            Duration::days(0), // no slack
            true,              // critical
        );

        let mut tasks = HashMap::new();
        tasks.insert("task1".to_string(), scheduled);

        let schedule = Schedule {
            tasks,
            critical_path: vec!["task1".to_string()],
            project_duration: Duration::days(5),
            project_end: finish,
            total_cost: None,
            total_cost_range: None,
            project_progress: 0,
            project_baseline_finish: finish,
            project_forecast_finish: finish,
            project_variance_days: 0,
            planned_value: 0,
            earned_value: 0,
            spi: 1.0,
        };

        let hover = hover_for_task(task, "task1", &project, Some(&schedule), &[]);
        let content = extract_hover_content(&hover);

        // Should show schedule info
        assert!(content.contains("📅 **Schedule:** 2025-01-06 → 2025-01-10"));
        assert!(content.contains("⏱️ Duration: 5 days"));
        assert!(content.contains("🔴 **Critical Path**"));
    }

    #[test]
    fn hover_task_with_schedule_slack() {
        let project = make_test_project();
        let task = find_task_by_id(&project.tasks, "task1").unwrap();

        // Create a schedule with this task having slack
        let start = NaiveDate::from_ymd_opt(2025, 1, 6).unwrap();
        let finish = NaiveDate::from_ymd_opt(2025, 1, 10).unwrap();
        let scheduled = make_scheduled_task(
            "task1",
            start,
            finish,
            Duration::days(5),
            Duration::days(3), // has slack
            false,             // not critical
        );

        let mut tasks = HashMap::new();
        tasks.insert("task1".to_string(), scheduled);

        let project_end = NaiveDate::from_ymd_opt(2025, 1, 13).unwrap();
        let schedule = Schedule {
            tasks,
            critical_path: vec![],
            project_duration: Duration::days(8),
            project_end,
            total_cost: None,
            total_cost_range: None,
            project_progress: 0,
            project_baseline_finish: project_end,
            project_forecast_finish: project_end,
            project_variance_days: 0,
            planned_value: 0,
            earned_value: 0,
            spi: 1.0,
        };

        let hover = hover_for_task(task, "task1", &project, Some(&schedule), &[]);
        let content = extract_hover_content(&hover);

        // Should show slack info
        assert!(content.contains("📅 **Schedule:**"));
        assert!(content.contains("🟢 Slack: 3 days"));
        assert!(!content.contains("Critical Path"));
    }

    #[test]
    fn hover_task_with_constraints_shows_list() {
        let project = make_test_project();
        let mut task = Task::new("constrained_task");
        task.constraints.push(TaskConstraint::StartNoEarlierThan(
            NaiveDate::from_ymd_opt(2025, 1, 13).unwrap(),
        ));
        task.constraints.push(TaskConstraint::FinishNoLaterThan(
            NaiveDate::from_ymd_opt(2025, 1, 20).unwrap(),
        ));

        let hover = hover_for_task(&task, "constrained_task", &project, None, &[]);
        let content = extract_hover_content(&hover);

        assert!(content.contains("Constraints:"));
        assert!(content.contains("start_no_earlier_than: 2025-01-13"));
        assert!(content.contains("finish_no_later_than: 2025-01-20"));
    }

    #[test]
    fn hover_task_with_constraint_effects_pinned() {
        let project = make_test_project();
        let mut task = Task::new("pinned_task");
        task.duration = Some(Duration::days(3));
        task.constraints.push(TaskConstraint::MustStartOn(
            NaiveDate::from_ymd_opt(2025, 1, 6).unwrap(),
        ));

        let start = NaiveDate::from_ymd_opt(2025, 1, 6).unwrap();
        let finish = NaiveDate::from_ymd_opt(2025, 1, 8).unwrap();
        let scheduled = make_scheduled_task(
            "pinned_task",
            start,
            finish,
            Duration::days(3),
            Duration::days(0),
            true,
        );

        let mut tasks = HashMap::new();
        tasks.insert("pinned_task".to_string(), scheduled);

        let schedule = Schedule {
            tasks,
            critical_path: vec!["pinned_task".to_string()],
            project_duration: Duration::days(3),
            project_end: finish,
            total_cost: None,
            total_cost_range: None,
            project_progress: 0,
            project_baseline_finish: finish,
            project_forecast_finish: finish,
            project_variance_days: 0,
            planned_value: 0,
            earned_value: 0,
            spi: 1.0,
        };

        let hover = hover_for_task(&task, "pinned_task", &project, Some(&schedule), &[]);
        let content = extract_hover_content(&hover);

        assert!(content.contains("Constraint Effects"));
        assert!(content.contains("📌"));
        assert!(content.contains("pinned"));
    }

    #[test]
    fn hover_task_with_constraint_effects_redundant() {
        let project = make_test_project();
        let mut task = Task::new("redundant_task");
        task.duration = Some(Duration::days(5));
        // Constraint is earlier than actual ES - should be redundant
        task.constraints.push(TaskConstraint::StartNoEarlierThan(
            NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
        ));

        let start = NaiveDate::from_ymd_opt(2025, 1, 10).unwrap();
        let finish = NaiveDate::from_ymd_opt(2025, 1, 14).unwrap();
        let scheduled = make_scheduled_task(
            "redundant_task",
            start,
            finish,
            Duration::days(5),
            Duration::days(2),
            false,
        );

        let mut tasks = HashMap::new();
        tasks.insert("redundant_task".to_string(), scheduled);

        let project_end = NaiveDate::from_ymd_opt(2025, 1, 16).unwrap();
        let schedule = Schedule {
            tasks,
            critical_path: vec![],
            project_duration: Duration::days(7),
            project_end,
            total_cost: None,
            total_cost_range: None,
            project_progress: 0,
            project_baseline_finish: project_end,
            project_forecast_finish: project_end,
            project_variance_days: 0,
            planned_value: 0,
            earned_value: 0,
            spi: 1.0,
        };

        let hover = hover_for_task(&task, "redundant_task", &project, Some(&schedule), &[]);
        let content = extract_hover_content(&hover);

        assert!(content.contains("Constraint Effects"));
        assert!(content.contains("○")); // Redundant marker
        assert!(content.contains("Redundant"));
    }

    // =========================================================================
    // hover_for_trait tests
    // =========================================================================

    #[test]
    fn hover_trait_with_description() {
        let project = make_test_project();
        let t = project.get_trait("senior").unwrap();

        let hover = hover_for_trait(t);
        let content = extract_hover_content(&hover);

        assert!(content.contains("**Trait: senior**"));
        assert!(content.contains("Senior level expertise"));
        assert!(content.contains("Rate multiplier: 1.5x"));
    }

    #[test]
    fn hover_trait_without_description() {
        let t = Trait::new("simple_trait").rate_multiplier(2.0);

        let hover = hover_for_trait(&t);
        let content = extract_hover_content(&hover);

        assert!(content.contains("**Trait: simple_trait**"));
        assert!(content.contains("Rate multiplier: 2x"));
        // Should not have description
        assert_eq!(content.matches("\n\n").count(), 1); // Only one separator
    }

    // =========================================================================
    // find_task_by_id tests
    // =========================================================================

    #[test]
    fn find_task_top_level() {
        let project = make_test_project();
        let task = find_task_by_id(&project.tasks, "task1");

        assert!(task.is_some());
        assert_eq!(task.unwrap().id, "task1");
    }

    #[test]
    fn find_task_nested() {
        let project = make_test_project();
        let task = find_task_by_id(&project.tasks, "child1");

        assert!(task.is_some());
        assert_eq!(task.unwrap().id, "child1");
    }

    #[test]
    fn find_task_not_found() {
        let project = make_test_project();
        let task = find_task_by_id(&project.tasks, "nonexistent");

        assert!(task.is_none());
    }

    // =========================================================================
    // get_inherited_rate tests
    // =========================================================================

    #[test]
    fn inherited_rate_from_parent() {
        let project = make_test_project();
        let rate = get_inherited_rate("developer", &project);

        assert!(rate.is_some());
        assert!(rate.unwrap().contains("$100 - $200/day from developer"));
    }

    #[test]
    fn inherited_rate_not_found() {
        let project = make_test_project();
        let rate = get_inherited_rate("no_rate_profile", &project);

        assert!(rate.is_none());
    }

    #[test]
    fn inherited_rate_fixed() {
        let project = make_test_project();
        let rate = get_inherited_rate("fixed_rate_dev", &project);

        assert!(rate.is_some());
        assert!(rate.unwrap().contains("$150/day from fixed_rate_dev"));
    }

    // =========================================================================
    // get_hover_info integration tests
    // =========================================================================

    #[test]
    fn hover_info_for_profile() {
        let project = make_test_project();
        let text = "assign: developer";

        let hover = get_hover_info(&project, None, &[], text, Position::new(0, 10));

        assert!(hover.is_some());
        let content = extract_hover_content(&hover.unwrap());
        assert!(content.contains("**Profile: developer**"));
    }

    #[test]
    fn hover_info_for_resource() {
        let project = make_test_project();
        let text = "resource: alice";

        let hover = get_hover_info(&project, None, &[], text, Position::new(0, 12));

        assert!(hover.is_some());
        let content = extract_hover_content(&hover.unwrap());
        assert!(content.contains("**Resource: alice**"));
    }

    #[test]
    fn hover_info_for_task() {
        let project = make_test_project();
        let text = "depends: task1";

        let hover = get_hover_info(&project, None, &[], text, Position::new(0, 10));

        assert!(hover.is_some());
        let content = extract_hover_content(&hover.unwrap());
        assert!(content.contains("**Task: task1**"));
    }

    #[test]
    fn hover_info_for_trait() {
        let project = make_test_project();
        let text = "traits: senior";

        let hover = get_hover_info(&project, None, &[], text, Position::new(0, 10));

        assert!(hover.is_some());
        let content = extract_hover_content(&hover.unwrap());
        assert!(content.contains("**Trait: senior**"));
    }

    #[test]
    fn hover_info_unknown_word() {
        let project = make_test_project();
        let text = "unknown_identifier";

        let hover = get_hover_info(&project, None, &[], text, Position::new(0, 5));

        assert!(hover.is_none());
    }

    #[test]
    fn hover_info_empty_position() {
        let project = make_test_project();
        let text = "   ";

        let hover = get_hover_info(&project, None, &[], text, Position::new(0, 1));

        assert!(hover.is_none());
    }

    // =========================================================================
    // Leveling diagnostic tests (L001-L004)
    // =========================================================================

    #[test]
    fn filter_diagnostics_l001_links_to_task() {
        use utf8proj_core::Severity;

        let diagnostics = vec![Diagnostic {
            code: DiagnosticCode::L001OverallocationResolved,
            severity: Severity::Info,
            message: "Task 'task1' shifted to resolve overallocation".into(),
            file: None,
            span: None,
            secondary_spans: vec![],
            notes: vec![],
            hints: vec![],
        }];

        let codes = filter_task_diagnostics("task1", &diagnostics);
        assert!(codes.contains(&DiagnosticCode::L001OverallocationResolved));

        let codes = filter_task_diagnostics("other_task", &diagnostics);
        assert!(!codes.contains(&DiagnosticCode::L001OverallocationResolved));
    }

    #[test]
    fn filter_diagnostics_l003_links_to_task() {
        use utf8proj_core::Severity;

        let diagnostics = vec![Diagnostic {
            code: DiagnosticCode::L003DurationIncreased,
            severity: Severity::Info,
            message: "Project duration increased due to leveling of 'task1'".into(),
            file: None,
            span: None,
            secondary_spans: vec![],
            notes: vec![],
            hints: vec![],
        }];

        let codes = filter_task_diagnostics("task1", &diagnostics);
        assert!(codes.contains(&DiagnosticCode::L003DurationIncreased));
    }

    #[test]
    fn filter_diagnostics_l004_links_to_milestone() {
        use utf8proj_core::Severity;

        let diagnostics = vec![Diagnostic {
            code: DiagnosticCode::L004MilestoneDelayed,
            severity: Severity::Warning,
            message: "Milestone 'launch' delayed by 3 days due to leveling".into(),
            file: None,
            span: None,
            secondary_spans: vec![],
            notes: vec![],
            hints: vec![],
        }];

        let codes = filter_task_diagnostics("launch", &diagnostics);
        assert!(codes.contains(&DiagnosticCode::L004MilestoneDelayed));
    }

    #[test]
    fn leveling_diagnostic_gets_scale_icon() {
        // Test that L codes get the ⚖️ icon in hover
        let code = DiagnosticCode::L001OverallocationResolved;
        let severity_icon = match code.as_str().chars().next() {
            Some('E') => "🔴",
            Some('W') => "🟡",
            Some('H') => "💡",
            Some('L') => "⚖️",
            Some('C') => "📆",
            _ => "ℹ️",
        };
        assert_eq!(severity_icon, "⚖️");
    }

    #[test]
    fn calendar_diagnostic_gets_calendar_icon() {
        let code = DiagnosticCode::C010NonWorkingDay;
        let severity_icon = match code.as_str().chars().next() {
            Some('E') => "🔴",
            Some('W') => "🟡",
            Some('H') => "💡",
            Some('L') => "⚖️",
            Some('C') => "📆",
            _ => "ℹ️",
        };
        assert_eq!(severity_icon, "📆");
    }

    // =========================================================================
    // Helper functions
    // =========================================================================

    fn extract_hover_content(hover: &Hover) -> String {
        match &hover.contents {
            HoverContents::Markup(markup) => markup.value.clone(),
            _ => String::new(),
        }
    }
}
