use crate::config::RenderConfig;
use crate::data::CollectedData;
use crate::layout::{
    BranchLabel, BranchVisualInfo, DateTick, NetworkLayout, PositionedCommit, PositionedMerge,
    PositionedTag,
};
use crate::stats::{precompute_frame_stats, FrameStats};
use crate::text::TextRenderer;
use rayon::prelude::*;
use std::io::Write;
use std::process::{Command, Stdio};
use tiny_skia::{Color, Paint, PathBuilder, Pixmap, Stroke, Transform};

// ── Sacred Timeline palette ─────────────────────────────────────────────────

fn sacred_gold() -> Color {
    Color::from_rgba8(255, 200, 60, 255)
}
fn sacred_gold_dim() -> Color {
    Color::from_rgba8(255, 200, 60, 80)
}
fn sacred_gold_glow() -> Color {
    Color::from_rgba8(255, 220, 100, 40)
}

fn category_color(category: &str) -> Color {
    match category {
        "feature" => Color::from_rgba8(66, 165, 245, 255),
        "bugfix" => Color::from_rgba8(239, 83, 80, 255),
        "release" => Color::from_rgba8(255, 215, 0, 255),
        "refactor" => Color::from_rgba8(186, 104, 200, 255),
        "docs" => Color::from_rgba8(129, 199, 132, 255),
        "ci" => Color::from_rgba8(77, 208, 225, 255),
        "test" => Color::from_rgba8(255, 167, 38, 255),
        "merge" => Color::from_rgba8(255, 200, 60, 200),
        "squash" => Color::from_rgba8(255, 183, 77, 255),
        "conflict" => Color::from_rgba8(244, 67, 54, 255),
        _ => Color::from_rgba8(158, 158, 158, 255),
    }
}

const LANE_COLORS: &[(u8, u8, u8)] = &[
    (77, 208, 225),  // cyan
    (239, 83, 80),   // coral
    (129, 199, 132), // green
    (186, 104, 200), // lavender
    (255, 167, 38),  // amber
    (66, 165, 245),  // blue
    (240, 98, 146),  // pink
    (174, 213, 129), // lime
];

/// Branch color based on slot and conflict/stale status.
fn branch_color(slot: usize, has_conflicts: bool, is_stale: bool) -> Color {
    if has_conflicts {
        Color::from_rgba8(244, 80, 60, 255)
    } else if is_stale {
        Color::from_rgba8(100, 100, 110, 100)
    } else {
        let (r, g, b) = LANE_COLORS[slot % LANE_COLORS.len()];
        Color::from_rgba8(r, g, b, 255)
    }
}

fn with_alpha(c: Color, a: f32) -> Color {
    Color::from_rgba(c.red(), c.green(), c.blue(), a).unwrap_or(c)
}

// ── Drawing helpers ─────────────────────────────────────────────────────────

fn fill_rounded_rect(
    pixmap: &mut Pixmap,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    r: f32,
    paint: &Paint,
) {
    let r = r.min(w / 2.0).min(h / 2.0);
    let mut pb = PathBuilder::new();
    pb.move_to(x + r, y);
    pb.line_to(x + w - r, y);
    pb.cubic_to(x + w, y, x + w, y, x + w, y + r);
    pb.line_to(x + w, y + h - r);
    pb.cubic_to(x + w, y + h, x + w, y + h, x + w - r, y + h);
    pb.line_to(x + r, y + h);
    pb.cubic_to(x, y + h, x, y + h, x, y + h - r);
    pb.line_to(x, y + r);
    pb.cubic_to(x, y, x, y, x + r, y);
    pb.close();
    if let Some(path) = pb.finish() {
        pixmap.fill_path(
            &path,
            paint,
            tiny_skia::FillRule::Winding,
            Transform::identity(),
            None,
        );
    }
}

