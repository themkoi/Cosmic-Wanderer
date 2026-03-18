use log::{debug, error};
use notify_rust::Notification;
use parking_lot::Mutex;
use shlex::Shlex;
use slint::{Image, Model, ModelRc, VecModel, set_xdg_app_id};
use slint::{Rgba8Pixel, SharedPixelBuffer};
use std::fs::File;
use std::path::PathBuf;
use std::{fs, vec};
use std::{
    error::Error,
    io::Read,
    os::unix::{net::UnixStream, process::CommandExt},
    process::{Command, Stdio},
    rc::Rc,
    sync::Arc,
    thread,
};

use dirs::cache_dir;

use fuzzy_matcher::FuzzyMatcher;
use fuzzy_matcher::skim::SkimMatcherV2;
use std::cmp::Ordering;

mod history;
use crate::history::*;

mod config;
use config::config_color_to_slint;

slint::include_modules!();

#[derive(serde::Deserialize, Clone)]
pub struct EntryIn {
    appid: String,
    app_name: String,
    exec: String,
    comment: String,
    icon_compressed: Vec<u8>, // RGBA bytes from daemon compressed
}
#[derive(Clone)]
pub struct Entry {
    appid: String,
    app_name: String,
    exec: String,
    comment: String,
    icon: SharedPixelBuffer<Rgba8Pixel>, // RGBA bytes from daemon
}

pub fn filter_and_sort_entries(text: &str, normalized_entries: &[Entry]) -> Vec<Entry> {
    let matcher = SkimMatcherV2::default();

    let mut matched_entries: Vec<(i64, Entry)> = normalized_entries
        .iter()
        .filter_map(|entry| {
            let search_string = format!(
                "{} {} {} {}",
                entry.app_name, entry.comment, entry.appid, entry.exec
            );

            matcher
                .fuzzy_match(&search_string, text)
                .map(|score| (score, entry.clone()))
        })
        .collect();

    matched_entries.sort_by(|a, b| {
        let score_cmp = b.0.cmp(&a.0);
        if score_cmp == Ordering::Equal {
            a.1.app_name.len().cmp(&b.1.app_name.len())
        } else {
            score_cmp
        }
    });

    matched_entries
        .into_iter()
        .map(|(_, entry)| entry)
        .collect()
}

fn theme_from_config(theme: &config::ThemeConfig) -> ThemeSlint {
    ThemeSlint {
        fullscreen: theme.maximise as bool,
        search_icon_enable: theme.search_icon_enable as bool,
        grid_config: slint_generatedAppWindow::GridConfig {
            enabled: theme.grid_config.enabled,
            col: theme.grid_config.col as i32,
            row: theme.grid_config.row as i32,
            button_color: config_color_to_slint(&theme.grid_config.button_color),
            selected_button_color: config_color_to_slint(&theme.grid_config.selected_button_color),
            button_text_color: config_color_to_slint(&theme.grid_config.button_text_color),
            selected_button_text_color: config_color_to_slint(
                &theme.grid_config.selected_button_text_color,
            ),
            arrow_button_width: theme.grid_config.arrow_button_width as f32,
            arrow_button_height: theme.grid_config.arrow_button_height as f32,
            sort_button_width: theme.grid_config.sort_button_width as f32,
            sort_button_height: theme.grid_config.sort_button_height as f32,
            button_border_radius: theme.grid_config.button_border_radius as f32,
        },
        window_background: config_color_to_slint(&theme.window_background),
        main_window_background: config_color_to_slint(&theme.main_window_background),
        item_background: config_color_to_slint(&theme.item_background),
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
        window_border_width: theme.window_border_width as f32,
        input_height: theme.input_height as f32,
        animation_time: theme.animation_duration as i64,
    }
}

fn get_cache_folder() -> PathBuf {
    let mut path = cache_dir().unwrap();
    path.push("cosmic-wanderer");
    fs::create_dir_all(&path).unwrap();

    path
}

