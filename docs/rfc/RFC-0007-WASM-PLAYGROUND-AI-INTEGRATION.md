# RFC-0007: WASM Playground with AI-Assisted Editing

**RFC Number:** 0007
**Status**: Draft (Phase 0 Prerequisite Required)
**Authors**: utf8proj contributors
**Created**: 2026-01-15
**Updated**: 2026-01-15
**Target version**: 0.3.0+
**Related**: RFC-0006 (Focus View), RFC-0008 (Progress-Aware CPM), utf8proj-wasm, playground/, utf8proj-lsp
**Confidence**: 75% (architecture validated, API design in progress)
**Prerequisites**: Phase 0 Feature Parity (see Section 9.0)

---

## 1. Executive Summary

This RFC proposes extending the utf8proj WASM playground to provide:

* **LSP-equivalent editing features** (hover, completions, go-to-definition) directly in the browser
* **A standardized JavaScript API** exposing project analysis functions
* **AI-assistant integration points** enabling browser-based AI plugins to read, analyze, and modify project schedules

The goal is to make utf8proj the **first project scheduling tool with native AI-assistant support**, enabling non-technical users to manage complex schedules through natural language while maintaining full auditability.

---

## 2. Context & Motivation

### 2.1 The Accessibility Gap

utf8proj currently offers two paths for users:

| Path | Audience | Barrier |
|------|----------|---------|
| CLI + VS Code + LSP | Technical users | Install toolchain, configure editor |
| Browser playground | Anyone | No LSP features (hover, completions) |

Non-technical users (project managers, executives, team leads) face a choice between:
- Full features with complex setup
- Simple access with limited assistance

### 2.2 The AI Opportunity

Modern AI assistants (ChatGPT, Claude, Copilot) can bridge this gap by:

1. **Translating intent to syntax**: "Add a testing phase after development" → valid `.proj` code
2. **Explaining errors**: Parse error at line 15 → "You're missing a closing brace"
3. **Analyzing schedules**: "Why is the project delayed?" → Critical path analysis
4. **Suggesting improvements**: "Bob is overallocated in March" → Resource leveling advice

However, AI assistants need **structured access** to project data. Raw text is insufficient for reliable schedule analysis.

### 2.3 Market Position

| Tool | Browser Editor | AI Integration | Zero Install |
|------|----------------|----------------|--------------|
| MS Project | Via 365 (paid) | Copilot (limited) | No |
| Asana/Monday | Yes | Basic AI features | Yes |
| TaskJuggler | No | None | No |
| **utf8proj** | **Planned** | **First-class** | **Yes** |

utf8proj can differentiate by being the **first scheduling tool designed for AI collaboration**.

---

## 3. Goals & Non-Goals

### 3.1 Goals

* Provide LSP-equivalent features (hover, completions, diagnostics) in browser via WASM
* Expose a documented JavaScript API for programmatic access
* Enable AI browser plugins to read, analyze, and modify projects
* Maintain zero-install simplicity (GitHub Pages hosting)
* Preserve utf8proj's core value: **explainability over magic**

### 3.2 Non-Goals

* Building a proprietary AI assistant (we expose APIs, not AI)
* Real-time collaboration (Google Docs-style editing)
* Offline-first PWA (network required for AI features)
* Mobile-optimized interface (desktop browsers primary target)

### 3.3 Design Principle: AI as Translator, Not Controller

> **The AI explains and suggests; the human decides.**
>
> utf8proj's explainability philosophy extends to AI integration. The AI should:
> - Explain *why* a schedule looks the way it does
> - Suggest changes with clear rationale
> - Never silently "optimize" without user consent
>
> This preserves the audit trail that makes utf8proj valuable.

---

## 4. Technical Architecture

### 4.1 Current State

```
┌─────────────────────────────────────────────────────────────┐
│  Browser (Current)                                          │
│  ┌─────────────────┐         ┌────────────────────────────┐ │
│  │  Monaco Editor  │────────►│  utf8proj-wasm             │ │
│  │                 │         │  • parse() → errors        │ │
│  │  (text only)    │◄────────│  • schedule() → JSON       │ │
│  │                 │         │  • render() → SVG/HTML     │ │
│  └─────────────────┘         └────────────────────────────┘ │
└─────────────────────────────────────────────────────────────┘
```

**Limitations:**
- No hover information
- No autocomplete
- No go-to-definition
- No structured API for external tools

