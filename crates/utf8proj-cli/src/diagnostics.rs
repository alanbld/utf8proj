//! Diagnostic formatting for CLI output
//!
//! This module implements diagnostic emitters for different output formats:
//! - `TerminalEmitter`: rustc-style colored output to stderr
//! - `JsonEmitter`: machine-readable JSON output
//!
//! Both emitters support:
//! - `--strict` mode: escalates warnings to errors, hints to warnings
//! - `--quiet` mode: suppresses all output except errors
//! - Path normalization for reproducible test output
//!
//! ## Exit Code Semantics
//!
//! Exit codes follow rustc conventions:
//!
//! | Exit Code | Meaning |
//! |-----------|---------|
//! | 0 | Success: no errors (warnings/hints/info allowed) |
//! | 1 | Failure: one or more errors emitted |
//!
//! ### Policy Effects
//!
//! - **Default mode**: Exit code determined by native errors only
//! - **`--strict` mode**: Warnings escalate to errors, hints to warnings
//!   - A project with only warnings will exit 1 in strict mode
//! - **`--quiet` mode**: Does NOT affect exit code, only output visibility
//! - **`--format=json`**: Exit code semantics identical to text mode
//!
//! ### Multiple Diagnostics
//!
//! Exit code is determined by the **highest effective severity** after policy:
//! - If any effective error exists → exit 1
//! - Otherwise → exit 0

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process;

use serde::Serialize;
use utf8proj_core::{Diagnostic, DiagnosticEmitter, Severity};

// ============================================================================
// Exit Code
// ============================================================================

/// Exit codes for CLI operations.
///
/// These follow rustc conventions and are stable API.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExitCode {
    /// Success: no errors (warnings/hints/info allowed)
    Success = 0,
    /// Failure: one or more errors emitted
    Failure = 1,
}

impl ExitCode {
    /// Determine exit code from error count.
    ///
    /// This is the central decision point for exit semantics.
    /// The error count should already reflect policy (strict mode escalation).
    pub fn from_error_count(count: usize) -> Self {
        if count > 0 {
            ExitCode::Failure
        } else {
            ExitCode::Success
        }
    }

    /// Check if this represents success
    pub fn is_success(self) -> bool {
        matches!(self, ExitCode::Success)
    }

    /// Check if this represents failure
    pub fn is_failure(self) -> bool {
        matches!(self, ExitCode::Failure)
    }

    /// Convert to std::process::ExitCode for main()
    pub fn to_process_exit_code(self) -> process::ExitCode {
        process::ExitCode::from(self as u8)
    }

    /// Get the numeric value
    pub fn code(self) -> i32 {
        self as i32
    }
}

impl From<ExitCode> for process::ExitCode {
    fn from(code: ExitCode) -> Self {
        process::ExitCode::from(code as u8)
    }
}

// ============================================================================
// Diagnostic Config
// ============================================================================

/// Configuration for diagnostic output
#[derive(Debug, Clone)]
pub struct DiagnosticConfig {
    /// Escalate severities: warnings become errors, hints become warnings
    pub strict: bool,
    /// Suppress all output except errors
    pub quiet: bool,
    /// Base path to strip from file paths (for reproducible output)
    pub base_path: Option<PathBuf>,
}

impl Default for DiagnosticConfig {
    fn default() -> Self {
        Self {
            strict: false,
            quiet: false,
            base_path: None,
        }
    }
}

impl DiagnosticConfig {
    /// Create a new config with strict mode enabled
    pub fn strict() -> Self {
        Self {
            strict: true,
            ..Default::default()
        }
    }

    /// Create a new config with quiet mode enabled
    pub fn quiet() -> Self {
        Self {
            quiet: true,
            ..Default::default()
        }
    }

