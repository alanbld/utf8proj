# RFC-0015: Benchmarking and Real-World Validation

**RFC Number:** 0015
**Status:** Draft
**Created:** 2026-01-26
**Author:** utf8proj contributors
**Related:** RFC-0003 (Resource Leveling), RFC-0008 (Progress-Aware CPM), RFC-0013 (Baseline Management), RFC-0014 (Scaling Resource Leveling)
**Target Version:** 0.13.0

---

## Executive Summary

This RFC establishes a **formal benchmarking and validation strategy** for utf8proj, treating benchmark datasets as first-class architectural artifacts rather than mere test inputs.

**Core insight:** utf8proj is evolving from a CPM solver into a **project control and forecasting engine**. Validating such an engine requires not just algorithmic correctness tests, but validation against real-world project execution data.

**Key decisions:**
- Three-tier benchmark architecture: Synthetic, Scaled, Real-World
- DSLIB (200+ real industrial projects) as architecturally critical dataset
- Benchmark contracts specifying required/optional entities
- Tolerance-based acceptance for real-world data (divergence is signal, not error)
- Dedicated benchmark tooling as core infrastructure

---

## 1. Problem Statement

### 1.1 Current State

utf8proj's current benchmark coverage:

| Dataset | Status | Tasks/Instance | What It Validates |
|---------|--------|----------------|-------------------|
| PSPLIB J30 | Implemented | 30 | Basic RCPSP correctness |
| Synthetic chain/diamond | Implemented | 100-10,000+ | Scalability |
| TaskJuggler comparison | Implemented | Variable | Compatibility |

**What's missing:**
- Large-scale RCPSP (J60, J120, RG300)
- Multi-skill scheduling (iMOPSE)
- **Real-world execution data** (baseline vs actual)
- Progress-aware recomputation validation
- Forecasting accuracy measurement

### 1.2 Why This Matters

utf8proj now supports:
- Immutable baselines (RFC-0013)
- Progress-aware scheduling (RFC-0008)
- Variance analysis (baseline vs current)

These features exist to answer: *"Is the project on track?"*

But we cannot validate the **semantic correctness** of those answers without real projects that have:
- Original plans (baselines)
- Actual execution data
- Known outcomes

### 1.3 The DSLIB Opportunity

The Ghent University OR&S **DSLIB** dataset contains **200+ real industrial projects** with:
- Baseline schedules (planned networks, resources)
- Actual execution data (real start/finish dates)
- Risk analysis data (Monte-Carlo simulation inputs)
- Project control metrics

**This is not just another benchmark. It is the validation corpus for utf8proj's project control capabilities.**

---

## 2. Benchmarking Goals

### 2.1 Properties Under Validation

| Property | Definition | Benchmark Source |
|----------|------------|------------------|
| **Algorithmic Correctness** | CPM/RCPSP produces optimal or near-optimal makespan | PSPLIB (known optima) |
| **Scalability** | Performance degrades gracefully with size | Synthetic (chain, diamond) |
| **Leveling Quality** | Resource conflicts resolved with minimal delay | RG300, iMOPSE |
| **Semantic Validity** | Schedule interpretation matches real-world execution | DSLIB |
| **Forecast Accuracy** | Variance predictions correlate with outcomes | DSLIB (actuals) |

### 2.2 What We Are NOT Validating

- **User interface usability** (not a benchmark concern)
- **Parser performance** (tested separately)
- **Render output quality** (visual inspection)

---

## 3. Benchmark Tiers

### Tier 1: Synthetic (Algorithmic Correctness)

**Purpose:** Prove the solver produces correct CPM/RCPSP solutions.

**Datasets:**
- PSPLIB J30, J60, J90, J120 (480-600 instances each)
- RG30/RG300 (Ghent University)

**Acceptance criteria:**
- Makespan ≤ known optimal (for instances with proven optima)
- Makespan within 5% of best-known solution (for open instances)
- Deterministic: identical inputs → identical outputs

**Why this tier exists:** These are controlled experiments with known answers. If utf8proj fails here, the core algorithm is broken.

### Tier 2: Scaled (Stress Testing)

**Purpose:** Prove the engine handles real-world project sizes.

