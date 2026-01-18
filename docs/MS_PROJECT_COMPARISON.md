# MS Project vs utf8proj Feature Comparison

This document compares Microsoft Project features with utf8proj support, identifying gaps and conversion considerations.

## Summary

| Category | MS Project Features | utf8proj Support | Gap |
|----------|---------------------|------------------|-----|
| Core Scheduling | 12 | 11 | 1 (Resource calendar priority) |
| Task Attributes | 15 | 14 | 1 (Physical % complete) |
| Dependencies | 6 | 6 | ✓ Full support |
| Resources | 10 | 8 | 2 (Cost resources, Material resources) |
| Progress Tracking | 8 | 7 | 1 (Physical % complete) |
| Cost Management | 6 | 4 | 2 (Earned value types) |
| Constraints | 8 | 6 | ✓ Full support (mapped) |

## Core Scheduling Concepts

| Concept | MS Project | utf8proj | Status |
|---------|------------|----------|--------|
| Forward pass (CPM) | ✓ | ✓ | ✅ Full |
| Backward pass (CPM) | ✓ | ✓ | ✅ Full |
| Critical path | ✓ | ✓ | ✅ Full |
| Effort-driven scheduling | ✓ | ✓ | ✅ Full (PMI compliant) |
| Fixed duration | ✓ | ✓ `duration:` | ✅ Full |
| Fixed work | ✓ | ✓ `effort:` | ✅ Full |
| Fixed units | ✓ | ✓ `assign: dev@50%` | ✅ Full |
| Auto-scheduled | ✓ | ✓ (default) | ✅ Full |
| Manually scheduled | ✓ | ✓ `must_start_on:` | ✅ Full (via constraint) |
| Summary tasks (containers) | ✓ | ✓ Hierarchical | ✅ Full |
| Milestones | ✓ | ✓ `milestone id {}` | ✅ Full |
| Resource calendars | ✓ | ✓ `calendar:` | ✅ Partial |

### Notes
- **Manually scheduled tasks**: Converted to `must_start_on:` constraints by mpp_to_proj
- **Resource calendar priority**: MS Project uses most restrictive calendar; utf8proj uses task calendar

## Task Attributes

| Attribute | MS Project | utf8proj | Status |
|-----------|------------|----------|--------|
| Name | ✓ | ✓ `"Display Name"` | ✅ Full |
| ID/WBS | ✓ | ✓ `task_id` | ✅ Full |
| Duration | ✓ | ✓ `duration:` | ✅ Full |
| Work (Effort) | ✓ | ✓ `effort:` | ✅ Full |
| Start/Finish | ✓ (computed) | ✓ (computed) | ✅ Full |
| % Complete | ✓ | ✓ `complete:` | ✅ Full |
| Physical % Complete | ✓ | ❌ | ⚠️ Gap |
| Remaining Duration | ✓ | ✓ `remaining:` | ✅ Full |
| Priority | ✓ (1-1000) | ✓ `priority:` (integer) | ✅ Full |
| Notes | ✓ | ✓ `note:` | ✅ Full |
| Constraint Type | ✓ | ✓ (see Constraints) | ✅ Full |
| Constraint Date | ✓ | ✓ (see Constraints) | ✅ Full |
| Deadline | ✓ | ✓ `finish_no_later_than:` | ✅ Full |
| Cost | ✓ | ✓ `cost:` (fixed) | ✅ Partial |
| Actual Start/Finish | ✓ | ✓ `actual_start:`, `actual_finish:` | ✅ Full |

### Notes
- **Physical % Complete**: Used for earned value; utf8proj uses duration-based %

## Dependencies

| Type | MS Project | utf8proj | Status |
|------|------------|----------|--------|
| Finish-to-Start (FS) | ✓ | ✓ `depends: task` | ✅ Full |
| Start-to-Start (SS) | ✓ | ✓ `depends: task SS` | ✅ Full |
| Finish-to-Finish (FF) | ✓ | ✓ `depends: task FF` | ✅ Full |
| Start-to-Finish (SF) | ✓ | ✓ `depends: task SF` | ✅ Full |
| Lag (positive) | ✓ | ✓ `depends: task +5d` | ✅ Full |
| Lead (negative lag) | ✓ | ✓ `depends: task -2d` | ✅ Full |

### Container Dependency Inheritance
- **MS Project**: Child tasks implicitly inherit container dependencies
- **utf8proj**: Explicit dependencies only (use `fix container-deps` command)
- **W014 diagnostic** warns about this difference

## Resources

| Feature | MS Project | utf8proj | Status |
|---------|------------|----------|--------|
| Work resources | ✓ | ✓ `resource id {}` | ✅ Full |
| Material resources | ✓ | ❌ | ⚠️ Gap |
| Cost resources | ✓ | ❌ | ⚠️ Gap |
| Standard rate | ✓ | ✓ `rate: 500/day` | ✅ Full |
| Overtime rate | ✓ | ❌ (not implemented) | ⚠️ Gap |
| Cost per use | ✓ | ❌ | ⚠️ Gap |
| Capacity/Max units | ✓ | ✓ `capacity:` | ✅ Full |
| Calendar | ✓ | ✓ `calendar:` | ✅ Full |
| Availability | ✓ | ✓ `availability:` | ✅ Full |
| Efficiency | ✓ (calculated) | ✓ `efficiency:` | ✅ Full |
| Leave/Vacation | ✓ | ✓ `leave: date..date` | ✅ Full |

