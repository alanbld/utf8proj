//! Baseline file parser and serializer (RFC-0013)
//!
//! This module provides parsing and serialization for `.baselines` sidecar files
//! that store frozen schedule snapshots for variance analysis.
//!
//! # File Format
//!
//! ```text
//! # project.proj.baselines
//! baseline original {
//!     saved: 2026-01-15T10:30:00Z
//!     description: "Initial approved plan"
//!
//!     design: 2026-01-01 -> 2026-01-10
//!     build: 2026-01-11 -> 2026-02-15
//! }
//! ```
//!
//! # Example Usage
//!
//! ```rust
//! use utf8proj_parser::baseline::{parse_baselines, serialize_baselines};
//! use utf8proj_core::baseline::{Baseline, BaselineStore, TaskSnapshot};
//!
//! // Parse a baselines file
//! let content = r#"
//! baseline original {
//!     saved: 2026-01-15T10:30:00Z
//!     design: 2026-01-01 -> 2026-01-10
//! }
//! "#;
//!
//! let store = parse_baselines(content).unwrap();
//! assert!(store.get("original").is_some());
//!
//! // Serialize back to string
//! let output = serialize_baselines(&store);
//! assert!(output.contains("baseline original"));
//! ```

use chrono::{DateTime, NaiveDate, Utc};
use pest::iterators::Pair;
use pest::Parser;
use pest_derive::Parser;

use utf8proj_core::baseline::{Baseline, BaselineStore, TaskSnapshot};

use crate::ParseError;

#[derive(Parser)]
#[grammar = "native/baseline_grammar.pest"]
struct BaselineParser;

// ============================================================================
// Parsing
// ============================================================================

/// Parse a baselines file content into a `BaselineStore`
pub fn parse_baselines(input: &str) -> Result<BaselineStore, ParseError> {
    let mut pairs = BaselineParser::parse(Rule::baseline_file, input).map_err(|e| {
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

    let mut store = BaselineStore::new();

    let file_pair = pairs.next().unwrap();
    for pair in file_pair.into_inner() {
        match pair.as_rule() {
            Rule::baseline_block => {
                let baseline = parse_baseline_block(pair)?;
                // Note: We use baselines map directly since add() checks for duplicates
                // and parsing a file should not fail on duplicates (file should be valid)
                store.baselines.insert(baseline.name.clone(), baseline);
            }
            Rule::EOI => {}
            _ => {}
        }
    }

    Ok(store)
}

fn parse_baseline_block(pair: Pair<Rule>) -> Result<Baseline, ParseError> {
    let mut inner = pair.into_inner();

    // First comes the identifier (baseline name)
    let name = parse_identifier(inner.next().unwrap());
    let mut baseline = Baseline::new(&name);

    // Then the meta attributes
    let meta_pair = inner.next().unwrap();
    parse_baseline_meta(meta_pair, &mut baseline)?;

    // Then the task snapshots
    for snap_pair in inner {
        if snap_pair.as_rule() == Rule::task_snapshot {
            let snapshot = parse_task_snapshot(snap_pair)?;
            baseline.add_task(snapshot);
        }
    }

    Ok(baseline)
}

fn parse_baseline_meta(pair: Pair<Rule>, baseline: &mut Baseline) -> Result<(), ParseError> {
    for attr in pair.into_inner() {
        match attr.as_rule() {
            Rule::saved_attr => {
                let datetime_pair = attr.into_inner().next().unwrap();
                baseline.saved = parse_iso8601_datetime(datetime_pair)?;
            }
            Rule::description_attr => {
                let string_pair = attr.into_inner().next().unwrap();
                baseline.description = Some(parse_string(string_pair));
            }
            Rule::parent_attr => {
                let id_pair = attr.into_inner().next().unwrap();
                baseline.parent = Some(parse_identifier(id_pair));
            }
            _ => {}
        }
    }
    Ok(())
}

fn parse_task_snapshot(pair: Pair<Rule>) -> Result<TaskSnapshot, ParseError> {
    let mut inner = pair.into_inner();

    let task_id = parse_qualified_id(inner.next().unwrap());
    let start = parse_date(inner.next().unwrap())?;
    let finish = parse_date(inner.next().unwrap())?;

    Ok(TaskSnapshot::new(task_id, start, finish))
}

fn parse_identifier(pair: Pair<Rule>) -> String {
    pair.as_str().to_string()
}

fn parse_qualified_id(pair: Pair<Rule>) -> String {
    pair.as_str().to_string()
}

fn parse_string(pair: Pair<Rule>) -> String {
    let s = pair.as_str();
    // Remove surrounding quotes and unescape
    let inner = &s[1..s.len() - 1];
    let mut result = String::with_capacity(inner.len());
    let mut chars = inner.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' {
            if let Some(&next) = chars.peek() {
                chars.next();
                match next {
                    '"' => result.push('"'),
                    '\\' => result.push('\\'),
                    'n' => result.push('\n'),
                    't' => result.push('\t'),
                    _ => {
                        result.push('\\');
                        result.push(next);
                    }
                }
            } else {
                result.push(c);
            }
        } else {
            result.push(c);
        }
    }
    result
}

