# MS Project to utf8proj Companion Tool

A high-fidelity Python-based converter to migrate Microsoft Project (`.mpp`) files into the native `utf8proj` (`.proj`) format or standard Project 2010 XML (MSPDI).

> [!IMPORTANT]
> **Disclaimer:** This is a Companion Tool, provided "as is", with no guarantee of support or maintenance. It is designed to assist in the transition from legacy formats to `utf8proj`.

## Features

- **Hierarchical Dependencies:** Automatically resolves complex task relationships into fully qualified hierarchical paths.
- **Robust Constraint Mapping:** Maps MS Project constraints (SNET, MSO, FNLT, etc.) to the appropriate `utf8proj` DSL keywords.
- **WBS Preservation:** Preserves the Work Breakdown Structure (WBS) prefix in task descriptions for perfect traceability.
- **String Safety:** Handles internal double-quote escaping correctly for robust parsing.
- **Verified Quality:** 98% unit test coverage ensuring reliable data conversion.

## Prerequisites

- **Python 3.10+**
- **Java Runtime Environment (JRE):** Required by the underlying MPXJ library.
- **uv:** Modern Python package manager (recommended).

## Setup

The tool uses `uv` for fast, isolated environment management.

```bash
# Run the setup script to create a virtual environment and install dependencies
./setup_companion.sh

# Activate the environment
source .venv/bin/activate
```

## Usage

```bash
# Convert to .proj (default)
python mpp_to_proj.py project.mpp

# Convert to a specific file or format
python mpp_to_proj.py project.mpp output.xml
```

## Development & Testing

This tool is verified with a comprehensive unit test suite using `pytest`.

```bash
# Run tests with coverage report
python -m pytest test_mpp_to_proj.py --cov=mpp_to_proj
```

---

*Design and implementation carried out in collaboration with **Gemini CLI**.*