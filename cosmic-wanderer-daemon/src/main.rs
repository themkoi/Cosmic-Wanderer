use freedesktop_desktop_entry::default_paths;
use image::codecs::png::PngEncoder;
use log::{debug, error};
use notify::event::{CreateKind, EventKind, ModifyKind, RemoveKind};
use notify::{RecursiveMode, Watcher, recommended_watcher};
use parking_lot::Mutex;
use serde::Serialize;
use std::fs::{self, File};
use std::path::PathBuf;
use std::{
    error::Error,
    io::Write,
    os::unix::net::UnixListener,
    sync::{Arc, mpsc::channel},
    thread,
};

use dirs::cache_dir;

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
    icon_compressed: Vec<u8>,
    width: u32,
    height: u32,
}

use image::{ColorType, ImageEncoder, ImageReader}; // needed imports
use resvg::tiny_skia::Pixmap;
use resvg::usvg::{self, Transform};

fn load_icon_compressed(path: &str) -> (Vec<u8>, u32, u32) {
    // SVG -> render to raster, compress to PNG
    if path.ends_with(".svg") {
        if let Ok(data) = std::fs::read(path) {
            if let Ok(tree) = usvg::Tree::from_data(&data, &usvg::Options::default()) {
                let width = tree.size().width().ceil() as u32;
                let height = tree.size().height().ceil() as u32;

                if let Some(mut pixmap) = Pixmap::new(width, height) {
                    let _ = resvg::render(&tree, Transform::default(), &mut pixmap.as_mut());

                    let rgba = pixmap.data();
                    let mut buf = Vec::new();
                    let encoder = PngEncoder::new(&mut buf);
                    // use write_image instead of encode
                    encoder
                        .write_image(rgba, width, height, ColorType::Rgba8.into())
                        .ok();

                    return (buf, width, height);
                }
            }
        }
        return (vec![], 0, 0);
    }

    // Raster image -> compress to PNG
    if let Ok(reader) = ImageReader::open(path) {
        // unwrap the decode result
        let img = reader.decode().expect("Failed to decode image");
        let rgba_img = img.to_rgba8(); // now this works
        let (w, h) = rgba_img.dimensions();

        let mut buf = Vec::new();
        let encoder = PngEncoder::new(&mut buf);
        encoder
            .write_image(rgba_img.as_raw(), w, h, ColorType::Rgba8.into())
            .ok();

        return (buf, w, h);
    }

    (vec![], 0, 0)
}

fn format_entries(entries: &[NormalDesktopEntry]) -> String {
    let data: Vec<EntryOut> = entries
        .iter()
        .map(|e| {
            let (icon_compressed, width, height) = load_icon_compressed(&e.icon);
            EntryOut {
                name: &e.app_name,
                appid: &e.appid,
                app_name: &e.app_name,
                exec: &e.exec,
                comment: &e.comment,
                icon: &e.icon,
                icon_compressed,
                width,
                height,
            }
        })
        .collect();
    serde_json::to_string(&data).unwrap()
}

fn get_cache_folder() -> PathBuf {
    let mut path = cache_dir().unwrap();
    path.push("cosmic-wanderer");
    fs::create_dir_all(&path).unwrap();

    path
}

fn save_compressed(entries: &[NormalDesktopEntry]) {
    let data = {
        let locked = entries;
        format_entries(&locked)
    };

    let mut location = get_cache_folder();
    location.push("entries.zst");
    let file = File::create(location).unwrap();
    let mut encoder = zstd::Encoder::new(file, 1).unwrap();

    if let Err(e) = encoder.write_all(data.as_bytes()) {
        error!("Compression/Write failed: {}", e);
    } else {
        encoder.finish().unwrap();
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    env_logger::init_from_env(
        env_logger::Env::default().filter_or(env_logger::DEFAULT_FILTER_ENV, "info"),
    );

    #[cfg(feature = "config_file")]
    let config = config::load_or_create_config().unwrap();
    #[cfg(not(feature = "config_file"))]
    let config = config::default_config();

    let mut manager = DesktopEntryManager::new();

    let entries = manager.get_normalized_entries(
        &config.general.icon_theme,
        &config.general.icon_size,
        config.general.blacklist.clone(),
    );
    save_compressed(&entries);

    let (tx, rx) = channel();
    let mut watcher = recommended_watcher(tx)?;
    drop(entries);

    for path in default_paths() {
        if path.exists() {
            watcher.watch(&path, RecursiveMode::Recursive)?;
        }
    }

    let file_watcher = thread::spawn(move || {
        for res in rx {
            match res {
                Ok(event) => match event.kind {
                    EventKind::Modify(ModifyKind::Data(_))
                    | EventKind::Create(CreateKind::Any)
                    | EventKind::Remove(RemoveKind::Any) => {
                        manager.refresh();

                        let entries = manager.get_normalized_entries(
                            &config.general.icon_theme,
                            &config.general.icon_size,
                            config.general.blacklist.clone(),
                        );
                        save_compressed(&entries);
                    }
                    _ => {}
                },
                Err(e) => error!("watch error: {:?}", e),
            }
        }
    });

    file_watcher.join().unwrap();

    Ok(())
}
