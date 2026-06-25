use crate::config;
use crate::error::Result;
use crate::traits::SourceManager;
use crate::types::Mirror;
use crate::utils;
use async_trait::async_trait;
use std::path::PathBuf;

pub struct HuggingFaceManager;

impl HuggingFaceManager {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl SourceManager for HuggingFaceManager {
    fn name(&self) -> &'static str {
        "huggingface"
    }

    fn requires_sudo(&self) -> bool {
        false
    }

    async fn is_installed(&self) -> bool {
        utils::command_exists("hf")
            || utils::command_exists("huggingface-cli")
            || std::env::var("HF_ENDPOINT").is_ok()
    }

    fn list_candidates(&self) -> Vec<Mirror> {
        config::get_candidates("huggingface")
    }

    fn config_path(&self) -> PathBuf {
        // Hugging Face clients read this from the shell environment.
        PathBuf::from("env:HF_ENDPOINT")
    }

    async fn current_url(&self) -> Result<Option<String>> {
        match std::env::var("HF_ENDPOINT") {
            Ok(val) if !val.is_empty() => Ok(Some(val)),
            _ => Ok(None),
        }
    }

    async fn set_source(&self, mirror: &Mirror) -> Result<()> {
        println!("To apply this mirror, please run the following command in your terminal:");
        println!();
        println!("    export HF_ENDPOINT=\"{}\"", mirror.url);
        println!();
        println!("To make it permanent, add the line above to your ~/.zshrc or ~/.bashrc.");
        Ok(())
    }

    async fn restore(&self) -> Result<()> {
        println!("To restore Hugging Face configuration, please unset the environment variable:");
        println!();
        println!("    unset HF_ENDPOINT");
        println!();
        println!("If you added it to your shell profile, please remove that line manually.");
        Ok(())
    }
}
