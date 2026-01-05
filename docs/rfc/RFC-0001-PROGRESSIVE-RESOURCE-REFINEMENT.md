# RFC-0001: Progressive Resource Refinement & Cost Ranges

**Status**: Draft (Design Complete)
**Authors**: utf8proj contributors (via ChatGPT PERT/CPM Specialist + Claude Code)
**Created**: 2026-01-05
**Updated**: 2026-01-05
**Target version**: 0.3.0+
**Related**: UTF8PROJ_RFC_MASTER.md, Resource Model, Cost Model, LSP v0
**Confidence**: 85% (design questions resolved)

---

## 1. Executive Summary

This RFC proposes a **first-class model for progressive resource refinement** in utf8proj, enabling:

* Abstract, role-based resource planning
* Multi-level specialization (role → skill → seniority → named person)
* Explicit **cost ranges** that narrow over time
* Honest modeling of uncertainty during early estimation
* Seamless transition from proposal → planning → execution
* Strong editor support via a Language Server (LSP)

The design intentionally avoids enforcing any specific project management methodology (PMI, Agile, etc.), while remaining *PMI-aware* and aligned with real-world planning practices.

---

## 2. Context & Motivation

### 2.1 How Real Projects Are Planned

In practice, project planning evolves progressively:

1. **Early proposal / ROM estimate**
   * "We need ~2 developers for ~3 months"
   * Costs are ranges, not fixed numbers

2. **Initial planning**
   * "One backend developer, one database developer"

3. **Refinement**
   * "Spring Boot backend, PL/SQL database"
   * "One senior, one junior"

4. **Execution**
   * "Alice and Bob, exact availability and rates"

This process is explicitly recognized by PMI as **progressive elaboration** and **range-of-magnitude estimation**.

### 2.2 What Existing Tools Do Poorly

| Tool        | Limitation                                                   |
|-------------|--------------------------------------------------------------|
| TaskJuggler | Fixed rates, no cost ranges, no refinement semantics         |
| MS Project  | Generic resources are placeholders, not modeled abstractions |
| Agile tools | Ignore cost and role modeling entirely                       |

Most tools **force premature precision** or treat abstraction as a temporary hack, losing:

* Traceability of assumptions
* Visibility into uncertainty
* Opportunities for meaningful diagnostics

### 2.3 utf8proj's Opportunity

utf8proj is uniquely positioned because it is:

* Text-first and diff-friendly
* Deterministic and auditable
* Designed around explicit structure rather than UI state
* Already focused on transparency over "solver magic"

This RFC leverages those strengths to make **uncertainty explicit instead of hidden**.

---

## 3. Goals & Non-Goals

### 3.1 Goals

* Encode **resource abstraction and refinement** explicitly
* Support **cost ranges** that narrow over time
* Preserve early estimates without rewriting tasks
* Enable tooling (LSP) to guide users toward clarity
* Remain methodology-neutral

### 3.2 Non-Goals

* Full resource leveling or optimization (see existing resource leveling)
* Mandatory PMI phases or artifacts
* Enforcing specific vocabularies or taxonomies
* Turning utf8proj into an Agile execution tracker

### 3.3 Non-Goal: Object-Oriented Modeling

> **The resource profile system in utf8proj is NOT object-oriented inheritance.**
>
> Profiles do not inherit behavior, methods, or identity. The `specializes` relationship represents **constraint refinement** and **progressive elaboration**, not "is-a" semantics.
>
> This is a **refinement lattice** (set narrowing), not a class hierarchy:
> - Constraints only ever *narrow*, never widen
> - Resolution is monotonic and deterministic
> - There is no overriding of behavior — only restriction
> - No runtime dispatch, no polymorphism, no dynamic binding

Think **type refinement** or **predicate logic**, not OO inheritance.

---

## 4. Core Concepts

### 4.1 Resource Profiles vs Resources

| Concept | Description | Example |
|---------|-------------|---------|
| **Resource Profile** | Abstract role or capability (not a person) | `developer`, `backend_dev`, `senior` |
| **Resource** | Concrete person or entity | `alice`, `bob` |

This symmetry is intentional:
```
resource_profile → abstract
resource         → concrete
```

### 4.2 Refinement Lattice (Not Inheritance)

Profiles form a **constraint lattice** where each level adds restrictions:

```
                    developer
                   rate ∈ [450, 700]
                   /              \
          backend_dev          frontend_dev
          rate ∈ [550, 800]    rate ∈ [500, 700]
          skills ⊇ {backend}   skills ⊇ {frontend}
              |                      |
        springboot_dev           react_dev
        skills ⊇ {springboot}    skills ⊇ {react}
              |
      springboot_senior
      (+ trait: senior × 1.3)
              |
            alice
            rate = 900 (fixed)
```

