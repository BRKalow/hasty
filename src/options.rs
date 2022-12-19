use std::path::PathBuf;

use clap::Parser;
use serde::Deserialize;

#[derive(Parser)]
#[clap(author, version, about, long_about = None)]
#[derive(Deserialize)]
pub struct HastyOptions {
    /// The directory of the project
    #[arg(short, long)]
    pub dir: Option<PathBuf>,

    /// The script to execute
    pub script: Option<String>,

    /// Package manager
    ///
    /// Valid options are "npm", "yarn", "pnpm"
    ///
    /// Default: "npm"
    #[arg(short, long, default_value_t = String::from("npm"))]
    pub package_manager: String,
}
