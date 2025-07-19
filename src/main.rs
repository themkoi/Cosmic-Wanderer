use freedesktop_desktop_entry::default_paths;
use log::{debug, error};
use notify::event::{CreateKind, EventKind, ModifyKind, RemoveKind};
use notify::{RecursiveMode, Watcher, recommended_watcher};
use notify_rust::Notification;
use shlex::Shlex;
use slint::{Image, Model, ModelRc, SharedString, VecModel, set_xdg_app_id};
use std::{
    error::Error,
    fs,
    os::unix::{net::UnixListener, process::CommandExt},
    process::{Command, Stdio},
    rc::Rc,
    sync::{Arc, Condvar, Mutex, mpsc::channel},
    thread::{self},
    time::Instant,
};
mod entries;
use entries::DesktopEntryManager;
use entries::NormalDesktopEntry;

mod history;
use crate::history::*;

mod config;
use config::{config_color_to_slint, load_or_create_config};

slint::include_modules!();

fn create_slint_items(normalized_entries: &[NormalDesktopEntry]) -> VecModel<AppItem> {
    let model = VecModel::default();
    for entry in normalized_entries {
        let icon = Image::load_from_path(entry.icon.as_ref()).unwrap_or_default();
        model.push(AppItem {
            app_name: entry.app_name.clone().into(),
            app_id: entry.appid.clone().into(),
            exec: entry.exec.clone().into(),
            comment: entry.comment.clone().into(),
            icon,
        });
    }
    model
}

fn theme_from_config(theme: &config::ThemeConfig) -> ThemeSlint {
    ThemeSlint {
        window_background: config_color_to_slint(&theme.window_background),
        selected_item_background: config_color_to_slint(&theme.selected_item_background),
        selected_text_color: config_color_to_slint(&theme.selected_text_color),
        unselected_text_color: config_color_to_slint(&theme.unselected_text_color),
        item_height: theme.item_height as f32,
        item_spacing: theme.item_spacing as f32,
        item_border_radius: theme.item_border_radius as f32,
        icon_size: theme.icon_size as f32,
        input_font_size: theme.input_font_size as f32,
        input_border_width: theme.input_border_width as f32,
        text_font_size: theme.text_font_size as f32,
        comment_font_size: theme.comment_font_size as f32,
        font_family: theme.font_family.clone().into(),
        font_weight: theme.font_weight,
        window_width: theme.window_width as f32,
        window_height: theme.window_height as f32,
        window_border_width: theme.window_border_width as f32,
        input_height: theme.input_height as f32,
        animation_time: theme.animation_duration as i64,
    }
}

