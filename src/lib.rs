pub mod options;
pub mod package_json;

use daggy::{
    petgraph::visit::{IntoNodeIdentifiers, Topo},
    Dag, NodeIndex, Walker,
};
use futures::future::join_all;
use package_json::{find_workspaces, PackageJSON};
use serde::Deserialize;
use std::{collections::HashMap, env, fs, path::PathBuf, time::SystemTime};
use tokio::{
    process::{Child, Command},
    sync::watch::{self, Receiver},
};

static CONFIG_FILE_NAME: &str = "hasty.json";
pub static TOPOLOGICAL_DEP_PREFIX: &str = "^";

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
        let name = config.command.clone();

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

    pub fn execute(&mut self) -> Child {
        self.status = ScriptStatus::Running;

        let mut command = Command::new("npm");

        let child = command
            .current_dir(&self.dir)
            .arg("run")
            .arg(self.config.command.clone())
            .spawn()
            .expect(&format!(
                "failed to spawn command {}",
                &self.config.command.clone()
            ));

        child
    }

    pub fn has_dependencies(&self) -> bool {
        if let Some(deps) = &self.config.dependencies {
            return deps.len() > 0;
        }

        false
    }

    pub fn dependencies(&self) -> Option<Vec<String>> {
        match self.config.dependencies.clone() {
            Some(deps) => Some(
                deps.iter()
                    .filter(|d| !d.starts_with(&TOPOLOGICAL_DEP_PREFIX))
                    .map(|d| {
                        if d.starts_with(&TOPOLOGICAL_DEP_PREFIX) == false {
                            return String::from(d);
                        }
                        return d.replace(TOPOLOGICAL_DEP_PREFIX, "");
                    })
                    .collect(),
            ),
            None => None,
        }
    }

    pub fn topological_dependencies(&self) -> Option<Vec<String>> {
        match self.config.dependencies.clone() {
            Some(deps) => Some(
                deps.iter()
                    .filter(|d| d.starts_with(&TOPOLOGICAL_DEP_PREFIX))
                    .map(|d| {
                        if d.starts_with(&TOPOLOGICAL_DEP_PREFIX) == false {
                            return String::from(d);
                        }
                        return d.replace(TOPOLOGICAL_DEP_PREFIX, "");
                    })
                    .collect(),
            ),
            None => None,
        }
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
    task_graph: Dag<String, u32, u32>,
    package_graph: Dag<String, u32, u32>,
    scripts: HashMap<String, Script>,
    deps: Vec<(String, String)>,
    workspaces: Vec<PackageJSON>,
}

impl Engine {
    pub fn new(config: Config, dir: PathBuf, called_script: &str) -> Self {
        let workspaces = find_workspaces(&dir);
        let mut package_graph = Dag::<String, u32, u32>::new();

        package_graph.add_node(String::from("__ROOT__"));

        Engine {
            called_script: String::from(called_script),
            dir,
            config,
            package_graph,
            task_graph: Dag::<String, u32, u32>::new(),
            scripts: HashMap::<String, Script>::new(),
            deps: Vec::new(),
            workspaces,
        }
    }

    pub fn add_dep(&mut self, from: &str, to: &str) {
        self.deps.push((String::from(from), String::from(to)));
    }

    pub fn add_deps_to_graph(&mut self) {
        for (from_id, to_id) in self.deps.iter() {
            let from_index = find_node_index(&self.task_graph, String::from(from_id));
            let to_index = find_node_index(&self.task_graph, String::from(to_id));

            if let (Some(from), Some(to)) = (from_index, to_index) {
                if self.task_graph.add_edge(from, to, 0).is_err() {
                    panic!("Cycle detected in the task graph: {} -> {}", from_id, to_id);
                }
            }
        }
    }

    pub fn add_script(&mut self, script: &Script) {
        self.scripts.insert(script.id(), script.clone());

        // add a node to the task graph if it's not a "__ROOT__" script
        if script.id().starts_with("__ROOT__") == false {
            self.task_graph.add_node(script.id());
        }
    }

    pub fn scripts(&self) -> &HashMap<std::string::String, Script> {
        &self.scripts
    }

    pub fn scripts_mut(&mut self) -> &mut HashMap<std::string::String, Script> {
        &mut self.scripts
    }

