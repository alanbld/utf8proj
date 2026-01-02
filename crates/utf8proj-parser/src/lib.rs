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
    let content = std::fs::read_to_string(path).map_err(|e| ParseError::InvalidValue(e.to_string()))?;

    match detect_format(path) {
        FileFormat::TaskJuggler => parse_tjp(&content),
        FileFormat::Native => parse_project(&content),
    }
}