    /// Set the base path for path normalization
    pub fn with_base_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.base_path = Some(path.into());
        self
    }

    /// Escalate severity according to strict mode rules
    pub fn effective_severity(&self, severity: Severity) -> Severity {
        if self.strict {
            match severity {
                Severity::Warning => Severity::Error,
                Severity::Hint => Severity::Warning,
                s => s,
            }
        } else {
            severity
        }
    }

    /// Check if a diagnostic should be shown based on quiet mode
    pub fn should_show(&self, severity: Severity) -> bool {
        if self.quiet {
            // In quiet mode, only show errors
            matches!(self.effective_severity(severity), Severity::Error)
        } else {
            true
        }
    }

    /// Normalize a file path for output
    pub fn normalize_path(&self, path: &Path) -> String {
        if let Some(base) = &self.base_path {
            if let Ok(stripped) = path.strip_prefix(base) {
                return stripped.display().to_string();
            }
        }
        path.display().to_string()
    }
}

/// Terminal emitter that outputs rustc-style diagnostics to stderr
pub struct TerminalEmitter<W: Write> {
    writer: W,
    config: DiagnosticConfig,
    error_count: usize,
    warning_count: usize,
}

impl<W: Write> TerminalEmitter<W> {
    pub fn new(writer: W, config: DiagnosticConfig) -> Self {
        Self {
            writer,
            config,
            error_count: 0,
            warning_count: 0,
        }
    }

    /// Get the number of errors emitted
    pub fn error_count(&self) -> usize {
        self.error_count
    }

    /// Get the number of warnings emitted
    pub fn warning_count(&self) -> usize {
        self.warning_count
    }

    /// Check if any errors were emitted
    pub fn has_errors(&self) -> bool {
        self.error_count > 0
    }

    /// Get the exit code based on emitted diagnostics.
    ///
    /// This is the authoritative exit code decision for terminal output.
    /// Returns `ExitCode::Failure` if any errors were emitted (after policy).
    pub fn exit_code(&self) -> ExitCode {
        ExitCode::from_error_count(self.error_count)
    }

    /// Format and write a diagnostic
    fn write_diagnostic(&mut self, diagnostic: &Diagnostic) -> std::io::Result<()> {
        let effective_severity = self.config.effective_severity(diagnostic.severity);

        // Don't show if quiet mode filters it
        if !self.config.should_show(diagnostic.severity) {
            return Ok(());
        }

        // Track counts based on effective severity
        match effective_severity {
            Severity::Error => self.error_count += 1,
            Severity::Warning => self.warning_count += 1,
            _ => {}
        }

        // Header line: severity[CODE]: message
        writeln!(
            self.writer,
            "{}[{}]: {}",
            effective_severity.as_str(),
            diagnostic.code.as_str(),
            diagnostic.message
        )?;

        // Source location: --> file:line
        if let Some(file) = &diagnostic.file {
            let normalized_path = self.config.normalize_path(file);
            if let Some(span) = &diagnostic.span {
                writeln!(self.writer, "  --> {}:{}", normalized_path, span.line)?;
            } else {
                writeln!(self.writer, "  --> {}", normalized_path)?;
            }
        }

        // Notes
        if !diagnostic.notes.is_empty() {
            writeln!(self.writer, "   |")?;
            for note in &diagnostic.notes {
                writeln!(self.writer, "   = {}", note)?;
            }
        }

        // Hints
        for hint in &diagnostic.hints {
            writeln!(self.writer, "   = hint: {}", hint)?;
        }

        // Trailing newline for readability
        writeln!(self.writer)?;

        Ok(())
    }
}

impl<W: Write> DiagnosticEmitter for TerminalEmitter<W> {
    fn emit(&mut self, diagnostic: Diagnostic) {
        // Ignore write errors in emit (stderr may be closed)
        let _ = self.write_diagnostic(&diagnostic);
    }
}

/// JSON emitter that outputs diagnostics in machine-readable format
pub struct JsonEmitter {
    diagnostics: Vec<JsonDiagnostic>,
    config: DiagnosticConfig,
}

/// JSON representation of a diagnostic
#[derive(Debug, Serialize)]
pub struct JsonDiagnostic {
    pub code: String,
    pub severity: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub column: Option<usize>,
    pub spans: Vec<JsonSpan>,
    pub notes: Vec<String>,
    pub hints: Vec<String>,
}

/// JSON representation of a source span
#[derive(Debug, Serialize)]
pub struct JsonSpan {
    pub start: usize,
    pub end: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

impl JsonEmitter {
    pub fn new(config: DiagnosticConfig) -> Self {
        Self {
            diagnostics: Vec::new(),
            config,
        }
    }

