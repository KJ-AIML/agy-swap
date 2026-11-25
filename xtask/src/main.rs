use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use cargo_metadata::{DependencyKind, MetadataCommand};

struct LayerPolicy {
    allowed_workspace_deps: &'static [&'static str],
    forbidden_external_deps: &'static [&'static str],
    forbidden_tokens: &'static [&'static str],
}

fn layer_policies() -> BTreeMap<&'static str, LayerPolicy> {
    let mut map = BTreeMap::new();

    map.insert(
        "agswitch-primitives",
        LayerPolicy {
            allowed_workspace_deps: &[],
            forbidden_external_deps: &[
                "tokio", "reqwest", "sqlx", "tauri", "gpui", "tray-icon", "sysinfo", "dirs",
            ],
            forbidden_tokens: &[
                "std::fs",
                "std::process",
                "std::net",
                "tokio::",
                "reqwest::",
                "sqlx::",
                "tauri::",
                "gpui::",
                "tray_icon::",
                "sysinfo::",
            ],
        },
    );

    map.insert(
        "agswitch-spec",
        LayerPolicy {
            allowed_workspace_deps: &["agswitch-primitives"],
            forbidden_external_deps: &[
                "tokio", "reqwest", "sqlx", "tauri", "gpui", "tray-icon", "sysinfo", "dirs",
            ],
            forbidden_tokens: &[
                "std::fs",
                "std::process",
                "std::net",
                "tokio::",
                "reqwest::",
                "sqlx::",
                "tauri::",
                "gpui::",
                "tray_icon::",
                "sysinfo::",
            ],
        },
    );

    map.insert(
        "agswitch-domain",
        LayerPolicy {
            allowed_workspace_deps: &["agswitch-primitives", "agswitch-spec"],
            forbidden_external_deps: &[
                "tokio", "reqwest", "sqlx", "tauri", "gpui", "tray-icon", "sysinfo", "dirs",
            ],
            forbidden_tokens: &[
                "std::fs",
                "std::process",
                "std::net",
                "tokio::",
                "reqwest::",
                "sqlx::",
                "tauri::",
                "gpui::",
                "tray_icon::",
                "sysinfo::",
            ],
        },
    );

    map.insert(
        "agswitch-store",
        LayerPolicy {
            allowed_workspace_deps: &[
                "agswitch-primitives",
                "agswitch-spec",
                "agswitch-domain",
            ],
            forbidden_external_deps: &["reqwest", "tauri", "gpui", "tray-icon", "sysinfo"],
            forbidden_tokens: &[
                "std::process",
                "std::net",
                "reqwest::",
                "tauri::",
                "gpui::",
                "tray_icon::",
                "sysinfo::",
            ],
        },
    );

    map.insert(
        "agswitch-engine",
        LayerPolicy {
            allowed_workspace_deps: &[
                "agswitch-primitives",
                "agswitch-spec",
                "agswitch-domain",
                "agswitch-store",
            ],
            forbidden_external_deps: &["reqwest", "tauri", "gpui", "tray-icon", "sysinfo", "dirs"],
            forbidden_tokens: &[
                "std::fs",
                "std::process",
                "std::net",
                "reqwest::",
                "tauri::",
                "gpui::",
                "tray_icon::",
                "sysinfo::",
            ],
        },
    );

    map.insert(
        "agswitch-integrations",
        LayerPolicy {
            allowed_workspace_deps: &["agswitch-primitives", "agswitch-engine"],
            forbidden_external_deps: &["tauri", "gpui", "tray-icon"],
            forbidden_tokens: &["tauri::", "gpui::", "tray_icon::"],
        },
    );

    map
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let task = args.first().map(String::as_str).unwrap_or("verify");

    let result = match task {
        "layers" => check_layers(),
        "purity" => check_purity(),
        "lints" => run_cargo(&["clippy", "--workspace", "--all-targets", "--", "-D", "warnings"]),
        "tests" => run_cargo(&["test", "--workspace"]),
        "verify" => check_layers()
            .and_then(|()| check_purity())
            .and_then(|()| {
                run_cargo(&[
                    "clippy",
                    "--workspace",
                    "--all-targets",
                    "--",
                    "-D",
                    "warnings",
                ])
            })
            .and_then(|()| run_cargo(&["test", "--workspace"])),
        other => Err(format!(
            "unknown xtask '{other}'. expected: verify | layers | purity | lints | tests"
        )),
    };

    match result {
        Ok(()) => {
            println!("xtask {task}: PASS");
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!("xtask {task}: FAIL\n{err}");
            ExitCode::FAILURE
        }
    }
}

