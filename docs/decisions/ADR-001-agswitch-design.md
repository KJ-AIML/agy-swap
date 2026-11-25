# ADR-001: AgSwitch Design (Cross-Platform Rust + GPUI Antigravity Account Switcher)

- **Status:** Accepted (Autonomous Fast-Track)
- **Date:** 2026-11-24
- **Runtime Profile:** `[user]` Native Cross-Platform Desktop (Rust + GPUI + System Tray / Menu Bar) & `[call]` Paired CLI + Atomic Local JSON Store

## 1. Intent

Developers running multiple **Google Antigravity** accounts (e.g., Personal Ultra, Work Pro, Backup Pro) hit 5-hour quota limits or need isolated Google sessions while keeping their local chat history, rules, and skills intact. The prior art (`poachersbz/antigravity-switcher`) solves this on Windows 11 using C# / WPF / WebView2, WMI, and NTFS junctions, leaving macOS users unsupported and coupling process control tightly to Windows-only APIs.

**AgSwitch** is a lightweight, native, cross-platform Rust desktop & CLI utility that runs on **both macOS and Windows**, opens cleanly from a **top menu bar / system tray icon** into a compact floating command popover with a top navigation bar, and strictly enforces account safety invariants inside a layered `L0`–`L5` Rust engine.

## 2. Goals, Non-Goals, and Anti-Goals

### Goals
- `[user]` Cross-platform native desktop app built in **Rust + GPUI**, supporting **macOS and Windows**.
- `[user]` Accessible from a **top menu bar / system tray icon** with a clean **top-navbar popover panel** following `modern-ui-design` tokens (dark-first obsidian + light mode toggle).
- `[call]` Strictly non-invasive multi-account isolation using `--user-data-dir` and per-process environment overrides (`HOME` on macOS, `USERPROFILE`/`APPDATA`/`LOCALAPPDATA` on Windows, plus `SSH_CONNECTION` for standalone OAuth isolation).
- `[call]` Optional per-account **Shared Chats & Skills** via OS-native directory links (POSIX symlinks on macOS, NTFS directory junctions on Windows) while keeping OAuth tokens and `UserData` strictly isolated.
- `[call]` Live local usage telemetry (5h/24h token & turn tracking, active context budget, cache hit rate, manual 5h cooldown toggle) parsed from `transcript.jsonl`, plus a paired CLI (`agswitch-cli`) sharing the exact same `L4` engine.

### Non-Goals (v1)
- Calling undocumented Google quota APIs or scraping authenticated browser sessions.
- Patching `Antigravity` binaries or mutating Antigravity's internal SQLite databases.
- Cloud sync of profiles, OAuth tokens, or settings across machines.
- Linux GUI tray support beyond compiling cleanly in CI.

### Anti-Goals (Failure Conditions)
- **AG1:** `[call]` **Never destroy the user's primary OS home directory (`~` / `C:\Users\<user>`) or shared `.gemini/antigravity` conversations** when removing an account, resetting login, or toggling shared chats.
- **AG2:** `[call]` **Never read, log, copy, or transmit OAuth token contents** (`jetski-standalone-oauth-token`)—check presence via metadata/existence only.
- **AG3:** `[call]` **Never crash the desktop shell or CLI on malformed transcripts, missing binaries, corrupt JSON, or handler panics.**
- **AG4:** `[call]` **Never modify directory symlinks/junctions or delete disk vaults while the target Antigravity profile is actively running** or without explicit destructive confirmation.
- **AG5:** `[call]` **Never execute an unregistered or unauthorized command**—every state change must traverse `Engine::execute` with centralized audit logging.

## 3. Core Features

| # | Feature | Covers (v1) | Does not cover (v1) |
| :--- | :--- | :--- | :--- |
| F1 | Account Vaults & Discovery | Primary profile + unlimited isolated vaults, auto-discovery, add/edit/hide/remove, authentication marker check | Importing accounts from other switchers |
| F2 | Smart Switching & Side-by-Side | One-click `Switch to Next Ready` (auto-skips 5h cooldown), `Switch` (graceful stop others + launch/focus), `Open`/`Focus` alongside | Auto-detecting server-side HTTP 429 limits |
| F3 | Shared Chats & Skills | Optional symlink (macOS) / NTFS junction (Windows) for `.gemini/{antigravity, config, skills}`, standalone conversation rescue & safe backup | Selective per-conversation sharing |
| F4 | Local Usage & Cooldown Telemetry | 5h/24h turns, steps, tokens, active context budget (300K), cache hit %, `ULTRA`/`PRO` tier badge, manual 5h cooldown toggle | Real-time remote billing/quota sync |
| F5 | Menu Bar / Tray + GPUI Popover | Top menu bar / system tray icon toggling a compact GPUI panel with top navbar (`Accounts`, `Activity`, `Rules`), dark/light themes, modals, and tray quick-switch menu | Global custom hotkey editor |
| F6 | Companion CLI | `agswitch-cli` (`status`, `list`, `next`, `switch`, `open`, `add`, `cooldown`, `audit`, `--json`) sharing L4+L5 | Interactive TUI wizard |

