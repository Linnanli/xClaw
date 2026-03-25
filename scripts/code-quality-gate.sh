#!/usr/bin/env bash
# IronClaw Code Quality Gate Check
#
# This script performs a comprehensive quality check including:
# 1. Formatting check (cargo fmt)
# 2. Static analysis (cargo clippy)
# 3. Unit & Integration tests (cargo nextest/test)
# 4. Custom safety checks (regex-based)
# 5. Architecture boundary checks
# 6. Security audit (optional, if cargo-audit is installed)
#
# Usage: bash scripts/code-quality-gate.sh

set -euo pipefail

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

PASSED=0
FAILED=0

check_command() {
    if ! command -v "$1" >/dev/null 2>&1; then
        log_warning "$1 not found, skipping related check"
        return 1
    fi
    return 0
}

log_header() {
    echo -e "\n${BLUE}======================================================================${NC}"
    echo -e "${BLUE}  $1${NC}"
    echo -e "${BLUE}======================================================================${NC}\n"
}

log_success() {
    echo -e "${GREEN}✓ $1${NC}"
    PASSED=$((PASSED + 1))
}

log_failure() {
    echo -e "${RED}✗ $1${NC}"
    FAILED=$((FAILED + 1))
}

log_warning() {
    echo -e "${YELLOW}! $1${NC}"
}

# 1. Cargo Format Check
log_header "Step 1: Checking Code Formatting"
if check_command "cargo"; then
    if cargo fmt --all -- --check; then
        log_success "Formatting is correct"
    else
        log_failure "Formatting issues found. Run 'cargo fmt' to fix."
    fi
fi

# 2. Cargo Clippy
log_header "Step 2: Running Clippy Static Analysis"
if check_command "cargo"; then
    if cargo clippy --all-targets --all-features -- -D warnings; then
        log_success "Clippy passed with no warnings"
    else
        log_failure "Clippy found issues or warnings"
    fi
fi

# 3. Tests
log_header "Step 3: Running Tests"
if check_command "cargo"; then
    if command -v cargo-nextest >/dev/null 2>&1; then
        if cargo nextest run --all-features; then
            log_success "All tests passed (via nextest)"
        else
            log_failure "Some tests failed"
        fi
    else
        log_warning "cargo-nextest not found, falling back to standard cargo test"
        if cargo test --all-features; then
            log_success "All tests passed"
        else
            log_failure "Some tests failed"
        fi
    fi
fi

# 4. Custom Safety Checks
log_header "Step 4: Running Custom Safety Checks"
if bash scripts/pre-commit-safety.sh; then
    log_success "Custom safety checks passed"
else
    log_failure "Custom safety checks found potential issues"
fi

# 5. Architecture Boundary Checks
log_header "Step 5: Checking Architecture Boundaries"
if bash scripts/check-boundaries.sh; then
    log_success "Architecture boundaries respected"
else
    log_failure "Architecture boundary violations found"
fi

# 6. Security Audit (Optional)
log_header "Step 6: Security Audit (Optional)"
if command -v cargo-audit >/dev/null 2>&1; then
    if cargo audit; then
        log_success "Security audit passed"
    else
        log_failure "Security vulnerabilities found in dependencies"
    fi
else
    log_warning "cargo-audit not installed, skipping security audit"
fi

# Summary
log_header "Quality Gate Summary"
echo -e "Total Checks Passed: ${GREEN}$PASSED${NC}"
echo -e "Total Checks Failed: ${RED}$FAILED${NC}"

if [ "$FAILED" -eq 0 ]; then
    echo -e "\n${GREEN}CONGRATULATIONS! The code quality gate has passed.${NC}"
    exit 0
else
    echo -e "\n${RED}FAILURE: The code quality gate has failed. Please fix the issues above.${NC}"
    exit 1
fi