fn parse_date(pair: Pair<Rule>) -> Result<NaiveDate, ParseError> {
    let s = pair.as_str();
    NaiveDate::parse_from_str(s, "%Y-%m-%d")
        .map_err(|_| ParseError::InvalidValue(format!("Invalid date: {}", s)))
}

fn parse_iso8601_datetime(pair: Pair<Rule>) -> Result<DateTime<Utc>, ParseError> {
    let s = pair.as_str();
    // Parse ISO 8601 datetime with timezone
    DateTime::parse_from_rfc3339(s)
        .map(|dt| dt.with_timezone(&Utc))
        .map_err(|_| ParseError::InvalidValue(format!("Invalid datetime: {}", s)))
}

// ============================================================================
// Serialization
// ============================================================================

/// Serialize a `BaselineStore` to a string in the baselines file format
pub fn serialize_baselines(store: &BaselineStore) -> String {
    let mut output = String::new();

    // Header comment
    output.push_str("# Auto-generated by utf8proj. Manual edits not recommended.\n\n");

    for (_, baseline) in store.iter() {
        serialize_baseline(&mut output, baseline);
        output.push('\n');
    }

    output
}

fn serialize_baseline(output: &mut String, baseline: &Baseline) {
    output.push_str(&format!("baseline {} {{\n", baseline.name));

    // Meta attributes
    output.push_str(&format!("    saved: {}\n", baseline.saved.to_rfc3339()));

    if let Some(ref desc) = baseline.description {
        output.push_str(&format!("    description: \"{}\"\n", escape_string(desc)));
    }

    if let Some(ref parent) = baseline.parent {
        output.push_str(&format!("    parent: {}\n", parent));
    }

    // Blank line before tasks if there are any
    if !baseline.tasks.is_empty() {
        output.push('\n');
    }

    // Task snapshots (already sorted by BTreeMap)
    for snapshot in baseline.tasks.values() {
        output.push_str(&format!(
            "    {}: {} -> {}\n",
            snapshot.task_id, snapshot.start, snapshot.finish
        ));
    }

    output.push_str("}\n");
}

fn escape_string(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '"' => result.push_str("\\\""),
            '\\' => result.push_str("\\\\"),
            '\n' => result.push_str("\\n"),
            '\t' => result.push_str("\\t"),
            _ => result.push(c),
        }
    }
    result
}

// ============================================================================
// File Operations
// ============================================================================

/// Get the baselines file path for a project file
pub fn baselines_path(project_path: &std::path::Path) -> std::path::PathBuf {
    let mut path = project_path.to_path_buf();
    let filename = path
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();
    path.set_file_name(format!("{}.baselines", filename));
    path
}

/// Load baselines from the sidecar file for a project
pub fn load_baselines(project_path: &std::path::Path) -> Result<BaselineStore, ParseError> {
    let baselines_file = baselines_path(project_path);

    if !baselines_file.exists() {
        return Ok(BaselineStore::new());
    }

    let content = std::fs::read_to_string(&baselines_file)
        .map_err(|e| ParseError::InvalidValue(format!("Failed to read baselines file: {}", e)))?;

    parse_baselines(&content)
}