**Datasets:**
- Synthetic chain: 1,000 → 10,000 → 50,000 tasks
- Synthetic diamond: deep parallelism, resource contention
- iMOPSE: multi-skill scheduling (100-200 tasks with skills)

**Acceptance criteria:**
- J30-sized problems: <100ms
- J120-sized problems: <500ms
- 1,000 tasks: <2s
- 10,000 tasks: <30s
- Memory: O(n) for n tasks (no exponential blowup)

**Why this tier exists:** Production projects range from 50 tasks (small teams) to 10,000+ (mega-projects). We must prove scaling behavior.

### Tier 3: Real-World (Semantic Validity)

**Purpose:** Prove utf8proj's schedule interpretations match how real projects actually execute.

**Datasets:**
- **DSLIB** (Ghent University) — 200+ industrial projects
- Future: User-contributed anonymized projects

**Acceptance criteria:** See Section 5.3 (fundamentally different from Tiers 1-2).

**Why this tier exists:** A mathematically optimal schedule is useless if it doesn't reflect how humans actually execute projects. Real-world data is the ultimate test.

---

## 4. Why DSLIB Cannot Be Treated Like PSPLIB

| Aspect | PSPLIB | DSLIB |
|--------|--------|-------|
| **Origin** | Generated by ProGen algorithm | Real companies, real projects |
| **Data type** | Precedence + resource constraints | Network + resources + actuals + risk |
| **Ground truth** | Optimal/best-known makespan | Actual execution history |
| **Validation question** | "Is the schedule optimal?" | "Does the schedule reflect reality?" |
| **Success metric** | Gap to optimal | Correlation with actuals |
| **Noise** | None (synthetic) | Human behavior, scope changes, delays |

### 4.1 Architectural Implications

DSLIB projects will likely **break assumptions** that work for PSPLIB:

1. **Non-deterministic outcomes:** Real projects have actual dates that diverge from any computed schedule.
2. **Scope changes:** Tasks added/removed mid-project.
3. **Progress asymmetry:** Some tasks finish early, others late — patterns may not be random.
4. **Calendar gaps:** Holidays, leaves, unplanned downtime.
5. **Resource substitution:** Different resources than originally planned.

**utf8proj must interpret divergence as signal, not error.**

### 4.2 What DSLIB Validates

- Baseline management (RFC-0013): Can we correctly snapshot and compare schedules?
- Progress-aware CPM (RFC-0008): Does recomputation from actuals produce sensible forecasts?
- Variance analysis: Do our variance calculations match project managers' intuitions?
- Forecasting: Does remaining-duration estimation correlate with actual outcomes?

---

## 5. Canonical Benchmark Contract

### 5.1 Required Entities (All Tiers)

Every benchmark instance **MUST** define:

```
tasks: [
  {
    id: string (unique),
    duration: integer (days or hours),
    predecessors: [task_id, ...]
  }
]
```

### 5.2 Required for Resource-Constrained (Tiers 1-2)

```
resources: [
  {
    id: string,
    capacity: integer (units available per period)
  }
]

task_resources: [
  {
    task_id: string,
    resource_id: string,
    demand: integer (units required)
  }
]
```

### 5.3 Required for Real-World Validation (Tier 3)

```
baseline: {
  saved: datetime,
  tasks: [
    {
      id: string,
      planned_start: date,
      planned_finish: date
    }
  ]
}

actuals: [
  {
    task_id: string,
    actual_start: date | null,
    actual_finish: date | null,
    percent_complete: number (0-100)
  }
]
```

### 5.4 Optional (Forward-Compatible)

These fields are optional but the system must handle their presence gracefully:

| Field | Tier | Purpose |
|-------|------|---------|
| `skills` | 2+ | Multi-skill RCPSP |
| `calendars` | 3 | Working days, holidays |
| `risk_distributions` | 3 | Monte-Carlo simulation |
| `cost` | 3 | Earned value analysis |
| `resource_efficiency` | 3 | Productivity factors |
| `constraints` | All | Must-start-on, deadlines |

---

## 6. Acceptance Criteria

### 6.1 Tier 1: Synthetic (Pass/Fail)

