# Implementation Plan: AgSwitch (Phase 1–2)

- **Design Doc:** `docs/decisions/ADR-001-agswitch-design.md`
- **Profile:** Native Desktop (Rust + GPUI + Tray/Menu Bar) & Paired CLI + Atomic Local JSON Store

## Global Constraints
- Rust Edition 2024, toolchain pinned at `1.99.0` in `rust-toolchain.toml`
- Workspace lints: `unsafe_code = "forbid"`, `unused_must_use = "deny"`, clippy deny `unwrap_used`, `expect_used`, `panic`, `todo`, `unimplemented`, `unreachable`, `dbg_macro`, `as_conversions`
- `clippy.toml` disallows `unwrap`/`expect`/`panic` even inside tests
- Engine rules `R1`–`R5` enforced in `L2 domain` and `L4 engine`
- Verification gate: `cargo run -p xtask -- verify`

## Plan-Level Calls (`[call]`)
- Workspace / binary names: `agswitch-cli`, `agswitch-desktop`
- Crate prefix: `agswitch-*`
- Env var prefix: `AGSWITCH_*` (e.g., `AGSWITCH_DATA_DIR`, `AGSWITCH_ANTIGRAVITY_BIN`)
- Config & data directory: platform `dirs::data_local_dir()/AgSwitch` (macOS: `~/Library/Application Support/AgSwitch`, Windows: `%LOCALAPPDATA%\AgSwitch`)
- Default Antigravity binary detection:
  - macOS: `/Applications/Antigravity.app/Contents/MacOS/Antigravity` (fallback `~/Applications/Antigravity.app/Contents/MacOS/Antigravity`)
  - Windows: `%LOCALAPPDATA%\Programs\antigravity\Antigravity.exe`

## Review Focus (Top 5 Failure Modes)

| # | Real-World Failure Mode | Owning Task | Guard / Test |
| :--- | :--- | :--- | :--- |
| 1 | Deleting primary OS home dir or following symlink/junction into primary `.gemini/antigravity` during account removal | Task 2 & Task 4 | `r1_blocks_primary_disk_delete_and_login_reset` + `os_workspace_unlink_preserves_target` |
| 2 | macOS vs Windows path/command-line mismatch failing to detect running Antigravity instances or misclassifying child processes | Task 2 & Task 5 | `cmdline_parser_handles_macos_and_windows_paths` |
| 3 | Partial/corrupt JSON config or ledger write on crash/interrupt | Task 3 | `json_store_atomic_roundtrip_and_corrupt_recovery` |
| 4 | Uncaught panic or malformed `transcript.jsonl` crashing the GPUI desktop shell | Task 2 & Task 4 | `r3_isolates_handler_panic_and_records_audit` + `transcript_parser_ignores_malformed_lines` |
| 5 | Layer drift (`L0`–`L4` pulling `std::fs`/`std::process`/`gpui`) or `as` / `unwrap` regressions | Task 1 | `cargo run -p xtask -- verify` (`layers` + `purity` + `lints`) |

## Verification Command
```bash
cargo run -p xtask -- verify
```

## Tasks

### Task 1: Workspace Scaffold & `xtask verify`
- **Files:** `Cargo.toml`, `rust-toolchain.toml`, `clippy.toml`, `.gitattributes`, `.gitignore`, `xtask/Cargo.toml`, `xtask/src/main.rs`
- **Produces:** Layered `L0`–`L5` + `apps/*` workspace skeleton and `xtask` running `layers`, `purity`, `lints`, and `tests`.
- **Verify:** `cargo run -p xtask -- verify`

### Task 2: `L0 primitives`, `L1 spec`, and `L2 domain`
- **Files:** `crates/agswitch-primitives/`, `crates/agswitch-spec/`, `crates/agswitch-domain/`
- **Produces:** Pure domain types, `CommandId`, `AccessPolicy`, DTOs, cross-platform `CommandLineParser` & `LaunchSpecBuilder`, smart cooldown-aware rotation, pure `transcript.jsonl` usage calculator, and `R1..R5` domain validators with unit tests.
- **Verify:** `cargo test -p agswitch-domain`

### Task 3: `L3 store`
- **Files:** `crates/agswitch-store/`
- **Produces:** `StateStore` trait, `JsonFileStore` (atomic `.tmp` write + rename for config, ledger, and audit log), and `MemoryStore` with round-trip tests.
- **Verify:** `cargo test -p agswitch-store`

### Task 4: `L4 engine`
- **Files:** `crates/agswitch-engine/`
- **Produces:** Port traits (`ProcessPort`, `WorkspaceLinkPort`, `TranscriptPort`, `ClockPort`), `CommandRegistry` (default-deny), `Engine::execute` (`CatchUnwindFut` panic boundary + centralized redacted audit trail), and scripted mock-rig tests verifying `R1..R5` and full account switching flows.
- **Verify:** `cargo test -p agswitch-engine`

### Task 5: `L5 integrations` (macOS + Windows)
- **Files:** `crates/agswitch-integrations/`
- **Produces:** `OsProcessAdapter` (`sysinfo` + cross-platform spawn/graceful-close/orphan-cleanup/focus), `OsWorkspaceLinkAdapter` (symlinks on macOS, junctions on Windows, rescue + safe unlink), `OsTranscriptAdapter` (cached bounded reader), and `SystemClockAdapter`.
- **Verify:** `cargo test -p agswitch-integrations`

### Task 6: App Shells (`agswitch-cli` + `agswitch-desktop` GPUI Menu-Bar App) & Final Gate
- **Files:** `apps/agswitch-cli/`, `apps/agswitch-desktop/`
- **Produces:** Paired CLI with `--json` output, and native GPUI desktop popover + top menu bar / system tray icon (`tray-icon`) with top navbar (`Accounts`, `Activity`, `Rules`), dark-first obsidian + light mode tokens, telemetry strip, account cards, inline add/edit form, and destructive confirmation modal.
- **Verify:** `cargo run -p xtask -- verify` + live `agswitch-cli status --json` smoke test + `agswitch-desktop` launch check.
