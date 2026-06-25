use crate::config;
use crate::error::{MirrorError, Result};
use crate::traits::SourceManager;
use crate::types::Mirror;
use crate::utils;
use async_trait::async_trait;
use regex::Regex;
use std::path::PathBuf;
use tokio::fs;

const LEGACY_APT_SOURCES: &str = "/etc/apt/sources.list";
const UBUNTU_DEB822_SOURCES: &str = "/etc/apt/sources.list.d/ubuntu.sources";

pub struct AptManager {
    distro: String,
    custom_path: Option<PathBuf>,
}

impl AptManager {
    pub fn new() -> Self {
        // Simple heuristic detection (synchronous is fine here for construction,
        // or we can detect lazily. For now, let's try to detect once).
        // Since we are inside a specific tool, we can try to read /etc/os-release
        let distro = Self::detect_distro().unwrap_or_else(|| "ubuntu".to_string());
        Self {
            distro,
            custom_path: None,
        }
    }

    #[cfg(test)]
    pub fn with_distro_and_path(distro: String, path: PathBuf) -> Self {
        Self {
            distro,
            custom_path: Some(path),
        }
    }

    fn detect_distro() -> Option<String> {
        // Quick check of os-release
        if let Ok(content) = std::fs::read_to_string("/etc/os-release") {
            if content.to_lowercase().contains("id=ubuntu") {
                return Some("ubuntu".to_string());
            } else if content.to_lowercase().contains("id=debian") {
                return Some("debian".to_string());
            }
        }
        // Fallback: check file existence
        if std::path::Path::new(LEGACY_APT_SOURCES).exists() {
            // Maybe try to guess from content?
            if let Ok(c) = std::fs::read_to_string(LEGACY_APT_SOURCES) {
                if c.contains("ubuntu") {
                    return Some("ubuntu".to_string());
                }
                if c.contains("debian") {
                    return Some("debian".to_string());
                }
            }
        }

        // Default to ubuntu if unknown, or maybe none?
        // Returning None might be safer, but let's default to ubuntu for now as it's common.
        None
    }
}

#[async_trait]
impl SourceManager for AptManager {
    fn name(&self) -> &'static str {
        "apt"
    }

    fn requires_sudo(&self) -> bool {
        true
    }

    async fn is_installed(&self) -> bool {
        utils::command_exists("apt")
            || utils::command_exists("apt-get")
            || fs::try_exists(self.config_path()).await.unwrap_or(false)
    }

    fn list_candidates(&self) -> Vec<Mirror> {
        let key = format!("apt-{}", self.distro);
        config::get_candidates(&key)
    }

    fn config_path(&self) -> PathBuf {
        if let Some(ref path) = self.custom_path {
            return path.clone();
        }

        // Ubuntu 24.04+ 常把官方源放在 Deb822 格式的 ubuntu.sources 中。
        // 检测到新版文件时优先使用，否则保持传统 sources.list 行为。
        if std::path::Path::new(UBUNTU_DEB822_SOURCES).exists() {
            PathBuf::from(UBUNTU_DEB822_SOURCES)
        } else {
            PathBuf::from(LEGACY_APT_SOURCES)
        }
    }

    async fn current_url(&self) -> Result<Option<String>> {
        let path = self.config_path();
        if !fs::try_exists(&path).await.unwrap_or(false) {
            return Ok(None);
        }

        let content = fs::read_to_string(&path).await?;

        if let Some(url) = Self::current_deb822_url(&content)? {
            return Ok(Some(url));
        }

        Self::current_legacy_url(&content)
    }

    async fn set_source(&self, mirror: &Mirror) -> Result<()> {
        let path = self.config_path();
        if !fs::try_exists(&path).await.unwrap_or(false) {
            return Err(MirrorError::Custom(format!(
                "Config file not found: {:?}",
                path
            )));
        }

        let content = fs::read_to_string(&path).await?;
        utils::backup_file(&path).await?;

        let target_url = if mirror.url.ends_with('/') {
            mirror.url.clone()
        } else {
            format!("{}/", mirror.url)
        };

        let new_content = if Self::is_deb822_sources(&content) {
            Self::replace_deb822_uris(&content, &target_url)?
        } else {
            self.replace_legacy_sources(&content, &target_url).await?
        };

        fs::write(&path, new_content).await?;
        Ok(())
    }

    async fn restore(&self) -> Result<()> {
        utils::restore_latest_backup(&self.config_path()).await
    }
}

impl AptManager {
    fn is_deb822_sources(content: &str) -> bool {
        content
            .lines()
            .any(|line| line.trim_start().to_ascii_lowercase().starts_with("uris:"))
    }

    fn current_deb822_url(content: &str) -> Result<Option<String>> {
        let re = Regex::new(r"(?m)^URIs:\s+(?P<url>https?://\S+)")?;

        if let Some(caps) = re.captures(content) {
            Ok(Some(caps["url"].to_string()))
        } else {
            Ok(None)
        }
    }

