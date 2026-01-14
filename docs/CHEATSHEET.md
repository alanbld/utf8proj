# utf8proj Cheat Sheet

**Keep at your desk for quick reference**

---

## Safe to Edit

| What | Example | Notes |
|------|---------|-------|
| **Dates** | `2026-01-15` | Always Year-Month-Day |
| **Durations** | `5d` `2w` `1m` | d=days, w=weeks, m=months |
| **Progress** | `complete: 75%` | Update as work completes |
| **Names** | `"Sprint 1"` | Text inside quotes |
| **Rates** | `rate: 800/day` | Change the number |
| **Capacity** | `capacity: 0.5` | 1.0=full, 0.5=half time |
| **Priority** | `priority: 800` | Higher = scheduled sooner |
| **Notes** | `note: "..."` | Add context |

---

## Do Not Change

| What | Example | Why |
|------|---------|-----|
| Keywords | `task` `project` `resource` | System words |
| Braces | `{ }` | Structure markers |
| Task IDs | `task sprint1` | Referenced elsewhere |
| Colons | `duration:` | Syntax |

---

## Duration Formats

```
5d     = 5 days
2w     = 2 weeks (10 working days)
1m     = 1 month (20 working days)
40h    = 40 hours (5 days)
```

---

## Dependency Types

| Code | Meaning | Use When |
|------|---------|----------|
| FS | Finish-to-Start | B starts after A finishes (default) |
| SS | Start-to-Start | B starts when A starts |
| FF | Finish-to-Finish | B finishes when A finishes |
| SF | Start-to-Finish | B finishes when A starts |

**With lag:** `depends: taskA SS +2d` (start 2 days after A starts)

---

## Common Edits

**Update progress:**
```
complete: 75%    ← change number
```

**Change duration:**
```
duration: 5d     ← change to 3d, 1w, etc.
```

**Add vacation:**
```
leave: 2026-03-15..2026-03-22
```

**Add holiday:**
```
holiday "Easter" 2026-04-06
```

---

## Keyboard Shortcuts (VS Code)

| Action | Windows | Mac |
|--------|---------|-----|
| Save | Ctrl+S | Cmd+S |
| Undo | Ctrl+Z | Cmd+Z |
| Redo | Ctrl+Y | Cmd+Shift+Z |
| Find | Ctrl+F | Cmd+F |
| Autocomplete | Ctrl+Space | Ctrl+Space |

---

## Squiggle Guide

| Color | Meaning | Action |
|-------|---------|--------|
| 🔴 Red | Error | Must fix |
| 🟡 Yellow | Warning | Review |
| None | Valid | Good to go |

**Hover over squiggles** to see what's wrong.

---

## If Something Breaks

1. **Undo:** Press Ctrl+Z (Cmd+Z on Mac) until squiggle disappears
2. **Don't save:** Close file without saving, reopen
3. **Ask for help:** Share the error message with your team lead

---

## File Structure

```
project "Name" {
    start: YYYY-MM-DD
}

resource id "Display Name" {
    rate: 000/day
}

task id "Display Name" {
    duration: 0d
    assign: resource_id
    depends: other_task_id
    complete: 0%
}

milestone id "Name" {
    depends: task_id
}
```

---

*utf8proj v0.2.0 | https://github.com/alanbld/utf8proj*