### 4.2 Proposed Architecture

```
┌──────────────────────────────────────────────────────────────────────────┐
│  Browser (Proposed)                                                      │
│                                                                          │
│  ┌──────────────────┐     ┌─────────────────────────────────────────┐    │
│  │  AI Plugin       │     │  window.utf8proj (Global API)           │    │
│  │  (3rd party)     │────►│                                         │    │
│  │                  │     │  Read:                                  │    │
│  │  "Add task..."   │     │  • getSource() → string                 │    │
│  └──────────────────┘     │  • getProjectJSON() → object            │    │
│          ▲                │  • getScheduleJSON() → object           │    │
│          │                │  • getDiagnostics() → array             │    │
│          │                │  • getResourceUtilization() → object    │    │
│          │                │                                         │    │
│  ┌───────┴──────┐         │  Write:                                 │    │
│  │ Monaco Editor│────────►│  • setSource(string)                    │    │
│  │              │         │  • applyEdit(edit)                      │    │
│  │ + Providers: │         │                                         │    │
│  │  • Hover     │◄────────│  Analysis:                              │    │
│  │  • Complete  │         │  • explainTask(id) → markdown           │    │
│  │  • Definition│         │  • whatIf(change) → impact              │    │
│  └──────────────┘         │  • suggestFixes() → array               │    │
│                           │                                         │    │
│                           └──────────────────┬──────────────────────┘    │
│                                              │                           │
│                                              ▼                           │
│                           ┌─────────────────────────────────────────┐    │
│                           │  utf8proj-wasm (Extended)               │    │
│                           │                                         │    │
│                           │  Core (existing):                       │    │
│                           │  • parse(), schedule(), render()        │    │
│                           │                                         │    │
│                           │  LSP-equivalent (new):                  │    │
│                           │  • get_hover(line, col) → markdown      │    │
│                           │  • get_completions(line, col) → items   │    │
│                           │  • get_definition(line, col) → location │    │
│                           │  • get_references(line, col) → array    │    │
│                           │                                         │    │
│                           │  AI-oriented (new):                     │    │
│                           │  • get_project_json() → structured      │    │
│                           │  • get_schedule_analysis() → insights   │    │
│                           │  • apply_edit(edit_json) → result       │    │
│                           │  • what_if(change_json) → delta         │    │
│                           └─────────────────────────────────────────┘    │
│                                                                          │
└──────────────────────────────────────────────────────────────────────────┘
```

### 4.3 Component Responsibilities

| Component | Responsibility |
|-----------|----------------|
| `utf8proj-wasm` | Core WASM module with parsing, scheduling, analysis |
| `window.utf8proj` | JavaScript API wrapper, state management, Monaco integration |
| Monaco Providers | Bridge between editor events and WASM functions |
| AI Manifest | Discoverable API description for AI plugins |

---

## 5. API Specification

### 5.1 WASM Exports (Rust)

```rust
// utf8proj-wasm/src/lib.rs - New exports

/// Returns hover information as markdown for the given position
#[wasm_bindgen]
pub fn get_hover(source: &str, line: u32, col: u32) -> Option<String> {
    let project = parse(source).ok()?;
    let schedule = solve(&project).ok()?;
    // Reuse logic from utf8proj-lsp/src/hover.rs
    hover::get_hover_info(&project, &schedule, line, col)
}

/// Returns completion items as JSON array
#[wasm_bindgen]
pub fn get_completions(source: &str, line: u32, col: u32) -> JsValue {
    let context = analyze_completion_context(source, line, col);
    let items = generate_completions(&context);
    serde_wasm_bindgen::to_value(&items).unwrap()
}

/// Returns definition location as JSON {line, col, length}
#[wasm_bindgen]
pub fn get_definition(source: &str, line: u32, col: u32) -> JsValue {
    let project = parse(source).ok();
    let location = navigation::find_definition(&project, line, col);
    serde_wasm_bindgen::to_value(&location).unwrap()
}

/// Returns structured project data (tasks, resources, calendars)
#[wasm_bindgen]
pub fn get_project_json(source: &str) -> JsValue {
    let project = parse(source).ok();
    serde_wasm_bindgen::to_value(&project).unwrap()
}

/// Returns schedule with analysis (critical path, slack, dates)
#[wasm_bindgen]
pub fn get_schedule_json(source: &str) -> JsValue {
    let project = parse(source).ok()?;
    let schedule = solve(&project).ok()?;
    let analysis = analyze_schedule(&project, &schedule);
    serde_wasm_bindgen::to_value(&analysis).unwrap()
}

/// Returns resource utilization by time period
#[wasm_bindgen]
pub fn get_resource_utilization(source: &str) -> JsValue {
    let project = parse(source).ok()?;
    let schedule = solve(&project).ok()?;
    let utilization = calculate_utilization(&project, &schedule);
    serde_wasm_bindgen::to_value(&utilization).unwrap()
}

/// Applies a structured edit and returns the modified source
#[wasm_bindgen]
pub fn apply_edit(source: &str, edit_json: &str) -> Result<String, JsValue> {
    let edit: Edit = serde_json::from_str(edit_json)?;
    let new_source = apply_structured_edit(source, &edit)?;
    Ok(new_source)
}

/// Analyzes impact of a hypothetical change
#[wasm_bindgen]
pub fn what_if(source: &str, change_json: &str) -> JsValue {
    let change: Change = serde_json::from_str(change_json).ok()?;
    let before = solve(&parse(source).ok()?).ok()?;
    let modified = apply_change(source, &change);
    let after = solve(&parse(&modified).ok()?).ok()?;
    let delta = compare_schedules(&before, &after);
    serde_wasm_bindgen::to_value(&delta).unwrap()
}
```