    pub async fn execute(&mut self) {
        let now = SystemTime::now();

        // Walk the graph in topological order, executing each script
        let mut topo = Topo::new(&self.task_graph.graph());

        let mut task_statuses = HashMap::<String, Receiver<ScriptStatus>>::new();
        let mut tasks = vec![];

        // TODO: how to wait for dependencies?
        while let Some(next_id) = topo.next(&self.task_graph.graph()) {
            let script_id = &self.task_graph[next_id];
            let mut script = self.scripts.get_mut(script_id).unwrap().clone();

            let (script_watcher, script_recv) = watch::channel(ScriptStatus::Waiting);

            task_statuses.insert(script_id.clone(), script_recv);

            let mut deps_channels = vec![];

            // subscribe to a task's dependencies status channels
            if script.has_dependencies() {
                for (from, to) in &self.deps {
                    if String::from(to) == script.id() {
                        deps_channels.push(task_statuses.get(from).unwrap().clone());
                    }
                }
            }

            // add a task that we can await later to ensure things get cleaned up correctly
            tasks.push(tokio::spawn(async move {
                let mut deps_remaining = deps_channels.len();

                // TODO: there's probably a better way to accomplish waiting for deps
                while deps_remaining > 0 {
                    for ch in deps_channels.iter_mut() {
                        // If the channel has a value of SciprtStatus::Finished
                        if *ch.borrow() == ScriptStatus::Finished {
                            deps_remaining -= 1;
                        }
                        ch.changed().await;
                    }
                }

                let mut child = script.execute();

                let status = match child.wait().await {
                    Ok(status) => Some(status),
                    Err(err) => {
                        println!("Error running script: {:?}", err);
                        None
                    }
                };

                script_watcher.send_replace(ScriptStatus::Finished);
            }));
            // );
        }

        join_all(tasks).await;

        println!("finished in: {}", now.elapsed().unwrap().as_secs());

        println!("{:?}", daggy::petgraph::dot::Dot::new(&self.task_graph));
    }

    pub fn resolve_workspace_scripts(&mut self) {
        let cur_scripts = self
            .scripts()
            .values()
            .map(|s| (s.id(), s.command.clone()))
            .collect::<Vec<(String, String)>>();

        let mut scripts_to_add = vec![];

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
                    let mut ws_script = self.scripts.get(script_id).unwrap().clone();

                    ws_script.package_name = ws.name.clone();

                    if let Some(ws_dir) = ws.dir.clone() {
                        ws_script.dir = ws_dir;
                    }

                    // ensure package-level deps are represented
                    if let Some(script_deps) = &ws_script.dependencies() {
                        for d in script_deps {
                            if ws_scripts.contains_key(d) {
                                self.deps
                                    .push((make_script_id(&ws.name, d), ws_script.id()));
                            }
                        }
                    }

                    // collect the script so we can add it to our engine after we're done iterating throught the workspaces
                    scripts_to_add.push(ws_script);
                }
            }
        }

        for script in scripts_to_add.iter() {
            self.add_script(script);
        }
    }

    pub fn build_package_graph(&mut self) {
        for ws in &self.workspaces {
            let pkg_node_id = self.package_graph.add_node(String::from(&ws.name));

            if let Some(ws_deps) = &ws.dependencies {
                for dep in ws_deps.keys() {
                    let dep_node_id = find_node_index(&self.package_graph, String::from(dep));

                    if None == dep_node_id {
                        self.package_graph
                            .add_parent(pkg_node_id, 1, String::from(dep));
                    } else if let Some(dep_node_id) = dep_node_id {
                        self.package_graph.add_edge(dep_node_id, pkg_node_id, 1);
                    }
                }
            }

            if let Some(ws_dev_deps) = &ws.dev_dependencies {
                for dep in ws_dev_deps.keys() {
                    let dep_node_id = find_node_index(&self.package_graph, String::from(dep));

                    if None == dep_node_id {
                        self.package_graph
                            .add_parent(pkg_node_id, 1, String::from(dep));
                    } else if let Some(dep_node_id) = dep_node_id {
                        self.package_graph.add_edge(dep_node_id, pkg_node_id, 1);
                    }
                }
            }
        }
    }

    pub fn add_topo_task_deps(&mut self) {
        let cur_scripts = self
            .scripts()
            .values()
            .map(|s| s.clone())
            .collect::<Vec<Script>>();

        for s in &cur_scripts {
            let package_name = &s.package_name;

            if s.has_dependencies() == false {
                continue;
            }

            // check the script's dependnecies for any topological dependencies. Uses the package_graph to determine topological task dependencies.
            for d in s.topological_dependencies().unwrap() {
                println!("{:?}", d);
                let package_node_index =
                    find_node_index(&self.package_graph, String::from(package_name)).unwrap();
                let mut package_parents = self.package_graph.parents(package_node_index);

                for (_, parent_package_index) in package_parents.walk_next(&self.package_graph) {
                    let parent_package_name = self
                        .package_graph
                        .node_weight(parent_package_index)
                        .unwrap();
                    let parent_package = self
                        .workspaces
                        .iter()
                        .find(|ws| ws.name == String::from(parent_package_name))
                        .unwrap();

                    if let Some(parent_scripts) = &parent_package.scripts {
                        if parent_scripts.get(&d) != None {
                            // The parent script contais the topological dependency, so add a dep to the task graph
                            self.deps
                                .push((make_script_id(parent_package_name, &d), s.id()));
                        }
                    }
                }
            }
        }
    }
}

fn find_node_index<NodeType: std::cmp::PartialEq>(
    graph: &Dag<NodeType, u32, u32>,
    node: NodeType,
) -> Option<NodeIndex> {
    return graph.node_identifiers().find(|i| node == graph[*i]);
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
