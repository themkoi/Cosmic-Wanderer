use shlex::Shlex;
use slint::{Image, ModelRc, SharedString, Timer, TimerMode, set_xdg_app_id};
use std::{
    collections::HashMap,
    error::Error,
    fs,
    os::unix::{net::UnixListener, process::CommandExt},
    process::{Command, Stdio},
    rc::Rc,
    sync::{LazyLock, Mutex},
    time::Instant,
};
mod entries;
use entries::DesktopEntryManager;
use entries::NormalDesktopEntry;

mod history;
use crate::history::*;

static SOCKET_PATH: &str = "/tmp/comsic-wanderer.sock";

slint::include_modules!();

// Global storage for desktop entries
static ENTRIES: Mutex<Vec<NormalDesktopEntry>> = Mutex::new(Vec::new());

fn get_entries() -> &'static Mutex<Vec<NormalDesktopEntry>> {
    &ENTRIES
}

static GLOBAL_HISTORY: LazyLock<Mutex<HistoryMap>> = LazyLock::new(|| Mutex::new(HashMap::new()));

fn get_history() -> &'static Mutex<HistoryMap> {
    &GLOBAL_HISTORY
}

fn create_slint_items(normalized_entries: &[NormalDesktopEntry]) -> slint::VecModel<AppItem> {
    slint::VecModel::from(
        normalized_entries
            .iter()
            .map(|entry| AppItem {
                app_name: entry.app_name.clone().into(),
                comment: entry.comment.clone().into(),
                icon: Image::load_from_path(entry.icon.as_ref())
                    .unwrap_or_else(|_| Image::default()),
            })
            .collect::<Vec<_>>(),
    )
}

fn main() -> Result<(), Box<dyn Error>> {
    env_logger::init_from_env(
        env_logger::Env::default().filter_or(env_logger::DEFAULT_FILTER_ENV, "off"),
    );

    let start = Instant::now();
    let manager = DesktopEntryManager::new();

    let loaded = load_history();
    *get_history().try_lock().unwrap() = loaded;

    let normalized_entries: Vec<NormalDesktopEntry> = manager.get_normalized_entries();
    let cloned_iter: std::iter::Cloned<std::slice::Iter<'_, NormalDesktopEntry>> =
        normalized_entries.iter().cloned();
    let search_entries: Vec<NormalDesktopEntry> = cloned_iter.collect();

    {
        let entries = search_entries;
        *get_entries().try_lock().unwrap() = entries;
    }

    let _ = set_xdg_app_id("cosmic-wanderer");
    let ui = AppWindow::new()?;
    let ui_weak = ui.as_weak();

    let slint_items = {
        let entries = get_entries().try_lock().unwrap();
        create_slint_items(&entries)
    };

    ui.set_appItems(ModelRc::new(Rc::new(slint_items)));

    let ui_weak_focus = ui.as_weak();
    let timer_focus = slint::Timer::default();
    timer_focus.start(
        slint::TimerMode::Repeated,
        std::time::Duration::from_millis(200),
        move || {
            if let Some(ui) = ui_weak_focus.upgrade() {
                if !ui.get_scopeFocused() {
                    ui.invoke_readFocus();
                }
            }
        },
    );

    let ui_weak_clone_text = ui.as_weak();
    ui.on_text_entered(move |text| {
        if !text.is_empty() {
            let sorted_entries =
                DesktopEntryManager::filter_and_sort_entries(&text, &normalized_entries);
            *get_entries().try_lock().unwrap() = sorted_entries;
        } else {
            let history = get_history().try_lock().unwrap();
            let sorted_entries = sorted_entries_by_usage(&normalized_entries, &history);
            *get_entries().try_lock().unwrap() = sorted_entries.iter().cloned().cloned().collect();
        }

        if let Some(ui) = ui_weak_clone_text.upgrade() {
            let model = create_slint_items(&get_entries().try_lock().unwrap());
            ui.set_appItems(ModelRc::new(Rc::new(model)));
        }
    });

    let ui_weak_clone_item = ui.as_weak();
    ui.on_item_clicked(move |idx| {
        let idx = idx as usize;
        let entries = get_entries().try_lock().unwrap();
        let entry = &entries[idx];

        let mut history = get_history().try_lock().unwrap();

        increment_usage(&mut *history, &entry.appid);
        save_history(&history);

        let command_string = entry
            .exec
            .replace("%U", "")
            .replace("%F", "")
            .replace("%u", "")
            .replace("%f", "");

        let command: Vec<String> = Shlex::new(&command_string).collect();

        if let Some((cmd, args)) = command.split_first() {
            let mut command = Command::new(cmd);
            command
                .args(args)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null());

            unsafe {
                command.pre_exec(|| {
                    libc::setsid(); // detach from terminal and parent process group
                    Ok(())
                });
            }

            command.spawn().expect("Failed to spawn detached process");
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
        std::time::Duration::from_millis(70),
        move || {
            while let Ok((_stream, _addr)) = listener.accept() {
                if let Some(ui) = ui_weak.clone().upgrade() {
                    ui.set_text_input(SharedString::from(""));
                    ui.invoke_text_entered(SharedString::from(""));
                    ui.invoke_focusText();
                    ui.set_selected_index(0);
                    ui.show().unwrap();
                }
            }
        },
    );

    slint::run_event_loop_until_quit().unwrap();
    Ok(())
}
