use std::{fs::File, io};

use crate::Script;
use log::info;
use sha2::{Digest, Sha256};

#[derive(Debug, Clone)]
pub struct Fingerprint {
    hashes: Vec<String>,
}

impl Fingerprint {
    pub fn new() -> Self {
        Fingerprint { hashes: vec![] }
    }

    // TODO: include system info, dependency info, and global config?
    pub fn compute(&mut self, script: &Script) -> &mut Self {
        if self.hashes.len() > 0 {
            return self;
        }

        if let Some(files) = &script.config.files {
            for script_file in files {
                let mut sha256 = Sha256::new();
                let mut file: Option<File> = None;

                let file_result = File::open(script.dir.join(script_file));

                match file_result {
                    Ok(x) => file = Some(x),
                    Err(error) => {
                        info!("error loading script file: {:?}", error);
                    }
                }

                if let Some(mut f) = file {
                    io::copy(&mut f, &mut sha256);
                    let result = sha256.finalize();

                    self.hashes.push(format!("{:x}", result));
                }
            }
        }

        return self;
    }

    pub fn string(&self) -> String {
        self.hashes.join(",")
    }
}

impl PartialEq for Fingerprint {
    fn eq(&self, other: &Self) -> bool {
        self.string() == other.string()
    }
}
