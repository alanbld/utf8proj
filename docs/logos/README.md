# utf8proj Logo Assets

Professional logo system for utf8proj.

## Files

| File | Size | Use Case |
|------|------|----------|
| `logo.svg` | 400×200 | README, social media, general branding |
| `logo-horizontal.svg` | 600×120 | Website headers, light backgrounds |
| `logo-horizontal-dark.svg` | 600×120 | Dark mode, dark backgrounds |
| `logo-horizontal-mono.svg` | 600×120 | Print, black & white |
| `logo-icon.svg` | 128×128 | Favicon, app icons, small displays |

## Design Elements

- **Curly braces `{ }`** - Text-based, code-driven project files
- **Timeline bar** - Gantt charts and scheduling
- **Tick marks** - Milestones, time intervals
- **Triangles ▼▲** - Dependencies, critical path

## Colors

| Color | Hex | Usage |
|-------|-----|-------|
| Teal | `#17a2b8` | Primary brand color |
| Teal Light | `#4dd4e4` | Dark mode variant |
| Dark Gray | `#2d3748` | Wordmark text |
| White | `#f7fafc` | Dark mode text |

## Usage

### In Markdown
```markdown
<img src="docs/logos/logo.svg" alt="utf8proj" width="400">
```

### In HTML (with dark mode support)
```html
<picture>
  <source srcset="logos/logo-horizontal-dark.svg" media="(prefers-color-scheme: dark)">
  <img src="logos/logo-horizontal.svg" alt="utf8proj" width="300">
</picture>
```

### As Favicon
```html
<link rel="icon" href="logos/logo-icon.svg" type="image/svg+xml">
```

## License

Licensed under MIT/Apache-2.0, same as the main project.
