mod config;
mod data;
mod layout;
mod render;
mod report;
mod text;
mod timeline;
mod waste_charts;

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

    // Render waste visualizations if requested
    if let Some(ref waste_dir) = config.waste_output_dir {
        eprintln!("Generating waste visualizations...");
        if let Some(ref stats) = data.statistics {
            if let Some(ref wm) = stats.waste_metrics {
                if let Err(e) = waste_charts::render_all(wm, waste_dir) {
                    eprintln!("Error rendering waste charts: {}", e);
                    std::process::exit(1);
                }
                eprintln!("Waste charts written to {:?}", waste_dir);
            } else {
                eprintln!("No waste metrics in data — skipping waste charts");
            }
        } else {
            eprintln!("No statistics in data — skipping waste charts");
        }
    }

    // Render video
    if let Err(e) = render::render_video(&data, &config) {
        eprintln!("Error rendering video: {}", e);
        std::process::exit(1);
    }
}
