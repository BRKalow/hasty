use hex::encode;
use log::{debug, info};
use std::path::PathBuf;

use crate::Script;

static CACHE_DIR: &str = ".hasty/cache";

pub trait Cache {
    fn new(working_dir: &PathBuf) -> Self;
    fn exists(&self, script: &Script) -> bool;
    // fn get(&self, script: &Script)
    // fn set(&self, script: &Script)
}

pub struct LocalCache {
    working_dir: PathBuf,
}

impl Cache for LocalCache {
    fn new(working_dir: &PathBuf) -> Self {
        LocalCache {
            working_dir: working_dir.into(),
        }
    }

    fn exists(&self, script: &Script) -> bool {
        let cache_dir = get_cache_dir(&self.working_dir);
        let script_cache_dir = cache_dir.join(format!("{}", encode(script.id())));
        let fingerprint_cache_dir =
            script_cache_dir.join(format!("{}", script.fingerprint.clone().unwrap()));

        let exists = fingerprint_cache_dir.exists();

        info!(
            target: &format!("{}:{}", "cache", script.id()),
            "dir: {:?}, exists: {}", fingerprint_cache_dir, exists
        );

        exists
    }
}

fn get_cache_dir(working_dir: &PathBuf) -> PathBuf {
    working_dir.join(CACHE_DIR)
}
