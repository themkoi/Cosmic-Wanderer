use freedesktop_desktop_entry::{DesktopEntry, Iter, default_paths, get_languages_from_env};
use slint::ModelRc;
use std::{error::Error, rc::Rc, thread, time};

slint::include_modules!();

struct NormalDesktopEntry {
    pub appid: String,
    pub exec: String,
    pub icon: String,
    pub path: String,
}

struct DesktopEntryManager {
    locales: Vec<String>,
    desktop_entries: Vec<DesktopEntry>,
}

impl DesktopEntryManager {
    fn new() -> Self {
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

    fn get_normalized_entries(&self) -> Vec<NormalDesktopEntry> {
        let mut entries = Vec::new();
        let mut seen_names = std::collections::HashSet::new();

        for entry in &self.desktop_entries {
            // Skip if we've already seen this appid (deduplication)
            let name = match entry.name(&self.locales) {
                Some(name) => name.to_string(),
                None => continue, // Skip entries without names
            };

            // Skip if we've already seen this name
            if seen_names.contains(&name) {
                continue;
            }
            // Get required fields with fallbacks
            let appid = entry.name(&self.locales).unwrap_or_default().to_string();
            let path = entry.path.to_str().unwrap_or_default().to_string();
            let exec = entry.exec().unwrap_or_default().to_string();
            let icon = entry.icon().unwrap_or_default().to_string();

            // Create the normalized entry
            let nde = NormalDesktopEntry {
                appid,
                path,
                exec,
                icon,
            };

            entries.push(nde);
            seen_names.insert(name);
        }

        entries
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let manager = DesktopEntryManager::new();
    let normalized_entries = manager.get_normalized_entries();

    let ui = AppWindow::new()?;
    let ui_weak = ui.as_weak();

    let slint_items = slint::VecModel::from(
        normalized_entries
            .iter()
            .map(|entry| AppItem {
                appid: entry.appid.clone().into(),
                iconPath: entry.icon.clone().into(),
            })
            .collect::<Vec<_>>(),
    );

    ui.set_appItems(ModelRc::new(Rc::new(slint_items)));

    // Focus handling thread
    let ui_weak_clone = ui_weak.clone();
    thread::spawn(move || {
        loop {
            slint::invoke_from_event_loop({
                let ui_weak = ui_weak_clone.clone();
                move || {
                    if let Some(ui) = ui_weak.upgrade() {
                        let scope_focused: bool = ui.get_scopeFocused();
                        if !scope_focused {
                            ui.invoke_readFocus();
                        }
                    }
                }
            })
            .unwrap();
            thread::sleep(time::Duration::from_millis(500));
        }
    });

    // Text input handler
    ui.on_text_entered(move |text| {
        println!("Search: {}", text);
    });

    // Focus change handler
    ui.on_focus_changed(move |focused| {
        if !focused {
            if let Some(ui) = ui_weak.upgrade() {
                ui.hide().unwrap();
            }
        }
    });

    ui.invoke_focusText();
    ui.run()?;
    Ok(())
}
