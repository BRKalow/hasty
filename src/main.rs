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

        let config = hasty::load_config_file(&options);

        let opts_script = match options.script {
            Some(x) => x,
            None => panic!("Script not provided"),
        };

        if config.pipeline.contains_key(&opts_script) == false {
            panic!("Pipeline does not contain the provided script")
        }

        let mut scripts: HashMap<String, Script> = HashMap::new();

        let mut script = Script::new(config.pipeline.get(&opts_script).unwrap().clone(), dir);

        scripts.insert(make_script_id("root", &script.command), script.clone());

        if script.has_dependencies() {
            // TODO: store IDs instead of Script structs, use to reference a map of scripts
            // TODO: support workspaces
            // TODO: build dependency graph between workspace packages
            // TODO: support topological command dependencies
            let mut dag = Dag::<String, u32, u32>::new();
            let root = dag.add_node(make_script_id("root", &script.command));

            let mut stack = vec![];
            let mut cur = Some(script);
            let mut parent = root;

            while let Some(s) = cur {
                if s.has_dependencies() {
                    for dep in s.dependencies().unwrap().into_iter() {
                        let script_config = config.pipeline.get(&dep).unwrap().clone();
                        let script_id = make_script_id("root", &script_config.command);
                        let mut child;

                        if scripts.contains_key(&script_id) == false {
                            child = Script::new(script_config, dir);
                            scripts.insert(script_id.clone(), child.clone());
                        }

                        let mut child_index = dag.node_identifiers().find(|i| script_id == dag[*i]);

                        // If the child already exists in the graph, add an edge from the child to the parent
                        if let Some(idx) = child_index {
                            dag.add_edge(idx, parent, 0);
                        } else {
                            let (_, new_index) = dag.add_parent(parent, 0, script_id.clone());
                            child_index = Some(new_index);
                        }

                        child = scripts.get(&script_id).unwrap().clone();

                        if child.has_dependencies() {
                            stack.append(
                                &mut child
                                    .dependencies()
                                    .unwrap()
                                    .into_iter()
                                    .map(|x| {
                                        Script::new(config.pipeline.get(&x).unwrap().clone(), dir)
                                    })
                                    .collect(),
                            );
                            // child_index should always be set at this point
                            parent = child_index.unwrap();
                        }
                    }
                } else {
                    let s_index = dag.node_identifiers().find(|i| s.id() == dag[*i]);

                    // If the script already exists in the graph, add an edge from the current script to the parent
                    if let Some(idx) = s_index {
                        dag.add_edge(idx, parent, 0);
                    } else {
                        dag.add_parent(parent, 0, s.id());
                    }

                    if scripts.contains_key(&s.id()) == false {
                        scripts.insert(s.id(), s);
                    }
                }

                cur = stack.pop();
            }

            // Walk the graph in topological order, executing each script
            let mut topo = Topo::new(&dag);

            while let Some(next_id) = topo.next(&dag) {
                let script_id = &mut dag[next_id];
                let s = scripts.get_mut(script_id).unwrap();
                s.execute();
            }
        } else {
            script.execute();
        }
    }
}
