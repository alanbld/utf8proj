//! # utf8proj-render
//!
//! Rendering backends for utf8proj schedules.
//!
//! This crate provides:
//! - SVG Gantt chart rendering
//! - Text-based output
//! - Custom renderer trait
//!
//! ## Example
//!
//! ```rust,ignore
//! use utf8proj_core::{Project, Schedule, Renderer};
//! use utf8proj_render::SvgRenderer;
//!
//! let renderer = SvgRenderer::default();
//! let svg = renderer.render(&project, &schedule)?;
//! ```

use utf8proj_core::{Project, RenderError, Renderer, Schedule};

/// SVG Gantt chart renderer
#[derive(Default)]
pub struct SvgRenderer {
    /// Width of the output in pixels
    pub width: u32,
    /// Height per task row in pixels
    pub row_height: u32,
}

impl SvgRenderer {
    pub fn new() -> Self {
        Self {
            width: 1200,
            row_height: 30,
        }
    }
}

impl Renderer for SvgRenderer {
    type Output = String;

    fn render(&self, _project: &Project, _schedule: &Schedule) -> Result<String, RenderError> {
        // TODO: Implement SVG rendering
        Ok(String::from("<svg></svg>"))
    }
}

/// Plain text renderer for console output
#[derive(Default)]
pub struct TextRenderer;

impl Renderer for TextRenderer {
    type Output = String;

    fn render(&self, project: &Project, _schedule: &Schedule) -> Result<String, RenderError> {
        // TODO: Implement text rendering
        Ok(format!("Project: {}\n", project.name))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn svg_renderer_creation() {
        let renderer = SvgRenderer::new();
        assert_eq!(renderer.width, 1200);
    }
}
