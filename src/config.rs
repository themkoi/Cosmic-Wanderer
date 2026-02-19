use serde::{Deserialize, Serialize};
use slint::Color;

#[cfg(feature = "config_file")]
use {
    config::{Config as ConfigLoader, File},
    dirs::config_dir,
    std::fs,
    std::path::{Path, PathBuf},
};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ConfigColor {
    red: u8,
    green: u8,
    blue: u8,
    alpha: u8,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GridConfig {
    pub enabled: bool,
    pub col: u8,
    pub row: u8,
    pub button_color: ConfigColor,
    pub selected_button_color: ConfigColor,
    pub button_text_color: ConfigColor,
    pub selected_button_text_color: ConfigColor,
    pub arrow_button_width: u16,
    pub arrow_button_height: u16,
    pub sort_button_width: u16,
    pub sort_button_height: u16,
    pub button_border_radius: u16,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ThemeConfig {
    pub maximise: bool,
    pub search_icon_enable: bool,
    pub grid_config: GridConfig,
    pub window_background: ConfigColor,
    pub main_window_background: ConfigColor,
    pub item_background: ConfigColor,
    pub selected_item_background: ConfigColor,
    pub selected_text_color: ConfigColor,
    pub unselected_text_color: ConfigColor,
    pub item_height: u16,
    pub item_spacing: u16,
    pub item_border_radius: u16,
    pub icon_size: u16,
    pub input_font_size: u16,
    pub input_border_width: u16,
    pub text_font_size: u16,
    pub comment_font_size: u16,
    pub font_family: String,
    pub font_weight: i32,
    pub window_border_width: u16,
    pub input_height: u16,
    pub animation_duration: u16,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GeneralConfig {
    pub icon_theme: String,
    pub socket_path: String,
    pub blacklist: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Config {
    pub theme: ThemeConfig,
    pub general: GeneralConfig,
}

#[cfg(not(feature = "quill_defaults"))]
pub fn default_config() -> Config {
    Config {
        theme: ThemeConfig {
            maximise: false,
            search_icon_enable: false,
            grid_config: GridConfig {
                enabled: false,
                col: 8,
                row: 4,
                button_color: ConfigColor {
                    red: 24,
                    green: 24,
                    blue: 37,
                    alpha: (0.8 * 255.0) as u8,
                },
                selected_button_color: ConfigColor {
                    red: 203,
                    green: 166,
                    blue: 247,
                    alpha: 255,
                },
                button_text_color: ConfigColor {
                    red: 205,
                    green: 214,
                    blue: 244,
                    alpha: 255,
                },
                selected_button_text_color: ConfigColor {
                    red: 24,
                    green: 24,
                    blue: 37,
                    alpha: 255,
                },
                arrow_button_width: 150,
                arrow_button_height: 100,
                sort_button_width: 150,
                sort_button_height: 100,
                button_border_radius: 10,
            },
            window_background: ConfigColor {
                red: 24,
                green: 24,
                blue: 37,
                alpha: (0.8 * 255.0) as u8,
            },
            main_window_background: ConfigColor {
                red: 0,
                green: 0,
                blue: 0,
                alpha: (0.0 * 255.0) as u8,
            },
            item_background: ConfigColor {
                red: 0,
                green: 0,
                blue: 0,
                alpha: (0.0 * 255.0) as u8,
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
            icon_size: 16, // Don't change, this is native size
            input_font_size: 20,
            input_border_width: 3,
            text_font_size: 17,
            comment_font_size: 12,
            font_family: "JetBrainsMono NF SemiBold".to_string(),
            font_weight: 650,
            window_border_width: 2,
            input_height: 70,
            animation_duration: 100,
        },
        general: GeneralConfig {
            icon_theme: "Papirus-Dark".to_string(),
            socket_path: "/tmp/comsic-wanderer.sock".to_string(),
            blacklist: Vec::new(),
        },
    }
}

#[cfg(feature = "quill_defaults")]
pub fn default_config() -> Config {
    Config {
        theme: ThemeConfig {
            maximise: true,
            search_icon_enable: true,
            grid_config: GridConfig {
                enabled: true,
                col: 5,
                row: 5,
                button_color: ConfigColor {
                    red: 255,
                    green: 255,
                    blue: 255,
                    alpha: 255,
                },
                selected_button_color: ConfigColor {
                    red: 0,
                    green: 0,
                    blue: 0,
                    alpha: 255,
                },
                button_text_color: ConfigColor {
                    red: 0,
                    green: 0,
                    blue: 0,
                    alpha: 255,
                },
                selected_button_text_color: ConfigColor {
                    red: 255,
                    green: 255,
                    blue: 255,
                    alpha: 255,
                },
                arrow_button_width: 150,
                arrow_button_height: 100,
                sort_button_width: 150,
                sort_button_height: 100,
                button_border_radius: 10,
            },
            window_background: ConfigColor {
                red: 255,
                green: 255,
                blue: 255,
                alpha: 255,
            },
            main_window_background: ConfigColor {
                red: 255,
                green: 255,
                blue: 255,
                alpha: 255,
            },
            item_background: ConfigColor {
                red: 255,
                green: 255,
                blue: 255,
                alpha: 255,
            },
            selected_item_background: ConfigColor {
                red: 0,
                green: 0,
                blue: 0,
                alpha: 255,
            },
            selected_text_color: ConfigColor {
                red: 255,
                green: 255,
                blue: 255,
                alpha: 255,
            },
            unselected_text_color: ConfigColor {
                red: 0,
                green: 0,
                blue: 0,
                alpha: 255,
            },
            item_height: 65,
            item_spacing: 5,
            item_border_radius: 10,
            icon_size: 64,
            input_font_size: 20,
            input_border_width: 3,
            text_font_size: 17,
            comment_font_size: 18,
            font_family: "Adwaita Sans Medium".to_string(),
            font_weight: 650,
            window_border_width: 0,
            input_height: 70,
            animation_duration: 0,
        },
        general: GeneralConfig {
            icon_theme: "Papirus-Dark".to_string(),
            socket_path: "/tmp/comsic-wanderer.sock".to_string(),
            blacklist: vec!["syncthing-start", "syncthing-ui", "vncviewer"]
                .into_iter()
                .map(String::from)
                .collect(),
        },
    }
}

pub fn config_color_to_slint(c: &ConfigColor) -> Color {
    Color::from_argb_u8(c.alpha, c.red, c.green, c.blue)
}

#[cfg(feature = "config_file")]
fn get_config_file() -> PathBuf {
    let mut path = config_dir().unwrap();
    path.push("cosmic-wanderer");
    fs::create_dir_all(&path).unwrap();
    path.push("config.toml");
    path
}

#[cfg(feature = "config_file")]
fn write_config<P: AsRef<Path>>(path: P, config: &Config) -> std::io::Result<()> {
    let toml_string = toml::to_string_pretty(config).expect("Failed to serialize config");
    fs::write(path, toml_string)
}

#[cfg(feature = "config_file")]
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