/// Save baselines to the sidecar file for a project
pub fn save_baselines(
    project_path: &std::path::Path,
    store: &BaselineStore,
) -> Result<(), std::io::Error> {
    let baselines_file = baselines_path(project_path);
    let content = serialize_baselines(store);
    std::fs::write(&baselines_file, content)
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn test_parse_empty_file() {
        let input = "";
        let store = parse_baselines(input).unwrap();
        assert!(store.is_empty());
    }

    #[test]
    fn test_parse_comment_only() {
        let input = "# This is a comment\n# Another comment";
        let store = parse_baselines(input).unwrap();
        assert!(store.is_empty());
    }

    #[test]
    fn test_parse_minimal_baseline() {
        let input = r#"
        baseline original {
            saved: 2026-01-15T10:30:00Z
        }
        "#;

        let store = parse_baselines(input).unwrap();
        assert_eq!(store.len(), 1);

        let baseline = store.get("original").unwrap();
        assert_eq!(baseline.name, "original");
        assert!(baseline.description.is_none());
        assert!(baseline.parent.is_none());
        assert!(baseline.tasks.is_empty());
    }

    #[test]
    fn test_parse_baseline_with_description() {
        let input = r#"
        baseline original {
            saved: 2026-01-15T10:30:00Z
            description: "Initial approved plan"
        }
        "#;

        let store = parse_baselines(input).unwrap();
        let baseline = store.get("original").unwrap();
        assert_eq!(baseline.description, Some("Initial approved plan".to_string()));
    }

    #[test]
    fn test_parse_baseline_with_parent() {
        let input = r#"
        baseline change_order_1 {
            saved: 2026-02-01T14:20:00Z
            parent: original
        }
        "#;

        let store = parse_baselines(input).unwrap();
        let baseline = store.get("change_order_1").unwrap();
        assert_eq!(baseline.parent, Some("original".to_string()));
    }

    #[test]
    fn test_parse_baseline_with_tasks() {
        let input = r#"
        baseline original {
            saved: 2026-01-15T10:30:00Z

            design: 2026-01-01 -> 2026-01-10
            build: 2026-01-11 -> 2026-02-15
            test: 2026-02-16 -> 2026-02-28
        }
        "#;

        let store = parse_baselines(input).unwrap();
        let baseline = store.get("original").unwrap();

        assert_eq!(baseline.task_count(), 3);

        let design = baseline.tasks.get("design").unwrap();
        assert_eq!(design.start, NaiveDate::from_ymd_opt(2026, 1, 1).unwrap());
        assert_eq!(design.finish, NaiveDate::from_ymd_opt(2026, 1, 10).unwrap());

        assert_eq!(baseline.project_finish, NaiveDate::from_ymd_opt(2026, 2, 28).unwrap());
    }

    #[test]
    fn test_parse_qualified_task_ids() {
        let input = r#"
        baseline original {
            saved: 2026-01-15T10:30:00Z

            phase1.design: 2026-01-01 -> 2026-01-10
            phase1.build: 2026-01-11 -> 2026-01-20
            phase2.design: 2026-01-21 -> 2026-01-25
        }
        "#;

        let store = parse_baselines(input).unwrap();
        let baseline = store.get("original").unwrap();

        assert!(baseline.tasks.contains_key("phase1.design"));
        assert!(baseline.tasks.contains_key("phase1.build"));
        assert!(baseline.tasks.contains_key("phase2.design"));
    }

    #[test]
    fn test_parse_multiple_baselines() {
        let input = r#"
        baseline original {
            saved: 2026-01-15T10:30:00Z
            design: 2026-01-01 -> 2026-01-10
        }

        baseline change_order_1 {
            saved: 2026-02-01T14:20:00Z
            description: "After scope change"
            parent: original
            design: 2026-01-01 -> 2026-01-12
        }
        "#;

        let store = parse_baselines(input).unwrap();
        assert_eq!(store.len(), 2);
        assert!(store.contains("original"));
        assert!(store.contains("change_order_1"));
    }

    #[test]
    fn test_parse_datetime_with_offset() {
        let input = r#"
        baseline test {
            saved: 2026-01-15T10:30:00+05:30
        }
        "#;

        let store = parse_baselines(input).unwrap();
        let baseline = store.get("test").unwrap();
        // Should be converted to UTC
        assert_eq!(
            baseline.saved,
            Utc.with_ymd_and_hms(2026, 1, 15, 5, 0, 0).unwrap()
        );
    }

    #[test]
    fn test_serialize_empty_store() {
        let store = BaselineStore::new();
        let output = serialize_baselines(&store);
        assert!(output.contains("Auto-generated"));
    }

    #[test]
    fn test_serialize_baseline() {
        let mut store = BaselineStore::new();
        let mut baseline = Baseline::new("original");
        baseline.saved = Utc.with_ymd_and_hms(2026, 1, 15, 10, 30, 0).unwrap();
        baseline.description = Some("Initial plan".to_string());
        baseline.add_task(TaskSnapshot::new(
            "design",
            NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(),
            NaiveDate::from_ymd_opt(2026, 1, 10).unwrap(),
        ));
        store.baselines.insert("original".to_string(), baseline);

        let output = serialize_baselines(&store);

        assert!(output.contains("baseline original {"));
        assert!(output.contains("saved: 2026-01-15T10:30:00+00:00"));
        assert!(output.contains("description: \"Initial plan\""));
        assert!(output.contains("design: 2026-01-01 -> 2026-01-10"));
    }

    #[test]
    fn test_roundtrip() {
        let input = r#"
        baseline original {
            saved: 2026-01-15T10:30:00Z
            description: "Initial approved plan"

            design: 2026-01-01 -> 2026-01-10
            build: 2026-01-11 -> 2026-02-15
        }

        baseline v2 {
            saved: 2026-02-01T14:20:00Z
            parent: original

            design: 2026-01-01 -> 2026-01-12
            build: 2026-01-13 -> 2026-02-20
        }
        "#;

        let store1 = parse_baselines(input).unwrap();
        let serialized = serialize_baselines(&store1);
        let store2 = parse_baselines(&serialized).unwrap();

        assert_eq!(store1.len(), store2.len());

        for name in store1.names() {
            let b1 = store1.get(name).unwrap();
            let b2 = store2.get(name).unwrap();

            assert_eq!(b1.name, b2.name);
            assert_eq!(b1.description, b2.description);
            assert_eq!(b1.parent, b2.parent);
            assert_eq!(b1.task_count(), b2.task_count());

            for (task_id, snap1) in &b1.tasks {
                let snap2 = b2.tasks.get(task_id).unwrap();
                assert_eq!(snap1.start, snap2.start);
                assert_eq!(snap1.finish, snap2.finish);
            }
        }
    }

    #[test]
    fn test_escape_string() {
        assert_eq!(escape_string("hello"), "hello");
        assert_eq!(escape_string("hello\"world"), "hello\\\"world");
        assert_eq!(escape_string("line1\nline2"), "line1\\nline2");
        assert_eq!(escape_string("path\\to\\file"), "path\\\\to\\\\file");
    }

    #[test]
    fn test_baselines_path() {
        use std::path::Path;

        let project = Path::new("/home/user/project.proj");
        let expected = Path::new("/home/user/project.proj.baselines");
        assert_eq!(baselines_path(project), expected);

        let project2 = Path::new("myproject.proj");
        let expected2 = Path::new("myproject.proj.baselines");
        assert_eq!(baselines_path(project2), expected2);
    }

    #[test]
    fn test_parse_invalid_syntax() {
        let input = "baseline { }"; // Missing name
        let result = parse_baselines(input);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_invalid_date() {
        let input = r#"
        baseline test {
            saved: 2026-01-15T10:30:00Z
            design: invalid-date -> 2026-01-10
        }
        "#;
        let result = parse_baselines(input);
        assert!(result.is_err());
    }
}
