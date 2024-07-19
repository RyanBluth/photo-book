use std::{
    fs::File,
    io::{Read, Write},
    path::PathBuf,
};

use serde::{Deserialize, Serialize};

use crate::{auto_persisting::PersistentModifiable, dirs::Dirs};

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("TOML deserialization error: {0}")]
    Toml(#[from] toml::de::Error),
    #[error("TOML serialization error: {0}")]
    TomlSer(#[from] toml::ser::Error),
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct Config {
    recent_projects: Option<Vec<PathBuf>>,
}

#[derive(Debug, Serialize, Deserialize)]
struct RecentProject {
    path: PathBuf,
    last_opened_or_created: u64,
}

pub enum ConfigModification {
    AddRecentProject(PathBuf),
}

impl Config {
    pub fn recent_projects(&self) -> &[PathBuf] {
        self.recent_projects.as_deref().unwrap_or(&[])
    }
}

impl PersistentModifiable<Config> for Config {
    type Error = ConfigError;
    type Modification = ConfigModification;

    fn load() -> Result<Config, ConfigError> {
        let config_path = Dirs::Config.path().join("config.toml");
        if config_path.exists() {
            let mut file = File::open(config_path)?;
            let mut buf = String::new();
            file.read_to_string(&mut buf)?;
            let config: Config = toml::from_str(&buf)?;
            Ok(config)
        } else {
            Ok(Config::default())
        }
    }

    fn save(&self) -> Result<(), ConfigError> {
        let config_path = Dirs::Config.path().join("config.toml");
        let mut file = File::create(config_path)?;
        file.write_all(toml::to_string(self)?.as_bytes())?;
        Ok(())
    }

    fn modify(&mut self, modification: ConfigModification) -> Result<(), ConfigError> {
        match modification {
            ConfigModification::AddRecentProject(project) => {
                if let Some(recent_projects) = &mut self.recent_projects {
                    if let Some(index) = recent_projects.iter().position(|p| p == &project) {
                        recent_projects.remove(index);
                    }
                    recent_projects.insert(0, project);
                } else {
                    self.recent_projects = Some(vec![project]);
                }
            }
        }

        self.save()?;
        Ok(())
    }
}
