use serde::{Deserialize, Serialize};

#[cfg(feature = "config_file")]
use {
    config::{Config as ConfigLoader, File},
    dirs::config_dir,
    std::fs,
    std::path::{Path, PathBuf},
};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GeneralConfig {
    pub icon_theme: String,
    pub icon_size: u16,
    pub socket_path: String,
    pub blacklist: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Config {
    pub general: GeneralConfig,
}

#[cfg(not(feature = "quill_defaults"))]
pub fn default_config() -> Config {
    Config {
        general: GeneralConfig {
            icon_theme: "Papirus-Dark".to_string(),
            icon_size: 48,
            socket_path: "/tmp/comsic-wanderer.sock".to_string(),
            blacklist: Vec::new(),
        },
    }
}

#[cfg(feature = "quill_defaults")]
pub fn default_config() -> Config {
    Config {
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

#[cfg(feature = "config_file")]
fn get_config_file() -> PathBuf {
    let mut path = config_dir().unwrap();
    path.push("cosmic-wanderer");
    fs::create_dir_all(&path).unwrap();
    path.push("config-daemon.toml");
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
