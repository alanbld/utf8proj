#!/usr/bin/env python3
"""
Convert PSPLIB instances to utf8proj (.proj) format.

PSPLIB is the standard benchmark library for Resource-Constrained Project
Scheduling Problems (RCPSP). This converter enables utf8proj to be validated
against known optimal solutions.

Usage:
    python3 psplib_to_proj.py <input.sm> [output.proj]
    python3 psplib_to_proj.py --batch <directory> <output_dir>

References:
    - PSPLIB: https://www.om-db.wi.tum.de/psplib/
    - Kolisch & Sprecher (1996): PSPLIB - A project scheduling library
"""

import re
import sys
import os
from pathlib import Path
from dataclasses import dataclass
from typing import Optional


@dataclass
class PSPLIBInstance:
    """Parsed PSPLIB instance data."""
    name: str
    jobs: int
    resources: int
    horizon: int
    # Job data: job_id -> (duration, successors, resource_demands)
    precedence: dict  # job_id -> list of successor job_ids
    durations: dict   # job_id -> duration
    demands: dict     # job_id -> {resource_id: demand}
    capacities: dict  # resource_id -> capacity


def parse_psplib(filepath: str) -> PSPLIBInstance:
    """Parse a PSPLIB .sm file."""
    with open(filepath, 'r') as f:
        content = f.read()

    name = Path(filepath).stem

    # Extract header info
    jobs_match = re.search(r'jobs \(incl\. supersource/sink \)\s*:\s*(\d+)', content)
    jobs = int(jobs_match.group(1)) if jobs_match else 0

    resources_match = re.search(r'renewable\s*:\s*(\d+)', content)
    num_resources = int(resources_match.group(1)) if resources_match else 4

    horizon_match = re.search(r'horizon\s*:\s*(\d+)', content)
    horizon = int(horizon_match.group(1)) if horizon_match else 100

    # Parse precedence relations
    precedence = {}
    prec_section = re.search(
        r'PRECEDENCE RELATIONS:.*?jobnr\.\s+#modes\s+#successors\s+successors\s*(.*?)(?:\*{5,}|REQUESTS)',
        content, re.DOTALL
    )
    if prec_section:
        for line in prec_section.group(1).strip().split('\n'):
            parts = line.split()
            if len(parts) >= 3:
                job_id = int(parts[0])
                num_successors = int(parts[2])
                successors = [int(s) for s in parts[3:3+num_successors]] if num_successors > 0 else []
                precedence[job_id] = successors

    # Parse requests/durations
    durations = {}
    demands = {}
    req_section = re.search(
        r'REQUESTS/DURATIONS:.*?-{5,}\s*(.*?)(?:\*{5,}|RESOURCEAVAILABILITIES|$)',
        content, re.DOTALL
    )
    if req_section:
        for line in req_section.group(1).strip().split('\n'):
            parts = line.split()
            if len(parts) >= 3 and parts[0].isdigit():
                job_id = int(parts[0])
                # mode = int(parts[1])  # We only handle single-mode
                duration = int(parts[2])
                durations[job_id] = duration

                # Resource demands (R1, R2, R3, R4, ...)
                job_demands = {}
                for i, demand in enumerate(parts[3:3+num_resources]):
                    if int(demand) > 0:
                        job_demands[f'R{i+1}'] = int(demand)
                demands[job_id] = job_demands

    # Parse resource capacities
    # Format:
    # RESOURCEAVAILABILITIES:
    #   R 1  R 2  R 3  R 4
    #    12   13    4   12
    capacities = {}
    cap_section = re.search(
        r'RESOURCEAVAILABILITIES:\s*\n\s*R\s+\d.*\n\s*(\d+(?:\s+\d+)*)',
        content
    )
    if cap_section:
        caps = cap_section.group(1).strip().split()
        for i, cap in enumerate(caps[:num_resources]):
            capacities[f'R{i+1}'] = int(cap)
    else:
        # Default capacities if not specified
        for i in range(num_resources):
            capacities[f'R{i+1}'] = 10

    return PSPLIBInstance(
        name=name,
        jobs=jobs,
        resources=num_resources,
        horizon=horizon,
        precedence=precedence,
        durations=durations,
        demands=demands,
        capacities=capacities
    )


