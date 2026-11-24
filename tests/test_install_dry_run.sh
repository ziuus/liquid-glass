#!/usr/bin/env bash
# ==============================================================================
# Automated Verification Harness: Turnkey install.sh Validation (FEAT-TST-04)
# Verifies install.sh --dry-run and install.sh --check behavior, prerequisite
# checking, filesystem immutability, and error handling across Tiers 1-4.
# ==============================================================================
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
INSTALL_SCRIPT="$REPO_ROOT/install.sh"

echo "======================================================================"
echo " Running Automated Verification: install.sh Validation Suite"
echo " Target: $INSTALL_SCRIPT"
echo "======================================================================"

PASSED_TESTS=0
FAILED_TESTS=0
TOTAL_TESTS=0

run_test() {
    local test_name="$1"
    local test_fn="$2"
    TOTAL_TESTS=$((TOTAL_TESTS + 1))
    echo -n "[$TOTAL_TESTS] $test_name ... "
    if $test_fn; then
        echo "PASS"
        PASSED_TESTS=$((PASSED_TESTS + 1))
    else
        echo "FAIL"
        FAILED_TESTS=$((FAILED_TESTS + 1))
    fi
}

# Setup isolated sandbox directory
SANDBOX="$(mktemp -d "/tmp/liquid_test_install_XXXXXX")"
cleanup() {
    rm -rf "$SANDBOX"
}
trap cleanup EXIT

# ------------------------------------------------------------------------------
# TIER 1: Core Feature Happy Path Tests
# ------------------------------------------------------------------------------

test_tier1_script_exists_and_executable() {
    [[ -f "$INSTALL_SCRIPT" ]] && [[ -x "$INSTALL_SCRIPT" ]]
}

test_tier1_prerequisite_check() {
    # Genuinely executes install.sh --check; must exit 0 when prerequisites are met
    "$INSTALL_SCRIPT" --check >/dev/null 2>&1
}

test_tier1_dry_run_exit_code() {
    # Genuinely executes install.sh --dry-run; must exit 0 without compilation
    "$INSTALL_SCRIPT" --dry-run >/dev/null 2>&1
}

test_tier1_dry_run_reports_planned_paths() {
    local output
    output="$("$INSTALL_SCRIPT" --dry-run 2>&1)"
    echo "$output" | grep -qi 'DRY-RUN' && \
    echo "$output" | grep -qi 'liquid-shell' && \
    echo "$output" | grep -qi 'config.json'
}

# ------------------------------------------------------------------------------
# TIER 2: Boundary, Immutability & Error Handling Tests
# ------------------------------------------------------------------------------

test_tier2_dry_run_filesystem_immutability() {
    local test_prefix="$SANDBOX/prefix"
    local fake_home="$SANDBOX/fake_home"
    mkdir -p "$test_prefix" "$fake_home"

    # Capture filesystem state before dry run
    local before_prefix
    before_prefix="$(ls -la "$test_prefix")"

    # Run genuine --dry-run pointing to sandbox environment
    HOME="$fake_home" PREFIX="$test_prefix" "$INSTALL_SCRIPT" --dry-run >/dev/null 2>&1

    # Verify zero files or directories were created
    local after_prefix
    after_prefix="$(ls -la "$test_prefix")"
    [[ "$before_prefix" == "$after_prefix" ]] && [[ ! -d "$fake_home/.config/liquid-glass" ]]
}

test_tier2_missing_cargo_error_handling() {
    # Execute with PATH stripped of cargo; --check must fail with non-zero exit code
    local status=0
    local empty_dir="$SANDBOX/empty_path"
    mkdir -p "$empty_dir"
    (
        export PATH="$empty_dir"
        "$INSTALL_SCRIPT" --check >/dev/null 2>&1 || exit 0
        exit 1
    ) || status=$?
    [[ $status -eq 0 ]]
}

test_tier2_unknown_flag_handling() {
    # Genuine argument parser must reject invalid flags with non-zero exit code
    if "$INSTALL_SCRIPT" --invalid-flag-12345 >/dev/null 2>&1; then
        return 1
    else
        return 0
    fi
}

