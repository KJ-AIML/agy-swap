use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use agswitch_primitives::AppError;
use agswitch_spec::{AuditEntry, SwitcherConfig, UsageLedger};

const MAX_AUDIT_ENTRIES: usize = 200;

/// Persistence boundary isolated strictly inside `L3 store`.
pub trait StateStore: Send + Sync {
    fn load_config(&self) -> Result<Option<SwitcherConfig>, AppError>;
    fn save_config(&self, config: &SwitcherConfig) -> Result<(), AppError>;
    fn load_ledger(&self) -> Result<UsageLedger, AppError>;
    fn save_ledger(&self, ledger: &UsageLedger) -> Result<(), AppError>;
    fn append_audit(&self, entry: &AuditEntry) -> Result<(), AppError>;
    fn load_recent_audits(&self, limit: usize) -> Result<Vec<AuditEntry>, AppError>;
}

/// Atomic local JSON file store (`switcher-config.json`, `usage-ledger.json`, `audit-log.jsonl`).
#[derive(Debug, Clone)]
pub struct JsonFileStore {
    data_dir: PathBuf,
}

impl JsonFileStore {
    pub fn new(data_dir: PathBuf) -> Self {
        Self { data_dir }
    }

    pub fn config_path(&self) -> PathBuf {
        self.data_dir.join("switcher-config.json")
    }

    pub fn ledger_path(&self) -> PathBuf {
        self.data_dir.join("usage-ledger.json")
    }

    pub fn audit_path(&self) -> PathBuf {
        self.data_dir.join("audit-log.jsonl")
    }

    fn write_atomic_bytes(path: &Path, bytes: &[u8]) -> Result<(), AppError> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| AppError::Store(format!("create_dir_all failed: {e}")))?;
        }
        let file_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("state.json");
        let tmp_path = path.with_file_name(format!(".{file_name}.tmp"));
        {
            let mut f = fs::File::create(&tmp_path)
                .map_err(|e| AppError::Store(format!("create tmp file failed: {e}")))?;
            f.write_all(bytes)
                .map_err(|e| AppError::Store(format!("write tmp file failed: {e}")))?;
            f.sync_all()
                .map_err(|e| AppError::Store(format!("sync tmp file failed: {e}")))?;
        }
        fs::rename(&tmp_path, path)
            .map_err(|e| AppError::Store(format!("atomic rename failed: {e}")))?;
        Ok(())
    }
}

impl StateStore for JsonFileStore {
    fn load_config(&self) -> Result<Option<SwitcherConfig>, AppError> {
        let path = self.config_path();
        if !path.exists() {
            return Ok(None);
        }
        let raw = fs::read_to_string(&path)
            .map_err(|e| AppError::Store(format!("read config failed: {e}")))?;
        match serde_json::from_str::<SwitcherConfig>(&raw) {
            Ok(cfg) => Ok(Some(cfg)),
            // Corrupt config recovers safely instead of crashing (AG3 / R3)
            Err(_) => Ok(None),
        }
    }

    fn save_config(&self, config: &SwitcherConfig) -> Result<(), AppError> {
        let json = serde_json::to_vec_pretty(config)
            .map_err(|e| AppError::Store(format!("serialize config failed: {e}")))?;
        Self::write_atomic_bytes(&self.config_path(), &json)
    }

    fn load_ledger(&self) -> Result<UsageLedger, AppError> {
        let path = self.ledger_path();
        if !path.exists() {
            return Ok(UsageLedger::default());
        }
        let raw = fs::read_to_string(&path)
            .map_err(|e| AppError::Store(format!("read ledger failed: {e}")))?;
        Ok(serde_json::from_str::<UsageLedger>(&raw).unwrap_or_default())
    }

    fn save_ledger(&self, ledger: &UsageLedger) -> Result<(), AppError> {
        let json = serde_json::to_vec_pretty(ledger)
            .map_err(|e| AppError::Store(format!("serialize ledger failed: {e}")))?;
        Self::write_atomic_bytes(&self.ledger_path(), &json)
    }

    fn append_audit(&self, entry: &AuditEntry) -> Result<(), AppError> {
        let mut entries = self.load_recent_audits(MAX_AUDIT_ENTRIES)?;
        entries.push(entry.clone());
        if entries.len() > MAX_AUDIT_ENTRIES {
            let drain_count = entries.len().saturating_sub(MAX_AUDIT_ENTRIES);
            entries.drain(0..drain_count);
        }
        let mut buf = String::new();
        for item in &entries {
            if let Ok(line) = serde_json::to_string(item) {
                buf.push_str(&line);
                buf.push('\n');
            }
        }
        Self::write_atomic_bytes(&self.audit_path(), buf.as_bytes())
    }