    /// Get collected diagnostics
    pub fn diagnostics(&self) -> &[JsonDiagnostic] {
        &self.diagnostics
    }

    /// Check if any errors were emitted
    pub fn has_errors(&self) -> bool {
        self.diagnostics
            .iter()
            .any(|d| d.severity == "error")
    }

    /// Get error count
    pub fn error_count(&self) -> usize {
        self.diagnostics
            .iter()
            .filter(|d| d.severity == "error")
            .count()
    }

    /// Get the exit code based on collected diagnostics.
    ///
    /// This is the authoritative exit code decision for JSON output.
    /// Returns `ExitCode::Failure` if any errors were collected (after policy).
    pub fn exit_code(&self) -> ExitCode {
        ExitCode::from_error_count(self.error_count())
    }

    /// Convert to JSON value for inclusion in schedule output
    pub fn to_json_value(&self) -> serde_json::Value {
        serde_json::to_value(&self.diagnostics).unwrap_or(serde_json::Value::Null)
    }
}

impl DiagnosticEmitter for JsonEmitter {
    fn emit(&mut self, diagnostic: Diagnostic) {
        let effective_severity = self.config.effective_severity(diagnostic.severity);

        // Don't include if quiet mode filters it
        if !self.config.should_show(diagnostic.severity) {
            return;
        }

        let json_diagnostic = JsonDiagnostic {
            code: diagnostic.code.as_str().to_string(),
            severity: effective_severity.as_str().to_string(),
            message: diagnostic.message.clone(),
            file: diagnostic
                .file
                .as_ref()
                .map(|p| self.config.normalize_path(p)),
            line: diagnostic.span.as_ref().map(|s| s.line),
            column: diagnostic.span.as_ref().map(|s| s.column),
            spans: diagnostic
                .span
                .iter()
                .map(|s| JsonSpan {
                    start: s.column.saturating_sub(1),
                    end: s.column.saturating_sub(1) + s.length,
                    label: s.label.clone(),
                })
                .chain(diagnostic.secondary_spans.iter().map(|s| JsonSpan {
                    start: s.column.saturating_sub(1),
                    end: s.column.saturating_sub(1) + s.length,
                    label: s.label.clone(),
                }))
                .collect(),
            notes: diagnostic.notes.clone(),
            hints: diagnostic.hints.clone(),
        };

        self.diagnostics.push(json_diagnostic);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use utf8proj_core::{DiagnosticCode, SourceSpan};

    fn make_test_diagnostic() -> Diagnostic {
        Diagnostic::new(
            DiagnosticCode::W001AbstractAssignment,
            "task 'api_dev' is assigned to abstract profile 'developer'",
        )
        .with_file(PathBuf::from("/project/test.proj"))
        .with_span(SourceSpan::new(17, 13, 9).with_label("abstract profile"))
        .with_note("cost range is $500 - $1,000 (100% spread)")
        .with_hint("assign a concrete resource to lock in exact cost")
    }

    #[test]
    fn terminal_emitter_basic_output() {
        let mut output = Vec::new();
        let config = DiagnosticConfig::default();
        let mut emitter = TerminalEmitter::new(&mut output, config);

        emitter.emit(make_test_diagnostic());

        let output_str = String::from_utf8(output).unwrap();
        assert!(output_str.contains("warning[W001]"));
        assert!(output_str.contains("task 'api_dev'"));
        assert!(output_str.contains("/project/test.proj:17"));
        assert!(output_str.contains("cost range is $500 - $1,000"));
        assert!(output_str.contains("hint: assign a concrete resource"));
    }

    #[test]
    fn terminal_emitter_strict_mode() {
        let mut output = Vec::new();
        let config = DiagnosticConfig::strict();
        let mut emitter = TerminalEmitter::new(&mut output, config);

        emitter.emit(make_test_diagnostic());

        // Check counts before converting output (which consumes it)
        assert_eq!(emitter.error_count(), 1);
        assert_eq!(emitter.warning_count(), 0);

        // Now we need to drop emitter to release borrow of output
        drop(emitter);

        let output_str = String::from_utf8(output).unwrap();
        // Warning should be escalated to error in strict mode
        assert!(output_str.contains("error[W001]"));
    }

    #[test]
    fn terminal_emitter_quiet_mode() {
        let mut output = Vec::new();
        let config = DiagnosticConfig::quiet();
        let mut emitter = TerminalEmitter::new(&mut output, config);

        // Warning should be suppressed in quiet mode
        emitter.emit(make_test_diagnostic());

        drop(emitter);
        let output_str = String::from_utf8(output).unwrap();
        assert!(output_str.is_empty());
    }

    #[test]
    fn terminal_emitter_quiet_shows_errors() {
        let mut output = Vec::new();
        let config = DiagnosticConfig::quiet();
        let mut emitter = TerminalEmitter::new(&mut output, config);

        let error = Diagnostic::error(
            DiagnosticCode::E001CircularSpecialization,
            "circular specialization detected",
        );
        emitter.emit(error);

        drop(emitter);
        let output_str = String::from_utf8(output).unwrap();
        assert!(output_str.contains("error[E001]"));
    }

    #[test]
    fn terminal_emitter_path_normalization() {
        let mut output = Vec::new();
        let config = DiagnosticConfig::default()
            .with_base_path("/project");
        let mut emitter = TerminalEmitter::new(&mut output, config);

        emitter.emit(make_test_diagnostic());

        drop(emitter);
        let output_str = String::from_utf8(output).unwrap();
        // Path should be normalized to relative
        assert!(output_str.contains("test.proj:17"));
        assert!(!output_str.contains("/project/test.proj"));
    }

    #[test]
    fn json_emitter_basic_output() {
        let config = DiagnosticConfig::default();
        let mut emitter = JsonEmitter::new(config);

        emitter.emit(make_test_diagnostic());

        let diagnostics = emitter.diagnostics();
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "W001");
        assert_eq!(diagnostics[0].severity, "warning");
        assert!(diagnostics[0].message.contains("api_dev"));
    }