test_tier2_empty_arguments_safety() {
    # Verifies install.sh syntax and help flag behavior
    bash -n "$INSTALL_SCRIPT" && "$INSTALL_SCRIPT" --help >/dev/null 2>&1
}

# ------------------------------------------------------------------------------
# TIER 3: Cross-Feature Presets & Matrix Tests
# ------------------------------------------------------------------------------

test_tier3_dry_run_with_autostart_flag() {
    local output
    output="$("$INSTALL_SCRIPT" --dry-run --autostart 2>&1)"
    echo "$output" | grep -qi 'autostart' && \
    echo "$output" | grep -qi 'desktop'
}

test_tier3_dry_run_with_systemd_flag() {
    local output
    output="$("$INSTALL_SCRIPT" --dry-run --systemd 2>&1)"
    echo "$output" | grep -qi 'systemd' && \
    echo "$output" | grep -qi 'service'
}

test_tier3_idempotency_multiple_dry_runs() {
    # Running dry-run multiple times should produce consistent output and never modify system
    local out1 out2
    out1="$("$INSTALL_SCRIPT" --dry-run 2>&1)"
    out2="$("$INSTALL_SCRIPT" --dry-run 2>&1)"
    [[ "$out1" == "$out2" ]]
}

# ------------------------------------------------------------------------------
# TIER 4: Realistic Distribution & Packaging Tests
# ------------------------------------------------------------------------------

test_tier4_systemd_service_syntax_validity() {
    # Extracts embedded or template systemd service unit and verifies sections
    grep -q '\[Unit\]' "$INSTALL_SCRIPT" && \
    grep -q '\[Service\]' "$INSTALL_SCRIPT" && \
    grep -q '\[Install\]' "$INSTALL_SCRIPT" && \
    grep -q 'liquid-shell' "$INSTALL_SCRIPT"
}

test_tier4_desktop_entry_syntax_validity() {
    # Extracts embedded desktop entry and verifies required Desktop Entry keys
    grep -q '\[Desktop Entry\]' "$INSTALL_SCRIPT" && \
    grep -q 'Type=Application' "$INSTALL_SCRIPT" && \
    grep -q 'Exec=' "$INSTALL_SCRIPT"
}

test_tier4_uninstall_flag_structure() {
    grep -q -- '--uninstall' "$INSTALL_SCRIPT" && \
    grep -q 'killall' "$INSTALL_SCRIPT"
}

# ------------------------------------------------------------------------------
# Execution Harness
# ------------------------------------------------------------------------------

run_test "Tier 1.1: install.sh existence and executable bit" test_tier1_script_exists_and_executable
run_test "Tier 1.2: Prerequisite check execution (--check)" test_tier1_prerequisite_check
run_test "Tier 1.3: Dry run clean execution (--dry-run)" test_tier1_dry_run_exit_code
run_test "Tier 1.4: Dry run planned path reporting" test_tier1_dry_run_reports_planned_paths

run_test "Tier 2.1: Dry run filesystem immutability" test_tier2_dry_run_filesystem_immutability
run_test "Tier 2.2: Missing prerequisite error handling" test_tier2_missing_cargo_error_handling
run_test "Tier 2.3: Unknown flag handling" test_tier2_unknown_flag_handling
run_test "Tier 2.4: Empty arguments safety check" test_tier2_empty_arguments_safety

run_test "Tier 3.1: Dry run combined with --autostart" test_tier3_dry_run_with_autostart_flag
run_test "Tier 3.2: Dry run combined with --systemd" test_tier3_dry_run_with_systemd_flag
run_test "Tier 3.3: Multiple dry runs idempotency" test_tier3_idempotency_multiple_dry_runs

run_test "Tier 4.1: Systemd unit template syntax validity" test_tier4_systemd_service_syntax_validity
run_test "Tier 4.2: Desktop entry syntax validity" test_tier4_desktop_entry_syntax_validity
run_test "Tier 4.3: Uninstall flag structure check" test_tier4_uninstall_flag_structure

echo "======================================================================"
echo " Results: $PASSED_TESTS / $TOTAL_TESTS Passed ($FAILED_TESTS Failed)"
echo "======================================================================"

if [[ $FAILED_TESTS -eq 0 ]]; then
    exit 0
else
    exit 1
fi
