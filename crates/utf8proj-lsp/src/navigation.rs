//! Navigation support for utf8proj LSP
//!
//! Provides go-to-definition and find-references for:
//! - Task IDs
//! - Resources
//! - Calendars
//! - Profiles
//! - Traits
//!
//! Pure graph navigation over the parsed model - no inference, no autocomplete.

use tower_lsp::lsp_types::{Location, Position, Range, Url};
use utf8proj_core::Project;

/// Symbol kind for navigation purposes
#[derive(Debug, Clone, PartialEq)]
pub enum SymbolKind {
    Task,
    Resource,
    Calendar,
    Profile,
    Trait,
}

/// A symbol definition with its location
#[derive(Debug, Clone)]
pub struct SymbolDefinition {
    pub id: String,
    #[allow(dead_code)] // May be used for richer hover info in future
    pub kind: SymbolKind,
    pub line: u32,
    pub column: u32,
}

/// Find the definition of a symbol at the given position
pub fn find_definition(
    project: &Project,
    text: &str,
    uri: &Url,
    position: Position,
) -> Option<Location> {
    let word = get_word_at_position(text, position)?;

    // Check what kind of symbol this is and find its definition
    if let Some(def) = find_symbol_definition(project, text, &word) {
        return Some(Location {
            uri: uri.clone(),
            range: Range::new(
                Position::new(def.line, def.column),
                Position::new(def.line, def.column + def.id.len() as u32),
            ),
        });
    }

    None
}

/// Find all references to the symbol at the given position
pub fn find_references(
    project: &Project,
    text: &str,
    uri: &Url,
    position: Position,
    include_definition: bool,
) -> Vec<Location> {
    let word = match get_word_at_position(text, position) {
        Some(w) => w,
        None => return vec![],
    };

    // Determine the symbol kind
    let kind = determine_symbol_kind(project, &word);
    if kind.is_none() {
        return vec![];
    }
    let kind = kind.unwrap();

    let mut locations = Vec::new();

    // Find all occurrences based on symbol kind
    let occurrences = find_all_occurrences(text, &word, &kind);

    for (line, col, is_definition) in occurrences {
        if !is_definition || include_definition {
            locations.push(Location {
                uri: uri.clone(),
                range: Range::new(
                    Position::new(line, col),
                    Position::new(line, col + word.len() as u32),
                ),
            });
        }
    }

    locations
}

/// Find the definition of a symbol in the project
fn find_symbol_definition(project: &Project, text: &str, word: &str) -> Option<SymbolDefinition> {
    // Check tasks (including nested)
    if find_task_by_id(&project.tasks, word).is_some() {
        if let Some((line, col)) = find_task_definition(text, word) {
            return Some(SymbolDefinition {
                id: word.to_string(),
                kind: SymbolKind::Task,
                line,
                column: col,
            });
        }
    }

    // Check resources
    if project.resources.iter().any(|r| r.id == word) {
        if let Some((line, col)) = find_resource_definition(text, word) {
            return Some(SymbolDefinition {
                id: word.to_string(),
                kind: SymbolKind::Resource,
                line,
                column: col,
            });
        }
    }

    // Check calendars
    if project.calendars.iter().any(|c| c.id == word) {
        if let Some((line, col)) = find_calendar_definition(text, word) {
            return Some(SymbolDefinition {
                id: word.to_string(),
                kind: SymbolKind::Calendar,
                line,
                column: col,
            });
        }
    }

    // Check profiles
    if project.profiles.iter().any(|p| p.id == word) {
        if let Some((line, col)) = find_profile_definition(text, word) {
            return Some(SymbolDefinition {
                id: word.to_string(),
                kind: SymbolKind::Profile,
                line,
                column: col,
            });
        }
    }

    // Check traits
    if project.traits.iter().any(|t| t.id == word) {
        if let Some((line, col)) = find_trait_definition(text, word) {
            return Some(SymbolDefinition {
                id: word.to_string(),
                kind: SymbolKind::Trait,
                line,
                column: col,
            });
        }
    }

    None
}