### 5.2 JavaScript Global API

```javascript
// playground/src/api.js

window.utf8proj = {
    // ─────────────────────────────────────────────────────────────
    // READ OPERATIONS
    // ─────────────────────────────────────────────────────────────

    /**
     * Get raw .proj source code
     * @returns {string} Current editor content
     */
    getSource: () => editor.getValue(),

    /**
     * Get parsed project structure
     * @returns {Object} {project, resources, tasks, calendars, errors}
     */
    getProjectJSON: () => {
        const source = editor.getValue();
        return JSON.parse(wasm.get_project_json(source));
    },

    /**
     * Get computed schedule with analysis
     * @returns {Object} {tasks: [{id, start, end, slack, critical}],
     *                    criticalPath: [id], projectEnd, projectDuration}
     */
    getScheduleJSON: () => {
        const source = editor.getValue();
        return JSON.parse(wasm.get_schedule_json(source));
    },

    /**
     * Get current diagnostics (errors, warnings, hints)
     * @returns {Array<{code, severity, message, line, col}>}
     */
    getDiagnostics: () => currentDiagnostics,

    /**
     * Get resource utilization by week
     * @returns {Object} {resourceId: [{week, allocated, capacity, overloaded}]}
     */
    getResourceUtilization: () => {
        const source = editor.getValue();
        return JSON.parse(wasm.get_resource_utilization(source));
    },

    // ─────────────────────────────────────────────────────────────
    // WRITE OPERATIONS
    // ─────────────────────────────────────────────────────────────

    /**
     * Replace entire source (triggers re-parse)
     * @param {string} source - New .proj content
     */
    setSource: (source) => {
        editor.setValue(source);
    },

    /**
     * Apply a structured edit
     * @param {Object} edit - {type: 'add_task'|'modify_task'|'delete_task'|..., ...params}
     * @returns {Object} {success, newSource, errors}
     */
    applyEdit: (edit) => {
        const source = editor.getValue();
        try {
            const newSource = wasm.apply_edit(source, JSON.stringify(edit));
            editor.setValue(newSource);
            return { success: true, newSource };
        } catch (e) {
            return { success: false, errors: [e.message] };
        }
    },

    // ─────────────────────────────────────────────────────────────
    // ANALYSIS OPERATIONS (AI-ORIENTED)
    // ─────────────────────────────────────────────────────────────

    /**
     * Get natural language explanation of a task
     * @param {string} taskId - Task identifier
     * @returns {string} Markdown explanation
     */
    explainTask: (taskId) => {
        const schedule = window.utf8proj.getScheduleJSON();
        const task = schedule.tasks.find(t => t.id === taskId);
        if (!task) return `Task '${taskId}' not found`;

        let explanation = `## ${task.name}\n\n`;
        explanation += `**Duration:** ${task.duration} days\n`;
        explanation += `**Dates:** ${task.start} → ${task.end}\n`;

        if (task.critical) {
            explanation += `\n⚠️ **On Critical Path** - delays here delay the project\n`;
        } else {
            explanation += `\n**Slack:** ${task.slack} days available\n`;
        }

        if (task.dependencies.length > 0) {
            explanation += `\n**Waiting for:** ${task.dependencies.join(', ')}\n`;
        }

        return explanation;
    },

    /**
     * Analyze impact of a hypothetical change
     * @param {Object} change - {type, taskId, field, value}
     * @returns {Object} {feasible, projectEndDelta, affectedTasks, warnings}
     */
    whatIf: (change) => {
        const source = editor.getValue();
        return JSON.parse(wasm.what_if(source, JSON.stringify(change)));
    },

    /**
     * Get AI-friendly suggestions for improvements
     * @returns {Array<{type, description, impact, edit}>}
     */
    suggestFixes: () => {
        const diagnostics = window.utf8proj.getDiagnostics();
        const utilization = window.utf8proj.getResourceUtilization();
        const suggestions = [];

        // Convert diagnostics to suggestions
        for (const d of diagnostics) {
            if (d.severity === 'warning') {
                suggestions.push({
                    type: 'fix_warning',
                    description: d.message,
                    impact: 'Resolves warning',
                    code: d.code
                });
            }
        }

        // Detect overallocations
        for (const [resourceId, weeks] of Object.entries(utilization)) {
            const overloaded = weeks.filter(w => w.overloaded);
            if (overloaded.length > 0) {
                suggestions.push({
                    type: 'resource_conflict',
                    description: `${resourceId} is overallocated in ${overloaded.length} weeks`,
                    impact: 'May cause schedule delays',
                    weeks: overloaded.map(w => w.week)
                });
            }
        }

        return suggestions;
    },

    // ─────────────────────────────────────────────────────────────
    // METADATA
    // ─────────────────────────────────────────────────────────────

    version: '0.3.0',
    capabilities: ['read', 'write', 'analyze', 'whatIf'],
    documentation: 'https://github.com/alanbld/utf8proj/blob/main/docs/API.md'
};
```

### 5.3 AI Discovery Manifest

```json
// playground/ai-manifest.json
{
    "$schema": "https://utf8proj.dev/schemas/ai-manifest-v1.json",
    "name": "utf8proj",
    "version": "0.3.0",
    "description": "Explainable project scheduling with AI-assisted editing",
    "homepage": "https://github.com/alanbld/utf8proj",

    "capabilities": {
        "read_project": {
            "description": "Parse and retrieve project structure",
            "methods": ["getSource", "getProjectJSON", "getScheduleJSON"]
        },
        "write_project": {
            "description": "Modify project content",
            "methods": ["setSource", "applyEdit"]
        },
        "analyze_schedule": {
            "description": "Get insights about scheduling",
            "methods": ["getDiagnostics", "getResourceUtilization", "explainTask"]
        },
        "what_if_analysis": {
            "description": "Analyze hypothetical changes",
            "methods": ["whatIf", "suggestFixes"]
        }
    },

    "api": {
        "global": "window.utf8proj",
        "schema": "https://utf8proj.dev/schemas/api-v1.json"
    },

    "examples": {
        "add_task": {
            "description": "Add a new task after an existing one",
            "code": "utf8proj.applyEdit({type: 'add_task', after: 'design', task: {id: 'review', name: 'Design Review', duration: '2d'}})"
        },
        "check_impact": {
            "description": "Check impact of extending a task",
            "code": "utf8proj.whatIf({type: 'modify', taskId: 'design', field: 'duration', value: '10d'})"
        }
    },

    "prompting_hints": {
        "context": "User is editing a project schedule in .proj format",
        "terminology": {
            "task": "A unit of work with duration and dependencies",
            "resource": "A person or team assigned to tasks",
            "critical_path": "Tasks that determine project end date",
            "slack": "How much a task can slip without affecting project end"
        },
        "common_requests": [
            "Add a task",
            "Change duration",
            "Add dependency",
            "Check who is overloaded",
            "Why is the project late",
            "What if we add more people"
        ]
    }
}
```

### 5.4 Structured Edit Types

```typescript
// Edit operation types for applyEdit()

