use shlex::Shlex;
use slint::{Image, ModelRc, SharedString, Timer, TimerMode, set_xdg_app_id};
use std::{
    cmp::Ordering,
    error::Error,
    fs,
    io::{BufRead, BufReader},
    os::unix::net::UnixListener,
    process::Command,
    rc::Rc,
    sync::{Mutex, OnceLock},
    thread, time,
    time::Instant,
};
mod entries;
use entries::DesktopEntryManager;
use entries::NormalDesktopEntry;
use fuzzy_matcher::FuzzyMatcher;
use fuzzy_matcher::skim::SkimMatcherV2;

static SOCKET_PATH: &str = "/tmp/comsic-wanderer.sock";

slint::include_modules!();

// Global storage for desktop entries
static ENTRIES: OnceLock<Mutex<Vec<NormalDesktopEntry>>> = OnceLock::new();

fn get_entries() -> &'static Mutex<Vec<NormalDesktopEntry>> {
    ENTRIES.get_or_init(|| Mutex::new(Vec::new()))
}

fn filter_and_sort_entries(
    text: &str,
    normalized_entries: &[NormalDesktopEntry],
    matcher: &SkimMatcherV2,
) -> Vec<NormalDesktopEntry> {
    let mut matched_entries: Vec<(i64, Vec<usize>, NormalDesktopEntry)> = normalized_entries
        .iter()
        .filter_map(|entry| {
            matcher
                .fuzzy_indices(&entry.appid, text)
                .map(|(score, indices)| (score, indices, entry.clone()))
        })
        .collect();

    matched_entries.sort_by(|a, b| {
        let score_cmp = b.0.cmp(&a.0);
        if score_cmp == Ordering::Equal {
            a.2.appid.len().cmp(&b.2.appid.len())
        } else {
            score_cmp
        }
    });

    matched_entries
        .into_iter()
        .map(|(_, _, entry)| entry)
        .collect()
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

    let slint_items = {
        let entries = get_entries().lock().unwrap();
        create_slint_items(&entries)
    };

    ui.set_appItems(ModelRc::new(Rc::new(slint_items)));

    let ui_weak_clone = ui.as_weak();
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
    let ui_weak_clone_text = ui.as_weak();
    ui.on_text_entered(move |text| {
        let sorted_entries = filter_and_sort_entries(&text, &normalized_entries, &matcher);
        *get_entries().lock().unwrap() = sorted_entries;

        if let Some(ui) = ui_weak_clone_text.upgrade() {
            let model = create_slint_items(&get_entries().lock().unwrap());
            ui.set_appItems(ModelRc::new(Rc::new(model)));
        }
    });

    let ui_weak_clone_item = ui.as_weak();
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

    let ui_weak_focus = ui_weak.clone();
    ui.on_focus_changed(move |focused| {
        if !focused {
            if let Some(ui) = ui_weak_focus.upgrade() {
                ui.hide().unwrap();
            }
        }
    });

    ui.invoke_focusText();
    println!("Time taken: {:?}", start.elapsed());
    let _ = fs::remove_file(SOCKET_PATH); // Clean up old socket

    let timer_listener = Timer::default();
    let listener = UnixListener::bind(SOCKET_PATH).expect("Failed to bind socket");
    listener
        .set_nonblocking(true)
        .expect("Failed to set non-blocking");

    timer_listener.start(
        TimerMode::Repeated,
        std::time::Duration::from_millis(50),
        move || {
            for stream_result in listener.incoming() {
                match stream_result {
                    Ok(_) => {
                        let ui_weak = ui_weak.clone();
                        if let Some(ui) = ui_weak.upgrade() {
                            ui.set_text_input(SharedString::from(""));
                            ui.invoke_text_entered(SharedString::from(""));
                            ui.invoke_focusText();
                            ui.show().unwrap();
                        }
                    }
                    Err(_) => break,
                }
            }
        },
    );

    slint::run_event_loop_until_quit().unwrap();
    Ok(())
}