fn stroke_rounded_rect(
    pixmap: &mut Pixmap,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    r: f32,
    paint: &Paint,
    stroke: &Stroke,
) {
    let r = r.min(w / 2.0).min(h / 2.0);
    let mut pb = PathBuilder::new();
    pb.move_to(x + r, y);
    pb.line_to(x + w - r, y);
    pb.cubic_to(x + w, y, x + w, y, x + w, y + r);
    pb.line_to(x + w, y + h - r);
    pb.cubic_to(x + w, y + h, x + w, y + h, x + w - r, y + h);
    pb.line_to(x + r, y + h);
    pb.cubic_to(x, y + h, x, y + h, x, y + h - r);
    pb.line_to(x, y + r);
    pb.cubic_to(x, y, x, y, x + r, y);
    pb.close();
    if let Some(path) = pb.finish() {
        pixmap.stroke_path(&path, paint, stroke, Transform::identity(), None);
    }
}

// ── Catmull-Rom spline helpers ──────────────────────────────────────────────

/// Convert a Catmull-Rom segment (p0, p1, p2, p3) into cubic Bezier control points.
/// Returns (cp1, cp2) where the Bezier goes from p1 to p2.
fn catmull_rom_to_cubic(
    p0: (f32, f32),
    p1: (f32, f32),
    p2: (f32, f32),
    p3: (f32, f32),
    alpha: f32,
) -> ((f32, f32), (f32, f32)) {
    let cp1 = (
        p1.0 + (p2.0 - p0.0) / (6.0 * alpha),
        p1.1 + (p2.1 - p0.1) / (6.0 * alpha),
    );
    let cp2 = (
        p2.0 - (p3.0 - p1.0) / (6.0 * alpha),
        p2.1 - (p3.1 - p1.1) / (6.0 * alpha),
    );
    (cp1, cp2)
}

/// Draw a Catmull-Rom spline through the given points.
fn draw_catmull_rom_spline(
    pixmap: &mut Pixmap,
    points: &[(f32, f32)],
    paint: &Paint,
    stroke: &Stroke,
) {
    if points.len() < 2 {
        return;
    }
    if points.len() == 2 {
        let mut pb = PathBuilder::new();
        pb.move_to(points[0].0, points[0].1);
        pb.line_to(points[1].0, points[1].1);
        if let Some(path) = pb.finish() {
            pixmap.stroke_path(&path, paint, stroke, Transform::identity(), None);
        }
        return;
    }

    let mut pb = PathBuilder::new();
    pb.move_to(points[0].0, points[0].1);

    let alpha = 1.0; // tension parameter

    for i in 0..points.len() - 1 {
        let p0 = if i == 0 { points[0] } else { points[i - 1] };
        let p1 = points[i];
        let p2 = points[i + 1];
        let p3 = if i + 2 < points.len() {
            points[i + 2]
        } else {
            points[i + 1]
        };

        let (cp1, cp2) = catmull_rom_to_cubic(p0, p1, p2, p3, alpha);
        pb.cubic_to(cp1.0, cp1.1, cp2.0, cp2.1, p2.0, p2.1);
    }

    if let Some(path) = pb.finish() {
        pixmap.stroke_path(&path, paint, stroke, Transform::identity(), None);
    }
}

// ── Legend ───────────────────────────────────────────────────────────────────

