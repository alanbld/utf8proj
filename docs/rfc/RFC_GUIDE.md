# utf8proj RFC Documentation Guide

This directory contains the authoritative design documentation for utf8proj.

## Documents

### 1. `UTF8PROJ_RFC_MASTER.md` - The Master Reference (85% confidence)
**Purpose:** Single source of truth for utf8proj architecture, design decisions, and implementation guidance.

**Contents:**
- Strategic vision and positioning
- Complete architecture and domain model
- DSL specification (.proj format)
- CLI interface specification
- Implementation roadmap (Phases 1-6)
- Confidence assessment by component

**Audience:**
- Claude Web KB (long-term memory)
- Claude Code CLI (local development)
- Human developers (onboarding)
- LLM assistants (code generation)

**Status:** Living document - updates with project evolution

---

### 2. `UTF8PROJ_DESIGN_SURVEY.md` - Design Refinement Questionnaire
**Purpose:** Address 29 design questions in areas with <80% confidence to reach >95% confidence before implementation.

**Structure:** 9 sections, 29 questions across:
- **Section A:** Progress-Aware CPM Algorithm (5 questions)
- **Section B:** Container Task Derivation (3 questions)
- **Section C:** History System - Sidecar Format (3 questions)
- **Section D:** History System - Embedded Format (3 questions)
- **Section E:** Playback Engine (3 questions)
- **Section F:** Excel Export (3 questions)
- **Section G:** Resource Leveling (3 questions)
- **Section H:** BDD/SAT Integration (3 questions)
- **Section I:** TaskJuggler Compatibility (3 questions)

**Response Format:** For each question, provide:
1. Technical Rationale
2. Trade-off Analysis
3. Recommended Approach
4. Implementation Notes
5. Test Strategy

**Audience:** Companion LLM Designer, Technical Architect, or human expert

**Estimated Time:** 4-6 hours for thorough completion

---

## Workflow

### Step 1: Review Master RFC
```bash
# Read the complete design
cat UTF8PROJ_RFC_MASTER.md
```

**Focus Areas:**
- Part II: Architecture & Domain Model
- Part III: DSL Specification
- Part VI: Areas Requiring Design Refinement (confidence table)

### Step 2: Complete Design Survey
```bash
# Open survey for editing
vim UTF8PROJ_DESIGN_SURVEY.md
# or
code UTF8PROJ_DESIGN_SURVEY.md
```

**Instructions:**
- Replace `[To be filled]` sections with detailed answers
- Use code examples where helpful
- Cross-reference Master RFC
- Be thorough - these decisions drive implementation

### Step 3: Iterate with Companion LLM
**Prompt for Companion LLM:**

```
I'm designing utf8proj, a Git-native project scheduling engine in Rust.

Please review the Master RFC (UTF8PROJ_RFC_MASTER.md) for context, then complete the Design Survey (UTF8PROJ_DESIGN_SURVEY.md).

For each of the 29 questions:
1. Provide technical rationale for your recommendation
2. Analyze trade-offs between alternatives
3. Specify the recommended approach with examples
4. Include implementation notes for key details
5. Outline a test strategy to validate

Goal: Raise design confidence from 85% to >95% before implementation begins.

Focus on:
- Correctness (especially CPM algorithm)
- User experience (CLI, file formats)
- Performance (1000+ task projects)
- Future extensibility

Please complete Section A (Progress-Aware CPM) first, then we'll iterate before moving to other sections.
```

### Step 4: Integrate Survey Results
Once survey is complete:

1. **Update Master RFC:**
   ```bash
   # Add "Resolved Design Decisions" section
   # Update confidence levels
   # Incorporate design rationale
   ```

2. **Create Implementation Checklist:**
   ```markdown
   ## Phase 3: Progress Tracking Implementation Checklist
   
   ### Week 7: Domain Model Extensions
   - [ ] Add progress fields to Task (percent_complete, actual_start, actual_finish, status)
   - [ ] Extend ScheduledTask with variance fields
   - [ ] Update Project for status_date and baseline
   
   ### Week 8: Progress-Aware CPM
   - [ ] Implement effective duration calculation (Survey Q1 decision)
   - [ ] Handle dependencies with partial completion (Survey Q2 decision)
   - [ ] Resolve actual dates vs dependencies conflicts (Survey Q3 decision)
   ...
   ```

3. **Begin Test-Driven Development:**
   ```bash
   # Start with integration tests
   touch tests/integration/progress_scheduling.rs
   
   # Write failing tests first
   # Implement to pass tests
   # Refactor and document
   ```

---

## Design Confidence Tracking

| Component | Initial | After Survey | Target |
|-----------|---------|--------------|--------|
| Progress-Aware CPM | 75% | → | 95% |
| Container Derivation | 80% | → | 95% |
| History - Sidecar | 70% | → | 95% |
| History - Embedded | 60% | → | 95% |
| Playback Engine | 65% | → | 95% |
| Excel Export | 70% | → | 95% |
| Resource Leveling | 65% | → | 95% |
| BDD/SAT Integration | 50% | → | 95% or Deferred |
| TJP Compatibility | 75% | → | 95% |
| **Overall** | **85%** | → | **95%** |

---

## Integration with Development Tools

### Claude Web KB
- Save Master RFC in Project Knowledge Base
- Reference in future conversations
- Update as implementation progresses

### Claude Code CLI
```bash
# Local development reference
cd /path/to/utf8proj
cat ../UTF8PROJ_RFC_MASTER.md | grep -A 20 "Progress-Aware CPM"

# Generate code from RFC
claude-code implement utf8proj-solver/src/progress_cpm.rs \
  --reference UTF8PROJ_RFC_MASTER.md \
  --section "2.4 Progress-Aware CPM Algorithm"
```

### Git Repository
```bash
# Commit RFC documents
git add UTF8PROJ_RFC_MASTER.md UTF8PROJ_DESIGN_SURVEY.md
git commit -m "docs: Add authoritative RFC and design survey"

# Track design decisions
git log --follow UTF8PROJ_RFC_MASTER.md
```

---

## Success Criteria

**Design Phase Complete When:**
- [ ] All 29 survey questions answered with >95% confidence
- [ ] Design decisions integrated into Master RFC
- [ ] No unresolved conflicts or ambiguities
- [ ] Test strategy defined for each component
- [ ] Implementation checklist created

**Then proceed to:** Phase 3 implementation (Progress Tracking)

---

## Questions or Issues?

**For design clarifications:**
- Review confidence table in Master RFC (Part VI, Section 6.1)
- Consult specific survey question for detailed context
- Use companion LLM to explore alternatives

**For implementation guidance:**
- Check Implementation Roadmap (Master RFC Part V)
- Review test-driven development workflow (Section 5.2)
- Consult domain model code examples (Part II, Section 2.3)

---

**Document Version:** 1.0  
**Last Updated:** 2026-01-04  
**Status:** Ready for design survey completion
