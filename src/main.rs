use clap::Parser;
use daggy::{petgraph::algo, Dag};
use hasty::{self, CommandConfig, Script};

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

        let script = Script::new(config.pipeline.get(&opts_script).unwrap().clone(), dir);

        if script.has_dependencies() {
            let empty_script = Script::new(
                CommandConfig {
                    command: "__empty_command".to_string(),
                    dependencies: None,
                    files: None,
                    output: None,
                },
                dir,
            );
            let mut dag = Dag::<Script, u32, u32>::new();
            let root = dag.add_node(script.clone());

            let mut cur = &script;
            let parent = root;

            // TODO: traverse all deps
            while cur.has_dependencies() {
                for dep in cur.dependencies().unwrap().into_iter() {
                    let child = Script::new(config.pipeline.get(&dep).unwrap().clone(), dir);

                    dag.add_child(parent, 0, child);
                }

                cur = &empty_script;
            }

            algo::toposort(&dag, None)
                .unwrap()
                .iter()
                .rev()
                .for_each(|n| {
                    let s = &dag[*n];
                    println!("execute {:?}", s);
                    s.execute().wait_with_output().unwrap();
                });
        } else {
            script.execute().wait_with_output().unwrap();
        }
    }
}
