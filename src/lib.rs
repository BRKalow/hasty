pub mod options;

use serde::Deserialize;
use std::{collections::HashMap, env, fs, path::PathBuf, process};

static CONFIG_FILE_NAME: &str = "hasty.json";

#[derive(Debug, Deserialize, Clone)]
pub struct CommandConfig {
    pub command: String,
    pub dependencies: Option<Vec<String>>,
    pub files: Option<Vec<String>>,
    pub output: Option<Vec<String>>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub pipeline: HashMap<String, CommandConfig>,
}

#[derive(Debug, PartialEq, Clone)]
pub enum ScriptStatus {
    Waiting,
    Ready,
    Running,
    Finished,
    Error,
}

#[derive(Debug, Clone)]
pub struct Script {
    pub status: ScriptStatus,
    config: CommandConfig,
    dir: PathBuf,
}

impl Script {
    pub fn new(config: CommandConfig, dir: &PathBuf) -> Self {
        let mut command = process::Command::new("npm");

        command
            .current_dir(dir)
            .arg("run")
            .arg(config.command.clone());

        let mut status = ScriptStatus::Ready;
        if let Some(ref _deps) = config.dependencies {
            status = ScriptStatus::Waiting;
        }

        Script {
            config,
            dir: dir.into(),
            status,
        }
    }

    pub fn execute(&self) -> std::process::Child {
        let mut command = process::Command::new("npm");

        command
            .current_dir(&self.dir)
            .arg("run")
            .arg(self.config.command.clone())
            .spawn()
            .expect(&format!(
                "failed to spawn command {}",
                &self.config.command.clone()
            ))
    }

    pub fn has_dependencies(&self) -> bool {
        if let Some(deps) = &self.config.dependencies {
            return deps.len() > 0;
        }

        false
    }

    pub fn dependencies(&self) -> Option<Vec<String>> {
        self.config.dependencies.clone()
    }
}

pub fn load_config_file(opts: &options::HastyOptions) -> Config {
    let mut dir = env::current_dir().unwrap();

    if let Some(opts_dir) = &opts.dir {
        dir = opts_dir.to_path_buf()
    }

    let raw = fs::read_to_string(dir.join(CONFIG_FILE_NAME)).unwrap();
    let config: Config = serde_json::from_str(&raw).unwrap();

    println!("config: {:?}", config);

    return config;
}

pub fn execute_command(cmd: &CommandConfig, dir: &PathBuf) -> std::process::Child {
    println!("executing command: {}", cmd.command);
    let command = process::Command::new("npm")
        .current_dir(dir)
        .arg("run")
        .arg(cmd.command.clone())
        .spawn()
        .expect("failed to call command");

    return command;
}