fn draw_legend(pixmap: &mut Pixmap, text_renderer: &TextRenderer, _width: u32, height: u32) {
    let legend_y = height as f32 - 95.0;
    let dim = Color::from_rgba8(160, 160, 170, 255);
    let bright = Color::from_rgba8(230, 230, 240, 255);

    text_renderer.draw_text(pixmap, "Commit Categories", 20.0, legend_y, 13.0, bright);

    let categories = [
        ("feature", "feature"),
        ("bugfix", "bugfix"),
        ("release", "release"),
        ("refactor", "refactor"),
        ("docs", "docs"),
        ("ci", "ci"),
        ("test", "test"),
        ("merge", "merge"),
        ("squash", "squash"),
        ("conflict", "conflict"),
        ("other", "other"),
    ];

    let mut x = 20.0;
    let row_y = legend_y + 20.0;

    for (label, cat) in &categories {
        let color = category_color(cat);
        let mut paint = Paint::default();
        paint.set_color(color);
        paint.anti_alias = true;

        fill_rounded_rect(pixmap, x, row_y - 8.0, 10.0, 10.0, 2.0, &paint);

        text_renderer.draw_text(pixmap, label, x + 14.0, row_y, 11.0, dim);
        x += 14.0 + text_renderer.measure_text(label, 11.0) + 14.0;
    }

    // Commit size legend
    let size_y = row_y + 20.0;
    text_renderer.draw_text(pixmap, "Commit size:", 20.0, size_y, 11.0, dim);

    let mut paint = Paint::default();
    paint.set_color(Color::from_rgba8(140, 140, 150, 200));
    paint.anti_alias = true;

    fill_rounded_rect(pixmap, 120.0, size_y - 6.0, 5.0, 5.0, 1.0, &paint);
    text_renderer.draw_text(pixmap, "few files, few lines", 130.0, size_y, 10.0, dim);

    fill_rounded_rect(pixmap, 290.0, size_y - 14.0, 18.0, 20.0, 2.0, &paint);
    text_renderer.draw_text(pixmap, "many files, many lines", 314.0, size_y, 10.0, dim);

    // Sacred timeline indicator
    let mut gold_paint = Paint::default();
    gold_paint.set_color(sacred_gold());
    gold_paint.anti_alias = true;
    let gold_stroke = Stroke {
        width: 3.0,
        ..Stroke::default()
    };
    let mut pb = PathBuilder::new();
    pb.move_to(520.0, size_y - 4.0);
    pb.line_to(560.0, size_y - 4.0);
    if let Some(path) = pb.finish() {
        pixmap.stroke_path(&path, &gold_paint, &gold_stroke, Transform::identity(), None);
    }
    text_renderer.draw_text(
        pixmap,
        "= Sacred Timeline    Branches bow below = divergence",
        566.0,
        size_y,
        10.0,
        dim,
    );
}

// ── Date axis ───────────────────────────────────────────────────────────────

fn draw_date_axis(pixmap: &mut Pixmap, text_renderer: &TextRenderer, ticks: &[DateTick]) {
    let tick_color = Color::from_rgba8(80, 80, 90, 255);
    let label_color = Color::from_rgba8(150, 150, 160, 255);

    let mut tick_paint = Paint::default();
    tick_paint.set_color(tick_color);
    let tick_stroke = Stroke {
        width: 1.0,
        ..Stroke::default()
    };

    let step = if ticks.len() > 30 {
        ticks.len() / 20
    } else {
        1
    };

    for (i, tick) in ticks.iter().enumerate() {
        if i % step != 0 {
            continue;
        }
        let mut pb = PathBuilder::new();
        pb.move_to(tick.x, 50.0);
        pb.line_to(tick.x, 62.0);
        if let Some(path) = pb.finish() {
            pixmap.stroke_path(&path, &tick_paint, &tick_stroke, Transform::identity(), None);
        }
        text_renderer.draw_text(pixmap, &tick.label, tick.x - 20.0, 48.0, 10.0, label_color);
    }
}

// ── Branch labels ───────────────────────────────────────────────────────────

fn draw_branch_labels(
    pixmap: &mut Pixmap,
    text_renderer: &TextRenderer,
    labels: &[BranchLabel],
    visible_x_limit: f32,
) {
    for bl in labels {
        if bl.x > visible_x_limit {
            continue;
        }

        let color = with_alpha(
            branch_color(bl.slot, bl.has_conflicts, bl.is_stale),
            0.9,
        );

        let label_x = (bl.x - 4.0).max(4.0);
        let label_y = bl.y - 14.0;

        text_renderer.draw_text(pixmap, &bl.name, label_x, label_y, 10.0, color);
    }
}

// ── Sacred Timeline line (main branch) ──────────────────────────────────────

fn draw_sacred_timeline(pixmap: &mut Pixmap, layout: &NetworkLayout, width: u32) {
    let main_y = layout.main_y;

    // Outer glow (wide, dim)
    let mut glow_paint = Paint::default();
    glow_paint.set_color(sacred_gold_glow());
    glow_paint.anti_alias = true;
    let glow_stroke = Stroke {
        width: 12.0,
        ..Stroke::default()
    };

    let mut pb = PathBuilder::new();
    pb.move_to(layout.margin_left, main_y);
    pb.line_to(width as f32 - layout.margin_right, main_y);
    if let Some(path) = pb.finish() {
        pixmap.stroke_path(&path, &glow_paint, &glow_stroke, Transform::identity(), None);
    }

    // Core line (bright gold)
    let mut core_paint = Paint::default();
    core_paint.set_color(sacred_gold_dim());
    core_paint.anti_alias = true;
    let core_stroke = Stroke {
        width: 3.0,
        ..Stroke::default()
    };

    let mut pb = PathBuilder::new();
    pb.move_to(layout.margin_left, main_y);
    pb.line_to(width as f32 - layout.margin_right, main_y);
    if let Some(path) = pb.finish() {
        pixmap.stroke_path(&path, &core_paint, &core_stroke, Transform::identity(), None);
    }
}