**Key properties:**
- You **cannot remove** parent constraints
- You **cannot widen** ranges
- Resolution is static and explainable

### 4.3 Traits (Not Mixins)

Traits are **scalar modifiers**, not behavior mixins:

| Property | Trait Behavior |
|----------|----------------|
| Behavior | None (no methods) |
| Effect | Numeric multipliers only |
| Composition | Multiplicative, explicit |
| Conflicts | Errors, not "last wins" |

---

## 5. DSL Syntax Specification

### 5.1 Resource Profiles

```proj
resource_profile developer {
    description: "Generic software developer"
    rate: {
        min: 450
        max: 700
        currency: USD
    }
}
```

**Rules:**
- `resource_profile` represents an abstract role or capability, not a concrete person
- Cost may be a range (min/max) or omitted (inherited)
- Profiles may be used directly in assignments

### 5.2 Specialization (Constraint Refinement)

```proj
resource_profile backend_dev {
    specializes: developer
    skills: [backend, java, sql]
    rate: {
        min: 550
        max: 800
    }
}
```

**Rules:**
- `specializes` adds constraints (does not override behavior)
- Rates may narrow or be omitted (inherits parent range)
- Multiple levels allowed
- **Cannot specialize multiple profiles** (error)

### 5.3 Traits (Separate Definitions)

```proj
trait senior {
    rate_multiplier: 1.3
    description: "5+ years experience"
}

trait junior {
    rate_multiplier: 0.8
    description: "0-2 years experience"
}

trait contractor {
    rate_multiplier: 1.2
    description: "External contractor (overhead)"
}
```

**Rules:**
- Traits are first-class blocks (not inline)
- Enables reuse, documentation, LSP hover
- Enables diagnostics for unknown traits

### 5.4 Composed Profiles

```proj
resource_profile springboot_dev {
    specializes: backend_dev
    skills: [springboot]
}

resource_profile springboot_senior {
    specializes: springboot_dev
    traits: [senior]
}
```

### 5.5 Concrete Resources

```proj
resource alice {
    specializes: springboot_senior
    availability: 0.8
    rate: 900
    email: "alice@company.com"
}
```

**Rules:**
- Named resources override all abstract rate ranges
- Availability is optional (defaults to 1.0)
- **A resource may specialize only ONE profile** (error otherwise)
- Rate collapses the range to a fixed point

### 5.6 Task Assignment

```proj
// Abstract assignment (estimation phase)
task implementation {
    effort: 40d
    assign: developer * 2
}

// Mixed assignment (partial staffing)
task implementation {
    assign: [alice, developer]  // 1 concrete + 1 abstract: ALLOWED
}

// Concrete assignment (execution phase)
task implementation {
    assign: [alice, bob]
}
```

**Semantics:**
- Abstract assignments are valid during estimation
- Mixed abstraction levels are allowed (with hint)
- Later refinement does NOT require modifying original task

---

## 6. Cost Semantics

### 6.1 Cost Range Propagation

| Metric | Calculation |
|--------|-------------|
| **Best case** | effort × min_rate × quantity |
| **Worst case** | effort × max_rate × quantity |
| **Expected** | (min + max) / 2 (midpoint, configurable) |

### 6.2 Expected Value Policy

**Default:** Midpoint

```
expected = (min + max) / 2
```

**Optional project setting:**

```proj
project "Example" {
    start: 2026-02-01
    cost_policy: midpoint  // or: pessimistic, optimistic
}
```

**Rationale:**
- Midpoint is simple, explainable, non-political
- Monte Carlo is overkill and belongs in external tooling
- utf8proj must *never* imply false statistical rigor

### 6.3 Trait Composition: Multiplicative

When multiple traits apply, multipliers compose **multiplicatively**:

```
base × senior(1.3) × contractor(1.2) = base × 1.56
```

**Rationale:**
- Scales naturally
- Matches economic reality (premiums compound)
- Order-independent
- Avoids "double counting" confusion

**Guardrail:** LSP warning if multiplier stack exceeds 2.0

### 6.4 Rate Inheritance

If a specialized profile **omits** rate, it **inherits** parent range:

```proj
resource_profile senior_dev {
    specializes: developer
    traits: [senior]
    // rate: omitted → inherits developer's [450, 700]
    // After trait: [585, 910]
}
```

**LSP hint** when refinement does not narrow cost.

### 6.5 Example Cost Calculation

