#!/usr/bin/env bash
#
# Run PSPLIB benchmark suite for utf8proj (RFC-0015)
#
# Downloads PSPLIB J30 instances, converts to .proj, schedules, and reports.
#
# Usage:
#   ./tools/benchmarks/run_psplib.sh                  # Full J30 benchmark
#   ./tools/benchmarks/run_psplib.sh --leveling       # With resource leveling
#   ./tools/benchmarks/run_psplib.sh --dataset j60    # J60 dataset
#   ./tools/benchmarks/run_psplib.sh --skip-download  # Reuse existing data
#
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"
DATA_DIR="$ROOT_DIR/data/psplib"
CONVERTER="$ROOT_DIR/tools/psplib_to_proj.py"
RUNNER="$SCRIPT_DIR/benchmark_runner.py"

# Defaults
DATASET="j30"
LEVELING=""
SKIP_DOWNLOAD=false
OUTPUT=""

# Parse args
while [[ $# -gt 0 ]]; do
    case $1 in
        --dataset) DATASET="$2"; shift 2 ;;
        --leveling|-l) LEVELING="--leveling"; shift ;;
        --skip-download) SKIP_DOWNLOAD=true; shift ;;
        --output|-o) OUTPUT="$2"; shift 2 ;;
        --help|-h)
            echo "Usage: $0 [OPTIONS]"
            echo ""
            echo "Options:"
            echo "  --dataset NAME    PSPLIB dataset: j30, j60, j90, j120 (default: j30)"
            echo "  --leveling, -l    Enable resource leveling"
            echo "  --skip-download   Reuse previously downloaded data"
            echo "  --output, -o FILE Write JSON report to FILE"
            echo "  --help, -h        Show this help"
            exit 0
            ;;
        *) echo "Unknown option: $1"; exit 1 ;;
    esac
done

# PSPLIB URLs (TU Munich mirror)
# Note: These URLs may change. Check https://www.om-db.wi.tum.de/psplib/ for current links.
PSPLIB_BASE="https://www.om-db.wi.tum.de/psplib/data"

SM_DIR="$DATA_DIR/${DATASET}_sm"
PROJ_DIR="$DATA_DIR/${DATASET}_proj"
OPT_FILE="$DATA_DIR/${DATASET}_opt.csv"

echo "╔══════════════════════════════════════════════════╗"
echo "║   utf8proj PSPLIB Benchmark (RFC-0015)           ║"
echo "╠══════════════════════════════════════════════════╣"
echo "║  Dataset:  $DATASET"
echo "║  Leveling: ${LEVELING:-no}"
echo "╚══════════════════════════════════════════════════╝"
echo ""

# ============================================================================
# Step 1: Ensure utf8proj binary exists
# ============================================================================
echo "Step 1: Building utf8proj..."
if [ -f "$ROOT_DIR/target/release/utf8proj" ]; then
    echo "  Using existing release build."
else
    echo "  Building release binary..."
    (cd "$ROOT_DIR" && cargo build --release --quiet)
fi
echo ""

# ============================================================================
# Step 2: Download PSPLIB data
# ============================================================================
if [ "$SKIP_DOWNLOAD" = true ] && [ -d "$SM_DIR" ]; then
    echo "Step 2: Skipping download (--skip-download, data exists)."
    SM_COUNT=$(find "$SM_DIR" -name "*.sm" -not -name "*opt*" -not -name "*hrs*" -not -name "*lb*" | wc -l | tr -d ' ')
    echo "  Found $SM_COUNT .sm instance files."
else
    echo "Step 2: Downloading PSPLIB $DATASET dataset..."
    mkdir -p "$SM_DIR"

    # PSPLIB distributes files individually or as archives.
    # For J30: files are named j301_1.sm through j3048_10.sm (480 instances)
    # The archives are typically .zip or available per-parameter-set.
    #
    # Since direct download URLs vary, we provide instructions for manual download
    # if automatic download fails.

    if [ ! -f "$SM_DIR/.downloaded" ]; then
        echo "  PSPLIB data must be downloaded manually from:"
        echo "    https://www.om-db.wi.tum.de/psplib/datasm${DATASET}.zip"
        echo ""
        echo "  Place .sm files in: $SM_DIR/"
        echo ""
        echo "  Alternatively, use the Python psplib package:"
        echo "    pip install psplib"
        echo "    python3 -c \"import psplib; psplib.download('$DATASET', '$SM_DIR')\""
        echo ""

        # Try automatic download via psplib Python package
        if command -v python3 &>/dev/null; then
            echo "  Attempting automatic download via psplib Python package..."
            python3 -c "
