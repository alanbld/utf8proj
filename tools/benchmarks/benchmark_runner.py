#!/usr/bin/env python3
"""
External benchmark runner for utf8proj (RFC-0015).

Orchestrates: convert -> schedule -> compare -> report.
Works with any dataset that has a converter producing .proj files.

Usage:
    # Run against a directory of .proj files
    python3 benchmark_runner.py --proj-dir data/j30_proj/ --output results.json

    # Run with optimal solutions for gap analysis
    python3 benchmark_runner.py --proj-dir data/j30_proj/ --optima data/j30_optima.csv

    # Run with leveling enabled
    python3 benchmark_runner.py --proj-dir data/j30_proj/ --leveling
"""

import argparse
import csv
import json
import os
import re
import subprocess
import sys
import time
from dataclasses import dataclass, asdict
from pathlib import Path
from typing import Optional


@dataclass
class InstanceResult:
    """Result of scheduling a single instance."""
    name: str
    tasks: int
    makespan_days: Optional[float]
    optimal_makespan: Optional[int]
    gap_percent: Optional[float]
    schedule_time_ms: float
    status: str  # "ok", "error", "optimal", "acceptable", "suboptimal"
    error_message: Optional[str] = None


@dataclass
class BenchmarkReport:
    """Aggregate benchmark report."""
    dataset: str
    total_instances: int
    successful: int
    errors: int
    optimal_count: int
    acceptable_count: int
    suboptimal_count: int
    no_reference_count: int
    avg_gap_percent: float
    max_gap_percent: float
    total_time_s: float
    avg_time_ms: float
    gap_threshold: float
    leveling: bool
    results: list


def find_utf8proj() -> str:
    """Find the utf8proj binary."""
    # Check if in PATH
    for name in ["utf8proj", "./target/release/utf8proj", "./target/debug/utf8proj"]:
        try:
            result = subprocess.run(
                [name, "--version"],
                capture_output=True, text=True, timeout=5
            )
            if result.returncode == 0:
                return name
        except (FileNotFoundError, subprocess.TimeoutExpired):
            continue

    # Check workspace root
    script_dir = Path(__file__).parent
    workspace_root = script_dir.parent.parent
    for profile in ["release", "debug"]:
        candidate = workspace_root / "target" / profile / "utf8proj"
        if candidate.exists():
            return str(candidate)

    print("ERROR: utf8proj binary not found. Run 'cargo build --release' first.",
          file=sys.stderr)
    sys.exit(1)


def load_optima(optima_path: str) -> dict:
    """Load optimal makespan values from a CSV file.

    Expected format (no header):
        instance_name,optimal_makespan
    Or PSPLIB format:
        param instance makespan
    """
    optima = {}
    with open(optima_path) as f:
        content = f.read().strip()

    for line in content.split('\n'):
        line = line.strip()
        if not line or line.startswith('#'):
            continue

        # Try CSV format: name,makespan
        if ',' in line:
            parts = line.split(',')
            if len(parts) >= 2:
                name = parts[0].strip()
                try:
                    optima[name] = int(parts[1].strip())
                except ValueError:
                    pass
            continue

        # Try PSPLIB format: param instance makespan
        parts = line.split()
        if len(parts) >= 3:
            try:
                param, instance, makespan = int(parts[0]), int(parts[1]), int(parts[2])
                # Generate j30-style name
                name = f"j30{param}_{instance}"
                optima[name] = makespan
            except ValueError:
                pass

    return optima


