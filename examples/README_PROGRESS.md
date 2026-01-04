# Progress Tracking in utf8proj

## Overview

Progress tracking allows you to monitor actual project execution against your baseline plan. Track completion percentages, actual dates, task status, and get accurate forecasts based on remaining work.

## Key Concepts

### 1. Completion Percentage
Track how much work is complete (0-100%):
```proj
task design "Design Phase" {
    duration: 10d
    complete: 60%    # 60% of work complete
}
```

**Remaining work calculation:**
- Formula: `remaining = duration × (1 - complete% / 100)`
- Example: 10 days × (1 - 60/100) = 4 days remaining

### 2. Actual Dates
Record when work actually started/finished:
```proj
task implementation "Implementation" {
    duration: 20d
    complete: 100%
    actual_start: 2026-01-15    # When work began
    actual_finish: 2026-02-10   # When work completed
}
```

**Actual dates override calculations:**
- `actual_finish` automatically sets `complete: 100%`
- Forecast dates use actuals when available

### 3. Task Status
Track the current state of work:
```proj
task backend "Backend Development" {
    duration: 15d
    status: in_progress    # or: not_started, complete, blocked, at_risk, on_hold
}
```

**Status Types:**
- `not_started` - Work hasn't begun
- `in_progress` - Actively being worked on
- `complete` - All work finished
- `blocked` - Cannot proceed (waiting on something)
- `at_risk` - May not finish on time
- `on_hold` - Temporarily paused

### 4. Status Date
Set the "as of" date for progress reporting:
```proj
project "My Project" {
    start: 2026-01-01
    status_date: 2026-02-15    # Report progress as of Feb 15
}
```

## Complete Example

```proj
project "Website Redesign" {
    start: 2026-01-01
    status_date: 2026-01-15
}

# COMPLETE: Finished early
task planning "Planning" {
    duration: 5d
    complete: 100%
    actual_start: 2026-01-01
    actual_finish: 2026-01-08    # Finished 2 days early!
    status: complete
}

# IN PROGRESS: On track
task design "Design" {
    duration: 10d
    complete: 60%
    actual_start: 2026-01-06
    status: in_progress
    depends: planning
}

# BLOCKED: Cannot start yet
task development "Development" {
    duration: 20d
    complete: 0%
    status: blocked    # Waiting for design approval
    depends: design
}
```

## CLI Usage

### View Progress
```bash
# Schedule with progress information
utf8proj schedule project.proj --show-progress

# Output shows:
# - Completion percentage
# - Current status
# - Forecast dates (adjusted for progress)
# - Remaining duration
# - Critical path indicator

Task         %Done Status         Start        Finish         Remain Critical
-----------------------------------------------------------------------------
planning     100%  Complete       2026-01-01   2026-01-08        0d
design        60%  In Progress    2026-01-06   2026-01-09        4d *
development    0%  Blocked        2026-01-10   2026-02-05       20d *
```

### Update Progress
**Manual editing:**
```bash
# Edit your .proj file directly
vim project.proj

# Update:
# - complete: percentage
# - actual_start/actual_finish: dates
# - status: current state
```

**Future:** `utf8proj progress` command for quick updates

## Progress Calculation Details

### Linear Interpolation (Default)
```
remaining_duration = original_duration × (1 - percent_complete / 100)

Example:
  Original: 20 days
  Complete: 50%
  Remaining: 20 × (1 - 0.5) = 10 days
```

### Forecast Dates
```
forecast_start = actual_start (if started) OR calculated from dependencies
forecast_finish = forecast_start + remaining_duration

Example:
  Actual start: 2026-02-01
  Remaining: 10 days
  Forecast finish: 2026-02-15 (accounting for weekends/holidays)
```

### Container Tasks (Future)
Parent task progress will be calculated from children using weighted average:
```
container_progress = Σ(child_duration × child_progress) / Σ(child_duration)

Example:
  Task A: 5 days, 100% complete
  Task B: 15 days, 20% complete
  Container: (5×100 + 15×20) / (5+15) = 800/20 = 40% complete
```

## Best Practices

### 1. Regular Updates
Update progress weekly or at key milestones:
```proj
# Weekly status update
task sprint3 "Sprint 3" {
    duration: 10d
    complete: 70%           # Updated every Friday
    actual_start: 2026-02-01
    status: in_progress
}
```

### 2. Use Status Effectively
Be specific about blockers and risks:
```proj
task integration "Integration Testing" {
    duration: 5d
    complete: 30%
    status: at_risk    # API changes breaking tests
}

task deployment "Deployment" {
    duration: 2d
    status: blocked    # Waiting on infrastructure approval
}
```

### 3. Record Actual Dates
Always record when work actually starts/finishes:
```proj
task design "Design Phase" {
    duration: 10d
    complete: 100%
    actual_start: 2026-01-15    # Planned: 2026-01-10 (started 5 days late)
    actual_finish: 2026-01-29   # Planned: 2026-01-24 (finished 5 days late)
    status: complete
}
```

This creates historical data for:
- Variance analysis
- Future estimation improvements
- Team velocity tracking

### 4. Set Status Date
Always set `status_date` to know when progress was reported:
```proj
project "My Project" {
    start: 2026-01-01
    status_date: 2026-02-15    # Progress as of this date
}
```

## Examples

See example files:
- `examples/04_progress_tracking.proj` - Simple progress example
- `examples/05_software_with_progress.proj` - Real-world software project

## Future Enhancements

Coming soon:
- **Container Progress:** Automatic calculation from children
- **Variance Reports:** Planned vs actual vs forecast
- **Progress CLI:** `utf8proj progress --task=X --complete=75`
- **History Playback:** Visualize progress evolution over time
- **EVM Metrics:** Earned Value Management (PV, EV, AC, CPI, SPI)
- **Excel Export:** Progress bars and status color coding

## Troubleshooting

### Issue: "Remaining duration seems wrong"
**Solution:** Check your `complete` percentage. Remember it's a percentage of work, not time:
```proj
# If task is taking longer than expected:
task backend "Backend" {
    duration: 20d
    complete: 50%    # 50% of work done
    # But you've already used 15 days!
    # Remaining: 20 × 0.5 = 10 days (total = 25 days)
}
```

### Issue: "Status doesn't match my completion percentage"
**Solution:** Status is independent of percentage. You can be "at_risk" even at high completion:
```proj
task testing "Testing" {
    duration: 10d
    complete: 90%        # Almost done
    status: at_risk      # But major bug found!
}
```

### Issue: "Forecast dates look wrong"
**Solution:** Ensure `actual_start` is set if work has begun:
```proj
task design "Design" {
    duration: 10d
    complete: 50%
    actual_start: 2026-01-15    # IMPORTANT: Set this!
    # Without actual_start, forecast may be calculated from wrong baseline
}
```

## Quick Reference

### Progress Syntax
```proj
task my_task "My Task" {
    # Basic
    duration: 10d
    
    # Progress tracking
    complete: 60%                      # 0-100
    actual_start: 2026-01-15          # YYYY-MM-DD
    actual_finish: 2026-01-25         # YYYY-MM-DD (sets complete to 100%)
    status: in_progress               # not_started | in_progress | complete | 
                                       # blocked | at_risk | on_hold
}
```

### CLI Commands
```bash
# View progress
utf8proj schedule project.proj --show-progress

# View status (future)
utf8proj status project.proj

# Update progress (future)
utf8proj progress --task=my_task --complete=75
```

---

**Progress tracking transforms utf8proj from a planning tool into a complete project management solution!** 🎉
