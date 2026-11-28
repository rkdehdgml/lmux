#![allow(dead_code)]

use crate::usage::{WindowUsage, aggregate_folder};
use std::path::PathBuf;

pub const FIVE_HOUR_LIMIT: u64 = 500_000;
pub const WEEKLY_LIMIT: u64 = 2_500_000;

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountLimits {
    pub five_hour: u64,
    pub weekly: u64,
    pub estimated: bool,
}

#[derive(Clone, serde::Serialize)]
pub struct AccountSummary {
    pub name: String,
    pub paths: Vec<String>,
    pub usage: WindowUsage,
    pub limits: AccountLimits,
}

pub struct AccountRegistry {
    accounts: Vec<(String, Vec<PathBuf>)>,
}

impl Default for AccountRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl AccountRegistry {
    pub fn new() -> Self {
        let mut accounts: Vec<(String, Vec<PathBuf>)> = Vec::new();

        if let Ok(profile) = std::env::var("USERPROFILE") {
            let path = PathBuf::from(profile).join(".claude");
            if path.is_dir() {
                accounts.push(("기본 계정".to_string(), vec![path]));
            }
        }

        for (name, path) in wsl_claude_paths() {
            accounts.push((name, vec![path]));
        }

        Self { accounts }
    }

    pub fn all_summaries(&self, now_ms: u64) -> Vec<AccountSummary> {
        self.accounts
            .iter()
            .map(|(name, paths)| {
                let mut combined = WindowUsage::default();
                for path in paths {
                    merge_window(&mut combined, &aggregate_folder(path, now_ms));
                }
                AccountSummary {
                    name: name.clone(),
                    paths: paths
                        .iter()
                        .map(|p| p.to_string_lossy().into_owned())
                        .collect(),
                    usage: combined,
                    limits: AccountLimits {
                        five_hour: FIVE_HOUR_LIMIT,
                        weekly: WEEKLY_LIMIT,
                        estimated: true,
                    },
                }
            })
            .collect()
    }
}

fn merge_window(dst: &mut WindowUsage, src: &WindowUsage) {
    dst.today.input += src.today.input;
    dst.today.output += src.today.output;
    dst.today.cache_creation += src.today.cache_creation;
    dst.today.cache_read += src.today.cache_read;
    dst.five_hour.input += src.five_hour.input;
    dst.five_hour.output += src.five_hour.output;
    dst.five_hour.cache_creation += src.five_hour.cache_creation;
    dst.five_hour.cache_read += src.five_hour.cache_read;
    dst.seven_day.input += src.seven_day.input;
    dst.seven_day.output += src.seven_day.output;
    dst.seven_day.cache_creation += src.seven_day.cache_creation;
    dst.seven_day.cache_read += src.seven_day.cache_read;
}

fn wsl_claude_paths() -> Vec<(String, PathBuf)> {
    let mut result = Vec::new();
    for distro in crate::shell::wsl_distros() {
        let home_root = PathBuf::from(format!(r"\\wsl$\{distro}\home"));
        let Ok(entries) = std::fs::read_dir(&home_root) else {
            continue;
        };
        for entry in entries.flatten() {
            let claude_path = entry.path().join(".claude");
            if claude_path.is_dir() {
                let user_name = entry.file_name().to_string_lossy().into_owned();
                result.push((format!("WSL:{distro}/{user_name}"), claude_path));
            }
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_new_includes_windows_claude() {
        let reg = AccountRegistry::new();
        let _ = reg.all_summaries(crate::usage::now_ms());
    }

    #[test]
    fn all_summaries_returns_estimated_limits() {
        let dir = tempfile::tempdir().unwrap();
        let reg = AccountRegistry {
            accounts: vec![("테스트".to_string(), vec![dir.path().to_path_buf()])],
        };
        let summaries = reg.all_summaries(crate::usage::now_ms());
        assert_eq!(summaries.len(), 1);
        assert!(summaries[0].limits.estimated);
        assert_eq!(summaries[0].limits.five_hour, FIVE_HOUR_LIMIT);
    }
}