fn fetch_entries_from_file() -> Vec<EntryIn> {
    let mut cache = get_cache_folder();
    cache.push("entries.zst");

    let file = match File::open(cache) {
        Ok(f) => f,
        Err(e) => {
            error!("file open failed: {}", e);
            return vec![];
        }
    };

    let mut decoder = match zstd::Decoder::new(file) {
        Ok(d) => d,
        Err(e) => {
            error!("Zstd decoder init failed: {}", e);
            return vec![];
        }
    };

    let mut buf = String::new();

    if decoder.read_to_string(&mut buf).is_ok() {
        serde_json::from_str(&buf).unwrap_or_default()
    } else {
        error!("Failed to decompress or read from file");
        vec![]
    }
}

fn decode_compressed_to_rgba(compressed_data: &[u8]) -> Option<SharedPixelBuffer<Rgba8Pixel>> {
    let img = image::load_from_memory(compressed_data).ok()?.to_rgba8();
    let (w, h) = img.dimensions();

    Some(SharedPixelBuffer::clone_from_slice(img.as_raw(), w, h))
}

fn create_slint_items(normalized_entries: &[Entry], grid_config: config::GridConfig) -> AppItems {
    let model = VecModel::default();

    for entry in normalized_entries.iter() {
        model.push(AppItem {
            app_name: entry.app_name.clone().into(),
            app_id: entry.appid.clone().into(),
            exec: entry.exec.clone().into(),
            comment: entry.comment.clone().into(),
            icon: Image::from_rgba8(entry.icon.clone()),
        });
    }

    // Apply grid padding if enabled
    let mut max_pages = 0;
    if grid_config.enabled {
        let page_size = (grid_config.row as usize) * (grid_config.col as usize);
        let current = model.row_count();
        let pages = ((current + page_size - 1) / page_size).max(1);
        let target = pages * page_size;
        for _ in current..target {
            model.push(AppItem {
                app_name: "".into(),
                app_id: "".into(),
                exec: "".into(),
                comment: "".into(),
                icon: Image::from_rgba8(SharedPixelBuffer::new(1, 1)),
            });
        }
        max_pages = pages as i32;
    }

    AppItems {
        app_items: ModelRc::from(Rc::new(model)),
        max_pages,
    }
}

pub fn send_notification(message: &str) {
    let _ = Notification::new()
        .summary("Cosmic wanderer")
        .body(message)
        .show();
}

use std::time::Instant;