### Feature Behavior
- **F1:** Profiles live under `<app_data>/Profiles/<SafeName>`. Removing the primary profile only sets `hide_default_profile = true` and never touches the OS home directory (`R1`). Removing an isolated profile with `delete_disk = false` adds its normalized path to `ignored_profile_paths` so auto-discovery does not re-add it.
- **F2:** `SwitchToNext` anchors on the currently active profile (or `last_active_profile_id`) and selects the next profile not in active 5h cooldown. Switching closes all other running Antigravity main instances gracefully, cleans up orphan sidecars whose command lines reference those isolated profile paths, then focuses or launches the target.
- **F3:** Enabling shared chats stops the target profile first, rescues any standalone `conversations/` and `brain/` entries into the primary `.gemini/antigravity` directory without overwriting, backs up the old directory to `_isolated_backup_*`, and links `antigravity`, `config`, and `skills`. Unlinking removes only the link/reparse point and never recurses into the link target (`R1`).
- **F4:** Reads `transcript.jsonl` files under primary and unlinked isolated `brain/` folders modified within the last 36 hours (with bounded line/file byte caps), attributes shared-mode timestamps via active heartbeat intervals stored in `usage-ledger.json`, and computes 5h load % (`PRO` = 120 steps, `ULTRA` = 350 steps).
- **F5:** Displays a menu bar / tray icon with both left-click panel toggle and right-click quick actions (`Switch Next`, per-account switch, `Toggle Panel`, `Quit`). Inside the GPUI panel, a top navbar switches cleanly between **Accounts**, **Activity** (engine audit log), and **Rules** (enforced `R1..R5` guardrails), plus Dark/Light theme toggle and explicit confirmation modals for destructive actions.

## 4. Engine-Enforced Rules (`R1..R5`)

| Rule | Protects | Enforcement Layer | Verification Test |
| :--- | :--- | :--- | :--- |
| `R1` | AG1 | `L2 domain::policy` + `L4 engine` blocks any disk deletion or login reset targeting the primary OS home profile, and blocks directory removal unless links are unlinked first | `engine_tests::r1_blocks_primary_disk_delete_and_login_reset` |
| `R2` | AG2 | `L2 domain` & `L5 integrations` strictly use path existence checks for OAuth markers; audit serializer redacts sensitive keys/paths | `domain_tests::r2_audit_redaction_and_marker_paths` |
| `R3` | AG3 | Workspace lints (`unsafe_code = "forbid"`, deny `unwrap_used`/`expect_used`/`panic`/`as_conversions`) + `CatchUnwindFut` boundary inside `Engine::execute` | `engine_tests::r3_isolates_handler_panic_and_records_audit` |
| `R4` | AG4 | `L4 engine` requires `confirmed == true` for destructive disk deletes / login resets and automatically stops running instances of a profile before relinking or deleting | `engine_tests::r4_requires_confirmation_for_destructive_ops` |
| `R5` | AG5 | `L4 engine::CommandRegistry` default-deny dispatch; every `Engine::execute` call validates `AccessPolicy` and writes one centralized `AuditEntry` | `engine_tests::r5_default_deny_and_centralized_audit_trail` |

## 5. Architecture & Stack

Follows `rust-architecture-baseline`:

```text
L0  crates/agswitch-primitives    Pure value types, enums, RuleId, AppError
L1  crates/agswitch-spec          CommandId, AccessPolicy, DTOs, Config, Ledger, LaunchSpec
L2  crates/agswitch-domain        Pure CLI/path parsers, rotation, launch specs, usage math, R1..R5 checks
L3  crates/agswitch-store         StateStore trait, JsonFileStore (atomic tmp+rename), MemoryStore
L4  crates/agswitch-engine        Port traits, CommandRegistry, Engine::execute (CatchUnwind + audit)
L5  crates/agswitch-integrations  Cross-platform macOS & Windows OS adapters (sysinfo, symlinks/junctions, transcripts)
App apps/agswitch-cli             Thin CLI binary sharing L4 + L5 (--json output)
App apps/agswitch-desktop         Native GPUI + tray-icon top menu bar popover app (macOS + Windows)
QA  xtask/                        Automated gates: layers, purity, lints, tests
```