```
task implementation {
    effort: 40d
    assign: developer * 2
}

resource_profile developer { rate: { min: 450, max: 700 } }

Cost range:
- Best:  40d × 450 × 2 = $36,000
- Worst: 40d × 700 × 2 = $56,000
- Expected: $46,000
- Spread: ±22%

After refinement to alice (rate: 900) + bob (rate: 750):
- Fixed: 40d × (900 + 750) = $66,000
- Spread: 0%
```

---

## 7. Diagnostics Catalog

### 7.1 Diagnostic Levels

| Level   | Purpose                                 | Action Required |
|---------|-----------------------------------------|-----------------|
| Error   | Invalid or contradictory model          | Must fix        |
| Warning | High risk or misleading structure       | Should review   |
| Hint    | Best practice or refinement suggestion  | Optional        |

### 7.2 Hints (Informational)

| Code | Message |
|------|---------|
| R001 | Task "{task}" uses abstract resource `{profile}`. Consider refining once staffing decisions are known. |
| R002 | Collapsed rate range; consider using `rate: {value}` instead. |
| R003 | Mixed abstraction level in assignment for task "{task}". |
| R004 | Profile "{profile}" does not narrow parent rate range. |

### 7.3 Warnings (Should Review)

| Code | Message |
|------|---------|
| R010 | Cost range for "{scope}" is wide (±{percent}%). Refinement recommended before baseline. |
| R011 | Resource profile "{profile}" has no rate defined and no parent to inherit from. |
| R012 | Trait multiplier stack exceeds 2.0 on profile "{profile}". |
| R013 | Resource leveling with abstract assignments is approximate. |
| R014 | Abstract calendar on profile "{profile}" conflicts with concrete calendar on resource "{resource}". |

### 7.4 Errors (Must Fix)

| Code | Message |
|------|---------|
| R100 | Resource "{resource}" specializes multiple profiles: {list}. A resource may specialize only one profile. |
| R101 | Circular specialization detected: {chain}. |
| R102 | Rate range [{min}, {max}] is inverted (min > max). |
| R103 | Unknown trait "{trait}" referenced in profile "{profile}". |
| R104 | Unknown profile "{profile}" referenced in specialization. |

### 7.5 Threshold-Based Escalation

Abstract assignments escalate from hint to warning when:
- Cost spread exceeds configurable threshold (default: ±25%)
- Phase explicitly marked "execution"

```proj
project "Example" {
    abstract_warning_threshold: 25%  // default
}
```

### 7.6 Strict Mode

CLI supports `--strict` for CI/CD gating:

```bash
utf8proj check --strict project.proj
```

Behavior:
- Hints treated as warnings
- Warnings treated as errors (configurable)

---

## 8. Compatibility with Existing utf8proj

### 8.1 Backward Compatibility

Existing `resource` syntax continues to work:

```proj
// Current syntax (still valid)
resource dev "Developer" {
    rate: 500
}
```

**Dual interpretation:**
- Point rate (`rate: 500`) → concrete resource
- Range rate (`rate: { min: 500, max: 700 }`) → treated as abstract, with LSP hint suggesting `resource_profile`

### 8.2 Migration Path

1. **Backward compatible**: Existing `resource` blocks continue to work
2. **New keyword**: `resource_profile` introduces abstract profiles
3. **Opt-in**: Projects can mix old and new syntax during transition
4. **No deprecation**: Old syntax remains valid indefinitely

### 8.3 Resource Leveling Interaction

| Assignment Type | Leveling Behavior |
|-----------------|-------------------|
| All concrete | Full leveling |
| Mixed | Approximate leveling + warning |
| All abstract | Approximate leveling + warning |

**Diagnostic R013:** "Resource leveling with abstract assignments is approximate."

Leveling does NOT:
- Block on abstract assignments
- Auto-expand abstracts to placeholders
- Silently assume capacity

### 8.4 Calendar & Availability Interaction

Availability multiplies with calendar:

```
effective_capacity = calendar_hours × availability
```

Example:
- Calendar: 8h/day, 5 days/week
- Availability: 0.8
- Effective: 6.4h/day

Calendars may be assigned to abstract profiles (inherited by concrete resources).

---

## 9. LSP Capabilities

### 9.1 Scope (No Scheduling Required)

**P0 (Must-have):**
- Syntax validation
- Unknown references (profiles, traits, resources)
- Duplicate IDs
- Specialization cycles (R101)
- Document outline (tasks, profiles, resources, traits)

**P1 (Strongly recommended):**
- Hover info:
  - Resource refinement chain
  - Cost range (approximate, without scheduling)
  - Abstract vs concrete status
  - Trait multiplier stack
