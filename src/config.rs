use config::{Config as ConfigLoader, File};
use dirs::config_dir;
use serde::{Deserialize, Serialize};
use std::{fs};
use std::path::{Path, PathBuf};
use slint::Color;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ConfigColor {
    red: u8,
    green: u8,
    blue: u8,
    alpha: u8,
}
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ThemeConfig {
    pub window_background: ConfigColor,
    pub selected_item_background: ConfigColor,
    pub selected_text_color: ConfigColor,
    pub unselected_text_color: ConfigColor,
    pub item_height: u32,
    pub item_spacing: u32,
    pub item_border_radius: u32,
    pub icon_size: u32,
    pub input_font_size: u32,
    pub input_border_width: u32,
    pub text_font_size: u32,
    pub comment_font_size: u32,
    pub font_family: String,
    pub font_weight: i32,
    pub window_width: u32,
    pub window_height: u32,
    pub window_border_width: u32,
    pub input_height: u32,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GeneralConfig {
    pub icon_theme: String,
    pub socket_path: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Config {
    pub theme: ThemeConfig,
    pub general: GeneralConfig,
}

fn default_config() -> Config {
    Config {
        theme: ThemeConfig {
            window_background: ConfigColor {
                red: 24,
                green: 24,
                blue: 37,
                alpha: (0.8 * 255.0) as u8,
            },
            selected_item_background: ConfigColor {
                red: 203,
                green: 166,
                blue: 247,
                alpha: 255,
            },
            selected_text_color: ConfigColor {
                red: 24,
                green: 24,
                blue: 37,
                alpha: 255,
            },
            unselected_text_color: ConfigColor {
                red: 205,
                green: 214,
                blue: 244,
                alpha: 255,
            },
            item_height: 65,
            item_spacing: 5,
            item_border_radius: 10,
            icon_size: 48,
            input_font_size: 20,
            input_border_width: 3,
            text_font_size: 17,
            comment_font_size: 12,
            font_family: "JetBrainsMono NF SemiBold".to_string(),
            font_weight: 650,
            window_width: 400,
            window_height: 580,
            window_border_width: 2,
            input_height: 70,
        },
        general: GeneralConfig {
            icon_theme: "Papirus-Dark".to_string(),
            socket_path: "/tmp/comsic-wanderer.sock".to_string(),
        },
    }
}

pub fn config_color_to_slint(c: &ConfigColor) -> Color {
    Color::from_argb_u8(c.alpha, c.red, c.green, c.blue)
}

fn get_config_file() -> PathBuf {
    let mut path = config_dir().unwrap();
    path.push("cosmic-wanderer");
    fs::create_dir_all(&path).unwrap();
    path.push("config.toml");
    path
}

fn write_config<P: AsRef<Path>>(path: P, config: &Config) -> std::io::Result<()> {
    let toml_string = toml::to_string_pretty(config).expect("Failed to serialize config");
    fs::write(path, toml_string)
}

pub fn load_or_create_config() -> Result<Config, Box<dyn std::error::Error>> {
    let path_bug = get_config_file();
    let path = &path_bug;
    if !path.exists() {
        let default = default_config();
        write_config(path, &default)?;
        return Ok(default);
    }

    let loaded = ConfigLoader::builder()
        .add_source(File::with_name(path.to_str().unwrap()))
        .build()?
        .try_deserialize::<Config>()?;

    Ok(loaded)
}

