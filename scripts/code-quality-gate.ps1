# IronClaw Code Quality Gate Check (PowerShell)
#
# This script performs a comprehensive quality check including:
# 1. Formatting check (cargo fmt)
# 2. Static analysis (cargo clippy)
# 3. Unit & Integration tests (cargo nextest/test)
# 4. Custom safety checks (regex-based)
# 5. Architecture boundary checks
# 6. Security audit (optional, if cargo-audit is installed)
#
# Usage: powershell -Command .\scripts\code-quality-gate.ps1

$ErrorActionPreference = "Stop"

# Colors for output
$RED = "`e[0;31m"
$GREEN = "`e[0;32m"
$YELLOW = "`e[1;33m"
$BLUE = "`e[0;34m"
$NC = "`e[0m"

$REPO_ROOT = Split-Path -Parent $PSScriptRoot
Set-Location $REPO_ROOT

$PASSED = 0
$FAILED = 0

function Check-Command {
    param([string]$Command)
    try {
        $null = Get-Command $Command -ErrorAction Stop
        return $true
    } catch {
        Log-Warning "$Command not found, skipping related check"
        return $false
    }
}

function Log-Header {
    param([string]$Message)
    Write-Host "`n$BLUE======================================================================$NC"
    Write-Host "$BLUE  $Message$NC"
    Write-Host "$BLUE======================================================================$NC`n"
}

function Log-Success {
    param([string]$Message)
    Write-Host "$GREEN✓ $Message$NC"
    $script:PASSED++
}

function Log-Failure {
    param([string]$Message)
    Write-Host "$RED✗ $Message$NC"
    $script:FAILED++
}

function Log-Warning {
    param([string]$Message)
    Write-Host "$YELLOW! $Message$NC"
}

# 1. Cargo Format Check
Log-Header "Step 1: Checking Code Formatting"
if (Check-Command "cargo") {
    try {
        cargo fmt --all -- --check
        Log-Success "Formatting is correct"
    } catch {
        Log-Failure "Formatting issues found. Run 'cargo fmt' to fix."
    }
}

# 2. Cargo Clippy
Log-Header "Step 2: Running Clippy Static Analysis"
if (Check-Command "cargo") {
    try {
        cargo clippy --all-targets --all-features -- -D warnings
        Log-Success "Clippy passed with no warnings"
    } catch {
        Log-Failure "Clippy found issues or warnings"
    }
}

# 3. Tests
Log-Header "Step 3: Running Tests"
if (Check-Command "cargo") {
    if (Get-Command "cargo-nextest" -ErrorAction SilentlyContinue) {
        try {
            cargo nextest run --all-features
            Log-Success "All tests passed (via nextest)"
        } catch {
            Log-Failure "Some tests failed"
        }
    } else {
        Log-Warning "cargo-nextest not found, falling back to standard cargo test"
        try {
            cargo test --all-features
            Log-Success "All tests passed"
        } catch {
            Log-Failure "Some tests failed"
        }
    }
}

# 4. Custom Safety Checks
Log-Header "Step 4: Running Custom Safety Checks"
try {
    & powershell -Command "& '$REPO_ROOT\scripts\pre-commit-safety.ps1'"
    Log-Success "Custom safety checks passed"
} catch {
    Log-Failure "Custom safety checks found potential issues"
}

# 5. Architecture Boundary Checks
Log-Header "Step 5: Checking Architecture Boundaries"
try {
    & powershell -Command "& '$REPO_ROOT\scripts\check-boundaries.ps1'"
    Log-Success "Architecture boundaries respected"
} catch {
    Log-Failure "Architecture boundary violations found"
}

# 6. Security Audit (Optional)
Log-Header "Step 6: Security Audit (Optional)"
if (Get-Command "cargo-audit" -ErrorAction SilentlyContinue) {
    try {
        cargo audit
        Log-Success "Security audit passed"
    } catch {
        Log-Failure "Security vulnerabilities found in dependencies"
    }
} else {
    Log-Warning "cargo-audit not installed, skipping security audit"
}

# Summary
Log-Header "Quality Gate Summary"
Write-Host "Total Checks Passed: $GREEN$PASSED$NC"
Write-Host "Total Checks Failed: $RED$FAILED$NC"

if ($FAILED -eq 0) {
    Write-Host "`n$GREEN`CONGRATULATIONS! The code quality gate has passed.$NC"
    exit 0
} else {
    Write-Host "`n$RED`FAILURE: The code quality gate has failed. Please fix the issues above.$NC"
    exit 1
}
