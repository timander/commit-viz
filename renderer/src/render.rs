use crate::config::RenderConfig;
use crate::data::CollectedData;
use crate::layout::{NetworkLayout, PositionedCommit, PositionedMerge, DateTick};
use crate::text::TextRenderer;
use std::io::Write;
use std::process::{Command, Stdio};
use tiny_skia::{Color, Paint, PathBuilder, Pixmap, Stroke, Transform};

/// Category → color mapping
fn category_color(category: &str) -> Color {
    match category {
        "feature" => Color::from_rgba8(66, 133, 244, 255),   // blue
        "bugfix" => Color::from_rgba8(234, 67, 53, 255),     // red
        "release" => Color::from_rgba8(255, 215, 0, 255),    // gold
        "refactor" => Color::from_rgba8(171, 71, 188, 255),  // purple
        "docs" => Color::from_rgba8(124, 179, 66, 255),      // lime
        "ci" => Color::from_rgba8(0, 172, 193, 255),         // teal
        "test" => Color::from_rgba8(255, 152, 0, 255),       // orange
        _ => Color::from_rgba8(158, 158, 158, 255),          // gray
    }
}

/// Lane colors for branch lines/merge curves
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

fn draw_legend(pixmap: &mut Pixmap, text_renderer: &TextRenderer, _width: u32, height: u32) {
    let legend_y = height as f32 - 100.0;
    let label_color = Color::from_rgba8(200, 200, 200, 255);
    let title_color = Color::from_rgba8(255, 255, 255, 255);

    text_renderer.draw_text(pixmap, "Legend", 20.0, legend_y, 16.0, title_color);

    let categories = [
        ("feature", "feature"),
        ("bugfix", "bugfix"),
        ("release", "release"),
        ("refactor", "refactor"),
        ("docs", "docs"),
        ("ci", "ci"),
        ("test", "test"),
        ("other", "other"),
    ];

    let mut x = 20.0;
    let row_y = legend_y + 22.0;

    for (label, cat) in &categories {
        let color = category_color(cat);

        // Draw color dot
        let mut paint = Paint::default();
        paint.set_color(color);
        paint.anti_alias = true;
        let mut pb = PathBuilder::new();
        pb.push_circle(x + 5.0, row_y - 4.0, 5.0);
        if let Some(path) = pb.finish() {
            pixmap.fill_path(
                &path,
                &paint,
                tiny_skia::FillRule::Winding,
                Transform::identity(),
                None,
            );
        }

        text_renderer.draw_text(pixmap, label, x + 14.0, row_y, 12.0, label_color);
        x += 14.0 + text_renderer.measure_text(label, 12.0) + 16.0;
    }

    // Dot size legend
    let size_y = row_y + 22.0;
    text_renderer.draw_text(pixmap, "Dot size = change volume (log scale)", 20.0, size_y, 11.0, label_color);

    // Draw example dots
    let mut ex_x = 300.0;
    for (label, radius) in &[("small", 3.0f32), ("medium", 7.0f32), ("large", 12.0f32)] {
        let mut paint = Paint::default();
        paint.set_color(Color::from_rgba8(200, 200, 200, 180));
        paint.anti_alias = true;
        let mut pb = PathBuilder::new();
        pb.push_circle(ex_x, size_y - 4.0, *radius);
        if let Some(path) = pb.finish() {
            pixmap.fill_path(
                &path,
                &paint,
                tiny_skia::FillRule::Winding,
                Transform::identity(),
                None,
            );
        }
        text_renderer.draw_text(
            pixmap,
            label,
            ex_x + radius + 4.0,
            size_y,
            11.0,
            label_color,
        );
        ex_x += radius + 4.0 + text_renderer.measure_text(label, 11.0) + 20.0;
    }
}

fn draw_date_axis(
    pixmap: &mut Pixmap,
    text_renderer: &TextRenderer,
    ticks: &[DateTick],
) {
    let tick_color = Color::from_rgba8(120, 120, 120, 255);
    let label_color = Color::from_rgba8(180, 180, 180, 255);

    let mut tick_paint = Paint::default();
    tick_paint.set_color(tick_color);
    let tick_stroke = Stroke {
        width: 1.0,
        ..Stroke::default()
    };

    // Only draw a subset of ticks if too many
    let step = if ticks.len() > 30 { ticks.len() / 20 } else { 1 };

    for (i, tick) in ticks.iter().enumerate() {
        if i % step != 0 {
            continue;
        }
        // Tick line
        let mut pb = PathBuilder::new();
        pb.move_to(tick.x, 40.0);
        pb.line_to(tick.x, 55.0);
        if let Some(path) = pb.finish() {
            pixmap.stroke_path(&path, &tick_paint, &tick_stroke, Transform::identity(), None);
        }

        text_renderer.draw_text(pixmap, &tick.label, tick.x - 20.0, 38.0, 10.0, label_color);
    }
}

