use glob::glob;
use std::{collections::HashMap, fs, path::PathBuf};

use serde::Deserialize;

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PackageJSON {
    pub name: String,
    pub version: Option<String>,
    pub scripts: Option<HashMap<String, String>>,
    pub dependencies: Option<HashMap<String, String>>,
    pub dev_dependencies: Option<HashMap<String, String>>,
    pub peer_dependencies: Option<HashMap<String, String>>,
    pub optional_dependencies: Option<HashMap<String, String>>,
    pub workspaces: Option<Vec<String>>,
    pub private: Option<bool>,

    #[serde(skip)]
    pub dir: Option<PathBuf>,
}

pub fn read_package_json(path: &PathBuf) -> PackageJSON {
    let raw = fs::read_to_string(path.join("package.json")).unwrap();
    let mut pkg: PackageJSON = serde_json::from_str(&raw).unwrap();

    pkg.dir = Some(path.clone());

    return pkg;
}

pub fn find_workspaces(root_dir: &PathBuf) -> Vec<PackageJSON> {
    let mut result: Vec<PackageJSON> = vec![];

    let pkg = read_package_json(root_dir);

    if let Some(workspaces) = pkg.workspaces {
        for ws in workspaces {
            let glob_with_root = root_dir.join(&ws);
            for entry in glob(glob_with_root.to_str().unwrap()).unwrap() {
                match entry {
                    Ok(p) => {
                        result.push(read_package_json(&p));
                    }
                    Err(e) => panic!("{}", e),
                }
            }
        }
    }

    println!("{:?}", result);

    result
}