type Edit =
    | AddTaskEdit
    | ModifyTaskEdit
    | DeleteTaskEdit
    | AddResourceEdit
    | ModifyResourceEdit
    | AddDependencyEdit
    | RemoveDependencyEdit
    | AssignResourceEdit
    | UnassignResourceEdit;

interface AddTaskEdit {
    type: 'add_task';
    parentId?: string;      // null for top-level
    afterId?: string;       // insert position
    task: {
        id: string;
        name: string;
        duration?: string;  // e.g., "5d"
        effort?: string;    // e.g., "10d"
        depends?: string[]; // task IDs
        assign?: string[];  // resource IDs
    };
}

interface ModifyTaskEdit {
    type: 'modify_task';
    taskId: string;
    changes: {
        name?: string;
        duration?: string;
        effort?: string;
        complete?: string;  // e.g., "75%"
        priority?: number;
    };
}

interface DeleteTaskEdit {
    type: 'delete_task';
    taskId: string;
    cascade?: boolean;  // delete children too
}

interface AddDependencyEdit {
    type: 'add_dependency';
    taskId: string;
    dependsOn: string;
    dependencyType?: 'FS' | 'SS' | 'FF' | 'SF';
    lag?: string;       // e.g., "+2d"
}

// ... additional edit types
```

---

## 6. Challenges & Mitigations

### 6.1 Technical Challenges

| Challenge | Impact | Mitigation |
|-----------|--------|------------|
| **Re-parsing on every request** | Performance degradation with large projects | Cache parsed AST in JavaScript; invalidate on edit |
| **WASM bundle size** | Slow initial load | Code splitting; lazy-load analysis functions |
| **Monaco position mapping** | Off-by-one errors | Consistent 1-based indexing; comprehensive tests |
| **Hover logic duplication** | Maintenance burden | Extract shared `utf8proj-hover` crate used by both LSP and WASM |
| **State synchronization** | Editor vs WASM state drift | Single source of truth in editor; WASM is stateless |

### 6.2 Security Challenges

| Challenge | Impact | Mitigation |
|-----------|--------|------------|
| **AI plugin injection** | Malicious plugins could exfiltrate data | Clear documentation that API is read/write; users choose plugins |
| **XSS via AI-generated content** | Injected scripts in suggestions | Sanitize all AI output; render as text, not HTML |
| **Denial of service** | Expensive `whatIf` calls | Rate limiting; timeout long computations |

### 6.3 UX Challenges

| Challenge | Impact | Mitigation |
|-----------|--------|------------|
| **AI hallucination** | Incorrect schedule modifications | Always show diff before applying; require confirmation |
| **API complexity** | Developers confused by many methods | Tiered API: simple (3 methods) vs advanced (full API) |
| **Error messages** | Cryptic errors frustrate users | Human-readable errors with suggested fixes |

---

## 7. Opportunities

### 7.1 Market Differentiation

| Opportunity | Value |
|-------------|-------|
| **"First AI-native scheduler"** | Marketing differentiation; attract early adopters |
| **Zero-install onboarding** | Lower barrier → more users → more contributors |
| **Plugin ecosystem** | Third-party AI integrations expand reach |

### 7.2 Technical Opportunities

| Opportunity | Value |
|-------------|-------|
| **Shared hover/completion logic** | Cleaner codebase; LSP and WASM use same core |
| **API-first architecture** | Enables future integrations (CI/CD, Slack bots, etc.) |
| **WASM performance data** | Real-world benchmarks for optimization |

### 7.3 User Experience Opportunities

| Opportunity | Value |
|-------------|-------|
| **Natural language editing** | Non-technical users can manage schedules |
| **Proactive suggestions** | AI catches issues before humans notice |
| **Learning by example** | AI explains *why* schedules work the way they do |

### 7.4 Ecosystem Opportunities

| Opportunity | Value |
|-------------|-------|
| **VS Code extension** | Same WASM in browser extension (future) |
| **GitHub Actions integration** | AI-assisted PR reviews for .proj changes |
| **Slack/Teams bot** | "What's the critical path?" in chat |

---

## 8. Innovation Assessment

### 8.1 Novelty Analysis

| Aspect | Industry State | utf8proj Innovation |
|--------|----------------|---------------------|
| **Browser scheduling** | Few tools (Gantt charts only) | Full CPM solver in WASM |
| **AI + project management** | Surface-level (task creation) | Deep schedule analysis, what-if |
| **LSP in browser** | Rare (Monaco + language server over WS) | Direct WASM integration |
| **AI manifest standard** | Emerging (OpenAI plugins) | Early adopter, scheduling-specific |

### 8.2 Competitive Analysis

| Competitor | AI Features | Our Advantage |
|------------|-------------|---------------|
| **MS Project + Copilot** | Generic Office AI | Domain-specific scheduling knowledge |
| **Monday.com AI** | Task suggestions | Full CPM/critical path analysis |
| **Notion AI** | Writing assistant | Structured project model |
| **Linear** | Issue triage | No scheduling/dependencies |

### 8.3 Innovation Value Score

| Criterion | Score (1-5) | Rationale |
|-----------|-------------|-----------|
| **Technical novelty** | 4 | WASM + Monaco + AI integration uncommon |
| **Market timing** | 5 | AI assistants mainstream; scheduling tools behind |
| **Barrier to copy** | 3 | Architecture is replicable; execution matters |
| **User value** | 5 | Transforms scheduling from expert to accessible |
| **Ecosystem potential** | 4 | API enables third-party innovation |

**Overall Innovation Score: 4.2/5** - High innovation value with strong market timing.

### 8.4 Research/Publication Potential

| Venue | Topic |
|-------|-------|
| **CHI/UIST** | AI-assisted project scheduling UX study |
| **ICSE/ASE** | WASM-based language services architecture |
| **Blog/Dev.to** | "Building AI-native developer tools" |

---

## 9. Implementation Phases

### Phase 0: Feature Parity (Prerequisite)

**Goal:** Sync WASM playground with current CLI capabilities before adding AI features.

**Rationale:** AI features need accurate, complete data to function correctly. The playground currently lacks several features that are in the CLI, which would cause AI suggestions to be incomplete or incorrect.

| Task | Scope | Effort |
|------|-------|--------|
| Add missing syntax keywords | `playground/src/main.js` | 1h |
| Add `status_date` to ProjectInfo output | `utf8proj-wasm/src/lib.rs` | 1h |
| Add `explicit_remaining` to TaskInfo output | `utf8proj-wasm/src/lib.rs` | 1h |
| Add more example templates (hierarchical, progress) | `utf8proj-wasm` + `main.js` | 2h |
| Expose focus view options (`--focus`, `--context-depth`) | `playground/` + WASM | 3h |
| Add export format dropdown (HTML, Mermaid, PlantUML) | `playground/` + WASM | 3h |
| Rebuild WASM package and test | `playground/build.sh` | 1h |

**Missing Syntax Keywords to Add:**
```javascript
// Add to nativeDslLanguage.keywords in main.js:
'remaining', 'status_date', 'profile', 'trait', 'specializes',
'skills', 'tag', 'note', 'cost', 'payment', 'leave', 'role', 'email'
```

**Gap Analysis (as of 2026-01-15):**

| Feature | CLI | Playground | Gap |
|---------|-----|------------|-----|
| Progress-aware CPM (RFC-0008) | ✅ | ❌ | `status_date` not exposed |
| Focus view (RFC-0006) | ✅ | ❌ | No `--focus` option |
| Explicit remaining duration | ✅ | ❌ | `remaining:` field missing |
| Multiple export formats | ✅ | Partial | Only HTML Gantt |
| Dependency types (SS/FF/SF) | ✅ | ✅ | OK |
| Resource leveling | ✅ | ✅ | OK |
| Calendar impact | ✅ | ✅ | OK |

**Deliverable:** Playground feature-complete with CLI

**Acceptance Criteria:**
- [ ] All syntax keywords highlighted
- [ ] `status_date` visible in project info
- [ ] `remaining` visible in task info
- [ ] 4+ example templates available
- [ ] Focus view working for HTML output
- [ ] Export dropdown with 3 formats

---

### Phase 1: Browser LSP Features (Week 1-2)

**Goal:** Hover and completions working in playground

| Task | Effort | Owner |
|------|--------|-------|
| Extract hover logic to shared crate | 4h | - |
| Add `get_hover()` WASM export | 2h | - |
| Register Monaco HoverProvider | 2h | - |
| Add `get_completions()` WASM export | 4h | - |
| Register Monaco CompletionProvider | 2h | - |
| Tests for position mapping | 2h | - |

**Deliverable:** Playground with hover and autocomplete

### Phase 2: JavaScript API (Week 2-3)

**Goal:** `window.utf8proj` API complete

| Task | Effort | Owner |
|------|--------|-------|
| Implement read operations | 3h | - |
| Implement write operations | 4h | - |
| Implement analysis operations | 4h | - |
| Create API documentation | 2h | - |
| Add TypeScript definitions | 2h | - |

**Deliverable:** Documented API for external tools

### Phase 3: AI Integration Points (Week 3-4)

**Goal:** AI plugins can interact with playground

| Task | Effort | Owner |
|------|--------|-------|
| Create ai-manifest.json | 2h | - |
| Implement structured edits | 6h | - |
| Add what-if analysis | 4h | - |
| Security review | 2h | - |
| Integration testing with sample AI | 4h | - |

**Deliverable:** AI-ready playground

### Phase 4: GitHub Pages Deployment (Week 4)

**Goal:** Public playground at `alanbld.github.io/utf8proj`

| Task | Effort | Owner |
|------|--------|-------|
| Create deployment workflow | 2h | - |
| Add caching/CDN configuration | 2h | - |
| Create landing page | 2h | - |
| Update documentation | 2h | - |

**Deliverable:** Public zero-install playground

---

## 10. Success Criteria

### 10.1 Technical Metrics

| Metric | Target |
|--------|--------|
| Hover response time | < 50ms |
| Completion response time | < 100ms |
| WASM bundle size | < 2MB (gzipped) |
| Time to interactive | < 3s on 3G |

### 10.2 User Metrics

| Metric | Target |
|--------|--------|
| Playground monthly active users | 100+ (3 months post-launch) |
| API integrations | 3+ third-party tools |
| User-reported AI success rate | > 80% (edits applied correctly) |

### 10.3 Quality Metrics

| Metric | Target |
|--------|--------|
| API test coverage | > 90% |
| Zero critical bugs in production | 3 months |
| Documentation completeness | All public APIs documented |

---

## 11. Open Questions

| Question | Options | Decision Needed By |
|----------|---------|-------------------|
| **Cache strategy** | Per-edit vs debounced vs manual | Phase 1 |
| **Edit confirmation UX** | Diff view vs inline preview vs modal | Phase 3 |
| **AI manifest format** | Custom vs OpenAI-compatible vs both | Phase 3 |
| **Offline support** | None vs ServiceWorker vs PWA | Post-v1 |

---

## 12. References

* [Monaco Editor API](https://microsoft.github.io/monaco-editor/api/index.html)
* [wasm-bindgen Guide](https://rustwasm.github.io/wasm-bindgen/)
* [OpenAI Plugin Manifest](https://platform.openai.com/docs/plugins/getting-started/plugin-manifest)
* [utf8proj-lsp hover implementation](../crates/utf8proj-lsp/src/hover.rs)
* [RFC-0001: Progressive Resource Refinement](./RFC-0001-PROGRESSIVE-RESOURCE-REFINEMENT.md)

---

## 13. Appendix: Sample AI Interaction

```
USER: "The project is showing as 3 weeks late. What's causing this?"

