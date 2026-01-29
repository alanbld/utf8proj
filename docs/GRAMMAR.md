# utf8proj Grammar Specification

This document defines the grammar for utf8proj project files (`.proj` extension) in BNF notation.

## File Structure

```bnf
<project-file> ::= <declaration>*

<declaration>  ::= <project-decl>
                 | <calendar-decl>
                 | <resource-decl>
                 | <resource-profile-decl>
                 | <trait-decl>
                 | <task-decl>
                 | <milestone-decl>
                 | <report-decl>
                 | <constraint-decl>
```

## Project Declaration

```bnf
<project-decl> ::= "project" <string> "{" <project-attr>* "}"

<project-attr> ::= "start" ":" <date>
                 | "end" ":" <date>
                 | "currency" ":" <identifier>
                 | "calendar" ":" <identifier>
                 | "timezone" ":" <timezone-value>
                 | "status_date" ":" <date>

<timezone-value> ::= [A-Za-z/_]+
```

## Calendar Declaration

```bnf
<calendar-decl> ::= "calendar" <string> "{" <calendar-attr>* "}"

<calendar-attr> ::= "working_hours" ":" <time-range-list>
                  | "working_days" ":" <day-list>
                  | "holiday" <string> (<date-range> | <date>)

<time-range-list> ::= <time-range> ("," <time-range>)*
<time-range>      ::= <time> "-" <time>
<time>            ::= [0-9]{2} ":" [0-9]{2}

<day-list> ::= <day> ("-" <day> | ("," <day>)*)
<day>      ::= "mon" | "tue" | "wed" | "thu" | "fri" | "sat" | "sun"
```

## Resource Declaration

```bnf
<resource-decl> ::= "resource" <identifier> <string>? "{" <resource-attr>* "}"

<resource-attr> ::= "specializes" ":" <identifier>
                  | "availability" ":" <number>
                  | "rate" ":" <money>
                  | "rate" ":" "{" <rate-range-attr>* "}"
                  | "capacity" ":" <number>
                  | "calendar" ":" <identifier>
                  | "efficiency" ":" <number>
                  | "email" ":" <string>
                  | "role" ":" <string>
                  | "leave" ":" <date-range>

<money> ::= <number> "/" <time-unit>
<time-unit> ::= "hour" | "day" | "week" | "month"

<rate-range-attr> ::= "min" ":" <number>
                    | "max" ":" <number>
                    | "currency" ":" <identifier>
```

## Resource Profile Declaration

```bnf
<resource-profile-decl> ::= "resource_profile" <identifier> <string>? "{" <profile-attr>* "}"

<profile-attr> ::= "description" ":" <string>
                 | "specializes" ":" <identifier>
                 | "skills" ":" "[" <identifier-list> "]"
                 | "traits" ":" "[" <identifier-list> "]"
                 | "rate" ":" "{" <rate-range-attr>* "}"
                 | "calendar" ":" <identifier>
                 | "efficiency" ":" <number>
```

## Trait Declaration

```bnf
<trait-decl> ::= "trait" <identifier> <string>? "{" <trait-attr>* "}"

<trait-attr> ::= "description" ":" <string>
               | "rate_multiplier" ":" <number>
```

## Task Declaration

```bnf
<task-decl> ::= "task" <identifier> <string> "{" <task-body> "}"

<task-body> ::= (<task-attr> | <task-decl> | <milestone-decl>)*

<task-attr> ::= "summary" ":" <string>
              | "effort" ":" <duration>
              | "duration" ":" <duration>
              | "depends" ":" <dependency-list>
              | "assign" ":" <resource-ref-list>
              | "priority" ":" <integer>
              | <constraint-type> ":" <date>
              | "milestone" ":" <boolean>
              | "complete" ":" <percentage>
              | "actual_start" ":" <date>
              | "actual_finish" ":" <date>
              | "status" ":" <status-keyword>
              | "note" ":" <string>
              | "tag" ":" <identifier-list>
              | "cost" ":" <number>
              | "payment" ":" <number>

<constraint-type> ::= "must_start_on"
                    | "must_finish_on"
                    | "start_no_earlier_than"
                    | "start_no_later_than"
                    | "finish_no_earlier_than"
                    | "finish_no_later_than"

<status-keyword> ::= "not_started" | "in_progress" | "complete"
                   | "blocked" | "at_risk" | "on_hold"
```

## Milestone Declaration

```bnf
<milestone-decl> ::= "milestone" <identifier> <string> "{" <milestone-attr>* "}"

<milestone-attr> ::= "summary" ":" <string>
                   | "depends" ":" <dependency-list>
                   | "note" ":" <string>
                   | "payment" ":" <number>
```

## Dependencies

```bnf
<dependency-list> ::= <dependency> ("," <dependency>)*

<dependency> ::= <task-ref> <dep-modifier>?

<task-ref> ::= <identifier> ("." <identifier>)*

<dep-modifier> ::= <dep-lag>
                 | <dep-type>
                 | <dep-percentage>

<dep-lag>  ::= ("+" | "-") <duration>
<dep-type> ::= "FS" | "SS" | "FF" | "SF"
<dep-percentage> ::= "." <percentage>
```

### Dependency Types

