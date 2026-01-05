//! Diagnostic conversion from utf8proj to LSP format

use tower_lsp::lsp_types::{
    Diagnostic as LspDiagnostic, DiagnosticSeverity, NumberOrString, Position, Range,
};

use utf8proj_core::{Diagnostic, Severity};

/// Convert utf8proj diagnostics to LSP diagnostics
pub fn to_lsp_diagnostics(diagnostics: &[Diagnostic]) -> Vec<LspDiagnostic> {
    diagnostics.iter().map(to_lsp_diagnostic).collect()
}

/// Convert a single diagnostic to LSP format
fn to_lsp_diagnostic(diag: &Diagnostic) -> LspDiagnostic {
    // Convert severity
    let severity = match diag.severity {
        Severity::Error => DiagnosticSeverity::ERROR,
        Severity::Warning => DiagnosticSeverity::WARNING,
        Severity::Hint => DiagnosticSeverity::HINT,
        Severity::Info => DiagnosticSeverity::INFORMATION,
    };

    // Build range from span if available
    let range = if let Some(ref span) = diag.span {
        Range::new(
            Position::new((span.line.saturating_sub(1)) as u32, (span.column.saturating_sub(1)) as u32),
            Position::new(
                (span.line.saturating_sub(1)) as u32,
                (span.column.saturating_sub(1) + span.length) as u32,
            ),
        )
    } else {
        // Default to start of file
        Range::new(Position::new(0, 0), Position::new(0, 1))
    };

    // Build message with notes and hints
    let mut message = diag.message.clone();

    if !diag.notes.is_empty() {
        message.push_str("\n\n");
        for note in &diag.notes {
            message.push_str("  = ");
            message.push_str(note);
            message.push('\n');
        }
    }

    if !diag.hints.is_empty() {
        if diag.notes.is_empty() {
            message.push_str("\n\n");
        }
        for hint in &diag.hints {
            message.push_str("  hint: ");
            message.push_str(hint);
            message.push('\n');
        }
    }

    LspDiagnostic {
        range,
        severity: Some(severity),
        code: Some(NumberOrString::String(diag.code.as_str().to_string())),
        code_description: None,
        source: Some("utf8proj".to_string()),
        message,
        related_information: None,
        tags: None,
        data: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use utf8proj_core::{DiagnosticCode, SourceSpan};

    #[test]
    fn convert_warning_diagnostic() {
        let diag = Diagnostic::new(
            DiagnosticCode::W001AbstractAssignment,
            "task 'api_dev' is assigned to abstract profile 'developer'",
        )
        .with_span(SourceSpan::new(10, 5, 12))
        .with_note("cost range is $500 - $1000")
        .with_hint("assign a concrete resource");

        let lsp_diag = to_lsp_diagnostic(&diag);

        assert_eq!(lsp_diag.severity, Some(DiagnosticSeverity::WARNING));
        assert_eq!(lsp_diag.code, Some(NumberOrString::String("W001".to_string())));
        assert!(lsp_diag.message.contains("task 'api_dev'"));
        assert!(lsp_diag.message.contains("cost range"));
        assert!(lsp_diag.message.contains("hint:"));
        assert_eq!(lsp_diag.range.start.line, 9); // 0-indexed
        assert_eq!(lsp_diag.range.start.character, 4); // 0-indexed
    }

    #[test]
    fn convert_error_diagnostic() {
        let diag = Diagnostic::error(
            DiagnosticCode::E001CircularSpecialization,
            "circular specialization detected: a -> b -> a",
        );

        let lsp_diag = to_lsp_diagnostic(&diag);

        assert_eq!(lsp_diag.severity, Some(DiagnosticSeverity::ERROR));
        assert_eq!(lsp_diag.code, Some(NumberOrString::String("E001".to_string())));
    }

    #[test]
    fn convert_hint_diagnostic() {
        let diag = Diagnostic::new(
            DiagnosticCode::H002UnusedProfile,
            "profile 'designer' is defined but never assigned",
        );

        let lsp_diag = to_lsp_diagnostic(&diag);

        assert_eq!(lsp_diag.severity, Some(DiagnosticSeverity::HINT));
    }

    #[test]
    fn convert_info_diagnostic() {
        let diag = Diagnostic::new(
            DiagnosticCode::I001ProjectCostSummary,
            "project 'Test' scheduled successfully",
        );

        let lsp_diag = to_lsp_diagnostic(&diag);

        assert_eq!(lsp_diag.severity, Some(DiagnosticSeverity::INFORMATION));
    }

    #[test]
    fn diagnostic_without_span_defaults_to_start() {
        let diag = Diagnostic::new(
            DiagnosticCode::W001AbstractAssignment,
            "test message",
        );

        let lsp_diag = to_lsp_diagnostic(&diag);

        assert_eq!(lsp_diag.range.start.line, 0);
        assert_eq!(lsp_diag.range.start.character, 0);
    }
}