def schedule_instance(
    utf8proj: str,
    proj_file: str,
    leveling: bool = False,
) -> tuple:
    """Schedule a single .proj file using utf8proj.

    Returns (makespan_days, tasks_count, time_ms, error_msg).
    """
    cmd = [utf8proj, "schedule", proj_file]
    if leveling:
        cmd.append("-l")

    start = time.monotonic()
    try:
        result = subprocess.run(
            cmd, capture_output=True, text=True, timeout=60
        )
        elapsed_ms = (time.monotonic() - start) * 1000
    except subprocess.TimeoutExpired:
        return None, 0, 60000, "timeout"

    if result.returncode != 0:
        # Extract error from stderr
        error = result.stderr.strip() or result.stdout.strip()
        return None, 0, elapsed_ms, error[:200]

    # Parse output for makespan and task count
    output = result.stdout
    makespan = None
    tasks = 0

    # Look for project duration in output
    # I001 format: "Project scheduled: 45 days, 12 tasks on critical path"
    for line in output.split('\n'):
        # Match duration patterns
        dur_match = re.search(r'(\d+(?:\.\d+)?)\s*(?:calendar\s+)?days?', line, re.IGNORECASE)
        if dur_match and makespan is None:
            makespan = float(dur_match.group(1))

        # Match task count
        task_match = re.search(r'(\d+)\s+tasks?\b', line, re.IGNORECASE)
        if task_match:
            tasks = max(tasks, int(task_match.group(1)))

    return makespan, tasks, elapsed_ms, None


def run_benchmark(
    proj_dir: str,
    optima: dict,
    leveling: bool = False,
    gap_threshold: float = 5.0,
    dataset_name: str = "unknown",
) -> BenchmarkReport:
    """Run benchmarks on all .proj files in a directory."""
    utf8proj = find_utf8proj()
    proj_path = Path(proj_dir)

    proj_files = sorted(proj_path.glob("*.proj"))
    if not proj_files:
        print(f"ERROR: No .proj files found in {proj_dir}", file=sys.stderr)
        sys.exit(1)

    print(f"utf8proj PSPLIB Benchmark Runner (RFC-0015)")
    print(f"============================================")
    print(f"  Binary:     {utf8proj}")
    print(f"  Directory:  {proj_dir}")
    print(f"  Instances:  {len(proj_files)}")
    print(f"  Optima:     {len(optima)} loaded")
    print(f"  Leveling:   {'yes' if leveling else 'no'}")
    print(f"  Gap threshold: {gap_threshold}%")
    print()

    results = []
    total_start = time.monotonic()

    for i, proj_file in enumerate(proj_files):
        name = proj_file.stem

        makespan, tasks, time_ms, error = schedule_instance(
            utf8proj, str(proj_file), leveling
        )

        optimal = optima.get(name)

        if error:
            status = "error"
            gap = None
        elif makespan is not None and optimal is not None:
            gap = ((makespan - optimal) / optimal) * 100
            if abs(gap) < 0.01:
                status = "optimal"
            elif gap <= gap_threshold:
                status = "acceptable"
            else:
                status = "suboptimal"
        elif makespan is not None:
            gap = None
            status = "ok"
        else:
            gap = None
            status = "error"
            error = "no makespan in output"

        result = InstanceResult(
            name=name,
            tasks=tasks,
            makespan_days=makespan,
            optimal_makespan=optimal,
            gap_percent=round(gap, 2) if gap is not None else None,
            schedule_time_ms=round(time_ms, 2),
            status=status,
            error_message=error,
        )
        results.append(result)

        # Progress indicator
        if (i + 1) % 50 == 0 or (i + 1) == len(proj_files):
            print(f"  [{i+1}/{len(proj_files)}] processed...")

    total_time = time.monotonic() - total_start

    # Compute summary
    successful = [r for r in results if r.status != "error"]
    errors = [r for r in results if r.status == "error"]
    optimal_count = len([r for r in results if r.status == "optimal"])
    acceptable_count = len([r for r in results if r.status == "acceptable"])
    suboptimal_count = len([r for r in results if r.status == "suboptimal"])
    no_ref = len([r for r in results if r.status == "ok"])

    gaps = [r.gap_percent for r in results if r.gap_percent is not None]
    avg_gap = sum(gaps) / len(gaps) if gaps else 0.0
    max_gap = max(gaps) if gaps else 0.0
    times = [r.schedule_time_ms for r in results]
    avg_time = sum(times) / len(times) if times else 0.0

    return BenchmarkReport(
        dataset=dataset_name,
        total_instances=len(results),
        successful=len(successful),
        errors=len(errors),
        optimal_count=optimal_count,
        acceptable_count=acceptable_count,
        suboptimal_count=suboptimal_count,
        no_reference_count=no_ref,
        avg_gap_percent=round(avg_gap, 2),
        max_gap_percent=round(max_gap, 2),
        total_time_s=round(total_time, 2),
        avg_time_ms=round(avg_time, 2),
        gap_threshold=gap_threshold,
        leveling=leveling,
        results=[asdict(r) for r in results],
    )


