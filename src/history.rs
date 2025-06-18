use serde::{Serialize, Deserialize};
use std::{collections::HashMap, fs, path::PathBuf};
use dirs::cache_dir;

use crate::entries::*;

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct HistoryData {
    pub usage_count: u32,
}

pub type HistoryMap = HashMap<String, HistoryData>;

pub fn get_history_file() -> PathBuf {
    let mut path = cache_dir().unwrap();
    path.push("cosmic-wanderer");
    fs::create_dir_all(&path).unwrap();
    path.push("history.toml");
    path
}

pub fn load_history() -> HistoryMap {
    let path = get_history_file();
    if let Ok(data) = fs::read_to_string(&path) {
        toml::from_str(&data).unwrap_or_default()
    } else {
        HashMap::new()
    }
}

pub fn save_history(history: &HistoryMap) {
    let toml_str = toml::to_string(history).unwrap();
    fs::write(get_history_file(), toml_str).unwrap();
}

pub fn increment_usage(history: &mut HistoryMap, appid: &str) {
    let entry = history.entry(appid.to_string()).or_default();
    entry.usage_count += 1;
}

pub fn sorted_entries_by_usage(
    entries: &[NormalDesktopEntry],
    usage: &HistoryMap,
) -> Vec<NormalDesktopEntry> {
    let mut sorted: Vec<_> = entries.to_vec(); // clones entries
    sorted.sort_by(|a, b| {
        let a_count = usage.get(&a.appid).map(|h| h.usage_count).unwrap_or(0);
        let b_count = usage.get(&b.appid).map(|h| h.usage_count).unwrap_or(0);
        b_count.cmp(&a_count)
    });
    sorted
}