AI PLUGIN:
1. Calls: utf8proj.getScheduleJSON()
2. Analyzes: criticalPath = ['design', 'backend', 'integration', 'testing']
3. Calls: utf8proj.getResourceUtilization()
4. Finds: 'alice' overallocated weeks 3-5

AI RESPONSE:
"The project end date is March 15 instead of the target February 22
(3 weeks late).

The critical path runs through: Design → Backend → Integration → Testing

The main bottleneck is Alice, who is assigned to both Backend and
Integration tasks during weeks 3-5, creating a resource conflict.

**Suggestions:**
1. Assign Bob to Integration (saves ~1 week)
2. Split Backend into parallel tracks (saves ~1 week)
3. Accept the delay and adjust expectations

Would you like me to model any of these options?"

USER: "Try option 1"

AI PLUGIN:
1. Calls: utf8proj.whatIf({type: 'reassign', taskId: 'integration',
                          from: 'alice', to: 'bob'})
2. Returns: {projectEndDelta: -5, newEnd: '2026-03-08', feasible: true}

AI RESPONSE:
"Reassigning Integration to Bob would:
- Move project end from March 15 → March 8 (saves 5 days)
- Bob's utilization increases from 60% to 85%
- Alice drops from 120% (overloaded) to 80%

Apply this change?"

USER: "Yes"

AI PLUGIN:
1. Calls: utf8proj.applyEdit({type: 'reassign', taskId: 'integration',
                             from: 'alice', to: 'bob'})
2. Editor updates with new source

AI RESPONSE:
"Done. Integration is now assigned to Bob. The project end date is
March 8. You've recovered 1 week of the 3-week delay."
```

---

**Document Version:** 1.0
**Last Updated:** 2026-01-14
**Status:** Draft - Ready for review
