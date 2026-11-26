# Changelog

## [0.1.2] — macOS LaunchServices (Microphone / TCC) & Keychain Fix

### macOS
- **Launch via macOS LaunchServices (`open -n -a`)**: Spawns `Antigravity.app` under `launchd` (`PPID = 1`) instead of as a direct child of `AGY Swap.app`, preserving `com.google.antigravity` as the TCC Responsible Process so Microphone (Voice mode), Camera, Screen Recording, Notifications, Keychain ACLs, and native window focus work across all isolated accounts.
- **Automatic login Keychain link (`Library/Keychains`)**: Isolated profiles override `HOME`, which previously caused `SecKeychainCopyDefault()` to fall back to root-owned `System.keychain` and trigger Keychain prompts on login. `ensure_os_runtime_links()` now symlinks `<profile>/Library/Keychains` → `~/Library/Keychains` idempotently while keeping Rule R1 safe-unlink guarantees on delete.
- **Clean Default-profile env isolation**: Clears `XDG_CONFIG_HOME` and `XDG_DATA_HOME` alongside `SSH_CONNECTION` when switching back to the primary OS profile.
- **macOS install script fix (`scripts/package-macos.sh`)**: Uses `pkill -f` on the bundle executable path instead of `pkill -x agswitch-desktop` (which macOS truncated at 15 characters), ensuring `--install` cleanly stops previous running instances.

### Release Artifacts
- **macOS (Apple Silicon / arm64):**
  - `AGY-Swap-v0.1.2-macos-arm64.dmg` (`sha256: c86b3bfa6bf91b6f742008d210d02884a5dcb3692063bbd91bf1712a199af5a6`)
  - `agswitch-cli-v0.1.2-macos-arm64.tar.gz` (`sha256: 75ad6bbe5b8f3b42311783034a802684e9774489f951de02338e99a556aba0fa`)
  - `SHA256SUMS-macos-arm64.txt`
- **Windows 10/11 (x86_64):** no changes in this release. `v0.1.1` remains the current Windows build.

## [0.1.1] — Windows Hide to Tray

### Windows
- **Hide to tray (`×`)**: The borderless Windows popover has no system minimize or close buttons, so a `×` button next to Quit (`⏻`) now hides the popover while AGY Swap keeps running in the System Tray. Left-click the tray icon to bring it back.

### Release Artifacts
- **Windows 10/11 (x86_64):** `AGY-Swap-v0.1.1-windows-x86_64.zip` (`sha256: d4b0fa6629c06c165bdda458e833b8b8dbd9cff3aa1f00d12a51816af6d7d4f2`) + `SHA256SUMS-windows-x86_64.txt`
- **macOS:** no changes in this release. `v0.1.0` remains the current macOS build.

## [0.1.0] — Initial Cross-Platform Release

### Highlights
- **Native GPUI Menu Bar & System Tray App (`agswitch-desktop`)** + paired CLI (`agswitch-cli`) built in Rust Edition 2024 (`L0`–`L5` layered architecture).
- **Isolated Multi-Account Switching**: Spawns each Google Antigravity account in its own `--user-data-dir` + isolated home directory (`HOME` on macOS, `USERPROFILE` on Windows) with live `LIVE` / `READY` / `5H LIMIT` detection.
- **Honest Local 5-Hour Rolling Timers**: Hardware dot-matrix countdown clocks (`5:00 ready` → `H:MM left`), segmented LED bars, and exact 5h/7d call & token breakdowns for Gemini and Claude & GPT — zero guessed quota percentages, zero OAuth reads (Rule R2).
- **Optional Shared Chats & Skills**: Per-profile POSIX symlink (macOS) / NTFS junction (Windows) linking `.gemini/{antigravity, config, skills}` while keeping login vaults separate.
- **PHOSPHOR Orb Lab & Customization**: 12 GPU dot-matrix animations × 5 themes per profile, Samo Obsidian (Dark) & Samo Alabaster (Light) themes, Full (`680×524`) & Compact (`320×420`) layouts, and Privacy **Hide Emails** toggle.
- **Safety Gates (`R1`–`R5`) & Safe Sandbox Mode**: Verified by `cargo run -p xtask -- verify`, `./scripts/test-safe.sh` (macOS), and `.\scripts\test-safe.ps1` (Windows).

### Windows 10/11 (x86_64)
- **System Tray popover stays alive**: GPUI 0.2.2 quits its Windows event loop when the last window closes, so a hidden keep-alive window now prevents toggling or dismissing the popover from quitting the app.
- **DPI-aware tray anchoring**: The monitor scale is inferred from the tray icon's physical rect (100%–200%+). The popover opens above the tray icon and stays fully on-screen, including when it grows from Compact to Full or on page navigation.
- **Theme-aware tray icon**: Dark glyph on light taskbars, white on dark, re-tinted live when the theme changes.
- **Antigravity detection & launching**: Probes `%LOCALAPPDATA%\Programs\Antigravity`, `Programs\Antigravity IDE`, `%LOCALAPPDATA%\Antigravity`, and `Program Files`. It launches fully detached (no console, own process group), matches `--user-data-dir` paths containing spaces, and stops windows gracefully.
- **NTFS junction Shared Chats**: Created with `mklink /J` (no Administrator rights) and removed with `RemoveDirectoryW`, which deletes only the reparse point and never the target.
- **Packaging**: `scripts/package-windows.ps1` builds the portable zip and checksum, embeds the app icon and version info, and supports `-Install` to `%LOCALAPPDATA%\Programs\AGY Swap\` with a Start Menu shortcut (no admin).

### Release Artifacts
- **macOS (Apple Silicon / arm64)** — built & verified:
  - `AGY-Swap-v0.1.0-macos-arm64.dmg` (`sha256: 8efc7d65999052447a887216942f81c8e71cf941e9f2f651c2f9ef312bac7504`)
  - `agswitch-cli-v0.1.0-macos-arm64.tar.gz` (`sha256: 8b64dd4a0eb42f2d92d6c686c66bd2067d5c2b2751c9e4dff7c8c9515f872506`)
- **Windows 10/11 (x86_64)** — built & verified on Windows 11 (MSVC, Issue #1):
  - `AGY-Swap-v0.1.0-windows-x86_64.zip` (`sha256: a0279bc1c935ba36f11ee74ab673e6a7468a5b75cc53c40126db4726cc8cb904`)
    - contains `AGY Swap/agswitch-desktop.exe`, `AGY Swap/agswitch.exe` (CLI), `README.md`, `LICENSE`
  - `SHA256SUMS-windows-x86_64.txt`
