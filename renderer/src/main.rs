mod config;
mod data;
mod render;
mod timeline;

use clap::Parser;
use config::RenderConfig;

fn main() {
    let config = RenderConfig::parse();

    eprintln!("Loading data from {:?}...", config.input);
    let data = match data::load_data(&config.input) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("Error loading data: {}", e);
            std::process::exit(1);
        }
    };

    eprintln!(
        "Loaded {} commits, {} branches, {} merges",
        data.commits.len(),
        data.branches.len(),
        data.merges.len()
    );

    if let Err(e) = render::render_video(&data, &config) {
        eprintln!("Error rendering video: {}", e);
        std::process::exit(1);
    }
}
