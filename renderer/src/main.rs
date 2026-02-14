mod config;
mod data;
mod layout;
mod render;
mod report;
mod text;
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

    // Render statistics report if requested
    if let Some(ref report_path) = config.report_output {
        eprintln!("Generating statistics report...");
        if let Err(e) = report::render_report(&data, report_path) {
            eprintln!("Error rendering report: {}", e);
            std::process::exit(1);
        }
        eprintln!("Report written to {:?}", report_path);
    }

    // Render video
    if let Err(e) = render::render_video(&data, &config) {
        eprintln!("Error rendering video: {}", e);
        std::process::exit(1);
    }
}
