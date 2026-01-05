//! Hover information provider for utf8proj LSP
//!
//! Provides contextual information when hovering over:
//! - Profile identifiers: shows rate range, specialization chain, traits
//! - Resource identifiers: shows rate, capacity, efficiency
//! - Task identifiers: shows duration, dependencies, assignments

use tower_lsp::lsp_types::{Hover, HoverContents, MarkupContent, MarkupKind, Position};

use utf8proj_core::{Project, ResourceProfile, ResourceRate, Task};

/// Get hover information for a position in the document
pub fn get_hover_info(project: &Project, text: &str, position: Position) -> Option<Hover> {
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
        return Some(hover_for_task(task));
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
                    range.min, range.max, range.expected()
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
        lines.push(format!("Efficiency: {}%", (resource.efficiency * 100.0) as i32));
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
fn hover_for_task(task: &Task) -> Hover {
    let mut lines = vec![format!("**Task: {}**", task.id)];

    if task.name != task.id {
        lines.push(format!("Name: {}", task.name));
    }

    if task.milestone {
        lines.push("Type: Milestone".to_string());
    } else if !task.children.is_empty() {
        lines.push(format!("Type: Container ({} children)", task.children.len()));
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
        let deps: Vec<&str> = task.depends.iter().map(|d| d.predecessor.as_str()).collect();
        lines.push(format!("Depends on: {}", deps.join(", ")));
    }

    if let Some(complete) = task.complete {
        if complete > 0.0 {
            lines.push(format!("Progress: {}%", (complete * 100.0) as i32));
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

#[cfg(test)]
mod tests {
    use super::*;

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
}