// ── Tags (vertical markers above main) ──────────────────────────────────────

fn draw_tags(
    pixmap: &mut Pixmap,
    text_renderer: &TextRenderer,
    tags: &[PositionedTag],
    visible_x_limit: f32,
) {
    let gold = sacred_gold();

    for tag in tags {
        if tag.x > visible_x_limit {
            continue;
        }

        // Vertical gold line from main_y upward
        let mut paint = Paint::default();
        paint.set_color(with_alpha(gold, 0.6));
        paint.anti_alias = true;
        let stroke = Stroke {
            width: 1.5,
            ..Stroke::default()
        };

        let mut pb = PathBuilder::new();
        pb.move_to(tag.x, tag.main_y);
        pb.line_to(tag.x, tag.label_y + 8.0);
        if let Some(path) = pb.finish() {
            pixmap.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
        }

        // Diamond marker at top
        let mut diamond_paint = Paint::default();
        diamond_paint.set_color(with_alpha(gold, 0.9));
        diamond_paint.anti_alias = true;
        let d = 5.0;
        let dy = tag.label_y + 8.0;
        let mut pb = PathBuilder::new();
        pb.move_to(tag.x, dy - d);
        pb.line_to(tag.x + d, dy);
        pb.line_to(tag.x, dy + d);
        pb.line_to(tag.x - d, dy);
        pb.close();
        if let Some(path) = pb.finish() {
            pixmap.fill_path(
                &path,
                &diamond_paint,
                tiny_skia::FillRule::Winding,
                Transform::identity(),
                None,
            );
        }

        // Tag name label
        let label_color = with_alpha(gold, 0.8);
        text_renderer.draw_text(
            pixmap,
            &tag.tag_name,
            tag.x - 20.0,
            tag.label_y,
            9.0,
            label_color,
        );
    }
}

// ── Stats overlay ───────────────────────────────────────────────────────────

fn threshold_color(value: f64, yellow_threshold: f64, red_threshold: f64) -> Color {
    if value > red_threshold {
        Color::from_rgba8(239, 83, 80, 255) // red
    } else if value > yellow_threshold {
        Color::from_rgba8(255, 200, 60, 255) // yellow/gold
    } else {
        Color::from_rgba8(129, 199, 132, 255) // green
    }
}

