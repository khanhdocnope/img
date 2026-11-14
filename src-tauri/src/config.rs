use crate::models::{Account, BotSettings, CategoryConfig, Config, EmojiSenderSettings, ProgressMap};
use parking_lot::RwLock;
use std::fs;
use std::path::{Path, PathBuf};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// Thống kê trọn đời, lưu riêng để không mất khi đặt lại tiến trình quét
#[derive(Default, Serialize, Deserialize)]
struct Stats {
    #[serde(default)]
    total_reactions: u64,
}

#[derive(Clone)]
pub struct ConfigManager {
    base_dir: PathBuf,
    config: Arc<RwLock<Config>>,
    progress: Arc<RwLock<ProgressMap>>,
    total_reactions: Arc<AtomicU64>,
}

impl ConfigManager {
    pub fn new<P: AsRef<Path>>(base_dir: P) -> Self {
        let base_dir = base_dir.as_ref().to_path_buf();
        let _ = fs::create_dir_all(&base_dir);

        let config_path = base_dir.join("config.json");
        let initial_config = Self::load_config_from_file(&config_path);

        let progress_path = base_dir.join("progress.json");
        let initial_progress = Self::load_progress_from_file(&progress_path);

        let stats_path = base_dir.join("stats.json");
        let initial_total = fs::read_to_string(&stats_path)
            .ok()
            .and_then(|c| serde_json::from_str::<Stats>(&c).ok())
            .map(|st| st.total_reactions)
            // Lần đầu: lấy tạm tổng từ tiến trình đã quét trước đây để không bắt đầu từ 0
            .unwrap_or_else(|| initial_progress.values().map(|c| c.total_reacted).sum());

        Self {
            base_dir,
            config: Arc::new(RwLock::new(initial_config)),
            progress: Arc::new(RwLock::new(initial_progress)),
            total_reactions: Arc::new(AtomicU64::new(initial_total)),
        }
    }

    fn load_config_from_file(path: &Path) -> Config {
        if path.exists() {
            if let Ok(content) = fs::read_to_string(path) {
                if let Ok(parsed) = serde_json::from_str::<Config>(&content) {
                    return parsed;
                }
            }
        }
        let default_config = Config::default();
        let _ = fs::write(path, serde_json::to_string_pretty(&default_config).unwrap_or_default());
        default_config
    }

    fn load_progress_from_file(path: &Path) -> ProgressMap {
        if path.exists() {
            if let Ok(content) = fs::read_to_string(path) {
                if let Ok(parsed) = serde_json::from_str::<ProgressMap>(&content) {
                    return parsed;
                }
            }
        }
        ProgressMap::new()
    }

    pub fn save_config(&self) -> Result<(), String> {
        let config_path = self.base_dir.join("config.json");
        let data = self.config.read();
        let json_str = serde_json::to_string_pretty(&*data)
            .map_err(|e| format!("Lỗi parse JSON config: {}", e))?;
        fs::write(&config_path, json_str)
            .map_err(|e| format!("Lỗi ghi file config.json: {}", e))?;
        Ok(())
    }

    pub fn get_config(&self) -> Config {
        self.config.read().clone()
    }

    pub fn get_active_account(&self) -> Option<Account> {
        let cfg = self.config.read();
        let active_id = cfg.active_account_id.as_deref()?;
        cfg.accounts.iter().find(|a| a.id == active_id).cloned()
    }

    pub fn set_active_account(&self, account_id: String) -> Result<(), String> {
        {
            let mut cfg = self.config.write();
            cfg.active_account_id = Some(account_id);
        }
        self.save_config()
    }

    pub fn add_or_update_account(&self, name: String, token: String, account_id: Option<String>) -> Result<String, String> {
        let final_id = {
            let mut cfg = self.config.write();
            let id = account_id.unwrap_or_else(|| format!("acc_{}", &uuid::Uuid::new_v4().to_string()[..8]));
            if let Some(existing) = cfg.accounts.iter_mut().find(|a| a.id == id) {
                existing.name = name;
                existing.token = token;
            } else {
                cfg.accounts.push(Account {
                    id: id.clone(),
                    name,
                    token,
                });
            }
            if cfg.active_account_id.is_none() {
                cfg.active_account_id = Some(id.clone());
            }
            id
        };
        self.save_config()?;
        Ok(final_id)
    }

    pub fn delete_account(&self, account_id: &str) -> Result<(), String> {
        {
            let mut cfg = self.config.write();
            cfg.accounts.retain(|a| a.id != account_id);
            if cfg.active_account_id.as_deref() == Some(account_id) {
                cfg.active_account_id = cfg.accounts.first().map(|a| a.id.clone());
            }
        }
        self.save_config()
    }

    pub fn update_bot_settings(&self, settings: BotSettings) -> Result<(), String> {
        {
            let mut cfg = self.config.write();
            cfg.bot_settings = settings;
        }
        self.save_config()
    }

    pub fn update_categories(&self, categories: Vec<CategoryConfig>) -> Result<(), String> {
        {
            let mut cfg = self.config.write();
            cfg.categories = categories;
        }
        self.save_config()
    }

    pub fn update_emoji_filters(&self, whitelist: Vec<String>, blacklist: Vec<String>) -> Result<(), String> {
        {
            let mut cfg = self.config.write();
            cfg.emoji_whitelist = whitelist;
            cfg.emoji_blacklist = blacklist;
        }
        self.save_config()
    }

    /// Cộng thêm 1 reaction vào tổng trọn đời (chỉ trong bộ nhớ, ghi file ở save_stats)
    pub fn count_reaction(&self) {
        self.total_reactions.fetch_add(1, Ordering::SeqCst);
    }

    pub fn get_total_reactions(&self) -> u64 {
        self.total_reactions.load(Ordering::SeqCst)
    }

    pub fn save_stats(&self) {
        let stats = Stats {
            total_reactions: self.get_total_reactions(),
        };
        if let Ok(json) = serde_json::to_string_pretty(&stats) {
            let _ = fs::write(self.base_dir.join("stats.json"), json);
        }
    }

    pub fn update_emoji_sender(&self, settings: EmojiSenderSettings) -> Result<(), String> {
        {
            let mut cfg = self.config.write();
            cfg.emoji_sender = settings;
        }
        self.save_config()
    }

    // --- Progress ---
    pub fn get_progress(&self) -> ProgressMap {
        self.progress.read().clone()
    }

    pub fn save_progress(&self, progress: ProgressMap) -> Result<(), String> {
        {
            let mut p = self.progress.write();
            *p = progress.clone();
        }
        let progress_path = self.base_dir.join("progress.json");
        let json_str = serde_json::to_string_pretty(&progress)
            .map_err(|e| format!("Lỗi parse progress JSON: {}", e))?;
        fs::write(&progress_path, json_str)
            .map_err(|e| format!("Lỗi ghi file progress.json: {}", e))?;
        Ok(())
    }

    pub fn reset_progress(&self) -> Result<(), String> {
        {
            let mut p = self.progress.write();
            p.clear();
        }
        let progress_path = self.base_dir.join("progress.json");
        let _ = fs::remove_file(progress_path);
        Ok(())
    }
}
