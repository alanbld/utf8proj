//! Native DSL parser for .proj files using pest.

use pest_derive::Parser;

#[derive(Parser)]
#[grammar = "native/grammar.pest"]
pub struct ProjectParser;

#[cfg(test)]
mod tests {
    use super::*;
    use pest::Parser;

    #[test]
    fn parse_empty_project() {
        let input = r#"project "Test" {
    start: 2025-01-01
}"#;
        let result = ProjectParser::parse(Rule::project_file, input);
        assert!(result.is_ok(), "Failed to parse: {:?}", result.err());
    }
}