def convert_to_proj(instance: PSPLIBInstance, optimal_makespan: Optional[int] = None) -> str:
    """Convert PSPLIB instance to utf8proj format."""
    lines = []

    # Header
    lines.append(f'# PSPLIB Instance: {instance.name}')
    lines.append(f'# Jobs: {instance.jobs}, Resources: {instance.resources}')
    if optimal_makespan:
        lines.append(f'# Optimal Makespan: {optimal_makespan} days')
    lines.append(f'# Source: https://www.om-db.wi.tum.de/psplib/')
    lines.append('')

    # Project declaration — assign continuous calendar for PSPLIB (no weekends)
    lines.append(f'project "{instance.name}" {{')
    lines.append('    start: 2026-01-05')
    lines.append('    calendar: continuous')
    lines.append('}')
    lines.append('')

    # PSPLIB uses continuous time (no weekends)
    lines.append('calendar "continuous" {')
    lines.append('    working_days: mon, tue, wed, thu, fri, sat, sun')
    lines.append('}')
    lines.append('')

    # Resources
    # Note: utf8proj capacity is 1.0 (100%) by default
    # PSPLIB demand/capacity ratio translates to utf8proj assignment percentage
    for res_id, capacity in sorted(instance.capacities.items()):
        lines.append(f'resource {res_id.lower()} "{res_id}" {{')
        lines.append(f'    rate: 100/day')
        lines.append(f'    # PSPLIB capacity: {capacity} units')
        lines.append('}')
    lines.append('')

    # Tasks (skip supersource job 1 and supersink which is the last job)
    # In PSPLIB, job 1 is supersource (duration 0) and job N is supersink (duration 0)
    real_jobs = sorted([j for j in instance.durations.keys()
                        if instance.durations[j] > 0])

    for job_id in real_jobs:
        duration = instance.durations[job_id]
        job_demands = instance.demands.get(job_id, {})
        successors = instance.precedence.get(job_id, [])

        # Find predecessors (jobs that have this job as successor)
        predecessors = []
        for pred_id, succ_list in instance.precedence.items():
            if job_id in succ_list and instance.durations.get(pred_id, 0) > 0:
                predecessors.append(pred_id)

        lines.append(f'task j{job_id} "Job {job_id}" {{')
        lines.append(f'    duration: {duration}d')

        # Resource assignments (combine all on one line)
        assignments = []
        for res_id, demand in sorted(job_demands.items()):
            if demand > 0:
                # Convert PSPLIB demand to percentage of capacity
                # E.g., demand 4 on capacity 12 = 33.33% (round to nearest %)
                capacity = instance.capacities.get(res_id, 10)
                pct = round(demand * 100 / capacity)
                assignments.append(f'{res_id.lower()}@{pct}%')
        if assignments:
            lines.append(f'    assign: {", ".join(assignments)}')

        # Dependencies
        if predecessors:
            pred_refs = ', '.join(f'j{p}' for p in sorted(predecessors))
            lines.append(f'    depends: {pred_refs}')

        lines.append('}')
    lines.append('')

    return '\n'.join(lines)


def parse_optimal_solutions(filepath: str) -> dict:
    """Parse PSPLIB optimal solutions file (e.g., j30opt.sm)."""
    solutions = {}
    with open(filepath, 'r') as f:
        for line in f:
            parts = line.split()
            if len(parts) >= 3 and parts[0].isdigit():
                param = int(parts[0])
                instance = int(parts[1])
                makespan = int(parts[2])
                # Instance naming: j30{param}_{instance}.sm
                solutions[(param, instance)] = makespan
    return solutions


def batch_convert(input_dir: str, output_dir: str, solutions_file: Optional[str] = None):
    """Convert all PSPLIB instances in a directory."""
    input_path = Path(input_dir)
    output_path = Path(output_dir)
    output_path.mkdir(parents=True, exist_ok=True)

    # Load optimal solutions if available
    solutions = {}
    if solutions_file and os.path.exists(solutions_file):
        solutions = parse_optimal_solutions(solutions_file)

    converted = 0
    for sm_file in sorted(input_path.glob('*.sm')):
        if 'opt' in sm_file.name or 'hrs' in sm_file.name or 'lb' in sm_file.name:
            continue  # Skip solution files

        try:
            instance = parse_psplib(str(sm_file))

            # Try to find optimal makespan
            optimal = None
            # Parse instance name like j301_1 -> param=1, instance=1
            match = re.match(r'j\d+(\d+)_(\d+)', sm_file.stem)
            if match and solutions:
                param, inst = int(match.group(1)), int(match.group(2))
                optimal = solutions.get((param, inst))

            proj_content = convert_to_proj(instance, optimal)

            output_file = output_path / f'{sm_file.stem}.proj'
            with open(output_file, 'w') as f:
                f.write(proj_content)

            converted += 1
            if converted % 100 == 0:
                print(f'  Converted {converted} instances...')

        except Exception as e:
            print(f'Warning: Failed to convert {sm_file.name}: {e}')

    return converted


def main():
    if len(sys.argv) < 2:
        print(__doc__)
        sys.exit(1)

    if sys.argv[1] == '--batch':
        if len(sys.argv) < 4:
            print('Usage: psplib_to_proj.py --batch <input_dir> <output_dir> [solutions_file]')
            sys.exit(1)

        input_dir = sys.argv[2]
        output_dir = sys.argv[3]
        solutions_file = sys.argv[4] if len(sys.argv) > 4 else None

        print(f'Converting PSPLIB instances from {input_dir}...')
        count = batch_convert(input_dir, output_dir, solutions_file)
        print(f'Converted {count} instances to {output_dir}')
    else:
        input_file = sys.argv[1]
        output_file = sys.argv[2] if len(sys.argv) > 2 else input_file.replace('.sm', '.proj')

        instance = parse_psplib(input_file)
        proj_content = convert_to_proj(instance)

        with open(output_file, 'w') as f:
            f.write(proj_content)

        print(f'Converted {input_file} -> {output_file}')
        print(f'  Jobs: {instance.jobs}, Resources: {instance.resources}')


if __name__ == '__main__':
    main()
