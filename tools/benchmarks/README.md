# utf8proj Benchmark Tools (RFC-0015)

External benchmark orchestration for validating utf8proj against standard scheduling datasets.

## Architecture

```
tools/
├── psplib_to_proj.py           # PSPLIB .sm → .proj converter
└── benchmarks/
    ├── benchmark_runner.py     # Core: schedule .proj files, compare vs optima, report
    ├── run_psplib.sh           # End-to-end PSPLIB benchmark (download → convert → run)
    └── README.md
```

The benchmark pipeline is:

1. **Convert** dataset instances to `.proj` format (e.g., `psplib_to_proj.py`)
2. **Schedule** each `.proj` file using `utf8proj schedule`
3. **Compare** makespan against known optimal solutions
4. **Report** gap analysis and performance metrics

This keeps dataset-specific logic out of the utf8proj binary.

## Quick Start

```bash
# Full PSPLIB J30 benchmark
./tools/benchmarks/run_psplib.sh

# With resource leveling
./tools/benchmarks/run_psplib.sh --leveling

# Reuse previously downloaded data
./tools/benchmarks/run_psplib.sh --skip-download

# Different dataset
./tools/benchmarks/run_psplib.sh --dataset j60
```

## benchmark_runner.py

Core orchestration script. Works with any directory of `.proj` files.

```bash
# Basic run
python3 tools/benchmarks/benchmark_runner.py --proj-dir data/psplib/j30_proj/

# With optimal solutions for gap analysis
python3 tools/benchmarks/benchmark_runner.py \
    --proj-dir data/psplib/j30_proj/ \
    --optima data/psplib/j30_opt.csv \
    --output results.json

# With leveling
python3 tools/benchmarks/benchmark_runner.py --proj-dir data/ --leveling
```

Options:
- `--proj-dir DIR` — Directory of `.proj` files (required)
- `--optima FILE` — CSV with optimal makespans (`name,value`)
- `--output FILE` — Write JSON report
- `--leveling` — Enable resource leveling
- `--gap PERCENT` — Acceptable gap threshold (default: 5.0%)
- `--dataset NAME` — Dataset name for the report

## PSPLIB Data

PSPLIB data must be downloaded from [TU Munich](https://www.om-db.wi.tum.de/psplib/).
The `run_psplib.sh` script will attempt automatic download via the `psplib` Python package:

```bash
pip install psplib
```

Or download manually and place `.sm` files in `data/psplib/j30_sm/`.