- Autocomplete on `assign:`:
  - Concrete resources (ranked first)
  - Most specific profiles
  - Generic profiles
- Autocomplete on `specializes:` and `traits:`

**P2 (Nice-to-have):**
- Quick fixes:
  - "Create resource_profile…"
  - "Refine abstract assignment"
  - "Convert to resource_profile"
- Semantic highlighting (profiles vs resources)

### 9.2 Cost in LSP

LSP shows **structural cost ranges** (not scheduled cost):

```
Hover on task "implementation":
─────────────────────────────────
Cost Range: $36,000 - $56,000 (±22%)
Assignment: developer × 2 (abstract)
─────────────────────────────────
```

This does NOT require CPM scheduling.

### 9.3 Architecture

```
.proj file
    ↓
  Parser (pest)
    ↓
  Semantic Model
    ↓
  Analysis API (shared, read-only, deterministic)
    ├─ Diagnostics
    ├─ Outline
    ├─ Hover Data
    └─ Suggestions
    ↓
LSP Server / CLI / MCP Server
```

---

## 10. Visualization

### 10.1 CLI Text Output

```
Project Cost Summary
────────────────────────────────────────
Phase 1: Development
  Cost: $81,000 - $126,000 (±22%)
  ├─ Backend: $44,000 - $60,000 (±15%)
  │   └─ [abstract: backend_dev × 2]
  └─ Frontend: $37,000 - $66,000 (±28%)
      └─ [abstract: frontend_dev]

Total: $81,000 - $126,000
Expected: $103,500
────────────────────────────────────────
```

### 10.2 Excel Export

| Task | Min Cost | Expected | Max Cost | Spread | Assignment |
|------|----------|----------|----------|--------|------------|
| Phase 1 | $81,000 | $103,500 | $126,000 | ±22% | |
| Backend | $44,000 | $52,000 | $60,000 | ±15% | backend_dev × 2 |
| Frontend | $37,000 | $51,500 | $66,000 | ±28% | frontend_dev |

Optional: Conditional formatting for high spread (>25%)

### 10.3 HTML Gantt

- Cost ranges in tooltip on hover
- Optional "Cost Lane" sidebar
- Never overload task bars visually

---

## 11. Edge Cases (Resolved)

### 11.1 Collapsed Range

```proj
resource_profile specific_dev {
    rate: { min: 500, max: 500 }
}
```

**Status:** Valid but triggers hint R002 ("consider `rate: 500`")

### 11.2 Mixed Abstract + Concrete Assignments

```proj
task impl {
    assign: [alice, developer]
}
```

**Status:** Allowed. Triggers hint R003.

### 11.3 Multiple Profile Specialization

```proj
resource alice {
    specializes: [backend_senior, devops_junior]  // ERROR
}
```

**Status:** Error R100. Create composite profile instead.

---

## 12. Implementation Phases

### Phase 1: Grammar & Parser (1-2 weeks)
- Add `resource_profile`, `trait` to pest grammar
- Parse into Resource struct with `is_abstract` flag
- Parse trait definitions
- No semantic validation yet

### Phase 2: Semantic Model (1-2 weeks)
- Build refinement lattice
- Validate specialization chains (no cycles, single parent)
- Compute effective rates from profiles + traits (multiplicative)
- Validate trait references

### Phase 3: Cost Propagation (1 week)
- Extend Schedule with cost ranges (min/expected/max)
- Propagate ranges through task hierarchy
- Add cost fields to JSON/Excel output
- Implement cost_policy setting

### Phase 4: Diagnostics (1 week)
- Implement diagnostic catalog (R001-R104)
- Integrate with CLI check command
- Implement --strict mode
- Prepare for LSP integration

### Phase 5: LSP Integration (2-3 weeks)
- Hover for profile chains and cost ranges
- Autocomplete for profiles/traits
- Quick fixes for common issues
- Semantic highlighting

---

## 13. Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| Syntax (`resource_profile`, `trait`) | 85% | Clear, grep-able, LSP-friendly |
| Cost semantics (midpoint, multiplicative) | 85% | Simple, explainable, no false rigor |
| Inheritance rules (single profile, implicit inherit) | 90% | Deterministic, no OO complexity |
| Existing integration (dual meaning, approximate leveling) | 85% | Backward compatible, gentle migration |
| Diagnostics (threshold + phase aware, --strict) | 90% | Enables CI/CD, flexible |
| LSP scope (structural cost, ranked autocomplete) | 85% | Immediate value, no scheduling needed |
| Visualization (CLI/Excel/HTML) | 80% | Clear guidance per output |
| Edge cases (collapsed, mixed, multi-profile) | 85% | All resolved with clear semantics |
| Future compat (calendars, availability) | 80% | Multiplicative, independent |
| **Overall** | **85%** | Ready for implementation |

