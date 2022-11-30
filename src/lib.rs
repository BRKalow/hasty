pub mod options;
pub mod package_json;

use daggy::{
    petgraph::visit::{IntoNodeIdentifiers, Topo},
    Dag,
};
use package_json::{find_workspaces, PackageJSON};
use serde::Deserialize;
use std::{
    cell::{Ref, RefCell, RefMut},
    collections::HashMap,
    env, fs,
    path::PathBuf,
    process,
};

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
    pub command: String,
    pub package_name: String,
    config: CommandConfig,
    pub dir: PathBuf,
}

impl Script {
    pub fn new(config: CommandConfig, dir: &PathBuf, package_name: &str) -> Self {
        let mut command = process::Command::new("npm");
        let name = config.command.clone();

        command.current_dir(dir).arg("run").arg(name.to_string());

        let mut status = ScriptStatus::Ready;
        if let Some(ref _deps) = config.dependencies {
            status = ScriptStatus::Waiting;
        }

        Script {
            config,
            package_name: package_name.to_string(),
            dir: dir.into(),
            command: name.to_string(),
            status,
        }
    }

    pub fn execute(&mut self) {
        self.status = ScriptStatus::Running;

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
            .wait_with_output()
            .unwrap();

        self.status = ScriptStatus::Finished;
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

    pub fn id(&self) -> String {
        make_script_id(&self.package_name, &self.command)
    }
}

impl PartialEq for Script {
    fn eq(&self, other: &Self) -> bool {
        self.config.command == other.config.command
    }
}

pub struct Engine {
    called_script: String,
    dir: PathBuf,
    config: Config,
    task_graph: RefCell<Dag<String, u32, u32>>,
    scripts: RefCell<HashMap<String, Script>>,
    deps: RefCell<Vec<(String, String)>>,
    workspaces: Vec<PackageJSON>,
}

impl Engine {
    pub fn new(config: Config, dir: PathBuf, called_script: &str) -> Self {
        let workspaces = find_workspaces(&dir);

        Engine {
            called_script: String::from(called_script),
            dir,
            config,
            task_graph: RefCell::new(Dag::<String, u32, u32>::new()),
            scripts: RefCell::new(HashMap::<String, Script>::new()),
            deps: RefCell::new(Vec::new()),
            workspaces,
        }
    }

    pub fn add_dep(&mut self, from: &str, to: &str) {
        self.deps
            .borrow_mut()
            .push((String::from(from), String::from(to)));
    }

    pub fn add_deps_to_graph(&self) {
        let mut task_graph = self.task_graph.borrow_mut();

        for (from_id, to_id) in self.deps.borrow().iter() {
            let from_index = task_graph
                .node_identifiers()
                .find(|i| String::from(from_id) == task_graph[*i]);
            let to_index = task_graph
                .node_identifiers()
                .find(|i| String::from(to_id) == task_graph[*i]);

            if let (Some(from), Some(to)) = (from_index, to_index) {
                task_graph.add_edge(from, to, 0);
            }
        }
    }

    pub fn add_script(&self, script: &Script) {
        self.scripts
            .borrow_mut()
            .insert(script.id(), script.clone());

        // add a node to the task graph if it's not a "__ROOT__" script
        if script.id().starts_with("__ROOT__") == false {
            self.task_graph.borrow_mut().add_node(script.id());
        }
    }

    pub fn scripts(&self) -> Ref<HashMap<std::string::String, Script>> {
        self.scripts.borrow()
    }

    pub fn scripts_mut(&self) -> RefMut<HashMap<std::string::String, Script>> {
        self.scripts.borrow_mut()
    }

    pub fn execute(&self) {
        // Walk the graph in topological order, executing each script
        let mut topo = Topo::new(&self.task_graph.borrow_mut().graph());

        // TODO: how to parallelize?
        while let Some(next_id) = topo.next(&self.task_graph.borrow().graph()) {
            let script_id = &self.task_graph.borrow()[next_id];
            self.scripts
                .borrow_mut()
                .get_mut(script_id)
                .unwrap()
                .execute();
        }
    }

    pub fn resolve_workspace_scripts(&self) {
        let cur_scripts = self
            .scripts()
            .values()
            .map(|s| (s.id(), s.command.clone()))
            .collect::<Vec<(String, String)>>();

        for ws in self.workspaces.iter() {
            let ws_scripts = match &ws.scripts {
                Some(x) => x,
                None => continue,
            };

            // ignore packages that don't include the main script we are running
            if ws_scripts.contains_key(&self.called_script) == false {
                continue;
            }

            for (script_id, script_name) in &cur_scripts {
                if ws_scripts.contains_key(script_name) {
                    let mut ws_script = self.scripts.borrow().get(script_id).unwrap().clone();

                    ws_script.package_name = ws.name.clone();

                    if let Some(ws_dir) = ws.dir.clone() {
                        ws_script.dir = ws_dir;
                    }

                    // ensure package-level deps are represented
                    if let Some(script_deps) = &ws_script.dependencies() {
                        for d in script_deps {
                            if ws_scripts.contains_key(d) {
                                self.deps
                                    .borrow_mut()
                                    .push((make_script_id(&ws.name, d), ws_script.id()));
                            }
                        }
                    }

                    Engine::add_script(self, &ws_script);
                }
            }
        }
    }
}

pub fn load_config_file(opts: &options::HastyOptions) -> Config {
    let mut dir = env::current_dir().unwrap();

    if let Some(opts_dir) = &opts.dir {
        dir = opts_dir.to_path_buf()
    }

    let raw = fs::read_to_string(dir.join(CONFIG_FILE_NAME)).unwrap();
    let config: Config = serde_json::from_str(&raw).unwrap();

    return config;
}

pub fn make_script_id(package_name: &str, script_name: &str) -> String {
    format!("{}#{}", package_name, script_name)
}
