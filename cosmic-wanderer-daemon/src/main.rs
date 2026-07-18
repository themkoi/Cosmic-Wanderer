use freedesktop_desktop_entry::default_paths;
use image::codecs::png::PngEncoder;
use log::{error};
use notify::event::{CreateKind, EventKind, ModifyKind, RemoveKind};
use notify::{RecursiveMode, Watcher, recommended_watcher};
use rsvg::Loader;
use serde::Serialize;
use std::fs::{self, File};
use std::path::PathBuf;
use std::{error::Error, io::Write, sync::mpsc::channel, thread};

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

use cairo::{Context, Format, ImageSurface, Rectangle};
use image::{ColorType, ImageEncoder, ImageReader}; 

pub fn render_svg_to_compressed(path: &str, width: i32, height: i32) -> (Vec<u8>, u32, u32) {
    let handle = Loader::new().read_path(path).unwrap();

    let mut surface = ImageSurface::create(Format::ARgb32, width, height).unwrap();

    {
        let cr = Context::new(&surface).unwrap();
        let renderer = rsvg::CairoRenderer::new(&handle);

        renderer
            .render_document(&cr, &Rectangle::new(0.0, 0.0, width as f64, height as f64))
            .unwrap();
    } 

    let mut data = surface.data().unwrap().to_vec();

    for px in data.chunks_exact_mut(4) {
        let a = px[3] as u32;

        if a > 0 {
            let r = px[2] as u32;
            let g = px[1] as u32;
            let b = px[0] as u32;

            px[0] = ((r * 255) / a) as u8;
            px[1] = ((g * 255) / a) as u8;
            px[2] = ((b * 255) / a) as u8;
        } else {
            px[0] = 0;
            px[1] = 0;
            px[2] = 0;
        }
    }
    let mut buf = Vec::new();
    let encoder = PngEncoder::new(&mut buf);

    encoder
        .write_image(&data, width.try_into().unwrap(), height.try_into().unwrap(), ColorType::Rgba8.into())
        .ok();

    (buf, width as u32, height as u32)
}

fn load_icon_compressed(path: &str, width: i32, height: i32) -> (Vec<u8>, u32, u32) {
    let std_path = std::path::Path::new(path);
    if !std_path.exists() || std_path.is_dir() {
        return (vec![], 0, 0);
    }

    if path.ends_with(".svg") {
        return render_svg_to_compressed(path, width, height);
    }

    if let Ok(mut file) = File::open(path) {
        let mut buffer = [0u8; 64];
        if let Ok(bytes_read) = std::io::Read::read(&mut file, &mut buffer) {
            if let Ok(utf8_str) = std::str::from_utf8(&buffer[..bytes_read]) {
                if utf8_str.contains("<svg") || utf8_str.contains("<?xml") {
                    return render_svg_to_compressed(path, width, height);
                }
            }
        }
    }

    if let Ok(reader) = ImageReader::open(path) {
        if let Ok(guessed_reader) = reader.with_guessed_format() {
            if let Ok(img) = guessed_reader.decode() {
                let rgba_img = img.to_rgba8();
                let (w, h) = rgba_img.dimensions();

                let mut buf = Vec::new();
                let encoder = PngEncoder::new(&mut buf);
                encoder
                    .write_image(rgba_img.as_raw(), w, h, ColorType::Rgba8.into())
                    .ok();

                return (buf, w, h);
            }
        }
    }

    (vec![], 0, 0)
}

fn format_entries(entries: &[NormalDesktopEntry], size: i32) -> String {
    let data: Vec<EntryOut> = entries
        .iter()
        .map(|e| {
            let (icon_compressed, width, height) = load_icon_compressed(&e.icon, size, size);
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

fn save_compressed(entries: &[NormalDesktopEntry], size: i32) {
    let data = {
        let locked = entries;
        format_entries(&locked, size)
    };

    let mut location = get_cache_folder();
    location.push("entries.txt");
    let mut file = File::create(location).unwrap();

    if let Err(e) = file.write_all(data.as_bytes()) {
        error!("write failed {}", e);
    } else {
        file.flush().unwrap();
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
    save_compressed(&entries, config.general.icon_size.into());

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
                        save_compressed(&entries, config.general.icon_size.into());
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
