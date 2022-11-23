use clap::Parser;
use daggy::{
    petgraph::{algo, visit::IntoNodeIdentifiers},
    Dag,
};
use hasty::{self, Script};

fn main() {
    let options = hasty::options::HastyOptions::parse();

    if let Some(ref dir) = options.dir {
        println!("dir: {}", dir.display());

        let config = hasty::load_config_file(&options);

        let opts_script = match options.script {
            Some(x) => x,
            None => panic!("Scipt not provided"),
        };

        if config.pipeline.contains_key(&opts_script) == false {
            panic!("Pipeline does not contain the provided script")
        }

        let mut script = Script::new(config.pipeline.get(&opts_script).unwrap().clone(), dir);

        if script.has_dependencies() {
            // TODO: store IDs instead of Script structs, use to reference a map of scripts
            // TODO: support workspaces
            // TODO: build dependency graph between workspace packages
            // TODO: support topological command dependencies
            let mut dag = Dag::<Script, u32, u32>::new();
            let root = dag.add_node(script.clone());

            let mut stack = vec![];
            let mut cur = Some(script);
            let mut parent = root;

            while let Some(s) = cur {
                if s.has_dependencies() {
                    for dep in s.dependencies().unwrap().into_iter() {
                        let child = Script::new(config.pipeline.get(&dep).unwrap().clone(), dir);

                        let mut child_index = dag.node_identifiers().find(|i| child == dag[*i]);

                        // If the child already exists in the graph, add an edge from the current parent to the child
                        if let Some(idx) = child_index {
                            dag.add_edge(parent, idx, 0);
                        } else {
                            let (_, new_node) = dag.add_child(parent, 0, child.clone());
                            child_index = Some(new_node)
                        }

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
                    let s_index = dag.node_identifiers().find(|i| s == dag[*i]);

                    // If the script already exists in the graph, add an edge from the current parent to the script
                    if let Some(idx) = s_index {
                        dag.add_edge(parent, idx, 0);
                    } else {
                        dag.add_child(parent, 0, s);
                    }
                }

                cur = stack.pop();
            }

            algo::toposort(&dag, None)
                .unwrap()
                .iter()
                .rev()
                .for_each(|n| {
                    let s = &mut dag[*n];
                    s.execute();
                });
        } else {
            script.execute();
        }
    }
}