---

## 14. Conclusion

This RFC encodes **how real planners think**, not how tools traditionally force them to think.

By modeling progressive refinement explicitly, utf8proj can:

* Remain simple at the surface
* Be honest about uncertainty
* Scale from proposal to execution
* Offer uniquely powerful editor tooling

This is not a deviation from best practice — it is a **formalization of it**.

---

## Appendix A: Example Project Evolution

### Stage 1: ROM Estimate

```proj
project "Widget Platform" {
    start: 2026-02-01
}

resource_profile developer {
    description: "Generic software developer"
    rate: { min: 450, max: 700 }
}

task phase1 "Development" {
    effort: 60d
    assign: developer * 3
}

// Cost: $81,000 - $126,000 (±22%)
```

### Stage 2: Initial Planning

```proj
resource_profile backend_dev {
    specializes: developer
    skills: [backend]
    rate: { min: 550, max: 750 }
}

resource_profile frontend_dev {
    specializes: developer
    skills: [frontend]
    rate: { min: 500, max: 700 }
}

task phase1 "Development" {
    task backend { effort: 40d, assign: backend_dev * 2 }
    task frontend { effort: 20d, assign: frontend_dev }
}

// Cost: $71,000 - $98,000 (±16%)
```

### Stage 3: Execution

```proj
trait senior {
    rate_multiplier: 1.3
    description: "Senior-level experience"
}

resource_profile backend_senior {
    specializes: backend_dev
    traits: [senior]
}

resource alice {
    specializes: backend_senior
    rate: 900
    availability: 0.9
}

resource bob {
    specializes: backend_dev
    rate: 650
}

resource charlie {
    specializes: frontend_dev
    rate: 550
}

task phase1 "Development" {
    task backend { assign: [alice, bob] }
    task frontend { assign: charlie }
}

// Cost: $61,000 (fixed, 0% spread)
```

---

## Appendix B: Grammar Additions (pest)

```pest
// New top-level definitions
resource_profile_def = {
    "resource_profile" ~ WHITESPACE+ ~ identifier ~ WHITESPACE* ~ "{" ~
    profile_body ~
    "}"
}

profile_body = {
    (description_attr | specializes_attr | skills_attr | traits_ref | rate_range_attr)*
}

specializes_attr = {
    "specializes:" ~ WHITESPACE* ~ identifier
}

skills_attr = {
    "skills:" ~ WHITESPACE* ~ "[" ~ identifier_list ~ "]"
}

traits_ref = {
    "traits:" ~ WHITESPACE* ~ "[" ~ identifier_list ~ "]"
}

rate_range_attr = {
    "rate:" ~ WHITESPACE* ~ "{" ~
    (rate_min ~ ","?)? ~
    (rate_max ~ ","?)? ~
    (rate_currency)? ~
    "}"
}

rate_min = { "min:" ~ WHITESPACE* ~ number }
rate_max = { "max:" ~ WHITESPACE* ~ number }
rate_currency = { "currency:" ~ WHITESPACE* ~ identifier }

// Trait definition
trait_def = {
    "trait" ~ WHITESPACE+ ~ identifier ~ WHITESPACE* ~ "{" ~
    trait_body ~
    "}"
}

trait_body = {
    (description_attr | rate_multiplier_attr)*
}

rate_multiplier_attr = {
    "rate_multiplier:" ~ WHITESPACE* ~ number
}

// Quantified assignment
quantified_assignment = {
    identifier ~ WHITESPACE* ~ "*" ~ WHITESPACE* ~ integer
}

assignment_list = {
    "[" ~ (identifier | quantified_assignment) ~ ("," ~ (identifier | quantified_assignment))* ~ "]"
}
```

---

## Appendix C: Design Decision Log

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Keyword | `resource_profile` | Explicit, grep-able, pairs with `resource` |
| Traits | Separate definitions | Reuse, LSP hover, diagnostics |
| Expected cost | Midpoint default | Simple, no false statistical rigor |
| Trait composition | Multiplicative | Scales naturally, order-independent |
| Rate inheritance | Implicit | Matches progressive elaboration |
| Multi-profile | Disallow | Prevents cost ambiguity |
| Mixed assignments | Allow | Realistic during transition |
| Leveling + abstract | Approximate + warn | Honest, not blocking |
| Calendars on profiles | Allow | Shift-based roles exist |

---

**Document Version:** 2.0
**Last Updated:** 2026-01-05
**Status:** Draft (Design Complete) - Ready for implementation planning