### Baseline Deviations
1. **Native GPUI UI instead of Tauri v2 + React (`ts-rs`):** `[user]` Explicitly requested Rust + GPUI supporting both macOS and Windows.
   - *What the baseline rule protected:* Single source of truth for UI/backend types with zero hand-duplicated DTOs, plus clean UI separation from business logic.
   - *How AgSwitch preserves that guarantee:* `apps/agswitch-desktop` consumes `L0`/`L1` Rust types directly at compile time (eliminating serialization/TS drift completely), invokes all mutations strictly through `L4 Engine::execute`, and implements the `modern-ui-design` dark-first obsidian + light mode token palette and mandatory UI states directly in GPUI. Because there is no TypeScript frontend, `xtask verify` runs `layers`, `purity`, `lints`, and `tests` (omitting `ts-rs` codegen).
2. **Rust Toolchain `1.99.0`:** Pinned in `rust-toolchain.toml` to match the installed local toolchain (`1.99.0`, Edition 2024) while preserving all strict lints (`unsafe_code = "forbid"`, deny `unwrap_used`, `expect_used`, `panic`, `todo`, `unimplemented`, `unreachable`, `dbg_macro`, `as_conversions`).

## 6. Build Order & First End-to-End Demo

- **Phase 1 — Foundation & Gates:** Workspace scaffold, `xtask verify` (`layers`, `purity`, `lints`, `tests`), `L0`–`L2` pure core + unit tests, `L3` atomic JSON & memory stores.
  - *Done when:* `cargo run -p xtask -- verify` passes green.
- **Phase 2 — Vertical Slice Demo:** `L4` command engine (`Engine::execute` + `R1..R5` + mock-rig tests), `L5` cross-platform macOS/Windows adapters, runnable `agswitch-cli`, and native `agswitch-desktop` GPUI menu-bar popover app.
  - *Done when:* `cargo run -p xtask -- verify` passes green, `agswitch-cli status --json` succeeds on macOS, and `agswitch-desktop` compiles cleanly and launches its GPUI window + menu bar icon.

## 7. Carry-Over & Prior Art

| Concept (`poachersbz/antigravity-switcher`) | Destination in `agswitch` | Notes / Left Behind |
| :--- | :--- | :--- |
| `AccountProfile`, `SwitcherConfig` | `L0 primitives` + `L1 spec` | Strong newtypes (`AccountId`, `AccountTier`), cross-platform OS roots |
| `CommandLineParser`, `LaunchSpecBuilder`, `ChooseNextProfile` | `L2 domain` | Pure functions supporting both macOS (`HOME`) and Windows (`USERPROFILE`/`APPDATA`/`LOCALAPPDATA`) |
| `AccountUsageTracker` math & JSONL parsing | `L2 domain` (pure math/parsing) + `L5 integrations` (file I/O) | Split I/O out of calculation so telemetry math is 100% pure and unit-tested |
| `SharedWorkspaceLinker` | `L5 integrations::workspace` | POSIX symlinks on macOS, NTFS junctions on Windows; `R1` & `R4` enforced in `L2`/`L4` |
| `WindowsProcessController` (WMI + DWM) | `L5 integrations::process` | Replaced WMI with safe `sysinfo`; cross-platform focus/close without `unsafe` |
| WPF + WebView2 + `Assets/ui.html` + `TrayIconService` | `apps/agswitch-desktop` (GPUI + `tray-icon`) | Rebuilt as native GPU-accelerated top menu bar popover with top navbar & dual theme |

## 8. Integration Ports

- `ProcessPort` (`L4`) → `OsProcessAdapter` (`L5`, macOS + Windows via `sysinfo` + OS process spawn/focus/stop)
- `WorkspaceLinkPort` (`L4`) → `OsWorkspaceLinkAdapter` (`L5`, symlinks on macOS, junctions on Windows, OAuth marker check, conversation rescue)
- `TranscriptPort` (`L4`) → `OsTranscriptAdapter` (`L5`, bounded `transcript.jsonl` reader with cache)
- `ClockPort` (`L4`) → `SystemClockAdapter` (`L5`, UTC wall clock)

## 9. Left to the Builder

- Exact GPUI spacing/radii micro-tuning, icon vector paths, and test fixture wording.

## 10. Open Questions

- None blocking v1 (all decisions finalized under autonomous fast-track; open for UI refinement after first working build).
