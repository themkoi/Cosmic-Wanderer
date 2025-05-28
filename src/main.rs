use shlex::Shlex;
use slint::{ModelRc, set_xdg_app_id};
use std::{
    error::Error,
    process::Command,
    rc::Rc,
    sync::{Mutex, OnceLock},
    thread, time,
    time::Instant,
    cmp::Ordering,
};
mod entries;
use entries::DesktopEntryManager;
use entries::NormalDesktopEntry;
use fuzzy_matcher::FuzzyMatcher;
use fuzzy_matcher::skim::{SkimMatcherV2,SkimScoreConfig};
use slint::Image;
use termion::style::{Invert, Reset};

slint::include_modules!();

// Global storage for desktop entries
static ENTRIES: OnceLock<Mutex<Vec<NormalDesktopEntry>>> = OnceLock::new();

fn get_entries() -> &'static Mutex<Vec<NormalDesktopEntry>> {
    ENTRIES.get_or_init(|| Mutex::new(Vec::new()))
}

fn create_slint_items(normalized_entries: &[NormalDesktopEntry]) -> slint::VecModel<AppItem> {
    slint::VecModel::from(
        normalized_entries
            .iter()
            .map(|entry| AppItem {
                appid: entry.appid.clone().into(),
                comment: entry.comment.clone().into(),
                icon: Image::load_from_path(entry.icon.as_ref())
                    .unwrap_or_else(|_| Image::default()),
            })
            .collect::<Vec<_>>(),
    )
}

type IndexType = usize;

fn wrap_matches(line: &str, indices: &[IndexType]) -> String {
    let mut ret = String::new();
    let mut peekable = indices.iter().peekable();
    for (idx, ch) in line.chars().enumerate() {
        let next_id = **peekable.peek().unwrap_or(&&(line.len() as IndexType));
        if next_id == (idx as IndexType) {
            ret.push_str(format!("{}{}{}", Invert, ch, Reset).as_str());
            peekable.next();
        } else {
            ret.push(ch);
        }
    }
    ret
}

fn main() -> Result<(), Box<dyn Error>> {
    let start = Instant::now();
    let manager = DesktopEntryManager::new();

    let normalized_entries: Vec<NormalDesktopEntry> = manager.get_normalized_entries();
    let cloned_iter: std::iter::Cloned<std::slice::Iter<'_, NormalDesktopEntry>> =
        normalized_entries.iter().cloned();
    let search_entries: Vec<NormalDesktopEntry> = cloned_iter.collect();

    {
        let entries = search_entries;
        *get_entries().lock().unwrap() = entries;
    }

    let _ = set_xdg_app_id("Cosmic Wanderer");
    let ui = AppWindow::new()?;
    let ui_weak = ui.as_weak();

    // Create slint items from global entries
    let slint_items = {
        let entries = get_entries().lock().unwrap();
        create_slint_items(&entries)
    };

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

    let matcher = SkimMatcherV2::default();
    let ui_weak_clone_text = ui_weak.clone();
ui.on_text_entered(move |text| {
    // 1. Collect matches with scores and indices
    let mut matched_entries: Vec<(i64, Vec<usize>, NormalDesktopEntry)> = normalized_entries
        .iter()
        .filter_map(|entry| {
            matcher.fuzzy_indices(&entry.appid, &text)
                .map(|(score, indices)| (score, indices, entry.clone()))
        })
        .collect();

    // 2. Sort by score (descending), then by name length (ascending)
    matched_entries.sort_by(|a, b| {
        // First, compare scores (higher scores come first)
        let score_cmp = b.0.cmp(&a.0);
        if score_cmp == Ordering::Equal {
            // If scores are equal, compare by name length (shorter names come first)
            a.2.appid.len().cmp(&b.2.appid.len())
        } else {
            score_cmp
        }
    });

    // 3. DEBUG: Print after sorting
    println!("After sorting (score + name length):");
    for (score, _, entry) in &matched_entries {
        println!("Score: {}, Length: {}, App ID: {}", score, entry.appid.len(), entry.appid);
    }

    // 4. Extract sorted entries
    let sorted_entries: Vec<NormalDesktopEntry> = matched_entries
        .into_iter()
        .map(|(_, _, entry)| entry)
        .collect();

    // 5. Update shared state
    *get_entries().lock().unwrap() = sorted_entries;

    // 6. Update UI
    if let Some(ui) = ui_weak_clone_text.upgrade() {
        let model = create_slint_items(&get_entries().lock().unwrap());
        ui.set_appItems(ModelRc::new(Rc::new(model)));
    }
});

    let ui_weak_clone_item = ui_weak.clone();
    ui.on_item_clicked(move |idx| {
        let idx = idx as usize;
        let entries = get_entries().lock().unwrap();
        let entry = &entries[idx];

        let command_string = entry
            .exec
            .replace("%U", "")
            .replace("%F", "")
            .replace("%u", "")
            .replace("%f", "");

        let command: Vec<String> = Shlex::new(&command_string).collect();

        if let Some((cmd, args)) = command.split_first() {
            let _ = Command::new(cmd)
                .args(args)
                .spawn()
                .expect("Failed to launch app");
        }

        if let Some(ui) = ui_weak_clone_item.upgrade() {
            ui.hide().unwrap();
        }
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
    println!("Time taken: {:?}", start.elapsed());
    ui.run()?;
    Ok(())
}