fn main() -> Result<(), Box<dyn Error>> {
    let start_time = Instant::now();
    env_logger::init_from_env(
        env_logger::Env::default().filter_or(env_logger::DEFAULT_FILTER_ENV, "info"),
    );

    unsafe {
        std::env::set_var("QT_QPA_PLATFORM", "wayland");
    }

    #[cfg(feature = "config_file")]
    let config = config::load_or_create_config().unwrap();
    #[cfg(not(feature = "config_file"))]
    let config = config::default_config();

    debug!("[{:?}] Loaded config", start_time.elapsed());

    let socket_path = config.general.socket_path.clone();

    let entries = Arc::new(Mutex::new(Vec::<Entry>::new()));
    let entries_for_thread = entries.clone();
    let _ = set_xdg_app_id("cosmic-wanderer");
    let grid_config = config.theme.grid_config.clone();

    let fetch_handle = std::thread::spawn(move || {
        debug!("[{:?}] fetching items", start_time.elapsed());
        let fetched = fetch_entries_from_file();
        let mut decoded_entry: Vec<Entry> = vec![];

        for entry_in in fetched.iter() {
            decoded_entry.push(Entry {
                app_name: entry_in.app_name.clone().into(),
                appid: entry_in.appid.clone().into(),
                exec: entry_in.exec.clone().into(),
                comment: entry_in.comment.clone().into(),
                icon: decode_compressed_to_rgba(&entry_in.icon_compressed.clone()).unwrap(),
            });
        }

        let history = load_history();
        let sorted = sorted_entries_by_usage(&decoded_entry, &history);
        let mut locked = entries_for_thread.lock();
        *locked = sorted.clone();
        debug!("[{:?}] fetched items", start_time.elapsed());

        sorted
    });
    let ui = AppWindow::new()?;
    debug!("[{:?}] UI created", start_time.elapsed());

    let theme = theme_from_config(&config.theme);
    ui.window().set_maximized(config.theme.maximise);
    ui.set_theme(theme.clone());

    let grid_config_clone = grid_config.clone();
    let ui_weak = ui.as_weak();
    std::thread::spawn(move || {
        let items = fetch_handle.join().expect("Fetch thread panicked");
        debug!("[{:?}] writing items to slint", start_time.elapsed());
        slint::invoke_from_event_loop(move || {
            if let Some(ui) = ui_weak.upgrade() {
                let slint_items = create_slint_items(&items, grid_config.clone());
                ui.set_appItems(slint_items);
                debug!("[{:?}] written items to ui", start_time.elapsed());
            }
        })
        .unwrap();
    });

    let ui_weak_clone_text = ui.as_weak();
    ui.on_text_entered(move |text| {
        debug!("[{:?}] User typed: '{}'", start_time.elapsed(), text);
        let locked_entries = entries.lock();
        let sorted_entries = if !text.is_empty() {
            filter_and_sort_entries(&text, &locked_entries)
        } else {
            let history = load_history();
            sorted_entries_by_usage(&locked_entries, &history)
        };

        let vec_model = create_slint_items(&sorted_entries, grid_config_clone.clone());

        if let Some(ui) = ui_weak_clone_text.upgrade() {
            ui.set_appItems(vec_model);

            ui.set_selected_index(0);
            ui.invoke_set_scroll(0.0);
        }
        drop(sorted_entries);
    });

    let ui_weak_clone_item = ui.as_weak();
    ui.on_item_clicked(move |idx| {
        debug!("[{:?}] Item clicked: index {}", start_time.elapsed(), idx);
        let idx = idx as usize;

        if let Some(ui) = ui_weak_clone_item.upgrade() {
            let entries = ui.get_appItems();
            if let Some(entry) = entries.app_items.row_data(idx) {
                if entry.exec.is_empty() {
                    debug!("[{:?}] Empty exec string, skipping", start_time.elapsed());
                    return;
                }
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

                    debug!(
                        "[{:?}] Launching command: {:?}",
                        start_time.elapsed(),
                        command
                    );

                    if let Err(e) = command.spawn() {
                        let msg = format!("Failed to spawn detached process: {}", e);
                        error!("[{:?}] {}", start_time.elapsed(), msg);
                        send_notification(&msg);
                    }
                }
                drop(entries);
                ui.hide().unwrap();
            }
        }
    });

    let ui_for_focus_thread = ui.as_weak();
    thread::spawn(move || {
        loop {
            std::thread::sleep(std::time::Duration::from_millis(50));
            let ui_for_closure = ui_for_focus_thread.clone();
            slint::invoke_from_event_loop(move || {
                if let Some(ui) = ui_for_closure.upgrade() {
                    if !ui.get_scopeFocused() {
                        if !ui.invoke_readFocus() {
                            ui.hide().unwrap();
                        }
                    }
                }
            })
            .unwrap_or_else(|e| {
                error!("[{:?}] Invoke failed focus: {}", start_time.elapsed(), e);
            });
        }
    });

    let ui_weak_clone_focus = ui.as_weak();
    ui.on_focus_changed(move |focused| {
        if !focused {
            if let Some(ui) = ui_weak_clone_focus.upgrade() {
                ui.hide().unwrap();
            }
        }
    });

    ui.show().ok();
    debug!("[{:?}] UI Initialized", start_time.elapsed());
    ui.invoke_focusText();

    ui.run()?;
    debug!("[{:?}] Application exited", start_time.elapsed());
    Ok(())
}
