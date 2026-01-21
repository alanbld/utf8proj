#!/usr/bin/env python3
"""
Generate a realistic large project file for utf8proj stress testing.

Structure:
- 10 departments (Engineering, QA, DevOps, etc.)
- 100 resources per department = 1,000 resources
- 10 major phases
- 10 work packages per phase
- 10 features per work package
- 10 tasks per feature = 10,000 tasks

Dependencies follow realistic patterns:
- Tasks within a feature are sequential or parallel
- Features depend on previous features in same work package
- Work packages depend on previous WP milestones
- Phases depend on previous phase completion
"""

import random
from datetime import date, timedelta

random.seed(42)  # Reproducible output

# Configuration
NUM_DEPARTMENTS = 10
RESOURCES_PER_DEPT = 100
NUM_PHASES = 10
WORK_PACKAGES_PER_PHASE = 10
FEATURES_PER_WP = 10
TASKS_PER_FEATURE = 10

DEPARTMENTS = [
    ("eng", "Engineering", 650),
    ("qa", "Quality Assurance", 550),
    ("devops", "DevOps", 600),
    ("data", "Data Engineering", 620),
    ("security", "Security", 680),
    ("platform", "Platform", 640),
    ("mobile", "Mobile", 610),
    ("frontend", "Frontend", 580),
    ("backend", "Backend", 630),
    ("infra", "Infrastructure", 590),
]

ROLES = ["Junior", "Mid", "Senior", "Lead", "Principal"]
ROLE_RATES = [0.6, 0.8, 1.0, 1.2, 1.5]

PHASE_NAMES = [
    "Discovery", "Architecture", "Foundation", "Core Development",
    "Integration", "Testing", "Performance", "Security Hardening",
    "Documentation", "Deployment"
]

def generate_resources():
    """Generate 1,000 resources across 10 departments."""
    lines = ["# Resources: 1,000 across 10 departments\n"]

    for dept_id, dept_name, base_rate in DEPARTMENTS:
        lines.append(f"# {dept_name} Department")
        for i in range(RESOURCES_PER_DEPT):
            role_idx = i % len(ROLES)
            role = ROLES[role_idx]
            rate_multiplier = ROLE_RATES[role_idx]
            rate = int(base_rate * rate_multiplier)

            res_id = f"{dept_id}_{i+1:03d}"
            res_name = f"{role} {dept_name} {i+1}"

            # Some resources have reduced capacity
            capacity = ""
            if random.random() < 0.1:
                capacity = f"\n    capacity: {random.choice([0.5, 0.75, 0.8])}"

            lines.append(f'''resource {res_id} "{res_name}" {{
    rate: {rate}/day
    role: "{role} {dept_name}"{capacity}
}}
''')

    return "\n".join(lines)


