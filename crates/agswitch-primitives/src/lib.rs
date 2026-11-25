use std::fmt;

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Canonical identifier for an Antigravity account slot.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AccountId(String);

impl AccountId {
    pub fn new(raw: impl Into<String>) -> Result<Self, AppError> {
        let trimmed = raw.into().trim().to_owned();
        if trimmed.is_empty() {
            return Err(AppError::Validation(
                "account id must not be empty".to_owned(),
            ));
        }
        Ok(Self(trimmed))
    }

    pub fn default_id() -> Self {
        Self("default".to_owned())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn eq_ignore_case(&self, other: &str) -> bool {
        self.0.eq_ignore_ascii_case(other)
    }
}

impl fmt::Display for AccountId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Subscription tier used for 5-hour load estimation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "UPPERCASE")]
pub enum AccountTier {
    #[default]
    Pro,
    Ultra,
}

impl AccountTier {
    pub const fn steps_5h_budget(self) -> u32 {
        match self {
            Self::Pro => 120,
            Self::Ultra => 350,
        }
    }

    pub const fn steps_7d_budget(self) -> u32 {
        match self {
            Self::Pro => 1_200,
            Self::Ultra => 3_500,
        }
    }

    pub const fn claude_gpt_5h_budget(self) -> u32 {
        match self {
            Self::Pro => 60,
            Self::Ultra => 180,
        }
    }

    pub const fn claude_gpt_7d_budget(self) -> u32 {
        match self {
            Self::Pro => 500,
            Self::Ultra => 1_500,
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Pro => "Pro",
            Self::Ultra => "Ultra",
        }
    }

    pub const fn badge(self) -> &'static str {
        match self {
            Self::Pro => "PRO",
            Self::Ultra => "ULTRA",
        }
    }

    pub fn parse_opt(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_uppercase().as_str() {
            "ULTRA" => Some(Self::Ultra),
            "PRO" => Some(Self::Pro),
            _ => None,
        }
    }
}

/// Model family classification parsed from local `transcript.jsonl` (`PLANNER_RESPONSE.model`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ModelFamily {
    Gemini,
    Claude,
    Gpt,
    #[default]
    Other,
}

impl ModelFamily {
    pub fn classify(raw: Option<&str>) -> Self {
        let Some(name) = raw else {
            return Self::Other;
        };
        let lower = name.trim().to_ascii_lowercase();
        if lower.is_empty() {
            return Self::Other;
        }
        if lower.contains("claude")
            || lower.contains("sonnet")
            || lower.contains("opus")
            || lower.contains("haiku")
            || lower.contains("anthropic")
        {
            Self::Claude
        } else if lower.contains("gpt")
            || lower.contains("openai")
            || lower.contains("codex")
            || lower.starts_with("o1")
            || lower.starts_with("o3")
            || lower.starts_with("o4")
        {
            Self::Gpt
        } else if lower.contains("gemini")
            || lower.contains("gemma")
            || lower.contains("neon")
            || lower.contains("flash")
        {
            Self::Gemini
        } else {
            Self::Other
        }
    }
}

/// Host operating system target for cross-platform path & launch spec generation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RuntimeOs {
    MacOs,
    Windows,
    Linux,
}

impl RuntimeOs {
    pub const fn current() -> Self {
        if cfg!(target_os = "macos") {
            Self::MacOs
        } else if cfg!(target_os = "windows") {
            Self::Windows
        } else {
            Self::Linux
        }
    }
}

/// Engine-enforced safety rule identifiers from ADR-001.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RuleId {
    R1,
    R2,
    R3,
    R4,
    R5,
}

impl RuleId {
    pub const fn all() -> [Self; 5] {
        [Self::R1, Self::R2, Self::R3, Self::R4, Self::R5]
    }

    pub const fn code(self) -> &'static str {
        match self {
            Self::R1 => "R1",
            Self::R2 => "R2",
            Self::R3 => "R3",
            Self::R4 => "R4",
            Self::R5 => "R5",
        }
    }

    pub const fn title(self) -> &'static str {
        match self {
            Self::R1 => "Protect primary OS home & shared chats",
            Self::R2 => "Zero OAuth token inspection or leakage",
            Self::R3 => "Panic-isolated, crash-free execution",
            Self::R4 => "Explicit confirmation & safe stop before mutation",
            Self::R5 => "Default-deny dispatch & centralized audit trail",
        }
    }

    pub const fn description(self) -> &'static str {
        match self {
            Self::R1 => {
                "Blocks disk deletion or login reset on the primary OS profile and unlinks symlinks/junctions before touching any vault."
            }
            Self::R2 => {
                "Checks OAuth auth strictly via file existence; never reads token contents and redacts secrets from audit logs."
            }
            Self::R3 => {
                "Enforces workspace deny lints (no unsafe/unwrap/panic) and wraps command handlers in a CatchUnwind boundary."
            }
            Self::R4 => {
                "Requires explicit confirmation for destructive actions and stops running instances before relinking or deleting."
            }
            Self::R5 => {
                "Every command must be registered with an AccessPolicy in Engine::execute and writes a central audit entry."
            }
        }
    }
}

/// Execution phases emitted during multi-step actions (for UI phase stepper feedback).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PhaseId {
    CheckingPolicy,
    StoppingActive,
    CleaningSidecars,
    LinkingWorkspace,
    LaunchingTarget,
    PersistingState,
    Completed,
}

impl PhaseId {
    pub const fn label(self) -> &'static str {
        match self {
            Self::CheckingPolicy => "Checking rules",
            Self::StoppingActive => "Closing active session",
            Self::CleaningSidecars => "Cleaning orphan sidecars",
            Self::LinkingWorkspace => "Updating workspace links",
            Self::LaunchingTarget => "Launching account",
            Self::PersistingState => "Saving state",
            Self::Completed => "Done",
        }
    }
}

/// Structured error type shared across all workspace layers.
#[derive(Debug, Error, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AppError {
    #[error("not found: {0}")]
    NotFound(String),
    #[error("validation error: {0}")]
    Validation(String),
    #[error("blocked by rule {rule:?}: {message}")]
    PolicyBlocked { rule: RuleId, message: String },
    #[error("unauthorized command execution: {0}")]
    Unauthorized(String),
    #[error("store error: {0}")]
    Store(String),
    #[error("integration error: {0}")]
    Integration(String),
    #[error("internal panic isolated by rule R3 in command {0}")]
    InternalPanic(String),
}