fn draw_stats_overlay(
    pixmap: &mut Pixmap,
    text_renderer: &TextRenderer,
    stats: &FrameStats,
    width: u32,
) {
    let panel_w: f32 = 320.0;
    let panel_h: f32 = 300.0;
    let panel_x = width as f32 - panel_w - 20.0;
    let panel_y: f32 = 60.0;

    // Semi-transparent background
    let mut bg_paint = Paint::default();
    bg_paint.set_color(Color::from_rgba8(18, 18, 24, 200));
    bg_paint.anti_alias = true;
    fill_rounded_rect(pixmap, panel_x, panel_y, panel_w, panel_h, 6.0, &bg_paint);

    // Gold border
    let mut border_paint = Paint::default();
    border_paint.set_color(with_alpha(sacred_gold(), 0.5));
    border_paint.anti_alias = true;
    let border_stroke = Stroke {
        width: 1.5,
        ..Stroke::default()
    };
    stroke_rounded_rect(
        pixmap,
        panel_x,
        panel_y,
        panel_w,
        panel_h,
        6.0,
        &border_paint,
        &border_stroke,
    );

    // Title
    let title_color = with_alpha(sacred_gold(), 0.9);
    let dim = Color::from_rgba8(160, 160, 170, 255);
    text_renderer.draw_text(
        pixmap,
        "Code Inventory",
        panel_x + 12.0,
        panel_y + 20.0,
        13.0,
        title_color,
    );

    let mut y = panel_y + 40.0;
    let label_x = panel_x + 12.0;
    let value_x = panel_x + 210.0;
    let line_h = 24.0;

    // Helper closure-like macro for drawing a stat line
    let lines: Vec<(&str, String, Color)> = vec![
        (
            "Unmerged commits",
            format!("{}", stats.unmerged_commits),
            threshold_color(stats.unmerged_commits as f64, 10.0, 50.0),
        ),
        (
            "Active branches",
            format!("{}", stats.active_branches),
            dim,
        ),
        (
            "Stale branches",
            format!("{}", stats.stale_branches),
            threshold_color(stats.stale_branches as f64, 2.0, 5.0),
        ),
        (
            "Unmerged lines",
            format_number(stats.unmerged_lines),
            threshold_color(stats.unmerged_lines as f64, 1000.0, 5000.0),
        ),
        (
            "Unmerged files",
            format!("{}", stats.unmerged_files),
            dim,
        ),
        (
            "Integration debt",
            format_number(stats.integration_debt),
            threshold_color(stats.integration_debt as f64, 10000.0, 50000.0),
        ),
        (
            "Days since release",
            format!("{:.0}", stats.days_since_release),
            threshold_color(stats.days_since_release, 14.0, 30.0),
        ),
        (
            "Awaiting release",
            format!("{}", stats.awaiting_release),
            dim,
        ),
        (
            "Oldest unmerged",
            format!("{:.0}d", stats.oldest_unmerged_days),
            threshold_color(stats.oldest_unmerged_days, 14.0, 30.0),
        ),
        (
            "Merge throughput",
            format!("{}/30d", stats.merge_throughput),
            dim,
        ),
    ];

    for (label, value, color) in &lines {
        text_renderer.draw_text(pixmap, label, label_x, y, 11.0, dim);
        text_renderer.draw_text(pixmap, value, value_x, y, 11.0, *color);
        y += line_h;
    }
}

fn format_number(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.1}K", n as f64 / 1_000.0)
    } else {
        format!("{}", n)
    }
}

// ── Title bar ───────────────────────────────────────────────────────────────

fn draw_title(pixmap: &mut Pixmap, text_renderer: &TextRenderer, data: &CollectedData) {
    let bright = Color::from_rgba8(230, 230, 240, 255);
    let dim = Color::from_rgba8(140, 140, 150, 255);

    let repo_name = &data.metadata.repo;
    let short_name = if repo_name.contains("github.com") {
        repo_name.rsplit("github.com/").next().unwrap_or(repo_name)
    } else {
        repo_name
    };

    text_renderer.draw_text(pixmap, short_name, 20.0, 28.0, 18.0, bright);

    let stats = format!(
        "{} commits | {} branches | {} merges",
        data.commits.len(),
        data.branches.len(),
        data.merges.len()
    );
    let stats_x = 20.0 + text_renderer.measure_text(short_name, 18.0) + 20.0;
    text_renderer.draw_text(pixmap, &stats, stats_x, 28.0, 12.0, dim);
}

// ── Main frame render ───────────────────────────────────────────────────────

