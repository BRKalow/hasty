use clap::Parser;
use hasty::{self, logger, make_script_id, Engine, Script};

#[tokio::main]
async fn main() {
    logger::init();

    let options = hasty::options::HastyOptions::parse();

    let working_dir = options
        .dir
        .clone()
        .unwrap_or(std::env::current_dir().unwrap());

    let config = hasty::load_config_file(&options);
    let mut tasks_to_execute = vec![];

    match options.script {
        Some(x) => {
            if config.pipeline.contains_key(&x) == false {
                panic!("Pipeline does not contain the provided script")
            }

            tasks_to_execute.push(x)
        }
        None => {
            for task in config.pipeline.keys() {
                tasks_to_execute.push(String::from(task));
            }
        }
    };

    let mut engine = Engine::new(config.clone(), &working_dir, tasks_to_execute.clone());

    for task in tasks_to_execute.iter() {
        let script = Script::new(
            config.pipeline.get(task).unwrap().clone(),
            &working_dir,
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

                let s = Script::new(
                    config.pipeline.get(&s).unwrap().clone(),
                    &working_dir,
                    "__ROOT__",
                );
                engine.add_script(&s);

                if s.has_dependencies() {
                    stack.append(&mut s.dependencies().unwrap());
                }
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
