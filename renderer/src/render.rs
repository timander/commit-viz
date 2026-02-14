use crate::config::RenderConfig;
use crate::data::CollectedData;
use crate::timeline::{Layout, PositionedCommit, PositionedMerge};
use std::io::Write;
use std::process::{Command, Stdio};
use tiny_skia::{Color, Paint, PathBuilder, Pixmap, Stroke, Transform};

const COMMIT_RADIUS: f32 = 5.0;

/// Color palette for different lanes.
const LANE_COLORS: &[(u8, u8, u8)] = &[
    (66, 133, 244),  // blue
    (234, 67, 53),   // red
    (251, 188, 4),   // yellow
    (52, 168, 83),   // green
    (171, 71, 188),  // purple
    (255, 112, 67),  // orange
    (0, 172, 193),   // teal
    (124, 179, 66),  // lime
];

fn lane_color(lane: usize) -> Color {
    let (r, g, b) = LANE_COLORS[lane % LANE_COLORS.len()];
    Color::from_rgba8(r, g, b, 255)
}

/// Render a single frame showing all commits up to `progress` (0.0 to 1.0).
fn render_frame(
    layout: &Layout,
    positioned_commits: &[PositionedCommit],
    positioned_merges: &[PositionedMerge],
    progress: f32,
    width: u32,
    height: u32,
) -> Pixmap {
    let mut pixmap = Pixmap::new(width, height).unwrap();

    // Background
    pixmap.fill(Color::from_rgba8(30, 30, 30, 255));

    // Draw lane lines
    let mut lane_paint = Paint::default();
    lane_paint.set_color(Color::from_rgba8(60, 60, 60, 255));
    lane_paint.anti_alias = true;
    let lane_stroke = Stroke {
        width: 1.0,
        ..Stroke::default()
    };

    for (_, lane) in &layout.branch_lanes {
        let y = layout.margin + (*lane as f32) * layout.lane_height + layout.lane_height / 2.0;
        let mut pb = PathBuilder::new();
        pb.move_to(layout.margin, y);
        pb.line_to(width as f32 - layout.margin, y);
        if let Some(path) = pb.finish() {
            pixmap.stroke_path(&path, &lane_paint, &lane_stroke, Transform::identity(), None);
        }
    }

    // Cutoff X for animation progress
    let usable_width = width as f32 - 2.0 * layout.margin;
    let cutoff_x = layout.margin + progress * usable_width;

    // Draw merge lines
    let mut merge_paint = Paint::default();
    merge_paint.set_color(Color::from_rgba8(100, 100, 100, 200));
    merge_paint.anti_alias = true;
    let merge_stroke = Stroke {
        width: 1.5,
        ..Stroke::default()
    };

    for m in positioned_merges {
        if m.from_x > cutoff_x {
            continue;
        }
        let mut pb = PathBuilder::new();
        pb.move_to(m.from_x, m.from_y);
        pb.line_to(m.to_x, m.to_y);
        if let Some(path) = pb.finish() {
            pixmap.stroke_path(&path, &merge_paint, &merge_stroke, Transform::identity(), None);
        }
    }

    // Draw commits
    for pc in positioned_commits {
        if pc.x > cutoff_x {
            continue;
        }
        let mut paint = Paint::default();
        paint.set_color(lane_color(pc.lane));
        paint.anti_alias = true;

        let mut pb = PathBuilder::new();
        pb.push_circle(pc.x, pc.y, COMMIT_RADIUS);
        if let Some(path) = pb.finish() {
            pixmap.fill_path(&path, &paint, tiny_skia::FillRule::Winding, Transform::identity(), None);
        }
    }

    pixmap
}

/// Render all frames and pipe to FFmpeg.
pub fn render_video(
    data: &CollectedData,
    config: &RenderConfig,
) -> Result<(), Box<dyn std::error::Error>> {
    let layout = Layout::from_data(data, config.width, config.height);
    let positioned_commits = layout.position_commits(data);
    let positioned_merges = layout.position_merges(data);

    // Duration: 1 second per 10 commits, minimum 5 seconds
    let duration_secs = ((data.commits.len() as f32 / 10.0).ceil() as u32).max(5);
    let total_frames = duration_secs * config.fps;

    eprintln!(
        "Rendering {} frames ({} seconds at {} fps)...",
        total_frames, duration_secs, config.fps
    );

    let output_path = config.output.to_str().unwrap_or("output.mp4");

    let mut ffmpeg = Command::new("ffmpeg")
        .args([
            "-y",
            "-f", "rawvideo",
            "-pix_fmt", "rgba",
            "-s", &format!("{}x{}", config.width, config.height),
            "-r", &config.fps.to_string(),
            "-i", "-",
            "-c:v", "libx264",
            "-pix_fmt", "yuv420p",
            "-preset", "fast",
            output_path,
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;

    let stdin = ffmpeg.stdin.as_mut().expect("Failed to open FFmpeg stdin");

    for frame_idx in 0..total_frames {
        let progress = frame_idx as f32 / (total_frames - 1).max(1) as f32;
        let pixmap = render_frame(
            &layout,
            &positioned_commits,
            &positioned_merges,
            progress,
            config.width,
            config.height,
        );
        stdin.write_all(pixmap.data())?;

        if frame_idx % config.fps == 0 {
            eprint!("\r  Frame {}/{}", frame_idx + 1, total_frames);
        }
    }

    drop(ffmpeg.stdin.take());
    let status = ffmpeg.wait()?;
    eprintln!();

    if status.success() {
        eprintln!("Video written to {}", output_path);
    } else {
        return Err(format!("FFmpeg exited with status: {}", status).into());
    }

    Ok(())
}
