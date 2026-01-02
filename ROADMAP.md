# utf8proj Roadmap

## Future Features

### 1. Interactive Gantt Chart (HTML/SVG)

**Goal:** Generate standalone HTML files with interactive SVG Gantt charts.

#### Technical Design

**Output Structure:**
```
output.html (single file, no dependencies)
├── Embedded CSS (styles)
├── SVG Gantt chart
│   ├── Timeline header (dates/weeks/months)
│   ├── Task bars (colored by status/critical path)
│   ├── Dependency arrows (SVG paths)
│   ├── Milestones (diamond shapes)
│   └── Resource labels
└── Embedded JS (interactivity)
```

**Core Components:**

1. **Timeline Renderer** (`crates/utf8proj-render/src/gantt/timeline.rs`)
   - Calculate visible date range from schedule
   - Support day/week/month granularity based on project duration
   - Generate SVG `<text>` and `<line>` elements for grid

2. **Task Bar Renderer** (`crates/utf8proj-render/src/gantt/tasks.rs`)
   - Calculate x position from start date
   - Calculate width from duration
   - Color coding: critical path (red), normal (blue), complete (green)
   - Progress bar overlay for % complete
   - Hover tooltips with task details

3. **Dependency Arrow Renderer** (`crates/utf8proj-render/src/gantt/deps.rs`)
   - SVG `<path>` elements with bezier curves
   - Arrow types: FS (end→start), SS (start→start), FF (end→end), SF (start→end)
   - Avoid overlapping arrows using vertical offset algorithm

4. **Hierarchy Renderer** (`crates/utf8proj-render/src/gantt/hierarchy.rs`)
   - Collapsible container tasks
   - Indentation for nested tasks
   - Summary bars for containers

**SVG Layout Algorithm:**
```
Row height: 30px
Task bar height: 20px (centered in row)
Left panel width: 250px (task names)
Timeline width: calculated from date range
Padding: 10px

For each task:
  y = row_index * row_height + padding
  x = left_panel_width + (task.start - project.start).days * pixels_per_day
  width = task.duration.days * pixels_per_day
```

**Interactivity (Vanilla JS):**
- Click task → highlight dependencies
- Hover → show tooltip with dates, resources, % complete
- Click container → collapse/expand children
- Zoom controls → change time scale

**File Structure:**
```
crates/utf8proj-render/src/
├── lib.rs (add GanttRenderer)
├── gantt/
│   ├── mod.rs
│   ├── timeline.rs
│   ├── tasks.rs
│   ├── deps.rs
│   ├── hierarchy.rs
│   └── template.html (embedded via include_str!)
```

**CLI Integration:**
```bash
utf8proj render project.tjp --format gantt -o schedule.html
utf8proj render project.tjp --format gantt --theme dark -o schedule.html
```

**Estimated Scope:** ~1500 lines of Rust + ~200 lines HTML/CSS/JS template

---

### 2. WASM + Browser Playground

**Goal:** Run utf8proj entirely in the browser via WebAssembly.

#### Technical Design

**Architecture:**
```
┌─────────────────────────────────────────────┐
│           Browser Playground UI             │
├─────────────────────────────────────────────┤
│  ┌─────────────┐    ┌───────────────────┐   │
│  │ Code Editor │    │   Gantt Preview   │   │
│  │  (Monaco)   │    │   (SVG output)    │   │
│  └─────────────┘    └───────────────────┘   │
├─────────────────────────────────────────────┤
│              WASM Bridge (JS)               │
├─────────────────────────────────────────────┤
│         utf8proj-wasm (Rust/WASM)           │
│  ┌─────────┐ ┌────────┐ ┌────────────────┐  │
│  │ Parser  │ │ Solver │ │ Gantt Renderer │  │
│  └─────────┘ └────────┘ └────────────────┘  │
└─────────────────────────────────────────────┘
```

**New Crate:** `crates/utf8proj-wasm/`