fn check_layers() -> Result<(), String> {
    println!("==> gate: layers");
    let metadata = MetadataCommand::new()
        .no_deps()
        .exec()
        .map_err(|e| format!("failed to run cargo metadata: {e}"))?;

    let workspace_names: BTreeSet<String> = metadata
        .packages
        .iter()
        .map(|p| p.name.to_string())
        .collect();
    let policies = layer_policies();
    let mut violations = Vec::new();

    for pkg in &metadata.packages {
        let Some(policy) = policies.get(pkg.name.as_str()) else {
            continue;
        };

        for dep in &pkg.dependencies {
            if dep.kind == DependencyKind::Development {
                continue;
            }
            if workspace_names.contains(&dep.name) {
                if !policy.allowed_workspace_deps.contains(&dep.name.as_str()) {
                    violations.push(format!(
                        "{} -> {} (disallowed internal workspace dependency)",
                        pkg.name, dep.name
                    ));
                }
            } else if policy.forbidden_external_deps.contains(&dep.name.as_str()) {
                violations.push(format!(
                    "{} -> {} (forbidden external dependency for this layer)",
                    pkg.name, dep.name
                ));
            }
        }
    }

    if violations.is_empty() {
        Ok(())
    } else {
        Err(format!("layer violations:\n- {}", violations.join("\n- ")))
    }
}

fn check_purity() -> Result<(), String> {
    println!("==> gate: purity");
    let metadata = MetadataCommand::new()
        .no_deps()
        .exec()
        .map_err(|e| format!("failed to run cargo metadata: {e}"))?;
    let policies = layer_policies();
    let mut violations = Vec::new();

    for pkg in &metadata.packages {
        let Some(policy) = policies.get(pkg.name.as_str()) else {
            continue;
        };

        let Some(pkg_dir) = pkg.manifest_path.parent() else {
            continue;
        };
        let src_dir = PathBuf::from(pkg_dir.as_str()).join("src");
        if !src_dir.exists() {
            continue;
        }

        let mut rs_files = Vec::new();
        collect_rs_files(&src_dir, &mut rs_files)?;

        for file in rs_files {
            let content = fs::read_to_string(&file)
                .map_err(|e| format!("failed to read {}: {e}", file.display()))?;
            let normalized_path = file.to_string_lossy().replace('\\', "/");

            for (idx, line) in content.lines().enumerate() {
                let trimmed = line.trim();
                if trimmed.starts_with("//") {
                    continue;
                }
                for token in policy.forbidden_tokens {
                    if trimmed.contains(token) {
                        violations.push(format!(
                            "{}:{} ({}) contains forbidden token '{}'",
                            normalized_path,
                            idx + 1,
                            pkg.name,
                            token
                        ));
                    }
                }
            }
        }
    }

    if violations.is_empty() {
        Ok(())
    } else {
        Err(format!("purity violations:\n- {}", violations.join("\n- ")))
    }
}

fn collect_rs_files(dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), String> {
    let entries =
        fs::read_dir(dir).map_err(|e| format!("failed to read dir {}: {e}", dir.display()))?;
    for entry in entries {
        let entry = entry.map_err(|e| format!("failed to inspect dir entry: {e}"))?;
        let path = entry.path();
        if path.is_dir() {
            collect_rs_files(&path, out)?;
        } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
            out.push(path);
        }
    }
    Ok(())
}

fn run_cargo(args: &[&str]) -> Result<(), String> {
    println!("==> gate: cargo {}", args.join(" "));
    let status = Command::new("cargo")
        .args(args)
        .status()
        .map_err(|e| format!("failed to launch cargo: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "cargo {} exited with status {}",
            args.join(" "),
            status
        ))
    }
}