/// Render a single frame showing commits up to index `visible_count`.
fn render_frame(
    layout: &NetworkLayout,
    positioned_commits: &[PositionedCommit],
    positioned_merges: &[PositionedMerge],
    date_ticks: &[DateTick],
    text_renderer: &TextRenderer,
    visible_count: usize,
    width: u32,
    height: u32,
) -> Pixmap {
    let mut pixmap = Pixmap::new(width, height).unwrap();
    pixmap.fill(Color::from_rgba8(25, 25, 30, 255));

    // Draw date axis
    draw_date_axis(&mut pixmap, text_renderer, date_ticks);

    // Draw lane lines (subtle horizontal guides)
    let mut lane_paint = Paint::default();
    lane_paint.set_color(Color::from_rgba8(45, 45, 50, 255));
    lane_paint.anti_alias = true;
    let lane_stroke = Stroke {
        width: 1.0,
        ..Stroke::default()
    };

    for lane in 0..layout.total_lanes {
        let y = layout.margin_top + (lane as f32 + 0.5) * layout.lane_height;
        let mut pb = PathBuilder::new();
        pb.move_to(layout.margin_left, y);
        pb.line_to(width as f32 - layout.margin_right, y);
        if let Some(path) = pb.finish() {
            pixmap.stroke_path(&path, &lane_paint, &lane_stroke, Transform::identity(), None);
        }
    }

    // Draw parent edges as bezier curves between consecutive commits on same branch
    let edge_stroke = Stroke {
        width: 1.5,
        ..Stroke::default()
    };

    // Connect consecutive commits on the same branch
    let visible = &positioned_commits[..visible_count.min(positioned_commits.len())];
    for i in 1..visible.len() {
        let curr = &visible[i];
        let prev = &visible[i - 1];

        // Check if this commit has the previous as a parent
        let is_parent = curr.commit.parents.contains(&prev.commit.sha);

        if is_parent || curr.commit.branch == prev.commit.branch {
            let mut paint = Paint::default();
            let color = lane_color(curr.lane);
            paint.set_color(Color::from_rgba(color.red(), color.green(), color.blue(), 0.6).unwrap());
            paint.anti_alias = true;

            let mut pb = PathBuilder::new();
            pb.move_to(prev.x, prev.y);

            if (curr.y - prev.y).abs() < 1.0 {
                // Same lane — straight line
                pb.line_to(curr.x, curr.y);
            } else {
                // Different lanes — bezier curve
                let mid_x = (prev.x + curr.x) / 2.0;
                pb.cubic_to(mid_x, prev.y, mid_x, curr.y, curr.x, curr.y);
            }

            if let Some(path) = pb.finish() {
                pixmap.stroke_path(&path, &paint, &edge_stroke, Transform::identity(), None);
            }
        }
    }

    // Draw merge curves
    let merge_stroke = Stroke {
        width: 2.0,
        ..Stroke::default()
    };

    for m in positioned_merges {
        if m.to_x > visible.last().map_or(0.0, |c| c.x) {
            continue;
        }

        let mut paint = Paint::default();
        paint.set_color({let c = lane_color(m.lane); Color::from_rgba(c.red(), c.green(), c.blue(), 0.7).unwrap()});
        paint.anti_alias = true;

        let mut pb = PathBuilder::new();
        pb.move_to(m.from_x, m.from_y);
        let mid_x = (m.from_x + m.to_x) / 2.0;
        pb.cubic_to(mid_x, m.from_y, mid_x, m.to_y, m.to_x, m.to_y);

        if let Some(path) = pb.finish() {
            pixmap.stroke_path(&path, &paint, &merge_stroke, Transform::identity(), None);
        }
    }

    // Draw commits as colored circles
    for pc in visible {
        let mut paint = Paint::default();
        paint.set_color(category_color(&pc.commit.category));
        paint.anti_alias = true;

        let mut pb = PathBuilder::new();
        pb.push_circle(pc.x, pc.y, pc.radius);
        if let Some(path) = pb.finish() {
            pixmap.fill_path(
                &path,
                &paint,
                tiny_skia::FillRule::Winding,
                Transform::identity(),
                None,
            );
        }

        // Draw tag indicator (gold ring) for tagged commits
        if !pc.commit.tags.is_empty() {
            let mut tag_paint = Paint::default();
            tag_paint.set_color(Color::from_rgba8(255, 215, 0, 200));
            tag_paint.anti_alias = true;
            let tag_stroke = Stroke {
                width: 2.0,
                ..Stroke::default()
            };
            let mut pb = PathBuilder::new();
            pb.push_circle(pc.x, pc.y, pc.radius + 3.0);
            if let Some(path) = pb.finish() {
                pixmap.stroke_path(
                    &path,
                    &tag_paint,
                    &tag_stroke,
                    Transform::identity(),
                    None,
                );
            }
        }
    }

    // Draw legend
    draw_legend(&mut pixmap, text_renderer, width, height);

    pixmap
}

/// Render video: 1 frame per commit (accumulating), fps computed from target duration.
pub fn render_video(
    data: &CollectedData,
    config: &RenderConfig,
) -> Result<(), Box<dyn std::error::Error>> {
    let layout = NetworkLayout::from_data(data, config.width, config.height);
    let positioned_commits = layout.position_commits(data);
    let positioned_merges = layout.position_merges(data);
    let date_ticks = layout.compute_date_ticks(data);
    let text_renderer = TextRenderer::new();

    let num_commits = data.commits.len();
    if num_commits == 0 {
        return Err("No commits to render".into());
    }

    // Calculate duration and fps mapping
    let duration_secs = config.duration_secs.unwrap_or_else(|| {
        ((num_commits as f32 / 10.0).ceil() as u32).max(5)
    });
    let total_frames = duration_secs * config.fps;

    eprintln!(
        "Rendering {} commits over {} frames ({} seconds at {} fps)...",
        num_commits, total_frames, duration_secs, config.fps
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
        // Map frame index to number of visible commits
        let progress = (frame_idx + 1) as f32 / total_frames as f32;
        let visible_count = ((progress * num_commits as f32).ceil() as usize).min(num_commits);

        let pixmap = render_frame(
            &layout,
            &positioned_commits,
            &positioned_merges,
            &date_ticks,
            &text_renderer,
            visible_count,
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
