# utf8proj v1.0 Release Checklist

**Target:** Production-ready release with API stability commitment
**Current:** v0.2.0
**Last Updated:** 2026-01-16

---

## Overview

| Category | Progress | Status |
|----------|----------|--------|
| Core Features | 95% | ✅ Ready |
| Playground | 85% | 🔄 Near ready |
| Documentation | 60% | 🔄 In progress |
| Testing | 86% | 🔄 Near ready |
| Polish | 70% | 🔄 In progress |

---

## 1. Core Features (✅ Complete)

All core scheduling features are implemented and tested:

- [x] CPM scheduling with all dependency types (FS, SS, FF, SF)
- [x] Effort-driven scheduling (PMI-compliant)
- [x] Resource leveling (RFC-0003)
- [x] Progress-aware scheduling (RFC-0008)
- [x] Hierarchical tasks with container derivation
- [x] Calendar support (working days, hours, holidays)
- [x] Multiple render formats (HTML, SVG, Mermaid, PlantUML, Excel)
- [x] Diagnostic system (compiler-grade error messages)
- [x] LSP (Language Server Protocol) for IDE support

---

## 2. Playground Enhancements

### 2.1 Completed
- [x] WASM module with scheduling
- [x] Monaco editor with syntax highlighting
- [x] Gantt chart preview
- [x] JSON output view
- [x] Example templates
- [x] Share via URL
- [x] Light/dark theme
- [x] Focus view (RFC-0006)
- [x] Version display in footer
- [x] Syntax reference page
- [x] GitHub link

### 2.2 Pending for v1.0
- [ ] **Excel export in browser** (rust_xlsxwriter has WASM support)
  - Add `wasm` feature to rust_xlsxwriter dependency
  - Expose `render_xlsx()` in Playground struct
  - Add download button for .xlsx files
  - Estimated: 2-3 hours

- [ ] **Mermaid/PlantUML export buttons**
  - Already implemented in WASM, just need UI buttons
  - Estimated: 30 minutes

- [ ] **Keyboard shortcuts help overlay**
  - Ctrl+Enter = Run (already works)
  - Document other shortcuts
  - Estimated: 1 hour

---

## 3. Documentation

### 3.1 Completed
- [x] CLAUDE.md (comprehensive codebase guide)
- [x] QUICK_REFERENCE.md
- [x] DIAGNOSTICS.md
- [x] EDITOR_SETUP.md
- [x] Syntax reference (playground/syntax.html)
- [x] RFC documents (0001-0008)

### 3.2 Pending for v1.0
- [ ] **README.md polish**
  - Update feature list
  - Add playground link prominently
  - Improve getting started section
  - Estimated: 1 hour

- [ ] **User tutorial**
  - Step-by-step guide for first project
  - Cover: basic tasks, dependencies, resources, calendars
  - Estimated: 2 hours

- [ ] **API documentation**
  - `cargo doc` with examples
  - Ensure all public types documented
  - Estimated: 2 hours

- [ ] **CHANGELOG.md**
  - Document all changes since v0.1.0
  - Follow Keep a Changelog format
  - Estimated: 1 hour

---

## 4. Testing

### 4.1 Current Coverage
| Module | Coverage | Target |
|--------|----------|--------|
| utf8proj-core | 91.9% | 90%+ ✅ |
| utf8proj-parser | 96.6% | 90%+ ✅ |
| utf8proj-solver | 99.0% | 90%+ ✅ |
| utf8proj-render | 97.5% | 90%+ ✅ |
| utf8proj-wasm | 91.4% | 90%+ ✅ |
| utf8proj-lsp | 90.3% | 90%+ ✅ |
| utf8proj-cli | 42.5% | 60%+ ❌ |
| **Overall** | ~86% | 85%+ ✅ |

### 4.2 Pending for v1.0
- [ ] **CLI test coverage to 60%+**
  - Add tests for `gantt` command variations
  - Add tests for `schedule` output formats
  - Add tests for error handling paths
  - Estimated: 3-4 hours

- [ ] **Integration tests**
  - End-to-end tests with real project files
  - Test all CLI commands with examples/
  - Estimated: 2 hours

- [ ] **WASM browser tests**
  - Run wasm-pack test in browser
  - Test all Playground methods
  - Estimated: 1 hour

---

## 5. Polish & Quality

### 5.1 Completed
- [x] Compiler-grade diagnostics
- [x] Exit code contract (0 = success, 1 = error)
- [x] JSON output format
- [x] Cross-platform release binaries
- [x] GitHub Actions CI/CD

### 5.2 Pending for v1.0
- [ ] **Version consistency check**
  - Ensure all Cargo.toml use workspace version
  - ✅ Fixed: utf8proj-wasm now uses workspace version
  - Verify on each release

- [ ] **Remove dead code warnings**
  - serialize_project/serialize_task now unused
  - Either remove or add `#[allow(dead_code)]` with justification
  - Estimated: 30 minutes

- [ ] **Performance benchmarks**
  - Document scheduling performance (tasks/second)
  - Target: 10,000 tasks in <1 second
  - Estimated: 1 hour

- [ ] **Error message review**
  - Ensure all errors are actionable
  - Check grammar and clarity
  - Estimated: 1 hour

---

## 6. Release Process

### 6.1 Pre-release Checklist
- [ ] All tests passing (`cargo test --workspace`)
- [ ] No compiler warnings (`cargo clippy --workspace`)
- [ ] Documentation builds (`cargo doc --workspace`)
- [ ] WASM builds (`cd playground && ./build.sh`)
- [ ] Playground deploys successfully
- [ ] Version bumped in workspace Cargo.toml
- [ ] CHANGELOG.md updated
- [ ] Git tag created (v1.0.0)

### 6.2 Release Artifacts
- [ ] GitHub Release with binaries (automated via release.yml)
- [ ] Playground deployed to GitHub Pages
- [ ] crates.io publish (optional for v1.0)

---

## 7. Post-v1.0 Backlog

Features explicitly deferred to post-v1.0:

| Feature | RFC | Notes |
|---------|-----|-------|
| Resource Leveling Phase 2 | RFC-0005 | Advanced algorithms, deferred pending demand |
| BDD Conflict Analysis | - | Experimental, needs more testing |
| Monte Carlo Simulation | - | Future RFC |
| Baseline Comparison | - | Future RFC |
| Multi-project Portfolios | - | Future RFC |
| Cost Tracking (EVM) | - | Future RFC |

---

## Estimated Total Effort

| Category | Hours |
|----------|-------|
| Playground (Excel export, buttons) | 3-4 |
| Documentation | 6 |
| CLI Testing | 4 |
| Polish | 3 |
| **Total** | ~16-17 hours |

---

## Decision Log

| Date | Decision | Rationale |
|------|----------|-----------|
| 2026-01-16 | Excel WASM export is feasible | rust_xlsxwriter has `wasm` feature |
| 2026-01-16 | Target 60% CLI coverage (not 80%) | CLI is thin wrapper, core logic well tested |
| 2026-01-16 | Defer crates.io publish | Can publish post-v1.0 after stability proven |

---

## Success Criteria for v1.0

1. ✅ All core features working and tested
2. ⏳ CLI coverage ≥ 60%
3. ⏳ Complete user documentation
4. ⏳ Playground with all export formats
5. ✅ Real-world validation (TTG, Iren TCR projects)
6. ⏳ CHANGELOG documenting all changes
7. ✅ Cross-platform release binaries

**When all items checked, tag v1.0.0!**
