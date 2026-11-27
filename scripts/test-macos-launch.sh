#!/usr/bin/env bash
set -euo pipefail

# Live macOS LaunchServices + Account Isolation E2E Test (100% safe — NEVER closes or touches real Antigravity).
# Uses a temporary mock `.app` bundle + isolated `AGSWITCH_DATA_DIR` to verify:
#   1. `OsProcessAdapter::launch_spec` launches via `/usr/bin/open -n -a` -> `PPID=1` (macOS TCC Microphone/Camera fix)
#   2. Each isolated account receives its own `HOME`, `SSH_CONNECTION`, and `--user-data-dir`
#   3. Legacy v0.1.2 `<profile>/Library/Keychains -> ~/Library/Keychains` symlinks are automatically unlinked
#   4. Isolated accounts do NOT resolve the primary user's `login.keychain-db` (preventing cross-account login leakage)

if [ "$(uname -s)" != "Darwin" ]; then
  echo "Skipping macOS LaunchServices test on $(uname -s)"
  exit 0
fi

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

TMP_DIR="$(mktemp -d -t agyswap-macos-launch-XXXXXX)"
cleanup() {
  rm -rf "$TMP_DIR"
}
trap cleanup EXIT

CLI="$ROOT_DIR/target/debug/agswitch-cli"
if [ ! -x "$CLI" ]; then
  cargo build -p agswitch-cli
fi

MOCK_APP="$TMP_DIR/MockAntigravity.app"
MOCK_BIN="$MOCK_APP/Contents/MacOS/Antigravity"
REPORT_FILE="$TMP_DIR/launch-report.txt"
DATA_DIR="$TMP_DIR/data"

mkdir -p "$MOCK_APP/Contents/MacOS" "$DATA_DIR"
cat > "$MOCK_APP/Contents/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleExecutable</key><string>Antigravity</string>
  <key>CFBundleIdentifier</key><string>dev.kjaiml.agyswap.mockantigravity</string>
  <key>CFBundleName</key><string>MockAntigravity</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>LSUIElement</key><true/>
</dict>
</plist>
PLIST

cat > "$MOCK_BIN" <<SH
#!/bin/sh
{
  echo "PPID=\$PPID"
  echo "HOME=\$HOME"
  echo "SSH_CONNECTION=\$SSH_CONNECTION"
  echo "XDG_CONFIG_HOME=\$XDG_CONFIG_HOME"
  echo "ARGC=\$#"
  for a in "\$@"; do
    echo "ARG=\$a"
  done
  if [ -L "\$HOME/Library/Keychains" ] || [ -e "\$HOME/Library/Keychains" ]; then
    echo "HAS_KEYCHAINS_ENTRY=1"
  else
    echo "HAS_KEYCHAINS_ENTRY=0"
  fi
  KC_OUT="\$(security default-keychain 2>&1 || true)"
  echo "DEFAULT_KEYCHAIN=\$KC_OUT"
} > "$REPORT_FILE"
SH
chmod +x "$MOCK_BIN"

wait_for_report() {
  local tries=0
  while [ ! -s "$REPORT_FILE" ] && [ "$tries" -lt 60 ]; do
    sleep 0.05
    tries=$((tries + 1))
  done
  if [ ! -s "$REPORT_FILE" ]; then
    echo "FAIL: MockAntigravity.app did not write $REPORT_FILE within 3s"
    exit 1
  fi
}

assert_line() {
  local expected="$1"
  if ! grep -Fq "$expected" "$REPORT_FILE"; then
    echo "FAIL: expected report to contain: $expected"
    echo "--- actual report ---"
    cat "$REPORT_FILE"
    exit 1
  fi
}

export AGSWITCH_DATA_DIR="$DATA_DIR"
export AGSWITCH_ANTIGRAVITY_BIN="$MOCK_BIN"

echo "==> [1/3] Add two isolated mock profiles (Acc Two, Acc Three)"
"$CLI" add "Acc Two" --subtitle "acc2@example.com" --tier pro
"$CLI" add "Acc Three" --subtitle "acc3@example.com" --tier pro

ACC2_ROOT="$DATA_DIR/Profiles/Acc Two"
ACC3_ROOT="$DATA_DIR/Profiles/Acc Three"

# Simulate legacy v0.1.2 symlink pointing to the user's real ~/Library/Keychains
mkdir -p "$ACC2_ROOT/Library"
ln -s "$HOME/Library/Keychains" "$ACC2_ROOT/Library/Keychains"
[ -L "$ACC2_ROOT/Library/Keychains" ]

echo "==> [2/3] Open 'acc-two' via LaunchServices and verify PPID=1 + auto-unlinked Keychains + isolated HOME"
rm -f "$REPORT_FILE"
"$CLI" open acc-two
wait_for_report
cat "$REPORT_FILE"

assert_line "PPID=1"
assert_line "HOME=$ACC2_ROOT"
assert_line "SSH_CONNECTION=127.0.0.1 1234 127.0.0.1 22"
assert_line "ARG=--user-data-dir=$ACC2_ROOT/UserData"
assert_line "HAS_KEYCHAINS_ENTRY=0"
if grep -Fq "login.keychain-db" "$REPORT_FILE"; then
  echo "FAIL: isolated profile must NOT resolve primary login.keychain-db!"
  exit 1
fi
if [ -L "$ACC2_ROOT/Library/Keychains" ] || [ -e "$ACC2_ROOT/Library/Keychains" ]; then
  echo "FAIL: legacy v0.1.2 Library/Keychains symlink was not unlinked!"
  exit 1
fi
# Rule R1 check: primary user's real ~/Library/Keychains must still exist untouched
[ -d "$HOME/Library/Keychains" ]

echo "==> [3/3] Open 'acc-three' alongside and verify distinct isolated HOME & UserData"
rm -f "$REPORT_FILE"
"$CLI" open acc-three
wait_for_report
cat "$REPORT_FILE"

assert_line "PPID=1"
assert_line "HOME=$ACC3_ROOT"
assert_line "ARG=--user-data-dir=$ACC3_ROOT/UserData"
assert_line "HAS_KEYCHAINS_ENTRY=0"

echo ""
echo "PASS: macOS LaunchServices (PPID=1) + isolated account Keychain separation verified!"
