use std::path::PathBuf;

use clap::Parser;
use serde::Deserialize;

#[derive(Parser)]
#[clap(author, version, about, long_about = None)]
#[derive(Deserialize)]
pub struct HastyOptions {
    // The directory of the project
    #[arg(short, long)]
    pub dir: Option<PathBuf>,

    // The script to execute
    pub script: Option<String>,
}