def generate_tasks():
    """Generate 10,000 tasks with realistic structure and dependencies."""
    lines = ["# Tasks: 10,000 in hierarchical structure\n"]

    task_count = 0
    prev_phase_milestone = None

    for phase_idx, phase_name in enumerate(PHASE_NAMES):
        phase_id = f"phase{phase_idx+1:02d}"
        lines.append(f'task {phase_id} "{phase_name}" {{')

        prev_wp_milestone = None

        for wp_idx in range(WORK_PACKAGES_PER_PHASE):
            wp_id = f"wp{wp_idx+1:02d}"
            wp_name = f"Work Package {phase_idx+1}.{wp_idx+1}"
            lines.append(f'    task {wp_id} "{wp_name}" {{')

            prev_feature_task = None

            for feat_idx in range(FEATURES_PER_WP):
                feat_id = f"feat{feat_idx+1:02d}"
                feat_name = f"Feature {phase_idx+1}.{wp_idx+1}.{feat_idx+1}"
                lines.append(f'        task {feat_id} "{feat_name}" {{')

                prev_task = None

                for task_idx in range(TASKS_PER_FEATURE):
                    task_id = f"t{task_idx+1:02d}"
                    task_types = ["Design", "Implement", "Review", "Test", "Document",
                                  "Integrate", "Optimize", "Validate", "Deploy", "Monitor"]
                    task_name = f"{task_types[task_idx]} {feat_name}"

                    # Determine effort (1-10 days)
                    effort = random.choice([1, 2, 3, 5, 5, 5, 8, 8, 10, 13])

                    # Assign 1-3 resources from appropriate departments
                    dept_idx = (phase_idx + wp_idx + feat_idx) % NUM_DEPARTMENTS
                    dept_id = DEPARTMENTS[dept_idx][0]
                    num_assignees = random.randint(1, 3)
                    assignees = []
                    for _ in range(num_assignees):
                        res_num = random.randint(1, RESOURCES_PER_DEPT)
                        assignees.append(f"{dept_id}_{res_num:03d}")
                    assign_str = ", ".join(assignees)

                    # Build dependencies
                    deps = []
                    if prev_task:
                        # 70% chance to depend on previous task in feature
                        if random.random() < 0.7:
                            deps.append(prev_task)
                    elif prev_feature_task:
                        # First task of feature depends on last task of previous feature
                        deps.append(prev_feature_task)
                    elif prev_wp_milestone:
                        # First task of WP depends on previous WP milestone
                        deps.append(prev_wp_milestone)
                    elif prev_phase_milestone:
                        # First task of phase depends on previous phase milestone
                        deps.append(prev_phase_milestone)

                    dep_str = f"\n            depends: {', '.join(deps)}" if deps else ""

                    # Priority based on phase (earlier = higher)
                    priority = 1000 - (phase_idx * 100) + random.randint(-10, 10)

                    lines.append(f'''            task {task_id} "{task_name}" {{
                effort: {effort}d
                assign: {assign_str}
                priority: {priority}{dep_str}
            }}''')

                    prev_task = task_id
                    task_count += 1

                prev_feature_task = f"{feat_id}.{prev_task}"
                lines.append("        }")  # close feature

            # Work package milestone
            wp_milestone_id = f"{wp_id}_complete"
            last_feat = f"feat{FEATURES_PER_WP:02d}.t{TASKS_PER_FEATURE:02d}"
            lines.append(f'''        milestone {wp_milestone_id} "WP {phase_idx+1}.{wp_idx+1} Complete" {{
            depends: {last_feat}
        }}''')

            prev_wp_milestone = f"{phase_id}.{wp_id}.{wp_milestone_id}"
            lines.append("    }")  # close work package

        # Phase milestone
        phase_milestone_id = f"{phase_id}_complete"
        last_wp = f"wp{WORK_PACKAGES_PER_PHASE:02d}.wp{WORK_PACKAGES_PER_PHASE:02d}_complete"
        lines.append(f'''    milestone {phase_milestone_id} "Phase {phase_idx+1} Complete" {{
        depends: {last_wp}
    }}''')

        prev_phase_milestone = f"{phase_id}.{phase_milestone_id}"
        lines.append("}")  # close phase
        lines.append("")

    return "\n".join(lines), task_count


def generate_project():
    """Generate the complete project file."""
    header = '''# Enterprise Transformation Program
# Generated stress test: 10,000 tasks, 1,000 resources
#
# Structure:
# - 10 phases
# - 10 work packages per phase (100 total)
# - 10 features per work package (1,000 total)
# - 10 tasks per feature (10,000 total)
# - 10 departments with 100 resources each (1,000 total)

project "Enterprise Transformation Program" {
    start: 2026-01-05
    end: 2030-12-31
    currency: USD
}

calendar "standard" {
    working_days: mon-fri
    working_hours: 09:00-12:00, 13:00-17:00
    holiday "New Year" 2026-01-01
    holiday "Memorial Day" 2026-05-25
    holiday "Independence Day" 2026-07-03..2026-07-04
    holiday "Labor Day" 2026-09-07
    holiday "Thanksgiving" 2026-11-26..2026-11-27
    holiday "Christmas" 2026-12-24..2026-12-25
}

'''

    resources = generate_resources()
    tasks, task_count = generate_tasks()

    return header + resources + "\n" + tasks, task_count


if __name__ == "__main__":
    import sys

    output_file = sys.argv[1] if len(sys.argv) > 1 else "examples/enterprise_10k.proj"

    print(f"Generating large project file...")
    content, task_count = generate_project()

    with open(output_file, "w") as f:
        f.write(content)

    lines = content.count("\n")
    print(f"Generated {output_file}:")
    print(f"  - {task_count:,} tasks")
    print(f"  - {NUM_DEPARTMENTS * RESOURCES_PER_DEPT:,} resources")
    print(f"  - {lines:,} lines")