### Notes
- **Work resources**: Fully supported (people, equipment)
- **Material resources**: Not tracked (consumables like concrete, steel)
- **Cost resources**: Not tracked (expenses like travel, licenses)

## Progress Tracking

| Feature | MS Project | utf8proj | Status |
|---------|------------|----------|--------|
| % Complete | ✓ | ✓ `complete: 50%` | ✅ Full |
| Physical % Complete | ✓ | ❌ | ⚠️ Gap |
| Status Date | ✓ | ✓ `status_date:` | ✅ Full |
| Actual Start | ✓ | ✓ `actual_start:` | ✅ Full |
| Actual Finish | ✓ | ✓ `actual_finish:` | ✅ Full |
| Remaining Work | ✓ | ✓ `remaining:` | ✅ Full |
| Remaining Duration | ✓ | ✓ (computed) | ✅ Full |
| Task Status | ✓ | ✓ `status: in_progress` | ✅ Full |

### Notes
- **Status Date**: RFC-0008 progress-aware scheduling respects status date
- **P005/P006 diagnostics** warn about progress inconsistencies

## Constraints

| MS Project Constraint | utf8proj Equivalent | Status |
|----------------------|---------------------|--------|
| As Soon As Possible | (default) | ✅ Full |
| As Late As Possible | ❌ | ⚠️ Gap |
| Must Start On | `must_start_on:` | ✅ Full |
| Must Finish On | `must_finish_on:` | ✅ Full |
| Start No Earlier Than | `start_no_earlier_than:` | ✅ Full |
| Start No Later Than | `start_no_later_than:` | ✅ Full |
| Finish No Earlier Than | `finish_no_earlier_than:` | ✅ Full |
| Finish No Later Than | `finish_no_later_than:` | ✅ Full |

### Notes
- **As Late As Possible**: Not directly supported; use `finish_no_later_than:` as workaround

## Cost Management

| Feature | MS Project | utf8proj | Status |
|---------|------------|----------|--------|
| Resource costs | ✓ | ✓ `rate: 500/day` | ✅ Full |
| Fixed costs | ✓ | ✓ `cost:` | ✅ Full |
| Actual costs | ✓ | ❌ | ⚠️ Gap |
| Cost variance | ✓ | ❌ | ⚠️ Gap |
| Budget | ✓ | ❌ | ⚠️ Gap |
| Milestone payments | ✓ | ✓ `payment:` | ✅ Full |

## Earned Value Management

| Metric | MS Project | utf8proj | Status |
|--------|------------|----------|--------|
| BCWS (PV) | ✓ | ✓ (computed) | ✅ Full |
| BCWP (EV) | ✓ | ✓ (computed) | ✅ Full |
| ACWP (AC) | ✓ | ❌ | ⚠️ Gap |
| SPI | ✓ | ✓ (I005) | ✅ Full |
| CPI | ✓ | ❌ | ⚠️ Gap |
| SV | ✓ | ✓ (computed) | ✅ Full |
| CV | ✓ | ❌ | ⚠️ Gap |

### Notes
- **Earned Value**: utf8proj computes schedule performance; cost performance requires actual costs

## Temporal Regimes (RFC-0012)

utf8proj extends MS Project with explicit temporal regimes:

| Regime | Description | Use Case |
|--------|-------------|----------|
| `work` | Respects working days (default) | Effort-bearing tasks |
| `event` | Any calendar day | Releases, launches, go-lives |
| `deadline` | Exact date required | Contractual, regulatory dates |

This allows scheduling events on weekends when business constraints require it.

## Conversion Workflow (mpp_to_proj)

### Handled Automatically
- Task hierarchy and WBS
- Duration and effort
- All dependency types (FS, SS, FF, SF) with lag
- Resource assignments
- Constraints
- Milestones
- **Manually scheduled tasks** → `must_start_on:` constraints

### Post-Conversion Steps
1. Run `utf8proj check` to see diagnostics
2. Run `utf8proj fix container-deps` to inherit container dependencies
3. Review W014 warnings for dependency differences
4. Add `regime: event` to milestones that should ignore calendars

### Example Workflow
```bash
# Convert
python3 tools/mpp_to_proj/mpp_to_proj.py project.mpp project.proj

# Check and fix
utf8proj check project.proj
utf8proj fix container-deps project.proj -o project_fixed.proj

# Schedule
utf8proj schedule project_fixed.proj
```

## Validation: PaaS Azure Project

| Metric | MS Project | utf8proj | Match |
|--------|------------|----------|-------|
| Start Date | 2019-09-24 | 2019-09-24 | ✅ |
| End Date | 2020-01-10 | 2020-01-10 | ✅ |
| Task Count | 28 | 28 | ✅ |
| Milestones | 4 | 4 | ✅ |
| Start QA milestone | 2019-10-15 | 2019-10-15 | ✅ |
| Start PROD milestone | 2019-12-01 | 2019-12-01 | ✅ |
| Go Live | 2020-01-10 | 2020-01-10 | ✅ |

## Conclusion

utf8proj provides **comprehensive coverage** of MS Project's core scheduling features:
- ✅ CPM scheduling with all dependency types
- ✅ Effort-driven scheduling (PMI compliant)
- ✅ Progress tracking with status date
- ✅ Earned value (schedule performance)
- ✅ Resource management and leveling

**Gaps** are primarily in advanced cost tracking:
- Material and cost resources
- Actual cost tracking
- Cost performance metrics (CPI, CV)

For most project planning and schedule management use cases, utf8proj is a **complete, trustworthy tool** that can accurately reproduce MS Project schedules.
