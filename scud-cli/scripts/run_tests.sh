#!/bin/bash
# Test runner script for socket feed tests
#
# Usage:
#   ./run_tests.sh              # Run all tests
#   ./run_tests.sh -k "test_"   # Run tests matching pattern
#   ./run_tests.sh --cov        # Run with coverage
#   ./run_tests.sh -x           # Stop on first failure

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo -e "${YELLOW}=== SCUD Socket Feed Tests ===${NC}"
echo ""

# Check for virtual environment
if [ -z "$VIRTUAL_ENV" ]; then
    echo -e "${YELLOW}Note: Not running in a virtual environment${NC}"
    echo "Consider creating one with: python -m venv venv && source venv/bin/activate"
    echo ""
fi

# Check for required packages
echo "Checking dependencies..."
MISSING_DEPS=""

python -c "import zmq" 2>/dev/null || MISSING_DEPS="$MISSING_DEPS pyzmq"
python -c "import pytest" 2>/dev/null || MISSING_DEPS="$MISSING_DEPS pytest"
python -c "import pytest_asyncio" 2>/dev/null || MISSING_DEPS="$MISSING_DEPS pytest-asyncio"

if [ -n "$MISSING_DEPS" ]; then
    echo -e "${RED}Missing dependencies:${NC}$MISSING_DEPS"
    echo ""
    echo "Install with: pip install -r requirements.txt"
    exit 1
fi

echo -e "${GREEN}All dependencies satisfied${NC}"
echo ""

# Parse arguments
COV_ARGS=""
if [[ "$*" == *"--cov"* ]]; then
    # Check for pytest-cov
    if python -c "import pytest_cov" 2>/dev/null; then
        COV_ARGS="--cov=. --cov-report=term-missing --cov-report=html"
        echo "Coverage reporting enabled"
    else
        echo -e "${YELLOW}Warning: pytest-cov not installed, skipping coverage${NC}"
        # Remove --cov from args
        set -- "${@/--cov/}"
    fi
fi

# Run tests
echo "Running tests..."
echo ""

if [ -n "$COV_ARGS" ]; then
    pytest $COV_ARGS "$@"
else
    pytest "$@"
fi

EXIT_CODE=$?

echo ""
if [ $EXIT_CODE -eq 0 ]; then
    echo -e "${GREEN}=== All tests passed ===${NC}"
else
    echo -e "${RED}=== Tests failed ===${NC}"
fi

exit $EXIT_CODE