```
PASS if:
  - For instances with proven optimum: makespan == optimum
  - For open instances: makespan <= best_known * 1.05
  - All precedence constraints satisfied
  - All resource constraints satisfied
  - Output is deterministic

FAIL if:
  - Any constraint violated
  - Makespan exceeds threshold
  - Non-deterministic behavior detected
```

### 6.2 Tier 2: Scaled (Performance Bounds)

```
PASS if:
  - Makespan is feasible (all constraints satisfied)
  - Wall-clock time <= threshold for instance size
  - Memory usage <= O(n) for n tasks
  - No timeouts or crashes

WARN if:
  - Time exceeds soft threshold but meets hard threshold
  - Memory usage higher than expected
```

### 6.3 Tier 3: Real-World (Correlation & Tolerance)

**Fundamentally different acceptance model:**

```
For each project with baseline + actuals:

1. STRUCTURAL VALIDITY:
   PASS if:
     - All baseline tasks can be parsed
     - All actual dates can be ingested
     - Variance calculation completes without error

2. SEMANTIC CORRELATION:
   MEASURE:
     - Pearson correlation between:
       - baseline_finish_variance (baseline vs computed)
       - actual_finish_variance (baseline vs actual)

   ACCEPTABLE if correlation > 0.6
   GOOD if correlation > 0.8

3. FORECAST ACCURACY (for in-progress projects):
   MEASURE:
     - Mean Absolute Error (MAE) of:
       - predicted_finish vs actual_finish (for tasks with actuals)

   Track MAE distribution across projects.
   No hard threshold — goal is trend improvement.

4. DIVERGENCE ANALYSIS:
   For each project, produce:
     - Tasks where utf8proj diverges significantly from actuals
     - Categorize: calendar issue, resource issue, scope change, other

   Divergence is OUTPUT, not failure.
```

### 6.4 When Divergence Is Acceptable

| Scenario | Acceptable? | Rationale |
|----------|-------------|-----------|
| Computed finish differs from actual by 1-2 days | Yes | Calendar rounding, human variation |
| Computed finish differs from actual by >20% | Flag for analysis | May indicate missing constraint |
| Task in baseline not in actuals | Yes | Scope reduction |
| Task in actuals not in baseline | Yes | Scope addition |
| All tasks early by similar amount | Investigate | Systematic calendar issue? |
| All tasks late by similar amount | Investigate | Missing holidays? Resource issue? |

---

## 7. Baseline & Progress Validation

### 7.1 Invariants That Must Never Be Violated

1. **Baseline immutability:** A baseline snapshot, once created, must produce identical comparison results forever (given the same current schedule).

2. **Variance arithmetic:**
   ```
   variance_days = current_early_finish - baseline_early_finish
   ```
   Must be exact (no floating-point drift).

3. **Task identity stability:** Task IDs in baselines must match task IDs in current schedule using the same qualification rules.

4. **Progress monotonicity:** A task's `percent_complete` can only increase (within a single project timeline).

### 7.2 How Actuals Affect Computed Schedules

Given DSLIB actual data, utf8proj should:

1. **Anchor completed tasks:** Tasks with `actual_finish` are locked; CPM does not recompute them.

2. **Adjust in-progress tasks:** Tasks with `actual_start` but no `actual_finish` use:
   ```
   remaining_duration = original_duration * (1 - percent_complete)
   effective_start = max(actual_start, today)
   ```

3. **Propagate to successors:** Successors of completed/in-progress tasks are rescheduled based on actual dates.

4. **Recompute critical path:** Critical path may shift based on actual execution.

### 7.3 Validation Tests from DSLIB

For each DSLIB project with actuals:

```
TEST 1: Baseline round-trip
  - Parse baseline data
  - Export to .proj.baselines format
  - Re-import
  - Assert: identical baseline data

TEST 2: Variance calculation
  - Load baseline
  - Load actuals as current progress
  - Compute variance
  - Assert: variance matches manual calculation

TEST 3: Forecast from midpoint
  - Load project state at 50% completion
  - Run progress-aware CPM
  - Compare predicted finish to actual finish
  - Record prediction error

TEST 4: Critical path stability
  - Verify critical path at project start
  - Re-verify at 25%, 50%, 75% completion
  - Track critical path changes
```

---

## 8. DSLIB-Specific Considerations

### 8.1 Architectural Assumptions Likely to Break