    fn load_recent_audits(&self, limit: usize) -> Result<Vec<AuditEntry>, AppError> {
        let path = self.audit_path();
        if !path.exists() {
            return Ok(Vec::new());
        }
        let raw = fs::read_to_string(&path)
            .map_err(|e| AppError::Store(format!("read audit log failed: {e}")))?;
        let mut parsed: Vec<AuditEntry> = raw
            .lines()
            .filter_map(|line| serde_json::from_str::<AuditEntry>(line).ok())
            .collect();
        if parsed.len() > limit {
            let start = parsed.len().saturating_sub(limit);
            parsed = parsed.split_off(start);
        }
        Ok(parsed)
    }
}

/// Deterministic in-memory store for unit and mock-rig engine tests.
#[derive(Debug, Default)]
pub struct MemoryStore {
    state: Mutex<MemoryState>,
}

#[derive(Debug, Default)]
struct MemoryState {
    config: Option<SwitcherConfig>,
    ledger: UsageLedger,
    audits: Vec<AuditEntry>,
}

impl MemoryStore {
    pub fn new() -> Self {
        Self::default()
    }
}

impl StateStore for MemoryStore {
    fn load_config(&self) -> Result<Option<SwitcherConfig>, AppError> {
        let guard = self
            .state
            .lock()
            .map_err(|e| AppError::Store(format!("memory lock poisoned: {e}")))?;
        Ok(guard.config.clone())
    }

    fn save_config(&self, config: &SwitcherConfig) -> Result<(), AppError> {
        let mut guard = self
            .state
            .lock()
            .map_err(|e| AppError::Store(format!("memory lock poisoned: {e}")))?;
        guard.config = Some(config.clone());
        Ok(())
    }

    fn load_ledger(&self) -> Result<UsageLedger, AppError> {
        let guard = self
            .state
            .lock()
            .map_err(|e| AppError::Store(format!("memory lock poisoned: {e}")))?;
        Ok(guard.ledger.clone())
    }

    fn save_ledger(&self, ledger: &UsageLedger) -> Result<(), AppError> {
        let mut guard = self
            .state
            .lock()
            .map_err(|e| AppError::Store(format!("memory lock poisoned: {e}")))?;
        guard.ledger = ledger.clone();
        Ok(())
    }

    fn append_audit(&self, entry: &AuditEntry) -> Result<(), AppError> {
        let mut guard = self
            .state
            .lock()
            .map_err(|e| AppError::Store(format!("memory lock poisoned: {e}")))?;
        guard.audits.push(entry.clone());
        if guard.audits.len() > MAX_AUDIT_ENTRIES {
            let drop_n = guard.audits.len().saturating_sub(MAX_AUDIT_ENTRIES);
            guard.audits.drain(0..drop_n);
        }
        Ok(())
    }

    fn load_recent_audits(&self, limit: usize) -> Result<Vec<AuditEntry>, AppError> {
        let guard = self
            .state
            .lock()
            .map_err(|e| AppError::Store(format!("memory lock poisoned: {e}")))?;
        let len = guard.audits.len();
        let start = len.saturating_sub(limit);
        Ok(guard.audits.get(start..).unwrap_or_default().to_vec())
    }
}

#[cfg(test)]
mod tests {
    use agswitch_spec::{AccountProfile, AuditOutcome, CommandId};
    use chrono::Utc;

    use super::*;

    #[test]
    fn json_store_atomic_roundtrip_and_corrupt_recovery() -> Result<(), String> {
        let tmp = tempfile::tempdir().map_err(|e| e.to_string())?;
        let store = JsonFileStore::new(tmp.path().to_path_buf());

        let cfg = SwitcherConfig {
            profiles_root_dir: "/tmp/profiles".to_owned(),
            antigravity_bin_path: "/Applications/Antigravity.app/Contents/MacOS/Antigravity"
                .to_owned(),
            last_active_profile_id: Some("default".to_owned()),
            hide_default_profile: false,
            ignored_profile_paths: Vec::new(),
            profiles: vec![AccountProfile::create_default()],
        };

        store.save_config(&cfg).map_err(|e| e.to_string())?;
        let loaded = store
            .load_config()
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "expected saved config".to_owned())?;
        assert_eq!(loaded, cfg);

        // Corrupt JSON recovers safely to None instead of panicking
        fs::write(store.config_path(), "{not valid json").map_err(|e| e.to_string())?;
        let recovered = store.load_config().map_err(|e| e.to_string())?;
        assert!(recovered.is_none());

        let entry = AuditEntry {
            timestamp_utc: Utc::now(),
            command: CommandId::SwitchNext,
            summary: "switched to work".to_owned(),
            outcome: AuditOutcome::Ok,
            rule: None,
        };
        store.append_audit(&entry).map_err(|e| e.to_string())?;
        let audits = store.load_recent_audits(10).map_err(|e| e.to_string())?;
        assert_eq!(audits.len(), 1);
        Ok(())
    }
}