    fn current_legacy_url(content: &str) -> Result<Option<String>> {
        // 查找第一条启用的 deb 源行。
        // 格式：deb [可选参数] http://... suite component...
        let re = Regex::new(r"(?m)^deb\s+(?:\[.*?\]\s+)?(?P<url>https?://\S+)\s+")?;

        if let Some(caps) = re.captures(content) {
            Ok(Some(caps["url"].to_string()))
        } else {
            Ok(None)
        }
    }

    fn replace_deb822_uris(content: &str, target_url: &str) -> Result<String> {
        if let Some(cur_url) = Self::current_deb822_url(content)? {
            Ok(content.replace(&cur_url, target_url))
        } else {
            Ok(content.to_string())
        }
    }

    async fn replace_legacy_sources(&self, content: &str, target_url: &str) -> Result<String> {
        // 传统 sources.list 格式保留 suite/component，仅替换仓库 URL。
        let current = Self::current_legacy_url(content)?;
        let new_content = if let Some(cur_url) = current {
            // 直接替换已检测到的当前源地址，避免正则转义带来的边界问题。
            content.replace(&cur_url, &target_url)
        } else {
            // 未检测到当前源时，只尝试替换发行版默认域名。
            let default_domains = if self.distro == "ubuntu" {
                vec!["archive.ubuntu.com/ubuntu/", "security.ubuntu.com/ubuntu/"]
            } else {
                vec!["deb.debian.org/debian/", "security.debian.org/debian/"]
            };

            let mut modified = content.to_string();
            for domain in default_domains {
                // 同时处理 HTTP 和 HTTPS 写法。
                modified = modified.replace(&format!("http://{}", domain), target_url);
                modified = modified.replace(&format!("https://{}", domain), target_url);
            }
            modified
        };

        Ok(new_content)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_apt_flow() -> Result<()> {
        let dir = tempdir()?;
        let config_path = dir.path().join("sources.list");

        // 准备传统 sources.list 测试内容。
        let initial_content = r#"
# Main repo
deb http://archive.ubuntu.com/ubuntu/ jammy main restricted
deb http://archive.ubuntu.com/ubuntu/ jammy-updates main restricted
# Security
deb http://security.ubuntu.com/ubuntu/ jammy-security main restricted
        "#;
        fs::write(&config_path, initial_content).await?;

        let manager = AptManager::with_distro_and_path("ubuntu".to_string(), config_path.clone());

        // current_url 会返回第一条启用源。
        assert_eq!(
            manager.current_url().await?,
            Some("http://archive.ubuntu.com/ubuntu/".to_string())
        );

        let mirror = Mirror {
            name: "TestApt".to_string(),
            url: "http://mirrors.test.com/ubuntu/".to_string(),
        };
        manager.set_source(&mirror).await?;

        let new_content = fs::read_to_string(&config_path).await?;
        assert!(new_content.contains("deb http://mirrors.test.com/ubuntu/ jammy main"));
        // 传统格式只替换检测到的主源，security 源保持原样。
        assert!(new_content.contains("deb http://security.ubuntu.com/ubuntu/ jammy-security"));

        manager.restore().await?;
        let restored_content = fs::read_to_string(&config_path).await?;
        assert_eq!(restored_content, initial_content);

        Ok(())
    }

    #[tokio::test]
    async fn test_apt_deb822_ubuntu_sources_flow() -> Result<()> {
        let dir = tempdir()?;
        let config_path = dir.path().join("ubuntu.sources");

        let initial_content = r#"Types: deb
URIs: http://archive.ubuntu.com/ubuntu/
Suites: noble noble-updates noble-backports
Components: main restricted universe multiverse
Signed-By: /usr/share/keyrings/ubuntu-archive-keyring.gpg

Types: deb
URIs: http://security.ubuntu.com/ubuntu/
Suites: noble-security
Components: main restricted universe multiverse
Signed-By: /usr/share/keyrings/ubuntu-archive-keyring.gpg
"#;
        fs::write(&config_path, initial_content).await?;

        let manager = AptManager::with_distro_and_path("ubuntu".to_string(), config_path.clone());

        assert_eq!(
            manager.current_url().await?,
            Some("http://archive.ubuntu.com/ubuntu/".to_string())
        );

        let mirror = Mirror {
            name: "TestAptDeb822".to_string(),
            url: "http://mirrors.test.com/ubuntu/".to_string(),
        };
        manager.set_source(&mirror).await?;

        let new_content = fs::read_to_string(&config_path).await?;
        assert!(new_content.contains("URIs: http://mirrors.test.com/ubuntu/"));
        assert_eq!(
            new_content
                .matches("URIs: http://mirrors.test.com/ubuntu/")
                .count(),
            1
        );
        assert!(new_content.contains("URIs: http://security.ubuntu.com/ubuntu/"));
        assert!(new_content.contains("Suites: noble noble-updates noble-backports"));
        assert!(new_content.contains("Signed-By: /usr/share/keyrings/ubuntu-archive-keyring.gpg"));

        manager.restore().await?;
        let restored_content = fs::read_to_string(&config_path).await?;
        assert_eq!(restored_content, initial_content);

        Ok(())
    }
}
