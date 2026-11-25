---
title: How AGY Swap Works
subtitle: Isolated multi-account switching, local 5h timers, and R1–R5 safety gates across macOS & Windows
eyebrow: Architecture Tour
theme: auto
cols: 3
---

Lead: AGY Swap switches Google Antigravity accounts by launching each account inside its own `--user-data-dir` and isolated home directory, guarded by a strict `L0`–`L5` Rust engine that enforces Rules `R1`–`R5` and computes 5h rolling timers purely from local transcripts with zero OAuth access.

## A Inside one account switch {span=3}

**How to read this:** Step through all four stages to trace one `Switch` command from the GPUI menu-bar popover through the `L4 Engine` safety gates to an isolated Antigravity profile and back.

```isokit
{
  "title": "One Switch Across L0–L5 Boundaries",
  "subtitle": "From a menu-bar click to an isolated Antigravity window with zero OAuth reads.",
  "meta": ["L0–L5 Rust", "Rule R2 Safe"],
  "legend": [
    { "kind": "req", "label": "command" },
    { "kind": "write", "label": "spawn / persist" },
    { "kind": "push", "label": "telemetry" }
  ],
  "metrics": [
    { "key": "oauth", "label": "OAuth reads", "unit": "" },
    { "key": "rules", "label": "Active gates", "unit": "/5" },
    { "key": "ms", "label": "Dispatch", "unit": " ms" }
  ],
  "components": {
    "ui":     { "type": "block", "x": -10, "y": 0,  "color": "blue",   "glyph": "window", "label": ["AGY Swap", "GPUI / CLI"], "pills": "below" },
    "engine": { "type": "block", "x": 0,   "y": 0,  "color": "orange", "glyph": "shield", "label": ["L4 Engine", "R1–R5 Guard"], "pills": "below" },
    "domain": { "type": "block", "x": 0,   "y": 9,  "color": "teal",   "glyph": "cpu",    "label": ["L2 Domain", "5h Timer"] },
    "store":  { "type": "db",    "x": 10,  "y": 9,  "color": "purple", "label": "L3 Store" },
    "vault":  { "type": "block", "x": 10,  "y": -1, "color": "green",  "glyph": "lock",   "label": ["L5 Vault", "Profile Home"], "pills": "below" }
  },
  "wires": {
    "cmd":      { "from": "ui",     "to": "engine", "pts": ["ui", "engine"], "label": "CommandRequest" },
    "check":    { "from": "engine", "to": "domain", "pts": ["engine", "domain"], "label": "Pure rules" },
    "spawn":    { "from": "engine", "to": "vault",  "pts": ["engine", ["vault", "engine"], "vault"], "label": "--user-data-dir" },
    "audit":    { "from": "engine", "to": "store",  "pts": ["engine", "store"], "label": "Audit JSON" },
    "scan":     { "from": "vault",  "to": "domain", "pts": ["vault", ["vault", "domain"], "domain"], "label": "transcript.jsonl" }
  },
  "goals": [
    { "text": "Every action passes default-deny R1–R5 gates", "at": 1 },
    { "text": "Shared chats link without sharing OAuth credentials", "at": 2 },
    { "text": "5h rolling timers come strictly from local transcripts", "at": 3 }
  ],
  "steps": [
    {
      "short": "Gate check",
      "chapter": "setup",
      "title": "Menu bar or CLI sends a typed command",
      "body": [
        "Both the native GPUI popover and `agswitch` CLI send the exact same `CommandRequest` into `Engine::execute`.",
        "The engine runs default-deny registry lookup (**R5**), verifies primary-profile safety (**R1**), and isolates panics behind `CatchUnwind` (**R3**)."
      ],
      "changes": ["+ GPUI & CLI entry", "+ L4 Engine R1–R5 guard", "+ Pure L2 Domain rules"],
      "metrics": { "oauth": 0, "rules": 5, "ms": 2 },
      "scene": {
        "entities": {
          "ui":     { "use": "ui",     "cpu": 0.14, "tags": [{ "text": "LIVE", "tone": "green", "icon": "check" }] },
          "engine": { "use": "engine", "cpu": 0.30, "tags": [{ "text": "R1–R5 OK", "tone": "green", "icon": "shield" }] },
          "domain": { "use": "domain", "cpu": 0.22, "tags": [{ "text": "5h timer", "tone": "blue", "icon": "timer" }] },
          "store":  { "use": "store",  "cpu": 0.18, "tags": [{ "text": "Redacted", "tone": "violet", "icon": "scroll" }] },
          "vault":  { "use": "vault",  "cpu": 0.26, "tags": [{ "text": "No OAuth read", "tone": "green", "icon": "lock" }] }
        },
        "links": ["cmd", "check", "spawn", "audit", "scan"],
        "flows": [
          { "route": ["cmd", "check"], "kind": "req", "bounce": true, "rate": 1.6 },
          { "route": ["cmd", "spawn"], "kind": "write", "rate": 1.4 },
          { "route": ["scan"], "kind": "push", "rate": 1.3 }
        ]
      }
    },
    {
      "short": "Isolate",
      "chapter": "solution",
      "title": "Spawn Antigravity inside an isolated profile vault",
      "body": [
        "`L5 Integrations` closes the previous window cleanly and spawns Antigravity with `--user-data-dir` plus an isolated `HOME` (macOS) or `USERPROFILE` (Windows).",
        "When **Shared Chats** is enabled, `.gemini/{antigravity, config, skills}` links via symlink or NTFS junction while login vaults stay 100% separate."
      ],
      "changes": ["+ Isolated L5 Vault", "~ Separate OAuth & UserData", "~ Optional chat symlink/junction"],
      "metrics": { "oauth": 0, "rules": 5, "ms": 48 },
      "scene": {
        "entities": {
          "ui":     { "use": "ui",     "cpu": 0.14 },
          "engine": { "use": "engine", "cpu": 0.42 },
          "domain": { "use": "domain", "cpu": 0.18 },
          "vault":  { "use": "vault",  "cpu": 0.35, "tags": [{ "text": "Zero OAuth read", "tone": "green", "icon": "lock" }] }
        },
        "links": ["cmd", "check", "spawn"],
        "flows": [
          { "route": ["cmd", "spawn"], "kind": "write", "rate": 1.9 }
        ]
      }
    },
    {
      "short": "5h timer",
      "chapter": "technique",
      "title": "Compute exact 5h rolling timers locally",
      "body": [
        "`L2 Domain` parses local `transcript.jsonl` files inside each profile to compute exact **5-hour rolling reset timers** (`H:MM`) and **7-day call/token totals** split by Gemini vs Claude & GPT.",
        "No server quotas are guessed and no private Google OAuth endpoints are ever called (**Rule R2**)."
      ],
      "changes": ["~ Local transcript.jsonl scan", "~ Exact H:MM 5h timers (no % guesses)"],
      "metrics": { "oauth": 0, "rules": 5, "ms": 14 },
      "scene": {
        "entities": {
          "ui":     { "use": "ui",     "cpu": 0.16 },
          "engine": { "use": "engine", "cpu": 0.25 },
          "domain": { "use": "domain", "cpu": 0.38, "tags": [{ "text": "5:00 -> 0:00", "tone": "blue", "icon": "timer" }] },
          "vault":  { "use": "vault",  "cpu": 0.20 }
        },
        "links": ["cmd", "check", "spawn", "scan"],
        "flows": [
          { "route": ["scan"], "kind": "push", "rate": 1.7 }
        ]
      }
    },
    {
      "short": "Audit",
      "chapter": "recap",
      "title": "Persist redacted audit trail and refresh UI",
      "body": [
        "`L3 Store` atomically writes profile metadata, cooldown timestamps, and a redacted audit log (`R2`) to local JSON state.",
        "The updated `DashboardSnapshot` flows back to the menu bar, PHOSPHOR LED orbs, and CLI."
      ],
      "changes": ["+ L3 Store atomic JSON", "+ Redacted R2 audit log", "~ Live snapshot to GPUI & CLI"],
      "metrics": { "oauth": 0, "rules": 5, "ms": 6 },
      "scene": {
        "entities": {
          "ui":     { "use": "ui",     "cpu": 0.12, "tags": [{ "text": "LIVE", "tone": "green", "icon": "check" }] },
          "engine": { "use": "engine", "cpu": 0.22 },
          "domain": { "use": "domain", "cpu": 0.14 },
          "store":  { "use": "store",  "cpu": 0.28, "tags": [{ "text": "Redacted", "tone": "violet", "icon": "scroll" }] },
          "vault":  { "use": "vault",  "cpu": 0.20 }
        },
        "links": ["cmd", "check", "spawn", "audit", "scan"],
        "flows": [
          { "route": ["cmd", "audit"], "kind": "write", "rate": 1.4 },
          { "route": ["cmd"], "kind": "push", "bounce": true, "rate": 1.4 }
        ]
      }
    }
  ]
}
```

```callout ok Key invariant
Every destructive or OS-level action flows through `Engine::execute` (L4), never directly from the GPUI view or CLI — and `cargo run -p xtask -- verify` enforces all layer boundaries automatically.
```
