" Vim syntax file for utf8proj (.proj files)
" Language: utf8proj
" Maintainer: utf8proj team
" Latest Revision: 2026-01-09

if exists("b:current_syntax")
  finish
endif

" Comments
syn match projComment "#.*$" contains=projTodo
syn keyword projTodo TODO FIXME XXX NOTE contained

" Block keywords
syn keyword projKeyword project task milestone resource resource_profile calendar trait report constraint

" Property keywords
syn keyword projProperty start end currency timezone
syn keyword projProperty effort duration depends assign priority complete actual_start actual_finish status note tag cost payment summary
syn keyword projProperty rate capacity efficiency availability specializes email role leave
syn keyword projProperty working_hours working_days holiday
syn keyword projProperty description skills traits rate_multiplier min max
syn keyword projProperty title type tasks resources columns critical_path timeframe format show scale width breakdown period
syn keyword projProperty target condition

" Constraint types
syn keyword projProperty must_start_on must_finish_on start_no_earlier_than start_no_later_than finish_no_earlier_than finish_no_later_than

" Dependency types
syn keyword projConstant FS SS FF SF

" Status keywords
syn keyword projConstant not_started in_progress complete blocked at_risk on_hold

" Days
syn keyword projConstant mon tue wed thu fri sat sun

" Booleans
syn keyword projBoolean true false

" Time units
syn keyword projConstant hour day week month

" Report keywords
syn keyword projConstant all highlight hide

" Numbers
syn match projNumber "-\?\<\d\+\>"
syn match projNumber "-\?\<\d\+\.\d\+\>"

" Dates (2025-02-01)
syn match projDate "\<\d\{4\}-\d\{2\}-\d\{2\}\>"

" Durations (5d, 2w, 8h, 3m)
syn match projDuration "\<\d\+[hdwm]\>"

" Percentages (50%)
syn match projPercentage "\<\d\+%"

" Time (09:00)
syn match projTime "\<\d\{2\}:\d\{2\}\>"

" Operators
syn match projOperator "\.\."
syn match projOperator "@"
syn match projOperator "\*"
syn match projOperator "/"
syn match projOperator "+"
syn match projOperator "-"

" Strings
syn region projString start='"' end='"' skip='\\"' contains=projEscape
syn match projEscape "\\." contained

" Braces and punctuation
syn match projBrace "[{}]"
syn match projBracket "\[\|\]"

" Linking to standard highlight groups
hi def link projComment Comment
hi def link projTodo Todo
hi def link projKeyword Keyword
hi def link projProperty Type
hi def link projConstant Constant
hi def link projBoolean Boolean
hi def link projNumber Number
hi def link projDate Special
hi def link projDuration Special
hi def link projPercentage Special
hi def link projTime Special
hi def link projOperator Operator
hi def link projString String
hi def link projEscape SpecialChar
hi def link projBrace Delimiter
hi def link projBracket Delimiter

let b:current_syntax = "proj"
