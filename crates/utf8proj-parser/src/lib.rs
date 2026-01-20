//! # utf8proj-parser
//!
//! Parser for utf8proj project files (.proj) and other input formats.
//!
//! This crate provides:
//! - Native DSL parser using pest grammar (.proj files)
//! - TaskJuggler parser (.tjp files)
//! - AST to domain model conversion
//!
//! ## Example
//!
//! ```rust
//! use utf8proj_parser::parse_project;
//!
//! let input = r#"
//! project "My Project" {
//!     start: 2025-02-01
//! }
//!
//! task hello "Hello" {
//!     duration: 1d
//! }
//! "#;
//!
//! let project = parse_project(input).unwrap();
//! assert_eq!(project.name, "My Project");
//! ```

pub mod baseline;
pub mod native;
pub mod tjp;

use thiserror::Error;

/// Parsing error
#[derive(Debug, Error)]
pub enum ParseError {
    #[error("Syntax error at line {line}, column {column}: {message}")]
    Syntax {
        line: usize,
        column: usize,
        message: String,
    },

    #[error("Invalid value: {0}")]
    InvalidValue(String),

    #[error("Unknown identifier: {0}")]
    UnknownIdentifier(String),
}

/// Supported file formats
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileFormat {
    /// Native utf8proj DSL (.proj)
    Native,
    /// TaskJuggler format (.tjp)
    TaskJuggler,
}

/// Detect file format from extension
pub fn detect_format(path: &std::path::Path) -> FileFormat {
    match path.extension().and_then(|e| e.to_str()) {
        Some("tjp") => FileFormat::TaskJuggler,
        _ => FileFormat::Native,
    }
}

/// Parse a project from the native DSL format
pub fn parse_project(input: &str) -> Result<utf8proj_core::Project, ParseError> {
    native::parse(input)
}

/// Parse a project from TaskJuggler format
pub fn parse_tjp(input: &str) -> Result<utf8proj_core::Project, ParseError> {
    tjp::parse(input)
}

/// Parse a project file from a path (auto-detects format)
pub fn parse_file(path: &std::path::Path) -> Result<utf8proj_core::Project, ParseError> {
    let content =
        std::fs::read_to_string(path).map_err(|e| ParseError::InvalidValue(e.to_string()))?;

    match detect_format(path) {
        FileFormat::TaskJuggler => parse_tjp(&content),
        FileFormat::Native => parse_project(&content),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn test_detect_format_tjp() {
        let path = Path::new("project.tjp");
        assert_eq!(detect_format(path), FileFormat::TaskJuggler);
    }

    #[test]
    fn test_detect_format_native() {
        let path = Path::new("project.proj");
        assert_eq!(detect_format(path), FileFormat::Native);
    }

    #[test]
    fn test_detect_format_unknown() {
        let path = Path::new("project.txt");
        assert_eq!(detect_format(path), FileFormat::Native);
    }

    #[test]
    fn test_detect_format_no_extension() {
        let path = Path::new("project");
        assert_eq!(detect_format(path), FileFormat::Native);
    }

    #[test]
    fn test_parse_file_not_found() {
        let path = Path::new("/nonexistent/path/to/file.proj");
        let result = parse_file(path);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_file_native() {
        use std::io::Write;
        let mut temp_file = tempfile::NamedTempFile::with_suffix(".proj").unwrap();
        writeln!(temp_file, r#"project "Test" {{ start: 2025-01-01 }}"#).unwrap();
        writeln!(temp_file, r#"task hello "Hello" {{ duration: 1d }}"#).unwrap();

        let result = parse_file(temp_file.path());
        assert!(result.is_ok());
        let project = result.unwrap();
        assert_eq!(project.name, "Test");
    }

    #[test]
    fn test_parse_file_tjp() {
        use std::io::Write;
        let mut temp_file = tempfile::NamedTempFile::with_suffix(".tjp").unwrap();
        writeln!(
            temp_file,
            r#"project test "Test" 2025-01-01 - 2025-12-31 {{}}"#
        )
        .unwrap();
        writeln!(temp_file, r#"task hello "Hello" {{ duration 1d }}"#).unwrap();

        let result = parse_file(temp_file.path());
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_error_display() {
        let err = ParseError::Syntax {
            line: 10,
            column: 5,
            message: "unexpected token".to_string(),
        };
        let msg = format!("{}", err);
        assert!(msg.contains("line 10"));
        assert!(msg.contains("column 5"));

        let err2 = ParseError::InvalidValue("bad value".to_string());
        assert!(format!("{}", err2).contains("bad value"));

        let err3 = ParseError::UnknownIdentifier("xyz".to_string());
        assert!(format!("{}", err3).contains("xyz"));
    }
}
