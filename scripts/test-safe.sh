#!/usr/bin/env bash
set -euo pipefail

# AGY Swap — 100% Safe Verification & Sandbox E2E Test Script
# NEVER touches real Antigravity processes, ~/.gemini, or ~/Library/Application Support/AgSwitch.
# Usage:
#   ./scripts/test-safe.sh        # Run full verify + automated sandbox E2E suite
#   ./scripts/test-safe.sh --ui   # Also launch interactive Sandbox GUI after tests pass

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

SANDBOX_DIR="$(mktemp -d -t agyswap-e2e-XXXXXX)"
cleanup() {
  rm -rf "$SANDBOX_DIR"
}
trap cleanup EXIT

echo "============================================================"
echo "1) Workspace Verify (layers + purity + clippy -D warnings + unit tests)"
echo "============================================================"
cargo run -p xtask -- verify

echo ""
echo "============================================================"
echo "2) Build CLI & Desktop binaries"
echo "============================================================"
cargo build -p agswitch-cli -p agswitch-desktop

CLI="$ROOT_DIR/target/debug/agswitch-cli"
SB=("--sandbox" "--sandbox-dir" "$SANDBOX_DIR")

assert_status_has() {
  local expected="$1"
  local out
  out="$("$CLI" "${SB[@]}" status)"
  if ! printf '%s\n' "$out" | grep -q "$expected"; then
    echo "FAIL: expected status to contain '$expected', got:"
    echo "$out"
    exit 1
  fi
}

echo ""
echo "============================================================"
echo "3) Safe Sandbox E2E Flow (isolated dir: $SANDBOX_DIR)"
echo "============================================================"

echo "-> [3.1] Initial status (5h + Weekly 7d + Gemini/Claude/GPT)"
STATUS_OUT="$("$CLI" "${SB[@]}" status)"
echo "$STATUS_OUT"
printf '%s\n' "$STATUS_OUT" | grep -q "Active: SC"
printf '%s\n' "$STATUS_OUT" | grep -q "\[READY   \] Trade"
printf '%s\n' "$STATUS_OUT" | grep -q "Claude"
printf '%s\n' "$STATUS_OUT" | grep -q "GPT"

echo ""
echo "-> [3.2] Switch Active: SC -> trade (simulated, real Antigravity untouched!)"
"$CLI" "${SB[@]}" switch trade
assert_status_has "Active: Trade"

echo ""
echo "-> [3.3] Open Alongside: work-claude"
"$CLI" "${SB[@]}" open work-claude
assert_status_has "Active: Trade, Work Claude"

echo ""
echo "-> [3.4] Stop running window: work-claude"
"$CLI" "${SB[@]}" stop work-claude
assert_status_has "Active: Trade"

echo ""
echo "-> [3.5] 5h Cooldown + Smart Next Rotation (must skip cooldown account)"
"$CLI" "${SB[@]}" cooldown work-claude
"$CLI" "${SB[@]}" next
assert_status_has "Active: SC"

echo ""
echo "-> [3.6] Add temporary account + toggle Shared Chats"
"$CLI" "${SB[@]}" add "QA Bot" --subtitle "qa.bot@gmail.com" --tier pro
"$CLI" "${SB[@]}" share qa-bot --enabled=false
"$CLI" "${SB[@]}" share qa-bot --enabled=true

echo ""
echo "-> [3.7] Rule R4 Guard: delete --delete-disk WITHOUT --yes must be BLOCKED"
if "$CLI" "${SB[@]}" delete qa-bot --delete-disk 2>/dev/null; then
  echo "FAIL: Rule R4 should have blocked unconfirmed destructive delete!"
  exit 1
else
  echo "PASS: Rule R4 blocked unconfirmed destructive delete"
fi

echo "-> [3.8] Rule R4 Confirmed: delete --delete-disk --yes must SUCCEED"
"$CLI" "${SB[@]}" delete qa-bot --delete-disk --yes

echo "-> [3.9] Rule R1 Guard: deleting primary OS profile disk must ALWAYS be BLOCKED"
if "$CLI" "${SB[@]}" delete default --delete-disk --yes 2>/dev/null; then
  echo "FAIL: Rule R1 should have blocked deleting primary OS profile!"
  exit 1
else
  echo "PASS: Rule R1 protected primary OS profile"
fi

echo ""
echo "-> [3.10] Centralized Audit Trail (latest 10)"
"$CLI" "${SB[@]}" audit --limit 10

echo ""
echo "============================================================"
echo "ALL SAFE E2E CHECKS PASSED!"
echo "============================================================"
echo "To test the interactive GUI safely without closing Antigravity:"
echo "  cargo run -p agswitch-desktop -- --sandbox"

if [[ "${1:-}" == "--ui" ]]; then
  echo ""
  echo "Launching AGY Swap Desktop in SANDBOX mode..."
  exec "$ROOT_DIR/target/debug/agswitch-desktop" --sandbox
fi