fn render_frame(
    layout: &NetworkLayout,
    positioned_commits: &[PositionedCommit],
    positioned_merges: &[PositionedMerge],
    branch_labels: &[BranchLabel],
    date_ticks: &[DateTick],
    positioned_tags: &[PositionedTag],
    _branch_infos: &[BranchVisualInfo],
    frame_stats: Option<&FrameStats>,
    text_renderer: &TextRenderer,
    data: &CollectedData,
    visible_count: usize,
    width: u32,
    height: u32,
) -> Pixmap {
    let mut pixmap = Pixmap::new(width, height).unwrap();
    pixmap.fill(Color::from_rgba8(18, 18, 24, 255));

    // Title
    draw_title(&mut pixmap, text_renderer, data);

    // Date axis
    draw_date_axis(&mut pixmap, text_renderer, date_ticks);

    // Sacred Timeline (golden main branch line)
    draw_sacred_timeline(&mut pixmap, layout, width);

    let visible = &positioned_commits[..visible_count.min(positioned_commits.len())];
    let visible_x_limit = visible.last().map_or(0.0, |c| c.x);

    // Tags above main
    draw_tags(&mut pixmap, text_renderer, positioned_tags, visible_x_limit);

    // Branch labels
    draw_branch_labels(&mut pixmap, text_renderer, branch_labels, visible_x_limit);

    // ── Draw branch splines (Catmull-Rom through branch commit positions) ───

    let mut branch_commits: std::collections::HashMap<&str, Vec<usize>> =
        std::collections::HashMap::new();
    for (i, pc) in visible.iter().enumerate() {
        branch_commits
            .entry(&pc.commit.branch)
            .or_default()
            .push(i);
    }

    for (branch_name, indices) in &branch_commits {
        if indices.len() < 2 {
            continue;
        }

        let is_default = *branch_name == layout.default_branch;
        let first_pc = &visible[indices[0]];

        let mut paint = Paint::default();
        if is_default {
            paint.set_color(with_alpha(sacred_gold(), 0.5));
        } else {
            let bc = branch_color(
                first_pc.slot,
                first_pc.branch_has_conflicts,
                first_pc.branch_is_stale,
            );
            paint.set_color(with_alpha(bc, 0.45));
        }
        paint.anti_alias = true;

        let stroke = Stroke {
            width: if is_default { 2.5 } else { 1.5 },
            ..Stroke::default()
        };

        // Collect points for spline
        let points: Vec<(f32, f32)> = indices.iter().map(|&i| (visible[i].x, visible[i].y)).collect();

        if is_default {
            // Default branch is straight — just draw line segments
            let mut pb = PathBuilder::new();
            pb.move_to(points[0].0, points[0].1);
            for p in &points[1..] {
                pb.line_to(p.0, p.1);
            }
            if let Some(path) = pb.finish() {
                pixmap.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
            }
        } else {
            // For non-default branches, prepend a point at main_y (departure)
            // and if merged, append a return point at main_y
            let mut spline_points = Vec::with_capacity(points.len() + 2);

            // Departure from main
            spline_points.push((points[0].0, layout.main_y));
            spline_points.extend_from_slice(&points);

            draw_catmull_rom_spline(&mut pixmap, &spline_points, &paint, &stroke);
        }
    }

    // ── Draw merge curves (variant → sacred timeline) ───────────────────────

    let merge_stroke = Stroke {
        width: 2.0,
        ..Stroke::default()
    };

    for m in positioned_merges {
        if m.to_x > visible_x_limit {
            continue;
        }

        let bc = branch_color(m.slot, m.has_conflicts, m.is_stale);
        let mut paint = Paint::default();
        paint.set_color(with_alpha(bc, 0.6));
        paint.anti_alias = true;

        let mut pb = PathBuilder::new();
        pb.move_to(m.from_x, m.from_y);
        let mid_x = (m.from_x + m.to_x) / 2.0;
        pb.cubic_to(mid_x, m.from_y, mid_x, m.to_y, m.to_x, m.to_y);

        if let Some(path) = pb.finish() {
            pixmap.stroke_path(&path, &paint, &merge_stroke, Transform::identity(), None);
        }

        // Merge point indicator: small diamond at the merge destination
        let mut merge_paint = Paint::default();
        merge_paint.set_color(with_alpha(sacred_gold(), 0.8));
        merge_paint.anti_alias = true;
        let d = 4.0;
        let mut pb = PathBuilder::new();
        pb.move_to(m.to_x, m.to_y - d);
        pb.line_to(m.to_x + d, m.to_y);
        pb.line_to(m.to_x, m.to_y + d);
        pb.line_to(m.to_x - d, m.to_y);
        pb.close();
        if let Some(path) = pb.finish() {
            pixmap.fill_path(
                &path,
                &merge_paint,
                tiny_skia::FillRule::Winding,
                Transform::identity(),
                None,
            );
        }
    }

    // ── Draw commits as sized rectangles ────────────────────────────────────

    for pc in visible {
        let color = category_color(&pc.commit.category);
        let half_w = pc.rect_w / 2.0;
        let half_h = pc.rect_h / 2.0;

        // Fill
        let mut paint = Paint::default();
        paint.set_color(with_alpha(color, 0.85));
        paint.anti_alias = true;
        fill_rounded_rect(
            &mut pixmap,
            pc.x - half_w,
            pc.y - half_h,
            pc.rect_w,
            pc.rect_h,
            2.0,
            &paint,
        );

        // Border for default branch commits (golden outline)
        if pc.is_default_branch {
            let mut border_paint = Paint::default();
            border_paint.set_color(with_alpha(sacred_gold(), 0.6));
            border_paint.anti_alias = true;
            let border_stroke = Stroke {
                width: 1.0,
                ..Stroke::default()
            };
            stroke_rounded_rect(
                &mut pixmap,
                pc.x - half_w,
                pc.y - half_h,
                pc.rect_w,
                pc.rect_h,
                2.0,
                &border_paint,
                &border_stroke,
            );
        }

        // Tag indicator: gold ring around tagged commits
        if !pc.commit.tags.is_empty() {
            let mut tag_paint = Paint::default();
            tag_paint.set_color(Color::from_rgba8(255, 215, 0, 220));
            tag_paint.anti_alias = true;
            let tag_stroke = Stroke {
                width: 2.0,
                ..Stroke::default()
            };
            stroke_rounded_rect(
                &mut pixmap,
                pc.x - half_w - 3.0,
                pc.y - half_h - 3.0,
                pc.rect_w + 6.0,
                pc.rect_h + 6.0,
                3.0,
                &tag_paint,
                &tag_stroke,
            );
        }
    }

    // Stats overlay
    if let Some(fs) = frame_stats {
        draw_stats_overlay(&mut pixmap, text_renderer, fs, width);
    }

    // Legend
    draw_legend(&mut pixmap, text_renderer, width, height);

    pixmap
}