| Assumption | Risk | Mitigation |
|------------|------|------------|
| All tasks have explicit IDs | DSLIB may use positional IDs | ID inference with warnings |
| Durations are in working days | DSLIB may use calendar days | Calendar mode flag |
| Resources have fixed capacity | DSLIB may have variable availability | Extend resource model (future) |
| No mid-project scope changes | DSLIB has added/removed tasks | Handle gracefully, report as B005/B006 |
| Calendars are known | DSLIB may omit calendar details | Assume 5-day week default |

### 8.2 Minimal DSLIB Subset (Phase 1)

For initial integration, support projects that have:

1. **Required:**
   - Task network (precedence graph)
   - Planned dates (baseline)
   - Actual dates (at least project-level)

2. **Deferred:**
   - Resource constraints (Phase 2)
   - Risk distributions (Phase 3)
   - Cost data (Phase 3)

**Estimated coverage:** ~80% of DSLIB projects should be parseable with Phase 1 support.

### 8.3 Success Metrics for DSLIB

| Metric | Target |
|--------|--------|
| Parse success rate | ≥95% of DSLIB projects |
| Baseline extraction rate | ≥90% of parsed projects |
| Variance calculation rate | 100% of projects with baseline + actuals |
| Correlation with actuals | ≥0.6 for forecast accuracy |
| Documented divergences | All significant divergences explained |

---

## 9. Tooling Architecture

### 9.1 Format Converters

**Decision:** Format converters are **core infrastructure**, not optional test utilities.

```
tools/
├── psplib_to_proj/      # PSPLIB → .proj converter
│   ├── src/
│   │   └── main.rs      # Rust implementation for speed
│   └── README.md
├── dslib_to_proj/       # DSLIB → .proj + .baselines converter
│   ├── src/
│   │   └── main.rs
│   └── README.md
├── imopse_to_proj/      # iMOPSE → .proj converter
│   └── ...
└── benchmark_runner/    # Unified benchmark orchestration
    ├── src/
    │   └── main.rs
    └── README.md
```

### 9.2 Converter Requirements

Each converter must:

1. **Parse source format completely** (no silent data loss)
2. **Emit valid .proj syntax** (parseable by utf8proj)
3. **Preserve all supported attributes**
4. **Emit warnings for unsupported attributes**
5. **Generate baseline files** (for datasets with planned dates)
6. **Generate actuals overlay** (for datasets with execution data)

### 9.3 Benchmark Runner

```bash
# Run all Tier 1 benchmarks
utf8proj-benchmark --tier 1

# Run specific dataset
utf8proj-benchmark --dataset psplib-j120

# Run DSLIB validation suite
utf8proj-benchmark --tier 3 --report dslib-validation.json

# Performance profiling mode
utf8proj-benchmark --tier 2 --profile --output flamegraph.svg
```

### 9.4 Versioning and Regression Testing

```yaml
# benchmarks/manifest.yaml
datasets:
  psplib-j30:
    source: https://www.om-db.wi.tum.de/psplib/data/...
    version: "1.0"
    instances: 480
    expected_results: benchmarks/psplib-j30-expected.json

  dslib:
    source: https://www.projectmanagement.ugent.be/...
    version: "2024-12"  # Dataset version date
    instances: 203
    # No expected_results — correlation-based validation
```

**CI Integration:**
```yaml
# .github/workflows/benchmarks.yml
benchmark-regression:
  runs-on: ubuntu-latest
  steps:
    - name: Run Tier 1 benchmarks
      run: utf8proj-benchmark --tier 1 --assert-no-regression

    - name: Run Tier 2 benchmarks (weekly)
      if: github.event.schedule
      run: utf8proj-benchmark --tier 2 --report artifacts/perf.json
```

---

## 10. Benchmark Governance

### 10.1 Ownership and Maintenance

| Dataset | Owner | Update Frequency |
|---------|-------|------------------|
| PSPLIB J30/J60/J120 | Core team | Static (upstream frozen) |
| Synthetic generators | Core team | As needed |
| iMOPSE | Core team | Static (upstream frozen) |
| DSLIB | Core team | Annual (track upstream) |
| User-contributed | Contributors | On submission |

### 10.2 Versioning Expected Results

Expected results are versioned alongside the codebase:

