use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
    path::{Path, PathBuf},
};

use anyhow::{bail, Result};

/// Stores references to all the paths relevant to eww, and abstracts access to these files and directories
#[derive(Debug, Clone)]
pub struct EwwPaths {
    pub log_file: PathBuf,
    pub log_dir: PathBuf,
    pub ipc_socket_file: PathBuf,
    pub config_dir: PathBuf,
}

impl EwwPaths {
    pub fn from_config_dir<P: AsRef<Path>>(config_dir: P) -> Result<Self> {
        let config_dir = config_dir.as_ref();
        if config_dir.is_file() {
            bail!("Please provide the path to the config directory, not a file within it")
        }

        if !config_dir.exists() {
            bail!("Configuration directory {} does not exist", config_dir.display());
        }

        let config_dir = config_dir.canonicalize()?;

        let mut hasher = DefaultHasher::new();
        format!("{}", config_dir.display()).hash(&mut hasher);
        // daemon_id is a hash of the config dir path to ensure that, given a normal XDG_RUNTIME_DIR,
        // the absolute path to the socket stays under the 108 bytes limit. (see #387, man 7 unix)
        let daemon_id = format!("{:x}", hasher.finish());

        let ipc_socket_file = std::env::var("XDG_RUNTIME_DIR")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|_| std::path::PathBuf::from("/tmp"))
            .join(format!("eww-server_{}", daemon_id));

        // 100 as the limit isn't quite 108 everywhere (i.e 104 on BSD or mac)
        if format!("{}", ipc_socket_file.display()).len() > 100 {
            log::warn!("The IPC socket file's absolute path exceeds 100 bytes, the socket may fail to create.");
        }

        let log_dir = std::env::var("XDG_CACHE_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from(std::env::var("HOME").unwrap()).join(".cache"))
            .join("eww");

        if !log_dir.exists() {
            log::info!("Creating log dir");
            std::fs::create_dir_all(&log_dir)?;
        }

        Ok(EwwPaths { config_dir, log_file: log_dir.join(format!("eww_{}.log", daemon_id)), log_dir, ipc_socket_file })
    }

    pub fn default() -> Result<Self> {
        let config_dir = std::env::var("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from(std::env::var("HOME").unwrap()).join(".config"))
            .join("eww");

        Self::from_config_dir(config_dir)
    }

    pub fn get_log_file(&self) -> &Path {
        self.log_file.as_path()
    }

    pub fn get_log_dir(&self) -> &Path {
        self.log_dir.as_path()
    }

    pub fn get_ipc_socket_file(&self) -> &Path {
        self.ipc_socket_file.as_path()
    }

    pub fn get_config_dir(&self) -> &Path {
        self.config_dir.as_path()
    }

    pub fn get_yuck_path(&self) -> PathBuf {
        self.config_dir.join("eww.yuck")
    }
}

impl std::fmt::Display for EwwPaths {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "config-dir: {}, ipc-socket: {}, log-file: {}",
            self.config_dir.display(),
            self.ipc_socket_file.display(),
            self.log_file.display()
        )
    }
}