def print_report(report: BenchmarkReport):
    """Print a human-readable benchmark report."""
    print()
    print("=" * 72)
    print(f"  BENCHMARK REPORT: {report.dataset}")
    print("=" * 72)
    print()
    print(f"  Total instances:    {report.total_instances}")
    print(f"  Successful:         {report.successful}")
    print(f"  Errors:             {report.errors}")
    print()

    if report.optimal_count + report.acceptable_count + report.suboptimal_count > 0:
        total_with_ref = report.optimal_count + report.acceptable_count + report.suboptimal_count
        print(f"  Gap Analysis (vs optimal, {total_with_ref} instances):")
        print(f"    Optimal (0%):         {report.optimal_count}")
        print(f"    Acceptable (≤{report.gap_threshold}%):  {report.acceptable_count}")
        print(f"    Suboptimal (>{report.gap_threshold}%):  {report.suboptimal_count}")
        print(f"    Average gap:          {report.avg_gap_percent:.2f}%")
        print(f"    Max gap:              {report.max_gap_percent:.2f}%")
        print()

    if report.no_reference_count > 0:
        print(f"  No reference (feasible): {report.no_reference_count}")
        print()

    print(f"  Performance:")
    print(f"    Total time:           {report.total_time_s:.2f}s")
    print(f"    Average per instance: {report.avg_time_ms:.2f}ms")
    print()

    # Show errors if any
    error_results = [r for r in report.results if r["status"] == "error"]
    if error_results:
        print(f"  Errors ({len(error_results)}):")
        for r in error_results[:10]:
            print(f"    {r['name']}: {r['error_message']}")
        if len(error_results) > 10:
            print(f"    ... and {len(error_results) - 10} more")
        print()

    # Show worst gaps
    gaps = [(r["name"], r["gap_percent"]) for r in report.results
            if r["gap_percent"] is not None and r["gap_percent"] > report.gap_threshold]
    if gaps:
        gaps.sort(key=lambda x: x[1], reverse=True)
        print(f"  Worst gaps (>{report.gap_threshold}%, showing top 10):")
        for name, gap in gaps[:10]:
            print(f"    {name}: {gap:.1f}%")
        print()

    print("=" * 72)


def main():
    parser = argparse.ArgumentParser(
        description="utf8proj external benchmark runner (RFC-0015)"
    )
    parser.add_argument(
        "--proj-dir", required=True,
        help="Directory containing .proj files to benchmark"
    )
    parser.add_argument(
        "--optima",
        help="CSV or PSPLIB-format file with optimal makespans"
    )
    parser.add_argument(
        "--output", "-o",
        help="Output JSON report file"
    )
    parser.add_argument(
        "--leveling", "-l", action="store_true",
        help="Enable resource leveling"
    )
    parser.add_argument(
        "--gap", type=float, default=5.0,
        help="Acceptable gap threshold percentage (default: 5.0)"
    )
    parser.add_argument(
        "--dataset", default="unknown",
        help="Dataset name for the report"
    )

    args = parser.parse_args()

    optima = {}
    if args.optima:
        optima = load_optima(args.optima)
        print(f"Loaded {len(optima)} optimal solutions from {args.optima}")

    report = run_benchmark(
        proj_dir=args.proj_dir,
        optima=optima,
        leveling=args.leveling,
        gap_threshold=args.gap,
        dataset_name=args.dataset,
    )

    print_report(report)

    if args.output:
        with open(args.output, 'w') as f:
            json.dump(asdict(report), f, indent=2)
        print(f"Report written to {args.output}")

    # Exit with error if any instances failed
    if report.errors > 0:
        sys.exit(1)


if __name__ == "__main__":
    main()
