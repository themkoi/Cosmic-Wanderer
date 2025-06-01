use freedesktop_desktop_entry::{DesktopEntry, Iter, default_paths, get_languages_from_env};
use freedesktop_icons::lookup;
use fuzzy_matcher::FuzzyMatcher;
use fuzzy_matcher::skim::SkimMatcherV2;
use std::cmp::Ordering;

#[derive(Clone)]
pub struct NormalDesktopEntry {
    pub app_name: String,
    pub comment: String,
    pub appid: String,
    pub categories: Option<Vec<String>>,
    pub exec: String,
    pub icon: String,
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
            let name = match entry.name(&self.locales) {
                Some(name) => name.to_string(),
                None => continue,
            };

            let normalized_name = normalize_name(&name);

            let mut replace = false;
            let mut index_opt: Option<usize> = None;

            if entry.no_display() {
                continue;
            }

            if seen_names
                .iter()
                .any(|n| normalize_name(n) == normalized_name)
            {
                index_opt = entries
                    .iter()
                    .position(|e: &NormalDesktopEntry| normalize_name(&e.app_name) == normalized_name);

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

            let icon_name = entry.icon().unwrap_or_default().to_string();

            let mut icon = lookup(&icon_name)
                .with_cache()
                .with_theme("Papirus-Dark")
                .find();

            let mut icon_path = icon.unwrap_or_default().to_string_lossy().to_string();

            if icon_path.is_empty() {
                icon = lookup("application-x-executable")
                    .with_cache()
                    .with_theme("Papirus-Dark")
                    .find();
                icon_path = icon.unwrap_or_default().to_string_lossy().to_string();
            }

            // Get required fields with fallbacks
            let app_name = entry.name(&self.locales).unwrap_or_default().to_string();
            let exec = entry.exec().unwrap_or_default().to_string();
            let icon = icon_path;
            let categories = entry
                .categories()
                .map(|v| v.iter().map(|s| s.to_string()).collect::<Vec<String>>());
            let comment = entry.comment(&self.locales).unwrap_or_default().to_string();
            let appid= entry.appid.clone();

            // Create the normalized entry
            let nde = NormalDesktopEntry {
                app_name,
                exec,
                icon,
                categories,
                comment,
                appid,
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

    pub fn filter_and_sort_entries(
        text: &str,
        normalized_entries: &[NormalDesktopEntry],
    ) -> Vec<NormalDesktopEntry> {
        let matcher = SkimMatcherV2::default();
        let mut matched_entries: Vec<(i64, Vec<usize>, NormalDesktopEntry)> = normalized_entries
            .iter()
            .filter_map(|entry| {
                matcher
                    .fuzzy_indices(&entry.app_name, text)
                    .map(|(score, indices)| (score, indices, entry.clone()))
            })
            .collect();

        matched_entries.sort_by(|a, b| {
            let score_cmp = b.0.cmp(&a.0);
            if score_cmp == Ordering::Equal {
                a.2.app_name.len().cmp(&b.2.app_name.len())
            } else {
                score_cmp
            }
        });

        matched_entries
            .into_iter()
            .map(|(_, _, entry)| entry)
            .collect()
    }
}