/// Determine what kind of symbol this word represents
fn determine_symbol_kind(project: &Project, word: &str) -> Option<SymbolKind> {
    if find_task_by_id(&project.tasks, word).is_some() {
        return Some(SymbolKind::Task);
    }
    if project.resources.iter().any(|r| r.id == word) {
        return Some(SymbolKind::Resource);
    }
    if project.calendars.iter().any(|c| c.id == word) {
        return Some(SymbolKind::Calendar);
    }
    if project.profiles.iter().any(|p| p.id == word) {
        return Some(SymbolKind::Profile);
    }
    if project.traits.iter().any(|t| t.id == word) {
        return Some(SymbolKind::Trait);
    }
    None
}

/// Find all occurrences of a symbol in the text
/// Returns (line, column, is_definition) tuples
fn find_all_occurrences(text: &str, word: &str, kind: &SymbolKind) -> Vec<(u32, u32, bool)> {
    let mut occurrences = Vec::new();

    for (line_num, line) in text.lines().enumerate() {
        let mut search_start = 0;
        while let Some(col) = find_word_in_line(line, word, search_start) {
            let is_def = is_definition_site(line, col, word, kind);
            occurrences.push((line_num as u32, col as u32, is_def));
            search_start = col + word.len();
        }
    }

    occurrences
}

/// Find a word in a line, ensuring it's a complete identifier
fn find_word_in_line(line: &str, word: &str, start: usize) -> Option<usize> {
    let search_area = &line[start..];
    let mut pos = 0;

    while let Some(idx) = search_area[pos..].find(word) {
        let abs_idx = start + pos + idx;

        // Check if it's a complete word (not part of a larger identifier)
        let before_ok = abs_idx == 0 || !is_identifier_char(line.chars().nth(abs_idx - 1)?);
        let after_ok = abs_idx + word.len() >= line.len()
            || !is_identifier_char(line.chars().nth(abs_idx + word.len())?);

        if before_ok && after_ok {
            return Some(abs_idx);
        }

        pos += idx + 1;
    }

    None
}

/// Check if this occurrence is a definition site
fn is_definition_site(line: &str, col: usize, _word: &str, kind: &SymbolKind) -> bool {
    let prefix = line[..col].trim_end();

    match kind {
        SymbolKind::Task => {
            // task <id> "name" { or milestone <id> "name" {
            prefix.ends_with("task") || prefix.ends_with("milestone")
        }
        SymbolKind::Resource => {
            // resource <id> "name" {
            prefix.ends_with("resource") && !prefix.ends_with("resource_profile")
        }
        SymbolKind::Calendar => {
            // calendar <id> { or calendar "name" { where id matches
            prefix.ends_with("calendar")
        }
        SymbolKind::Profile => {
            // resource_profile <id> "name" {
            prefix.ends_with("resource_profile")
        }
        SymbolKind::Trait => {
            // trait <id> "name" {
            prefix.ends_with("trait")
        }
    }
}

/// Find task definition line: "task <id>" pattern
fn find_task_definition(text: &str, task_id: &str) -> Option<(u32, u32)> {
    for (line_num, line) in text.lines().enumerate() {
        // Match "task <id>" or "milestone <id>"
        if let Some(col) = find_definition_pattern(line, "task", task_id) {
            return Some((line_num as u32, col as u32));
        }
        if let Some(col) = find_definition_pattern(line, "milestone", task_id) {
            return Some((line_num as u32, col as u32));
        }
    }
    None
}

/// Find resource definition line
fn find_resource_definition(text: &str, resource_id: &str) -> Option<(u32, u32)> {
    for (line_num, line) in text.lines().enumerate() {
        // Match "resource <id>" but not "resource_profile <id>"
        if let Some(col) = find_definition_pattern(line, "resource", resource_id) {
            // Make sure it's not resource_profile
            let trimmed = line.trim_start();
            if trimmed.starts_with("resource ") && !trimmed.starts_with("resource_profile") {
                return Some((line_num as u32, col as u32));
            }
        }
    }
    None
}

/// Find calendar definition line
fn find_calendar_definition(text: &str, calendar_id: &str) -> Option<(u32, u32)> {
    for (line_num, line) in text.lines().enumerate() {
        if let Some(col) = find_definition_pattern(line, "calendar", calendar_id) {
            return Some((line_num as u32, col as u32));
        }
    }
    None
}

