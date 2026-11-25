# Changelog

## [0.1.0] — Initial Cross-Platform Release

### Highlights
- **Native GPUI Menu Bar & System Tray App (`agswitch-desktop`)** + paired CLI (`agswitch-cli`) built in Rust Edition 2024 (`L0`–`L5` layered architecture).
- **Isolated Multi-Account Switching**: Spawns each Google Antigravity account in its own `--user-data-dir` + isolated home directory (`HOME` on macOS, `USERPROFILE` on Windows) with live `LIVE` / `READY` / `5H LIMIT` detection.
- **Honest Local 5-Hour Rolling Timers**: Hardware dot-matrix countdown clocks (`5:00 ready` → `H:MM left`), segmented LED bars, and exact 5h/7d call & token breakdowns for Gemini and Claude & GPT — zero guessed quota percentages, zero OAuth reads (Rule R2).
- **Optional Shared Chats & Skills**: Per-profile POSIX symlink (macOS) / NTFS junction (Windows) linking `.gemini/{antigravity, config, skills}` while keeping login vaults separate.
- **PHOSPHOR Orb Lab & Customization**: 12 GPU dot-matrix animations × 5 themes per profile, Samo Obsidian (Dark) & Samo Alabaster (Light) themes, Full (`680×524`) & Compact (`320×420`) layouts, and Privacy **Hide Emails** toggle.
- **Safety Gates (`R1`–`R5`) & Safe Sandbox Mode**: Verified by `cargo run -p xtask -- verify` and `./scripts/test-safe.sh`.

### Release Artifacts
- **macOS (Apple Silicon / arm64)** — built & verified:
  - `AGY-Swap-v0.1.0-macos-arm64.dmg` (`sha256: 8efc7d65999052447a887216942f81c8e71cf941e9f2f651c2f9ef312bac7504`)
  - `agswitch-cli-v0.1.0-macos-arm64.tar.gz` (`sha256: 8b64dd4a0eb42f2d92d6c686c66bd2067d5c2b2751c9e4dff7c8c9515f872506`)
- **Windows (x86_64)** — pending native build & verification (tracked in Issue #1):
  - `AGY-Swap-v0.1.0-windows-x86_64.zip` (to be added by Windows build)