import sys
try:
    import psplib
    print('  psplib package found, downloading...')
    # psplib downloads to a cache; we need to extract .sm files
    instances = psplib.parse(psplib.get_dataset_path('$DATASET'))
    print(f'  Loaded {len(instances)} instances via psplib')
    # Write marker
    open('$SM_DIR/.downloaded', 'w').write('psplib')
except ImportError:
    print('  psplib package not installed. Install with: pip install psplib')
    print('  Or download manually from the URL above.')
    sys.exit(1)
except Exception as e:
    print(f'  Download failed: {e}')
    sys.exit(1)
" 2>&1 || true
        fi

        # Check if we have data
        SM_COUNT=$(find "$SM_DIR" -name "*.sm" 2>/dev/null | wc -l | tr -d ' ')
        if [ "$SM_COUNT" -eq 0 ]; then
            echo ""
            echo "  No .sm files found. Please download PSPLIB data manually."
            echo "  See: https://www.om-db.wi.tum.de/psplib/"
            exit 1
        fi
    fi
fi
echo ""

# ============================================================================
# Step 3: Convert .sm to .proj
# ============================================================================
echo "Step 3: Converting PSPLIB instances to .proj format..."
mkdir -p "$PROJ_DIR"

EXISTING_PROJ=$(find "$PROJ_DIR" -name "*.proj" 2>/dev/null | wc -l | tr -d ' ')
SM_COUNT=$(find "$SM_DIR" -name "*.sm" -not -name "*opt*" -not -name "*hrs*" -not -name "*lb*" 2>/dev/null | wc -l | tr -d ' ')

if [ "$EXISTING_PROJ" -ge "$SM_COUNT" ] && [ "$SM_COUNT" -gt 0 ] && [ "$SKIP_DOWNLOAD" = true ]; then
    echo "  Reusing $EXISTING_PROJ existing .proj files."
else
    python3 "$CONVERTER" --batch "$SM_DIR" "$PROJ_DIR"
fi

PROJ_COUNT=$(find "$PROJ_DIR" -name "*.proj" | wc -l | tr -d ' ')
echo "  Converted: $PROJ_COUNT .proj files in $PROJ_DIR"
echo ""

# ============================================================================
# Step 4: Prepare optimal solutions
# ============================================================================
echo "Step 4: Preparing optimal solutions..."

# Look for optimal solutions file in the SM directory
OPT_SM=$(find "$SM_DIR" -name "*opt*" -o -name "*optimal*" 2>/dev/null | head -1)
if [ -n "$OPT_SM" ]; then
    echo "  Found optimal solutions: $OPT_SM"
    # Convert to our CSV format
    python3 -c "
import sys
with open('$OPT_SM') as f:
    lines = f.readlines()
count = 0
with open('$OPT_FILE', 'w') as out:
    for line in lines:
        parts = line.split()
        if len(parts) >= 3:
            try:
                param, inst, ms = int(parts[0]), int(parts[1]), int(parts[2])
                name = f'j30{param}_{inst}'
                out.write(f'{name},{ms}\n')
                count += 1
            except ValueError:
                pass
print(f'  Extracted {count} optimal values to $OPT_FILE')
" 2>&1
    OPTIMA_ARG="--optima $OPT_FILE"
else
    echo "  No optimal solutions file found. Running without gap analysis."
    OPTIMA_ARG=""
fi
echo ""

# ============================================================================
# Step 5: Run benchmarks
# ============================================================================
echo "Step 5: Running benchmarks..."
echo ""

if [ -z "$OUTPUT" ]; then
    OUTPUT="$DATA_DIR/${DATASET}_results_$(date +%Y%m%d_%H%M%S).json"
fi

python3 "$RUNNER" \
    --proj-dir "$PROJ_DIR" \
    $OPTIMA_ARG \
    --dataset "PSPLIB-${DATASET^^}" \
    --output "$OUTPUT" \
    $LEVELING

echo ""
echo "Results saved to: $OUTPUT"
