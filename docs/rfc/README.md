# utf8proj RFC Index

This directory contains all Request for Comments (RFC) documents for utf8proj.

## RFC Registry

| RFC | Title | Status | Created |
|-----|-------|--------|---------|
| [RFC-0001](RFC-0001-ARCHITECTURE.md) | Architecture - Project Scheduling Engine | ✅ Implemented | 2026-01-01 |
| [RFC-0002](RFC-0002-CPM-CORRECTNESS.md) | CPM Correctness & Strategic Evolution | ✅ Implemented | 2026-01-03 |
| [RFC-0003](RFC-0003-CONTAINER-DEPENDENCY-SEMANTICS.md) | Container Dependency Semantics | ✅ Implemented | 2026-01-06 |
| [RFC-0004](RFC-0004-PROGRESSIVE-RESOURCE-REFINEMENT.md) | Progressive Resource Refinement & Cost Ranges | ✅ Implemented | 2026-01-05 |
| [RFC-0005](RFC-0005-RESOURCE-LEVELING-STATUS.md) | Resource Leveling | ⚠️ Phase 1 Only | 2026-01-09 |
| [RFC-0006](RFC-0006-FOCUS-VIEW.md) | Focus View for Gantt Charts | ✅ Implemented | 2026-01-15 |
| [RFC-0007](RFC-0007-WASM-PLAYGROUND-AI-INTEGRATION.md) | WASM Playground with AI-Assisted Editing | 📝 Draft | 2026-01-15 |
| [RFC-0008](RFC-0008-PROGRESS-AWARE-CPM.md) | Progress-Aware CPM | ✅ Implemented | 2026-01-15 |

## Implementation Summary

| Category | Count | RFCs |
|----------|-------|------|
| ✅ Fully Implemented | 6 | 0001, 0002, 0003, 0004, 0006, 0008 |
| ⚠️ Partially Implemented | 1 | 0005 (Phase 1) |
| 📝 Design Only | 1 | 0007 |

## Status Definitions

| Status | Meaning |
|--------|---------|
| **Draft** | Under discussion, not yet approved |
| **Approved** | Design approved, awaiting implementation |
| **Implemented** | Fully implemented in codebase |
| **Deferred** | Postponed pending user demand |
| **Rejected** | Not accepted |

## Other Documents

| Document | Purpose |
|----------|---------|
| [RFC_GUIDE.md](RFC_GUIDE.md) | How to work with RFCs |
| [UTF8PROJ_RFC_MASTER.md](UTF8PROJ_RFC_MASTER.md) | Master design reference |

## Creating a New RFC

1. Use the next available number (currently RFC-0009)
2. Follow the naming convention: `RFC-NNNN-SHORT-TITLE.md`
3. Include standard header fields (RFC Number, Status, Created, Related)
4. Update this README with the new RFC entry