// ── Video rendering ─────────────────────────────────────────────────────────

pub fn render_video(
    data: &CollectedData,
    config: &RenderConfig,
) -> Result<(), Box<dyn std::error::Error>> {
    let layout = NetworkLayout::from_data(data, config.width, config.height);
    let (positioned_commits, branch_infos) = layout.position_commits_dynamic(data);
    let positioned_merges = layout.position_merges_dynamic(data, &positioned_commits);
    let branch_labels = layout.compute_branch_labels(&positioned_commits);
    let date_ticks = layout.compute_date_ticks(data);
    let positioned_tags = layout.position_tags(&positioned_commits);
    let text_renderer = TextRenderer::new();

    let num_commits = data.commits.len();
    if num_commits == 0 {
        return Err("No commits to render".into());
    }

    // Pre-compute frame stats for the overlay
    let frame_stats = precompute_frame_stats(data, &layout.default_branch);

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
            "-f",
            "rawvideo",
            "-pix_fmt",
            "rgba",
            "-s",
            &format!("{}x{}", config.width, config.height),
            "-r",
            &config.fps.to_string(),
            "-i",
            "-",
            "-c:v",
            "libx264",
            "-pix_fmt",
            "yuv420p",
            "-preset",
            "fast",
            output_path,
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;

    let stdin = ffmpeg.stdin.as_mut().expect("Failed to open FFmpeg stdin");

    // Render frames in parallel batches
    let batch_size = rayon::current_num_threads() * 2;
    let mut frame_idx = 0u32;

    while frame_idx < total_frames {
        let batch_end = (frame_idx + batch_size as u32).min(total_frames);
        let indices: Vec<u32> = (frame_idx..batch_end).collect();

        let frames: Vec<Pixmap> = indices
            .par_iter()
            .map(|&idx| {
                let progress = (idx + 1) as f32 / total_frames as f32;
                let visible_count =
                    ((progress * num_commits as f32).ceil() as usize).min(num_commits);
                let tr = TextRenderer::new();

                // Get the frame stats for this visible_count (1-indexed to 0-indexed)
                let fs = if visible_count > 0 {
                    frame_stats.get(visible_count - 1)
                } else {
                    None
                };

                render_frame(
                    &layout,
                    &positioned_commits,
                    &positioned_merges,
                    &branch_labels,
                    &date_ticks,
                    &positioned_tags,
                    &branch_infos,
                    fs,
                    &tr,
                    data,
                    visible_count,
                    config.width,
                    config.height,
                )
            })
            .collect();

        for pixmap in &frames {
            stdin.write_all(pixmap.data())?;
        }

        if frame_idx % config.fps == 0 || batch_end == total_frames {
            eprint!("\r  Frame {}/{}", batch_end, total_frames);
        }

        frame_idx = batch_end;
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
