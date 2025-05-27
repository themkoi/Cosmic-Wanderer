use freedesktop_desktop_entry::{
    DesktopEntry, Iter, default_paths, desktop_entries, get_languages_from_env,
};
use freedesktop_icons::lookup;

pub struct NormalDesktopEntry {
    pub appid: String,
    pub categories: Option<Vec<String>>,
    pub exec: String,
    pub icon: String,
    pub path: String,
}

pub struct DesktopEntryManager {
    locales: Vec<String>,
    desktop_entries: Vec<DesktopEntry>,
}

fn normalize_name(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric())
        .collect()
}

impl DesktopEntryManager {
    pub fn new() -> Self {
        let locales = get_languages_from_env();
        let paths = Iter::new(default_paths());
        let desktop_entries = paths
            .filter_map(|path| DesktopEntry::from_path(path, Some(&locales)).ok())
            .collect();

        Self {
            locales,
            desktop_entries,
        }
    }

    pub fn get_normalized_entries(&self) -> Vec<NormalDesktopEntry> {
        let mut entries = Vec::new();
        let mut seen_names: std::collections::HashSet<String> = std::collections::HashSet::new();

        for entry in &self.desktop_entries {
            let entry_copy = entry;
            // Skip if we've already seen this appid (deduplication)
            let name = match entry.name(&self.locales) {
                Some(name) => name.to_string(),
                None => continue, // Skip entries without names
            };

            // Skip if we've already seen this name
            let normalized_name = normalize_name(&name);

            let mut replace = false;
            let mut index_opt: Option<usize> = None;

            if entry_copy.no_display() {
                continue;
            }

            if seen_names
                .iter()
                .any(|n| normalize_name(n) == normalized_name)
            {
                index_opt = entries
                    .iter()
                    .position(|e: &NormalDesktopEntry| normalize_name(&e.appid) == normalized_name);

                if let Some(index) = index_opt {
                    if entries[index].icon.is_empty() {
                        if !entry.icon().unwrap_or_default().is_empty() {
                            replace = true;
                        } else {
                            continue;
                        }
                    } else {
                        continue;
                    }
                }
            }

            let icon_name = entry_copy.icon().unwrap_or_default().to_string();

            let icon = lookup(&icon_name)
                .with_cache()
                .with_theme("Papirus-Dark")
                .find()
                .or_else(|| {
                    entry_copy
                        .name(&self.locales)
                        .unwrap_or_default()
                        .to_string();
                    lookup(&icon_name).with_cache().find()
                });

            let icon_path = icon.unwrap_or_default().to_string_lossy().to_string();

            // Get required fields with fallbacks
            let appid = entry_copy
                .name(&self.locales)
                .unwrap_or_default()
                .to_string();
            let path = entry_copy.path.to_str().unwrap_or_default().to_string();
            let exec = entry_copy.exec().unwrap_or_default().to_string();
            let icon = icon_path;
            let categories = entry_copy
                .categories()
                .map(|v| v.iter().map(|s| s.to_string()).collect::<Vec<String>>());

            // Create the normalized entry
            let nde = NormalDesktopEntry {
                appid,
                path,
                exec,
                icon,
                categories,
            };

            if replace == true {
                if let Some(index) = index_opt {
                    println!("replacing");
                    entries[index] = nde;
                }
            } else {
                entries.push(nde);
            }
            seen_names.insert(name.clone());
        }

        entries
    }
}