/// Find profile definition line
fn find_profile_definition(text: &str, profile_id: &str) -> Option<(u32, u32)> {
    for (line_num, line) in text.lines().enumerate() {
        if let Some(col) = find_definition_pattern(line, "resource_profile", profile_id) {
            return Some((line_num as u32, col as u32));
        }
    }
    None
}

/// Find trait definition line
fn find_trait_definition(text: &str, trait_id: &str) -> Option<(u32, u32)> {
    for (line_num, line) in text.lines().enumerate() {
        if let Some(col) = find_definition_pattern(line, "trait", trait_id) {
            return Some((line_num as u32, col as u32));
        }
    }
    None
}

/// Find "<keyword> <id>" pattern in a line, return column of id
fn find_definition_pattern(line: &str, keyword: &str, id: &str) -> Option<usize> {
    let trimmed = line.trim_start();
    let indent = line.len() - trimmed.len();

    // Check if line starts with keyword
    if !trimmed.starts_with(keyword) {
        return None;
    }

    // Find the id after the keyword
    let after_keyword = &trimmed[keyword.len()..];
    if !after_keyword.starts_with(char::is_whitespace) {
        return None;
    }

    let after_keyword = after_keyword.trim_start();

    // Check if id matches
    if after_keyword.starts_with(id) {
        let char_after_id = after_keyword.chars().nth(id.len());
        // Ensure it's a complete match (followed by whitespace, quote, or brace)
        if char_after_id.is_none()
            || char_after_id == Some(' ')
            || char_after_id == Some('\t')
            || char_after_id == Some('"')
            || char_after_id == Some('{')
        {
            // Calculate column: indent + keyword + spaces + position
            let keyword_end = indent + keyword.len();
            let spaces = line[keyword_end..].len() - line[keyword_end..].trim_start().len();
            return Some(keyword_end + spaces);
        }
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

    let chars: Vec<char> = line.chars().collect();

    // Find start of word
    let mut start = col;
    while start > 0 && is_identifier_char(chars[start - 1]) {
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

/// Check if a character is valid in an identifier
fn is_identifier_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || c == '-'
}

/// Find a task by ID in a task tree
fn find_task_by_id<'a>(
    tasks: &'a [utf8proj_core::Task],
    id: &str,
) -> Option<&'a utf8proj_core::Task> {
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
        let text = "depends: task_a";
        let word = get_word_at_position(text, Position::new(0, 10));
        assert_eq!(word, Some("task_a".to_string()));
    }

    #[test]
    fn get_word_at_position_with_hyphen() {
        let text = "assign: my-resource";
        let word = get_word_at_position(text, Position::new(0, 12));
        assert_eq!(word, Some("my-resource".to_string()));
    }

    #[test]
    fn find_word_in_line_basic() {
        let line = "depends: task_a, task_b";
        assert_eq!(find_word_in_line(line, "task_a", 0), Some(9));
        assert_eq!(find_word_in_line(line, "task_b", 0), Some(17));
    }

    #[test]
    fn find_word_in_line_not_partial() {
        let line = "depends: task_abc";
        // Should not match "task_a" as partial
        assert_eq!(find_word_in_line(line, "task_a", 0), None);
    }

    #[test]
    fn find_definition_pattern_task() {
        let line = "    task design \"Design Phase\" {";
        let col = find_definition_pattern(line, "task", "design");
        assert_eq!(col, Some(9)); // "    task " = 9 chars
    }

    #[test]
    fn find_definition_pattern_resource() {
        let line = "resource dev \"Developer\" {";
        let col = find_definition_pattern(line, "resource", "dev");
        assert_eq!(col, Some(9)); // "resource " = 9 chars
    }

    #[test]
    fn is_definition_site_task() {
        let line = "task design \"Design\" {";
        assert!(is_definition_site(line, 5, "design", &SymbolKind::Task));
    }

    #[test]
    fn is_definition_site_not_reference() {
        let line = "    depends: design";
        assert!(!is_definition_site(line, 13, "design", &SymbolKind::Task));
    }
}
