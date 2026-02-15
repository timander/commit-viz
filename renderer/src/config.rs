use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "commit-viz-renderer", about = "Render commit timeline video")]
pub struct RenderConfig {
    /// Path to input JSON data file
    #[arg(short, long)]
    pub input: PathBuf,

    /// Output video file path
    #[arg(short, long, default_value = "output.mp4")]
    pub output: PathBuf,

    /// Frames per second
    #[arg(long, default_value_t = 30)]
    pub fps: u32,

    /// Video width
    #[arg(long, default_value_t = 1920)]
    pub width: u32,

    /// Video height
    #[arg(long, default_value_t = 1080)]
    pub height: u32,

    /// Target video duration in seconds (overrides auto-calculation)
    #[arg(long)]
    pub duration_secs: Option<u32>,

    /// Rendering style: network or timeline
    #[arg(long, default_value = "network")]
    pub style: String,

    /// Output path for statistics report PNG
    #[arg(long)]
    pub report_output: Option<PathBuf>,

    /// Output directory for waste visualization PNGs
    #[arg(long)]
    pub waste_output_dir: Option<PathBuf>,
}
