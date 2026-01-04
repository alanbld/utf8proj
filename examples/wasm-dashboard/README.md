# utf8proj WASM Dashboard

Interactive browser-based project scheduling dashboard powered by WebAssembly.

## Features

- **Zero Installation**: Works directly in the browser, no server required
- **Real-time Scheduling**: CPM scheduling runs in < 200ms
- **Interactive Gantt Chart**: Canvas-based visualization with progress bars
- **Click-to-Edit Progress**: Update task completion and see immediate rescheduling
- **File Upload**: Drag-and-drop .proj files
- **Demo Projects**: Built-in examples to explore

## Quick Start

```bash
# Build the WASM package
cd crates/utf8proj-wasm
wasm-pack build --target web --out-dir ../../examples/wasm-dashboard/pkg

# Serve the dashboard
cd ../../examples/wasm-dashboard
python3 -m http.server 8080

# Open http://localhost:8080 in your browser
```

## Usage

1. **Select Demo**: Click one of the demo buttons (Simple, Software, CRM)
2. **Edit Progress**: Change the percentage in the task list
3. **Watch Gantt Update**: See the chart re-render with new dates
4. **Upload Your Own**: Drop a .proj file to schedule your project

## Keyboard Shortcuts

- `Enter`: Apply progress change
- Arrow keys in input: Increment/decrement progress

## Technical Details

- **WASM Size**: ~530KB (unoptimized)
- **Total Page**: < 600KB
- **Browser Support**: All modern browsers with WASM support
- **Offline**: Works completely offline once loaded

## API Exports

The WASM module exports:

- `schedule(source: string) -> string`: Parse and schedule, returns JSON
- `update_task_progress(source: string, task_id: string, percent: number) -> string`: Update progress in source
- `get_project_info(source: string) -> string`: Quick project metadata

## Building

Requires:
- Rust with `wasm32-unknown-unknown` target
- wasm-pack (`cargo install wasm-pack`)

```bash
rustup target add wasm32-unknown-unknown
wasm-pack build --target web
```