pub fn send_notification(message: &str) {
    if let Err(e) = Notification::new()
        .summary("Cosmic wanderer")
        .body(message)
        .show()
    {
        error!("Failed to show notification: {}", e);
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let config = load_or_create_config()?;
    env_logger::init_from_env(
        env_logger::Env::default().filter_or(env_logger::DEFAULT_FILTER_ENV, "off"),
    );

    let start = Instant::now();
    let mut manager = DesktopEntryManager::new();

    let normalized_entries = Arc::new(Mutex::new(
        manager.get_normalized_entries(&config.general.icon_theme.clone()),
    ));

    // Setup watcher channel and watcher
    let (tx, rx) = channel();

    let mut watcher = recommended_watcher(tx)?;

    for path in default_paths() {
        if path.exists() {
            watcher.watch(&path, RecursiveMode::Recursive)?;
        }
    }
    let normalized_entries_watcher_cloned = normalized_entries.clone();
    let icon_theme = config.general.icon_theme.clone();
    thread::spawn(move || {
        for res in rx {
            match res {
                Ok(event) => match &event.kind {
                    EventKind::Modify(ModifyKind::Data(_))
                    | EventKind::Create(CreateKind::Any)
                    | EventKind::Remove(RemoveKind::Any) => {
                        debug!("Entry changed");
                        manager.refresh();
                        let mut entries = normalized_entries_watcher_cloned.lock().unwrap();
                        *entries = manager.get_normalized_entries(&icon_theme);
                    }
                    _ => {}
                },
                Err(e) => error!("Watch error: {:?}", e),
            }
        }
    });

    let _ = set_xdg_app_id("cosmic-wanderer");
    let ui = AppWindow::new()?;
    let ui_weak = ui.as_weak();

    let theme = theme_from_config(&config.theme);
    ui.set_theme(theme);
    let slint_items = {
        let locked_entries = normalized_entries.lock().unwrap();
        create_slint_items(&*locked_entries)
    };
    ui.set_appItems(ModelRc::new(Rc::new(slint_items)));
    let park = Arc::new(Mutex::new(true));
    let ui_weak_focus = ui.as_weak();
    let park_thread = park.clone();

    let pair = Arc::new((Mutex::new(false), Condvar::new()));
    let pair_clone = pair.clone();

    let focus_thread = thread::spawn(move || {
        loop {
            let ui_for_closure = ui_weak_focus.clone();
            let park_inner = park_thread.clone();

            slint::invoke_from_event_loop(move || {
                if let Some(ui) = ui_for_closure.upgrade() {
                    if !ui.get_scopeFocused() {
                        if !ui.invoke_readFocus() {
                            let mut p = park_inner.lock().unwrap();
                            *p = true;
                        }
                    }
                }
            })
            .unwrap();

            if *park_thread.lock().unwrap() {
                let (lock, cvar) = &*pair_clone;
                let ui_for_closure = ui_weak_focus.clone();
                slint::invoke_from_event_loop(move || {
                    if let Some(ui) = ui_for_closure.upgrade() {
                        ui.invoke_text_entered(SharedString::from("nothing"));
                    }
                })
                .unwrap();
                *lock.lock().unwrap() = false;
                cvar.notify_one();
                debug!("Parking thread.");
                thread::park();
                debug!("Thread unparked");
                *park_thread.lock().unwrap() = false;
                let (lock, cvar) = &*pair_clone;
                *lock.lock().unwrap() = true;
                cvar.notify_one();
                debug!("notified");
            }

            std::thread::sleep(std::time::Duration::from_millis(50));
        }
    });
    let ui_weak_clone_text = ui.as_weak();
    let normalized_entries_cloned = normalized_entries.clone();
    ui.on_text_entered(move |text| {
        let locked_entries = normalized_entries_cloned.lock().unwrap();
        let sorted_entries = if !text.is_empty() {
            DesktopEntryManager::filter_and_sort_entries(&text, &locked_entries)
        } else {
            let history = load_history();
            sorted_entries_by_usage(&locked_entries, &history)
        };

        let vec_model = create_slint_items(&sorted_entries); // Convert to VecModel<AppItem>

        if let Some(ui) = ui_weak_clone_text.upgrade() {
            ui.set_appItems(ModelRc::new(Rc::new(vec_model)));

            ui.set_selected_index(0);
            ui.invoke_set_scroll(0.0);
        }
        drop(sorted_entries);
    });

    let ui_weak_clone_item = ui.as_weak();
    let park_clicked = park.clone();
    ui.on_item_clicked(move |idx| {
        let idx = idx as usize;

        if let Some(ui) = ui_weak_clone_item.upgrade() {
            let entries = ui.get_appItems();
            if let Some(entry) = entries.row_data(idx) {
                let mut history = load_history();

                increment_usage(&mut history, &entry.app_id);
                save_history(&history);
                drop(history);

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
                            libc::setsid();
                            Ok(())
                        });
                    }

                    debug!("launched command: {:?}", command);
                    debug!("With envs: {:#?}", std::env::vars());
                    command.envs(std::env::vars());

                    if let Err(e) = command.spawn() {
                        let msg = format!("Failed to spawn detached process: {}", e);
                        error!("{}", msg);
                        send_notification(&msg);
                    }
                }
                drop(entries);
                let park_inner = park_clicked.clone();
                if let Some(ui) = ui_weak_clone_item.upgrade() {
                    ui.hide().unwrap();
                    let mut p = park_inner.lock().unwrap();
                    *p = true;
                }
            }
        }
    });

    let park_focus = park.clone();
    let ui_weak_focus = ui_weak.clone();
    ui.on_focus_changed(move |focused| {
        let park_inner: Arc<Mutex<bool>> = park_focus.clone();
        if !focused {
            if let Some(ui) = ui_weak_focus.upgrade() {
                ui.hide().unwrap();
                let mut p = park_inner.lock().unwrap();
                *p = true;
            }
        }
    });

    let pair_for_timer = pair.clone();

    let timer = slint::Timer::default();
    timer.start(
        slint::TimerMode::Repeated,
        std::time::Duration::from_secs(1),
        move || {
            let (lock, cvar) = &*pair_for_timer;
            let mut ready = lock.try_lock().unwrap();
            while !*ready {
                debug!("blocking Ui");
                ready = cvar.wait(ready).unwrap();
                debug!("waking Ui");
                *ready = true;
            }
        },
    );

    ui.invoke_focusText();
    debug!("Time taken: {:?}", start.elapsed());
    let _ = fs::remove_file(&config.general.socket_path); // Clean up old socket

    let park_listener = park.clone();
    let listener = UnixListener::bind(&config.general.socket_path).expect("Failed to bind socket");
    let ui_weak_clone = ui_weak.clone();
    thread::spawn(move || {
        while let Ok((_stream, _addr)) = listener.accept() {
            focus_thread.thread().unpark();

            let ui_for_closure = ui_weak_clone.clone(); // Clone it for the closure
            let mut p = park_listener.lock().unwrap();
            *p = false;
            slint::invoke_from_event_loop(move || {
                if let Some(ui) = ui_for_closure.upgrade() {
                    ui.set_text_input(SharedString::from(""));
                    ui.invoke_text_entered(SharedString::from(""));
                    ui.invoke_focusText();
                    ui.set_selected_index(0);
                    ui.invoke_set_scroll(0.0);
                    ui.show().unwrap();
                }
            })
            .unwrap();
        }
    });

    drop(config);

    slint::run_event_loop_until_quit().unwrap();
    Ok(())
}