    #[test]
    fn json_emitter_strict_mode() {
        let config = DiagnosticConfig::strict();
        let mut emitter = JsonEmitter::new(config);

        emitter.emit(make_test_diagnostic());

        let diagnostics = emitter.diagnostics();
        // Warning should be escalated to error
        assert_eq!(diagnostics[0].severity, "error");
    }

    #[test]
    fn json_emitter_quiet_mode() {
        let config = DiagnosticConfig::quiet();
        let mut emitter = JsonEmitter::new(config);

        emitter.emit(make_test_diagnostic());

        // Warning should be suppressed
        assert!(emitter.diagnostics().is_empty());
    }

    #[test]
    fn json_emitter_to_json_value() {
        let config = DiagnosticConfig::default();
        let mut emitter = JsonEmitter::new(config);

        emitter.emit(make_test_diagnostic());

        let json = emitter.to_json_value();
        assert!(json.is_array());
        let arr = json.as_array().unwrap();
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0]["code"], "W001");
    }

    #[test]
    fn info_diagnostic_unchanged_in_strict() {
        let config = DiagnosticConfig::strict();
        let mut emitter = JsonEmitter::new(config);

        let info = Diagnostic::new(
            DiagnosticCode::I001ProjectCostSummary,
            "project 'Test' scheduled successfully",
        );
        emitter.emit(info);

        // Info should remain info in strict mode
        assert_eq!(emitter.diagnostics()[0].severity, "info");
    }

    #[test]
    fn hint_becomes_warning_in_strict() {
        let config = DiagnosticConfig::strict();
        let mut emitter = JsonEmitter::new(config);

        let hint = Diagnostic::new(
            DiagnosticCode::H002UnusedProfile,
            "profile 'dev' is defined but never assigned",
        );
        emitter.emit(hint);

        // Hint should become warning in strict mode
        assert_eq!(emitter.diagnostics()[0].severity, "warning");
    }

    // =========================================================================
    // Exit Code Tests
    // =========================================================================

    #[test]
    fn exit_code_zero_errors_is_success() {
        assert_eq!(ExitCode::from_error_count(0), ExitCode::Success);
        assert!(ExitCode::Success.is_success());
        assert!(!ExitCode::Success.is_failure());
        assert_eq!(ExitCode::Success.code(), 0);
    }

    #[test]
    fn exit_code_with_errors_is_failure() {
        assert_eq!(ExitCode::from_error_count(1), ExitCode::Failure);
        assert_eq!(ExitCode::from_error_count(5), ExitCode::Failure);
        assert!(ExitCode::Failure.is_failure());
        assert!(!ExitCode::Failure.is_success());
        assert_eq!(ExitCode::Failure.code(), 1);
    }

    #[test]
    fn terminal_emitter_exit_code_no_errors() {
        let mut output = Vec::new();
        let config = DiagnosticConfig::default();
        let mut emitter = TerminalEmitter::new(&mut output, config);

        // Emit only warnings - should be success
        emitter.emit(make_test_diagnostic());

        assert_eq!(emitter.exit_code(), ExitCode::Success);
    }

    #[test]
    fn terminal_emitter_exit_code_with_errors() {
        let mut output = Vec::new();
        let config = DiagnosticConfig::default();
        let mut emitter = TerminalEmitter::new(&mut output, config);

        let error = Diagnostic::error(
            DiagnosticCode::E001CircularSpecialization,
            "circular specialization detected",
        );
        emitter.emit(error);

        assert_eq!(emitter.exit_code(), ExitCode::Failure);
    }

    #[test]
    fn terminal_emitter_exit_code_strict_escalation() {
        let mut output = Vec::new();
        let config = DiagnosticConfig::strict();
        let mut emitter = TerminalEmitter::new(&mut output, config);

        // Warning escalates to error in strict mode
        emitter.emit(make_test_diagnostic());

        assert_eq!(emitter.exit_code(), ExitCode::Failure);
    }

    #[test]
    fn terminal_emitter_exit_code_quiet_doesnt_affect_code() {
        let mut output = Vec::new();
        let config = DiagnosticConfig {
            strict: true,
            quiet: true,
            base_path: None,
        };
        let mut emitter = TerminalEmitter::new(&mut output, config);

        // Warning escalates to error (strict), output suppressed (quiet)
        // But exit code should still be failure
        emitter.emit(make_test_diagnostic());

        assert_eq!(emitter.exit_code(), ExitCode::Failure);
    }

    #[test]
    fn json_emitter_exit_code_no_errors() {
        let config = DiagnosticConfig::default();
        let mut emitter = JsonEmitter::new(config);

        // Emit only warnings - should be success
        emitter.emit(make_test_diagnostic());

        assert_eq!(emitter.exit_code(), ExitCode::Success);
    }

    #[test]
    fn json_emitter_exit_code_with_errors() {
        let config = DiagnosticConfig::default();
        let mut emitter = JsonEmitter::new(config);

        let error = Diagnostic::error(
            DiagnosticCode::E001CircularSpecialization,
            "circular specialization detected",
        );
        emitter.emit(error);

        assert_eq!(emitter.exit_code(), ExitCode::Failure);
    }

    #[test]
    fn json_emitter_exit_code_strict_escalation() {
        let config = DiagnosticConfig::strict();
        let mut emitter = JsonEmitter::new(config);

        // Warning escalates to error in strict mode
        emitter.emit(make_test_diagnostic());

        assert_eq!(emitter.exit_code(), ExitCode::Failure);
    }

    #[test]
    fn hint_only_is_success_even_in_strict() {
        // Hints become warnings in strict, not errors
        let config = DiagnosticConfig::strict();
        let mut emitter = JsonEmitter::new(config);

        let hint = Diagnostic::new(
            DiagnosticCode::H002UnusedProfile,
            "profile 'dev' is defined but never assigned",
        );
        emitter.emit(hint);

        // Hint → Warning in strict, which is NOT an error
        assert_eq!(emitter.exit_code(), ExitCode::Success);
    }

    #[test]
    fn info_only_is_always_success() {
        let config = DiagnosticConfig::strict();
        let mut emitter = JsonEmitter::new(config);

        let info = Diagnostic::new(
            DiagnosticCode::I001ProjectCostSummary,
            "project scheduled successfully",
        );
        emitter.emit(info);

        // Info stays info, always success
        assert_eq!(emitter.exit_code(), ExitCode::Success);
    }
}
