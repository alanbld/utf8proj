//! # utf8proj-parser
//!
//! Parser for utf8proj project files (.proj) and other input formats.
//!
//! This crate provides:
//! - Native DSL parser using pest grammar
//! - AST to domain model conversion
//!
//! ## Example
//!
//! ```rust,ignore
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
//! let project = parse_project(input)?;
//! ```

pub mod native;

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

/// Parse a project from the native DSL format
pub fn parse_project(_input: &str) -> Result<utf8proj_core::Project, ParseError> {
    // TODO: Implement full parser
    Ok(utf8proj_core::Project::new("Placeholder"))
}
