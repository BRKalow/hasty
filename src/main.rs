use clap::Parser;
use hasty::{self, logger, make_script_id, Engine, Script};

#[tokio::main]
async fn main() {
    logger::init();

    let options = hasty::options::HastyOptions::parse();

    if let Some(ref dir) = options.dir {
        let config = hasty::load_config_file(&options);

        let opts_script = match options.script {
            Some(x) => x,
            None => panic!("Script not provided"),
        };

        if config.pipeline.contains_key(&opts_script) == false {
            panic!("Pipeline does not contain the provided script")
        }

        let mut engine = Engine::new(config.clone(), dir.to_path_buf(), &opts_script);

        let script = Script::new(
            config.pipeline.get(&opts_script).unwrap().clone(),
            dir,
            "__ROOT__",
        );

        engine.add_script(&script);

        // add all root dependencies now so we can iterate through the touched scripts later
        if script.has_dependencies() {
            let mut stack = vec![];

            stack.append(&mut script.dependencies().unwrap());

            while stack.len() > 0 {
                let s = stack.pop().unwrap();

                if engine
                    .scripts()
                    .contains_key(&make_script_id("__ROOT__", &s))
                {
                    continue;
                }

                let s = Script::new(config.pipeline.get(&s).unwrap().clone(), dir, "__ROOT__");
                engine.add_script(&s);

                if s.has_dependencies() {
                    stack.append(&mut s.dependencies().unwrap());
                }
            }
        }

        engine.build_package_graph();

        engine.resolve_workspace_scripts();

        engine.add_topo_task_deps();

        // populate graph dependencies
        engine.add_deps_to_graph();

        engine.execute(options.dry_run).await;
    }
}
