#!/bin/bash
#
# Setup script for the mpp_to_proj companion tool using uv.
# This script creates a virtual environment and installs all dependencies.
#

set -e

# Colors for output
GREEN='\033[0;32m'
BLUE='\033[0;34m'
RED='\033[0;31m'
NC='\033[0m'

echo -e "${BLUE}=========================================${NC}"
echo -e "${BLUE}   mpp_to_proj Companion Tool Setup      ${NC}"
echo -e "${BLUE}=========================================${NC}"

# Check if uv is installed
if ! command -v uv &> /dev/null; then
    echo -e "${RED}Error: uv is not installed.${NC}"
    echo "Please install uv first: https://docs.astral.sh/uv/"
    exit 1
fi

echo -e "${GREEN}✓ uv is installed: $(uv --version)${NC}"

# Create virtual environment if it doesn't exist
if [ ! -d ".venv" ]; then
    echo -e "${BLUE}Creating virtual environment...${NC}"
    uv venv
fi

# Install dependencies
echo -e "${BLUE}Installing dependencies...${NC}"
uv pip install mpxj>=15.1.0 jpype1>=1.6.0 pytest>=7.4.0 pytest-cov>=4.1.0

echo -e "${GREEN}✓ Environment setup complete.${NC}"
echo -e ""
echo -e "To use the tool:"
echo -e "  source .venv/bin/activate"
echo -e "  python mpp_to_proj.py your_project.mpp"
echo -e ""
echo -e "To run tests with coverage:"
echo -e "  python -m pytest test_mpp_to_proj.py --cov=mpp_to_proj"
echo -e "${BLUE}=========================================${NC}"