```
benchmarks/
├── expected/
│   ├── psplib-j30-v1.json      # Version 1 baseline
│   ├── psplib-j30-v2.json      # After algorithm improvement
│   └── CHANGELOG.md            # Documents why results changed
└── manifest.yaml
```

**When expected results change:**
1. Document the change in `benchmarks/expected/CHANGELOG.md`
2. Require PR review for any regression (worse results)
3. Celebrate improvements (better results) but still document

### 10.3 Adding New Benchmarks

Process for adding a new benchmark dataset:

1. **Proposal:** Open RFC or issue describing dataset and validation goals
2. **Converter:** Implement converter with full test coverage
3. **Baseline:** Run benchmark suite, establish expected results
4. **Documentation:** Add to manifest with source, version, citation
5. **CI Integration:** Add to appropriate tier workflow

### 10.4 Benchmark Result Archival

Benchmark results are archived for trend analysis:

```bash
# CI stores results in artifacts
benchmarks/results/
├── 2026-01-26-abc123.json   # Commit hash for traceability
├── 2026-01-25-def456.json
└── ...
```

Retention: 90 days for Tier 1, 365 days for Tier 2/3.

---

## 11. Performance Threshold Management

### 11.1 Reference Machine Specification

All performance thresholds are calibrated against:

```yaml
reference_machine:
  cpu: GitHub Actions runner (2-core x86_64)
  memory: 7 GB
  os: Ubuntu 22.04
  rust: stable (latest)
```

**Local machines** may be faster. Thresholds are intentionally conservative to pass on CI.

### 11.2 Threshold Update Process

When algorithms improve:

1. **Document improvement** in RFC or commit message
2. **Propose new thresholds** based on new baseline measurements
3. **Update both soft and hard thresholds** in manifest
4. **Keep 20% headroom** above measured performance

Example:
```yaml
# Before optimization
j120_threshold_ms: 500

# After RFC-0014 optimization (measured: 180ms)
j120_threshold_ms: 250  # 180ms + 40% headroom
```

### 11.3 RFC-0014 Specific Benchmarks

Resource leveling (RFC-0014) has additional performance requirements:

| Scenario | Tasks | Resources | Threshold |
|----------|-------|-----------|-----------|
| Light contention | 1,000 | 10 | <2s |
| Heavy contention | 1,000 | 3 | <5s |
| Massive scale | 10,000 | 50 | <60s |
| Optimal solver (small) | 100 | 5 | <1s |

Memory usage during leveling:
- Peak memory ≤ 2x base schedule memory
- No memory leaks across repeated runs

---

## 12. DSLIB License and Data Handling

### 12.1 License Compliance

DSLIB is provided by Ghent University OR&S for **academic and research purposes**.

**Our approach:**
- utf8proj provides **conversion scripts only**
- Users must **download DSLIB directly** from Ghent University
- No DSLIB data is committed to the utf8proj repository
- Users must comply with Ghent University's terms of use

### 12.2 Automated Download Script

```bash
# tools/dslib_to_proj/download.sh
#!/bin/bash
set -e

DSLIB_URL="https://www.projectmanagement.ugent.be/..."
DSLIB_DIR="data/dslib"

echo "Downloading DSLIB from Ghent University OR&S..."
echo "By proceeding, you agree to Ghent University's terms of use."
read -p "Continue? [y/N] " confirm

if [[ $confirm == [yY] ]]; then
    mkdir -p "$DSLIB_DIR"
    curl -L "$DSLIB_URL" -o "$DSLIB_DIR/dslib.zip"
    unzip "$DSLIB_DIR/dslib.zip" -d "$DSLIB_DIR"
    echo "DSLIB downloaded to $DSLIB_DIR"
else
    echo "Download cancelled."
    exit 1
fi
```

### 12.3 Citation Requirements

Any publication or documentation using DSLIB results must cite:

> Batselier, J., & Vanhoucke, M. (2015). Construction and evaluation framework for a real-life project database. *International Journal of Project Management*, 33(3), 697-710.

### 12.4 Data Separation

```
utf8proj/
├── tools/dslib_to_proj/     # Committed (converter code)
├── data/                     # .gitignore'd
│   └── dslib/               # User downloads here
└── benchmarks/
    └── expected/
        └── dslib-correlation.json  # Committed (expected metrics only)
```