```rust
// crates/utf8proj-wasm/src/lib.rs
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct Playground {
    // Cached state
}

#[wasm_bindgen]
impl Playground {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self { ... }

    /// Parse and schedule, return JSON result
    #[wasm_bindgen]
    pub fn schedule(&mut self, input: &str, format: &str) -> Result<JsValue, JsError> {
        // format: "tjp" | "native"
        // Returns: { tasks: [...], critical_path: [...], errors: [] }
    }

    /// Render to SVG Gantt chart
    #[wasm_bindgen]
    pub fn render_gantt(&self) -> Result<String, JsError> {
        // Returns SVG string
    }

    /// Get validation errors
    #[wasm_bindgen]
    pub fn validate(&self, input: &str, format: &str) -> JsValue {
        // Returns: { errors: [{ line, column, message }] }
    }
}
```

**Playground UI Components:**

1. **Editor Panel** (left side)
   - Monaco editor with custom TJP/native syntax highlighting
   - Real-time error markers from `validate()`
   - Example templates dropdown
   - File upload for .tjp/.proj files

2. **Preview Panel** (right side)
   - SVG Gantt chart (from `render_gantt()`)
   - Toggle: Gantt / JSON output / TJP output
   - Zoom/pan controls

3. **Toolbar**
   - Format selector (TJP / Native DSL)
   - Download buttons (HTML, TJP, JSON)
   - Share link (encodes project in URL)
   - Theme toggle (light/dark)

**Build Pipeline:**
```bash
# Build WASM
cd crates/utf8proj-wasm
wasm-pack build --target web --out-dir ../../playground/pkg

# Serve playground
cd playground
npm run dev
```

**Workspace Changes:**
```toml
# Cargo.toml (workspace)
[workspace]
members = [
    "crates/utf8proj-core",
    "crates/utf8proj-parser",
    "crates/utf8proj-solver",
    "crates/utf8proj-render",
    "crates/utf8proj-cli",
    "crates/utf8proj-wasm",  # NEW
]

# crates/utf8proj-wasm/Cargo.toml
[package]
name = "utf8proj-wasm"
version = "0.1.0"

[lib]
crate-type = ["cdylib", "rlib"]

[dependencies]
utf8proj-core.workspace = true
utf8proj-parser.workspace = true
utf8proj-solver.workspace = true
utf8proj-render.workspace = true
wasm-bindgen = "0.2"
serde = { version = "1", features = ["derive"] }
serde-wasm-bindgen = "0.6"

[dependencies.web-sys]
version = "0.3"
features = ["console"]
```

**Playground File Structure:**
```
playground/
├── index.html
├── src/
│   ├── main.ts
│   ├── editor.ts      (Monaco setup)
│   ├── preview.ts     (Gantt rendering)
│   ├── examples.ts    (Template projects)
│   └── share.ts       (URL encoding)
├── styles/
│   └── main.css
├── pkg/               (WASM output, gitignored)
└── package.json
```

**Syntax Highlighting (Monaco):**
```typescript
// Custom language definition for native DSL
monaco.languages.register({ id: 'utf8proj' });
monaco.languages.setMonarchTokensProvider('utf8proj', {
    keywords: ['project', 'task', 'resource', 'calendar', 'depends', 'assign', 'effort'],
    tokenizer: {
        root: [
            [/"[^"]*"/, 'string'],
            [/\d{4}-\d{2}-\d{2}/, 'number.date'],
            [/\d+[dwh]/, 'number.duration'],
            [/#.*$/, 'comment'],
            [/[a-z_]\w*/, { cases: { '@keywords': 'keyword', '@default': 'identifier' } }],
        ]
    }
});
```

**Performance Considerations:**
- WASM binary size: Target < 500KB gzipped
- Parse + schedule: Target < 100ms for 1000 tasks
- Use `console_error_panic_hook` for debugging
- Lazy-load Monaco editor

**Deployment:**
- GitHub Pages (static hosting)
- Custom domain: playground.utf8proj.dev (future)
- CDN for WASM binary

**Estimated Scope:**
- utf8proj-wasm: ~300 lines Rust
- Playground UI: ~1000 lines TypeScript + HTML/CSS
- Build/deploy config: ~100 lines

---

## Implementation Priority

| Feature | Complexity | Impact | Status |
|---------|------------|--------|--------|
| Resource Leveling | High | High | **Done** |
| Gantt Chart | Medium | High | **Done** |
| WASM Playground | Medium | Medium | Next |

## Dependencies

```
Resource Leveling (no deps)
    ↓
Gantt Chart (benefits from leveling visualization)
    ↓
WASM Playground (needs Gantt for preview)
```