| Type | Name | Description |
|------|------|-------------|
| `FS` | Finish-to-Start | Successor starts after predecessor finishes (default) |
| `SS` | Start-to-Start | Successor starts when predecessor starts |
| `FF` | Finish-to-Finish | Successor finishes when predecessor finishes |
| `SF` | Start-to-Finish | Successor finishes when predecessor starts |

## Resource References

```bnf
<resource-ref-list> ::= <resource-ref> ("," <resource-ref>)*

<resource-ref> ::= <identifier> <resource-modifier>?

<resource-modifier> ::= "*" <integer>           # Quantity (e.g., dev*2)
                      | "@" <percentage>        # Allocation (e.g., dev@50%)
                      | "(" <percentage> ")"    # Allocation (e.g., dev(50%))
```

## Report Declaration

```bnf
<report-decl> ::= "report" <identifier> <string> "{" <report-attr>* "}"

<report-attr> ::= "title" ":" <string>
                | "type" ":" <identifier>
                | "tasks" ":" <task-filter>
                | "resources" ":" <resource-filter>
                | "columns" ":" <column-list>
                | "critical_path" ":" ("highlight" | "show" | "hide")
                | "timeframe" ":" <date-range>
                | "format" ":" <identifier>
                | "show" ":" <identifier-list>
                | "scale" ":" <identifier>
                | "width" ":" <integer>
                | "breakdown" ":" <identifier-list>
                | "period" ":" <identifier>

<task-filter>     ::= "all" | <identifier-list>
<resource-filter> ::= "all" | "show" | "hide" | <identifier-list>
<column-list>     ::= <identifier> ("," <identifier>)*
```

## Constraint Declaration

```bnf
<constraint-decl> ::= "constraint" <identifier> "{" <constraint-attr>* "}"

<constraint-attr> ::= "type" ":" ("soft" | "hard")
                    | "target" ":" <task-ref>
                    | "condition" ":" <constraint-expr>
                    | "priority" ":" <integer>
                    | "resources" ":" <resource-filter>

<constraint-expr> ::= <any-text-until-newline>
```

## Primitives

```bnf
<duration> ::= <number> <duration-unit>
<duration-unit> ::= "h" | "d" | "w" | "m"

<date> ::= [0-9]{4} "-" [0-9]{2} "-" [0-9]{2}
<date-range> ::= <date> ".." <date>

<number>  ::= "-"? [0-9]+ ("." [0-9]+)?
<integer> ::= "-"? [0-9]+
<percentage> ::= [0-9]+ "%"

<boolean> ::= "true" | "false"

<string> ::= '"' <string-char>* '"'
<string-char> ::= <escape-seq> | <any-char-except-quote-or-backslash>
<escape-seq> ::= '\' <any-char>

<identifier> ::= [A-Za-z_] [A-Za-z0-9_-]*
<identifier-list> ::= <identifier> ("," <identifier>)*
```

### Escape Sequences

| Sequence | Character |
|----------|-----------|
| `\"` | Double quote |
| `\\` | Backslash |
| `\n` | Newline |
| `\t` | Tab |

## Comments

```bnf
<comment> ::= "#" <any-text-until-newline>
```

Comments start with `#` and extend to the end of the line.

## Whitespace

Whitespace (spaces, tabs, newlines) is ignored between tokens.

## Examples

### Minimal Project

```proj
project "Hello World" {
    start: 2025-01-01
}

task hello "Hello Task" {
    duration: 1d
}
```

### Full Project

```proj
project "Website Redesign" {
    start: 2025-02-01
    end: 2025-06-30
    currency: USD
}

calendar "standard" {
    working_hours: 09:00-12:00, 13:00-17:00
    working_days: mon-fri
    holiday "Independence Day" 2025-07-04
}

resource dev "Developer" {
    rate: 850/day
    capacity: 1.0
    calendar: standard
}

task design "Design Phase" {
    task wireframes "Wireframes" {
        effort: 3d
        assign: dev
    }
    task mockups "Mockups" {
        effort: 5d
        assign: dev
        depends: wireframes
    }
}

task development "Development" {
    depends: design.mockups

    task frontend "Frontend Development" {
        effort: 10d
        assign: dev
        priority: 800
    }
}

milestone launch "Launch" {
    depends: development
}

report gantt "timeline.svg" {
    tasks: all
    critical_path: highlight
}
```

### Dependencies with Modifiers

```proj
task a "Task A" { duration: 5d }
task b "Task B" { duration: 3d, depends: a }           # FS (default)
task c "Task C" { duration: 2d, depends: a +2d }       # FS with 2-day lag
task d "Task D" { duration: 4d, depends: a SS }        # Start-to-Start
task e "Task E" { duration: 3d, depends: a FF }        # Finish-to-Finish
task f "Task F" { duration: 2d, depends: a -1d }       # FS with 1-day lead
```

### Resource Profiles

```proj
trait senior {
    description: "5+ years experience"
    rate_multiplier: 1.3
}

resource_profile developer {
    description: "Software developer"
    rate: { min: 500, max: 800, currency: USD }
}

resource_profile senior_dev {
    specializes: developer
    traits: [senior]
}

resource alice {
    specializes: senior_dev
    rate: 900/day
    availability: 0.8
}

task impl "Implementation" {
    effort: 40d
    assign: developer*2, alice
}
```