---

## 13. Implementation Plan

### Phase 1: Foundation (v0.13.0)

**Week 1-2: PSPLIB Integration**
- [ ] Build `psplib_to_proj` converter (Rust)
- [ ] Start with J30 only to validate pipeline
- [ ] Add J60, J120 incrementally
- [ ] Add makespan validation tests
- [ ] CI integration for Tier 1

**Week 3-4: Scaled Benchmarks**
- [ ] Enhance synthetic generators (chain, diamond)
- [ ] Add iMOPSE converter
- [ ] Performance baselines for Tier 2
- [ ] Memory profiling integration

### Phase 2: DSLIB Integration (v0.14.0)

**Week 1-2: DSLIB Parser**
- [ ] Analyze DSLIB format documentation
- [ ] Build `dslib_to_proj` converter
- [ ] Handle baseline + actuals export
- [ ] Parse success rate target: 95%

**Week 3-4: Validation Framework**
- [ ] Implement correlation metrics
- [ ] Build divergence analyzer
- [ ] Create validation reports
- [ ] Document all divergence categories

### Phase 3: Continuous Validation (v0.15.0+)

- [ ] Automated benchmark dashboard
- [ ] Regression detection alerts
- [ ] User-contributed dataset support
- [ ] Anonymization pipeline for contributed data

---

## 14. Open Questions

| Question | Options | Recommendation |
|----------|---------|----------------|
| Should converters be in Rust or Python? | Rust (fast, same toolchain), Python (easier prototyping) | **Rust** for production converters |
| Should we publish benchmark results? | Internal only, public leaderboard | **Public** — transparency builds trust |
| Multi-project support in DSLIB? | Treat as separate, model dependencies | **Separate** for Phase 1; dependencies in Phase 2 |

---

## 15. Risks

| Risk | Impact | Mitigation |
|------|--------|------------|
| DSLIB format undocumented | High — can't parse | Contact Ghent OR&S for documentation |
| DSLIB requires non-trivial extensions | Medium — scope creep | Strict Phase 1 minimal subset |
| Benchmarks slow down CI | Medium — developer friction | Tier 1 on every PR, Tier 2/3 nightly |
| Correlation metrics misleading | Low — wrong conclusions | Multiple metrics, manual review |

---

## 16. Success Criteria

- [ ] PSPLIB J30/J60/J120 passing with ≤5% gap to best-known
- [ ] Synthetic benchmarks demonstrating O(n) scaling
- [ ] ≥95% of DSLIB projects parseable
- [ ] Correlation analysis report for DSLIB validation
- [ ] CI pipeline with regression detection
- [ ] Public benchmark documentation

---

## 17. References

### Datasets
- [PSPLIB](https://www.om-db.wi.tum.de/psplib/library.html) — Project Scheduling Problem Library
- [Ghent University OR&S](https://www.projectmanagement.ugent.be/research/data) — DSLIB and related datasets
- [iMOPSE](http://imopse.ii.pwr.wroc.pl/download.html) — Multi-Skill RCPSP benchmarks
- [RG300](http://www.projectmanagement.ugent.be/?q=research/data/RanGen) — Large-scale RCPSP instances

### Related RFCs
- RFC-0003: Resource Leveling
- RFC-0008: Progress-Aware CPM
- RFC-0013: Baseline Management
- RFC-0014: Scaling Resource Leveling

### Academic Papers
- Kolisch & Sprecher (1996): "PSPLIB — A project scheduling problem library"
- Batselier & Vanhoucke (2015): "Construction and evaluation framework for a real-life project database"
- Myszkowski et al. (2015): "A new benchmark dataset for Multi-Skill Resource-Constrained Project Scheduling Problem"

---

## Changelog

| Date | Change |
|------|--------|
| 2026-01-26 | Initial draft |
| 2026-01-26 | Added Benchmark Governance (Section 10) |
| 2026-01-26 | Added Performance Threshold Management (Section 11) |
| 2026-01-26 | Added DSLIB License and Data Handling (Section 12) |
| 2026-01-26 | Added RFC-0014 specific leveling benchmarks |

---

**Document Version:** 0.2
**Status:** Draft — Ready for implementation
