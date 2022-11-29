use clap::Parser;
use daggy::{
    petgraph::visit::{IntoNodeIdentifiers, Topo},
    Dag,
};
use hasty::{self, make_script_id, Script};
use std::collections::HashMap;

fn main() {
    let options = hasty::options::HastyOptions::parse();

    if let Some(ref dir) = options.dir {
        println!("dir: {}", dir.display());

        let workspaces = hasty::package_json::find_workspaces(dir);
        let config = hasty::load_config_file(&options);

        let opts_script = match options.script {
            Some(x) => x,
            None => panic!("Script not provided"),
        };

        if config.pipeline.contains_key(&opts_script) == false {
            panic!("Pipeline does not contain the provided script")
        }

        let mut scripts: HashMap<String, Script> = HashMap::new();
        let mut deps: Vec<(String, String)> = vec![];
        let mut task_graph = Dag::<String, u32, u32>::new();

        let script = Script::new(
            config.pipeline.get(&opts_script).unwrap().clone(),
            dir,
            "__ROOT__",
        );

        scripts.insert(script.id(), script.clone());

        // add all root dependencies now so we can iterate through the touched scripts later
        if script.has_dependencies() {
            let mut stack = vec![];

            stack.append(&mut script.dependencies().unwrap());

            while stack.len() > 0 {
                let s = stack.pop().unwrap();

                if scripts.contains_key(&make_script_id("__ROOT__", &s)) {
                    continue;
                }

                let s = Script::new(config.pipeline.get(&s).unwrap().clone(), dir, "__ROOT__");
                scripts.insert(s.id(), s.clone());

                if s.has_dependencies() {
                    stack.append(&mut s.dependencies().unwrap());
                }
            }
        }

        let cur_scripts = scripts
            .values()
            .map(|s| (s.id(), s.command.clone()))
            .collect::<Vec<(String, String)>>();

        // check each workspace for the same scripts, add if found
        for ws in workspaces {
            let ws_scripts = match ws.scripts {
                Some(x) => x,
                None => continue,
            };

            // ignore packages that don't include the main script we are running
            if ws_scripts.contains_key(&script.command) == false {
                continue;
            }

            for (script_id, script_name) in &cur_scripts {
                if ws_scripts.contains_key(script_name) {
                    let s = scripts.get(script_id).unwrap();

                    let mut ws_script = s.clone();

                    ws_script.package_name = ws.name.clone();

                    if let Some(ws_dir) = ws.dir.clone() {
                        ws_script.dir = ws_dir;
                    }

                    // ensure package-level deps are represented
                    if let Some(script_deps) = &ws_script.dependencies() {
                        for d in script_deps {
                            if ws_scripts.contains_key(d) {
                                deps.push((make_script_id(&ws.name, d), ws_script.id()))
                            }
                        }
                    }

                    scripts.insert(ws_script.id(), ws_script);
                }
            }
        }

        // add all valid tasks to the graph
        for s in scripts.keys() {
            if s.starts_with("__ROOT__") {
                continue;
            }

            task_graph.add_node(s.clone());
        }

        // populate graph dependencies
        for (from_id, to_id) in deps {
            let from_index = task_graph
                .node_identifiers()
                .find(|i| from_id == task_graph[*i]);
            let to_index = task_graph
                .node_identifiers()
                .find(|i| to_id == task_graph[*i]);

            if let (Some(from), Some(to)) = (from_index, to_index) {
                task_graph.add_edge(from, to, 0);
            }
        }

        // Walk the graph in topological order, executing each script
        let mut topo = Topo::new(&task_graph);

        while let Some(next_id) = topo.next(&task_graph) {
            let script_id = &mut task_graph[next_id];
            let s = scripts.get_mut(script_id).unwrap();
            s.execute();
        }
    }
}
