mod config;
mod data;
mod layout;
mod render;
mod report;
mod stats;
mod text;

mod change_flow_charts;

use clap::Parser;
use config::RenderConfig;
use std::time::Instant;

fn main() {
    let total_start = Instant::now();
    let config = RenderConfig::parse();

    let num_threads = rayon::current_num_threads();
    eprintln!("Parallelization: {} threads available (rayon auto-detected)", num_threads);

    // Phase 1: Load data
    let phase_start = Instant::now();
    eprintln!("Loading data from {:?}...", config.input);
    let data = match data::load_data(&config.input) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("Error loading data: {}", e);
            std::process::exit(1);
        }
    };

    eprintln!(
        "Loaded {} commits, {} branches, {} merges [{:.2}s]",
        data.commits.len(),
        data.branches.len(),
        data.merges.len(),
        phase_start.elapsed().as_secs_f64()
    );

    // Phase 2: Statistics report
    if let Some(ref report_path) = config.report_output {
        let phase_start = Instant::now();
        eprintln!("Generating statistics report...");
        if let Err(e) = report::render_report(&data, report_path) {
            eprintln!("Error rendering report: {}", e);
            std::process::exit(1);
        }
        eprintln!("Report written to {:?} [{:.2}s]", report_path, phase_start.elapsed().as_secs_f64());
    }

    // Phase 3: Change flow visualizations (parallel chart rendering)
    if let Some(ref cf_dir) = config.change_flow_dir {
        let phase_start = Instant::now();
        eprintln!("Generating change flow visualizations ({} threads)...", num_threads);
        if let Some(ref stats) = data.statistics {
            if let Some(ref cf) = stats.change_flow {
                if let Err(e) = change_flow_charts::render_all(cf, cf_dir) {
                    eprintln!("Error rendering change flow charts: {}", e);
                    std::process::exit(1);
                }
                eprintln!("Change flow charts written to {:?} [{:.2}s]", cf_dir, phase_start.elapsed().as_secs_f64());
            } else {
                eprintln!("No change flow metrics in data — skipping charts");
            }
        } else {
            eprintln!("No statistics in data — skipping charts");
        }
    }

    // Phase 4: Video rendering (parallel frame generation)
    let phase_start = Instant::now();
    if let Err(e) = render::render_video(&data, &config) {
        eprintln!("Error rendering video: {}", e);
        std::process::exit(1);
    }
    eprintln!("Video rendering complete [{:.2}s]", phase_start.elapsed().as_secs_f64());

    eprintln!("Total elapsed: {:.2}s", total_start.elapsed().as_secs_f64());
}
