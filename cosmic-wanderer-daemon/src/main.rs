use freedesktop_desktop_entry::default_paths;
use log::{debug, error};
use notify::event::{CreateKind, EventKind, ModifyKind, RemoveKind};
use notify::{RecursiveMode, Watcher, recommended_watcher};
use parking_lot::Mutex;
use serde::Serialize;
use std::{
    error::Error,
    io::Write,
    os::unix::net::UnixListener,
    sync::{Arc, mpsc::channel},
    thread,
};

mod entries;
use entries::{DesktopEntryManager, NormalDesktopEntry};

mod config;

#[derive(Serialize)]
struct EntryOut<'a> {
    name: &'a str,
    appid: &'a str,
    app_name: &'a str,
    exec: &'a str,
    comment: &'a str,
    icon: &'a str,
}

fn format_entries(entries: &[NormalDesktopEntry]) -> String {
    let data: Vec<EntryOut> = entries
        .iter()
        .map(|e| EntryOut {
            name: &e.app_name,
            appid: &e.appid,
            app_name: &e.app_name,
            exec: &e.exec,
            comment: &e.comment,
            icon: &e.icon,
        })
        .collect();

    serde_json::to_string(&data).unwrap()
}

fn main() -> Result<(), Box<dyn Error>> {
    env_logger::init_from_env(
        env_logger::Env::default().filter_or(env_logger::DEFAULT_FILTER_ENV, "info"),
    );

    #[cfg(feature = "config_file")]
    let config = config::load_or_create_config().unwrap();
    #[cfg(not(feature = "config_file"))]
    let config = config::default_config();
    let socket_path = config.general.socket_path.clone();

    let mut manager = DesktopEntryManager::new();

    let entries = Arc::new(Mutex::new(manager.get_normalized_entries(
        &config.general.icon_theme,
        &config.general.icon_size,
        config.general.blacklist.clone(),
    )));

    let (tx, rx) = channel();
    let mut watcher = recommended_watcher(tx)?;

    for path in default_paths() {
        if path.exists() {
            watcher.watch(&path, RecursiveMode::Recursive)?;
        }
    }

    let entries_clone = entries.clone();
    let icon_theme = config.general.icon_theme.clone();
    let icon_size = config.general.icon_size.clone();
    let blacklist = config.general.blacklist.clone();

    thread::spawn(move || {
        for res in rx {
            match res {
                Ok(event) => match event.kind {
                    EventKind::Modify(ModifyKind::Data(_))
                    | EventKind::Create(CreateKind::Any)
                    | EventKind::Remove(RemoveKind::Any) => {
                        manager.refresh();

                        let mut locked = entries_clone.lock();
                        *locked = manager.get_normalized_entries(
                            &icon_theme,
                            &icon_size,
                            blacklist.clone(),
                        );

                        debug!("entries refreshed: {}", locked.len());
                    }
                    _ => {}
                },
                Err(e) => error!("watch error: {:?}", e),
            }
        }
    });

    let _ = std::fs::remove_file(&socket_path);
    let listener = UnixListener::bind(&socket_path)?;

    debug!("listening on socket: {}", socket_path);

    for stream in listener.incoming() {
        match stream {
            Ok(mut stream) => {
                let data = {
                    let locked = entries.lock();
                    format_entries(&locked)
                };
                debug!("listening on socket: {}", data);

                if let Err(e) = stream.write_all(data.as_bytes()) {
                    error!("write failed: {}", e);
                }
            }
            Err(e) => error!("connection failed: {}", e),
        }
    }

    Ok(())
}
