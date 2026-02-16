use crate::data::ChangeFlowMetrics;
use crate::text::TextRenderer;
use rayon::prelude::*;
use std::path::Path;
use tiny_skia::{Color, FillRule, Paint, PathBuilder, Pixmap, Stroke, Transform};

const WIDTH: u32 = 1920;
const HEIGHT: u32 = 1080;
fn bg() -> Color {
    Color::from_rgba8(18, 18, 24, 255)
}

fn white() -> Color {
    Color::from_rgba8(230, 230, 240, 255)
}
fn light() -> Color {
    Color::from_rgba8(190, 190, 200, 255)
}
fn dim() -> Color {
    Color::from_rgba8(130, 130, 145, 255)
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

/// Green→yellow→red gradient. t: 0.0=green, 0.5=yellow, 1.0=red
fn heat_color(t: f32) -> Color {
    let t = t.clamp(0.0, 1.0);
    let r;
    let g;
    if t < 0.5 {
        let s = t * 2.0;
        r = (s * 255.0) as u8;
        g = 255;
    } else {
        let s = (t - 0.5) * 2.0;
        r = 255;
        g = ((1.0 - s) * 255.0) as u8;
    }
    Color::from_rgba8(r, g, 40, 255)
}

fn magenta() -> Color {
    Color::from_rgba8(255, 0, 200, 255)
}

fn fill_rect(pixmap: &mut Pixmap, x: f32, y: f32, w: f32, h: f32, color: Color) {
    let mut paint = Paint::default();
    paint.set_color(color);
    let mut pb = PathBuilder::new();
    pb.move_to(x, y);
    pb.line_to(x + w, y);
    pb.line_to(x + w, y + h);
    pb.line_to(x, y + h);
    pb.close();
    if let Some(path) = pb.finish() {
        pixmap.fill_path(&path, &paint, FillRule::Winding, Transform::identity(), None);
    }
}

fn fill_rect_alpha(pixmap: &mut Pixmap, x: f32, y: f32, w: f32, h: f32, color: Color, alpha: f32) {
    if let Some(c) = Color::from_rgba(color.red(), color.green(), color.blue(), alpha) {
        fill_rect(pixmap, x, y, w, h, c);
    }
}

fn draw_line(pixmap: &mut Pixmap, x1: f32, y1: f32, x2: f32, y2: f32, color: Color, width: f32) {
    let mut paint = Paint::default();
    paint.set_color(color);
    paint.anti_alias = true;
    let stroke = Stroke { width, ..Stroke::default() };
    let mut pb = PathBuilder::new();
    pb.move_to(x1, y1);
    pb.line_to(x2, y2);
    if let Some(path) = pb.finish() {
        pixmap.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
    }
}

fn draw_dashed_line(pixmap: &mut Pixmap, x1: f32, y1: f32, x2: f32, y2: f32, color: Color, width: f32, dash_len: f32) {
    let dx = x2 - x1;
    let dy = y2 - y1;
    let len = (dx * dx + dy * dy).sqrt();
    if len < 1.0 {
        return;
    }
    let nx = dx / len;
    let ny = dy / len;
    let mut pos = 0.0;
    let mut drawing = true;
    while pos < len {
        let seg = dash_len.min(len - pos);
        if drawing {
            let sx = x1 + nx * pos;
            let sy = y1 + ny * pos;
            let ex = x1 + nx * (pos + seg);
            let ey = y1 + ny * (pos + seg);
            draw_line(pixmap, sx, sy, ex, ey, color, width);
        }
        pos += seg;
        drawing = !drawing;
    }
}

fn fill_circle(pixmap: &mut Pixmap, cx: f32, cy: f32, r: f32, color: Color) {
    let mut paint = Paint::default();
    paint.set_color(color);
    paint.anti_alias = true;
    let mut pb = PathBuilder::new();
    pb.push_circle(cx, cy, r);
    if let Some(path) = pb.finish() {
        pixmap.fill_path(&path, &paint, FillRule::Winding, Transform::identity(), None);
    }
}

fn draw_hatched_rect(pixmap: &mut Pixmap, x: f32, y: f32, w: f32, h: f32, color: Color) {
    fill_rect(pixmap, x, y, w, h, color);
    // Draw diagonal hatch lines
    let hatch_color = Color::from_rgba8(0, 0, 0, 120);
    let spacing = 6.0;
    let mut offset = 0.0;
    while offset < w + h {
        let x1 = x + (offset - h).max(0.0);
        let y1 = y + (h - (offset - (offset - h).max(0.0))).max(0.0);
        let x2 = x + offset.min(w);
        let y2 = y + (offset - offset.min(w)).max(0.0);
        draw_line(pixmap, x1, y1, x2, y2, hatch_color, 1.0);
        offset += spacing;
    }
}

/// Parse "YYYY-MM-DD" to (year, month, day)
fn parse_date(s: &str) -> (i32, u32, u32) {
    let parts: Vec<&str> = s.split('-').collect();
    if parts.len() >= 3 {
        let y = parts[0].parse().unwrap_or(2020);
        let m = parts[1].parse().unwrap_or(1);
        let d = parts[2].parse().unwrap_or(1);
        (y, m, d)
    } else {
        (2020, 1, 1)
    }
}

fn month_name(m: u32) -> &'static str {
    match m {
        1 => "Jan", 2 => "Feb", 3 => "Mar", 4 => "Apr",
        5 => "May", 6 => "Jun", 7 => "Jul", 8 => "Aug",
        9 => "Sep", 10 => "Oct", 11 => "Nov", 12 => "Dec",
        _ => "?",
    }
}

fn save_chart(pixmap: &Pixmap, dir: &Path, name: &str) -> Result<(), Box<dyn std::error::Error>> {
    let path = dir.join(name);
    pixmap.save_png(&path)?;
    eprintln!("  Wrote {:?}", path);
    Ok(())
}

// ============================================================
// Chart 1: Commit-to-Release Heatmap (calendar grid)
// ============================================================
pub fn render_commit_to_release_heatmap(
    wm: &ChangeFlowMetrics,
    text: &TextRenderer,
    dir: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut pixmap = Pixmap::new(WIDTH, HEIGHT).unwrap();
    pixmap.fill(bg());

    text.draw_text(&mut pixmap, "Commit-to-Release Latency Heatmap", 40.0, 50.0, 28.0, white());
    text.draw_text(&mut pixmap, "How quickly do commits reach a tagged release? Green = shipped within days. Red = waited weeks.", 40.0, 78.0, 13.0, dim());
    text.draw_text(&mut pixmap, "Magenta = never released. Clusters of red suggest batch-heavy releases or delivery bottlenecks.", 40.0, 94.0, 13.0, dim());

    let entries = &wm.commit_to_release_days;
    if entries.is_empty() {
        text.draw_text(&mut pixmap, "No data available", 40.0, 130.0, 18.0, dim());
        save_chart(&pixmap, dir, "01_release_heatmap.png")?;
        return Ok(());
    }

    // Stats line
    let stats = format!(
        "Median: {:.1}d | P90: {:.1}d | Released within 7d: {:.1}%",
        wm.release_median_latency, wm.release_p90_latency, wm.release_pct_within_7d
    );
    text.draw_text(&mut pixmap, &stats, 40.0, 115.0, 16.0, light());

    // Calendar layout: rows=day-of-week (Mon-Sun), columns=weeks
    // Parse all dates and find range
    let dates: Vec<(i32, u32, u32)> = entries.iter().map(|e| parse_date(&e.date)).collect();
    if dates.is_empty() {
        save_chart(&pixmap, dir, "01_release_heatmap.png")?;
        return Ok(());
    }

    let cell_size = 12.0f32;
    let cell_gap = 2.0f32;
    let left_margin = 80.0f32;
    let top_margin = 145.0f32;

    // Group entries by week index and day-of-week
    // Simple approach: use sequential day index from first date
    let first_date = &entries[0].date;
    let (fy, fm, fd) = parse_date(first_date);

    // day_of_week: 0=Mon ... 6=Sun (approximate using Zeller-like)
    fn day_of_week(y: i32, m: u32, d: u32) -> u32 {
        // Tomohiko Sakamoto's algorithm
        let t = [0i32, 3, 2, 5, 0, 3, 5, 1, 4, 6, 2, 4];
        let mut y = y;
        if m < 3 { y -= 1; }
        let dow = (y + y/4 - y/100 + y/400 + t[(m - 1) as usize] + d as i32) % 7;
        // Result: 0=Sun, 1=Mon, ..., 6=Sat. Convert to 0=Mon
        ((dow + 6) % 7) as u32
    }

    fn days_from_epoch(y: i32, m: u32, d: u32) -> i64 {
        // Approximate days from a reference point for indexing
        let y = y as i64;
        let m = m as i64;
        let d = d as i64;
        365 * y + y / 4 - y / 100 + y / 400 + (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1
    }

    let first_epoch = days_from_epoch(fy, fm, fd);
    let first_dow = day_of_week(fy, fm, fd);

    // Draw day labels
    let day_labels = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
    for (i, label) in day_labels.iter().enumerate() {
        text.draw_text(&mut pixmap, label, 40.0, top_margin + (i as f32) * (cell_size + cell_gap) + cell_size, 10.0, dim());
    }

    // Draw cells
    let max_weeks = ((WIDTH as f32 - left_margin - 40.0) / (cell_size + cell_gap)) as usize;
    let mut last_month_label = 0u32;

    for entry in entries {
        let (ey, em, ed) = parse_date(&entry.date);
        let epoch = days_from_epoch(ey, em, ed);
        let day_offset = (epoch - first_epoch) as i32;
        if day_offset < 0 { continue; }

        let dow = day_of_week(ey, em, ed);
        let week_idx = ((day_offset as u32 + first_dow) / 7) as usize;
        if week_idx >= max_weeks { continue; }

        let x = left_margin + week_idx as f32 * (cell_size + cell_gap);
        let y = top_margin + dow as f32 * (cell_size + cell_gap);

        let color = if entry.unreleased_count > 0 && entry.avg_days_to_release < 0.0 {
            magenta()
        } else if entry.avg_days_to_release < 0.0 {
            magenta()
        } else {
            let t = (entry.avg_days_to_release as f32 / 30.0).clamp(0.0, 1.0);
            heat_color(t)
        };

        fill_rect(&mut pixmap, x, y, cell_size, cell_size, color);

        // Month label at top
        if em != last_month_label && dow == 0 {
            let label = format!("{} {}", month_name(em), ey);
            text.draw_text(&mut pixmap, &label, x, top_margin - 8.0, 10.0, dim());
            last_month_label = em;
        }
    }

    // Legend
    let legend_y = top_margin + 7.0 * (cell_size + cell_gap) + 40.0;
    text.draw_text(&mut pixmap, "Legend:", 40.0, legend_y, 14.0, white());
    let legend_items = [
        ("0-3 days", heat_color(0.0)),
        ("7-14 days", heat_color(0.35)),
        ("30+ days", heat_color(1.0)),
        ("Unreleased", magenta()),
    ];
    let mut lx = 120.0;
    for (label, color) in &legend_items {
        fill_rect(&mut pixmap, lx, legend_y - 10.0, 14.0, 14.0, *color);
        text.draw_text(&mut pixmap, label, lx + 18.0, legend_y, 12.0, light());
        lx += 18.0 + text.measure_text(label, 12.0) + 20.0;
    }

    text.draw_text(&mut pixmap, "commit-viz", 40.0, HEIGHT as f32 - 20.0, 10.0, Color::from_rgba8(70, 70, 80, 255));
    save_chart(&pixmap, dir, "01_release_heatmap.png")
}

// ============================================================
// Chart 2: Branch Lifespan Gantt
// ============================================================
pub fn render_branch_lifespan_gantt(
    wm: &ChangeFlowMetrics,
    text: &TextRenderer,
    dir: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut pixmap = Pixmap::new(WIDTH, HEIGHT).unwrap();
    pixmap.fill(bg());

    text.draw_text(&mut pixmap, "Branch Lifespan Gantt Chart", 40.0, 50.0, 28.0, white());
    text.draw_text(&mut pixmap, "How long do branches live before merging? Short green bars = rapid integration. Long red bars = diverging work.", 40.0, 78.0, 13.0, dim());
    text.draw_text(&mut pixmap, "Hatched bars with '!' = branches that never merged, increasing stale-code and merge-conflict risk.", 40.0, 94.0, 13.0, dim());

    let branches = &wm.branch_lifespans;
    if branches.is_empty() {
        text.draw_text(&mut pixmap, "No branch data available", 40.0, 130.0, 18.0, dim());
        save_chart(&pixmap, dir, "02_branch_gantt.png")?;
        return Ok(());
    }

    let stats = format!(
        "Median lifespan: {:.1}d | Unmerged: {} | Longest: {:.1}d",
        wm.branch_median_lifespan, wm.branch_unmerged_count, wm.branch_longest_days
    );
    text.draw_text(&mut pixmap, &stats, 40.0, 115.0, 16.0, light());

    // Show up to 30 branches
    let max_branches = 30.min(branches.len());
    let display_branches = &branches[..max_branches];

    let chart_left = 250.0f32;
    let chart_right = WIDTH as f32 - 60.0;
    let chart_top = 145.0f32;
    let bar_height = 22.0f32;
    let bar_gap = 4.0f32;

    // Find time range from branch data
    fn parse_iso_epoch(s: &str) -> f64 {
        // Extract date portion and approximate
        let d = &s[..10.min(s.len())];
        let (y, m, day) = parse_date(d);
        y as f64 * 365.25 + m as f64 * 30.44 + day as f64
    }

    let mut min_t = f64::MAX;
    let mut max_t = f64::MIN;
    for b in display_branches {
        let t0 = parse_iso_epoch(&b.first_commit);
        let t1 = parse_iso_epoch(&b.last_commit);
        if t0 < min_t { min_t = t0; }
        if t1 > max_t { max_t = t1; }
    }
    let range = (max_t - min_t).max(1.0);

    for (i, b) in display_branches.iter().enumerate() {
        let y = chart_top + i as f32 * (bar_height + bar_gap);
        if y + bar_height > HEIGHT as f32 - 60.0 { break; }

        // Branch name (truncated)
        let name = if b.branch.len() > 28 {
            format!("{}...", &b.branch[..25])
        } else {
            b.branch.clone()
        };
        text.draw_text(&mut pixmap, &name, 10.0, y + bar_height - 4.0, 11.0, light());

        let t0 = parse_iso_epoch(&b.first_commit);
        let t1 = parse_iso_epoch(&b.last_commit);
        let x0 = chart_left + ((t0 - min_t) / range) as f32 * (chart_right - chart_left);
        let x1 = chart_left + ((t1 - min_t) / range) as f32 * (chart_right - chart_left);
        let bar_w = (x1 - x0).max(4.0);

        let color = if b.lifespan_days < 7.0 {
            heat_color(0.0) // green
        } else if b.lifespan_days < 30.0 {
            heat_color(0.3) // yellow-ish
        } else if b.lifespan_days < 90.0 {
            heat_color(0.65) // orange
        } else {
            heat_color(1.0) // red
        };

        if b.merged {
            fill_rect(&mut pixmap, x0, y, bar_w, bar_height, color);
        } else {
            draw_hatched_rect(&mut pixmap, x0, y, bar_w, bar_height, Color::from_rgba8(220, 50, 50, 200));
            text.draw_text(&mut pixmap, "!", x0 + bar_w + 4.0, y + bar_height - 4.0, 14.0,
                Color::from_rgba8(255, 80, 80, 255));
        }
    }

    // Legend
    let ly = HEIGHT as f32 - 50.0;
    let legend_items = [
        ("<7d", heat_color(0.0)),
        ("7-30d", heat_color(0.3)),
        ("30-90d", heat_color(0.65)),
        (">90d", heat_color(1.0)),
    ];
    let mut lx = 40.0;
    text.draw_text(&mut pixmap, "Legend:", lx, ly, 14.0, white());
    lx += 70.0;
    for (label, color) in &legend_items {
        fill_rect(&mut pixmap, lx, ly - 10.0, 14.0, 14.0, *color);
        text.draw_text(&mut pixmap, label, lx + 18.0, ly, 12.0, light());
        lx += 18.0 + text.measure_text(label, 12.0) + 16.0;
    }
    // Unmerged legend
    draw_hatched_rect(&mut pixmap, lx, ly - 10.0, 14.0, 14.0, Color::from_rgba8(220, 50, 50, 200));
    text.draw_text(&mut pixmap, "unmerged", lx + 18.0, ly, 12.0, light());

    text.draw_text(&mut pixmap, "commit-viz", 40.0, HEIGHT as f32 - 20.0, 10.0, Color::from_rgba8(70, 70, 80, 255));
    save_chart(&pixmap, dir, "02_branch_gantt.png")
}

// ============================================================
// Chart 3: Commit Velocity & Drought Periods
// ============================================================
pub fn render_velocity_drought(
    wm: &ChangeFlowMetrics,
    text: &TextRenderer,
    dir: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut pixmap = Pixmap::new(WIDTH, HEIGHT).unwrap();
    pixmap.fill(bg());

    text.draw_text(&mut pixmap, "Commit Velocity & Drought Periods", 40.0, 50.0, 28.0, white());
    text.draw_text(&mut pixmap, "Is the team committing consistently? Red spans = 7+ consecutive days with zero commits.", 40.0, 78.0, 13.0, dim());
    text.draw_text(&mut pixmap, "Frequent or long droughts may signal single-contributor dependency, blocked work, or seasonal patterns.", 40.0, 94.0, 13.0, dim());

    let velocity = &wm.daily_velocity;
    if velocity.is_empty() {
        text.draw_text(&mut pixmap, "No velocity data", 40.0, 130.0, 18.0, dim());
        save_chart(&pixmap, dir, "03_velocity_drought.png")?;
        return Ok(());
    }

    let stats = format!(
        "Droughts (7+ days): {} | Longest: {}d | Total drought days: {}",
        wm.drought_count, wm.longest_drought_days, wm.total_drought_days
    );
    text.draw_text(&mut pixmap, &stats, 40.0, 115.0, 16.0, light());

    let chart_left = 80.0f32;
    let chart_right = WIDTH as f32 - 40.0;
    let chart_top = 145.0f32;
    let chart_bottom = 750.0f32;
    let chart_w = chart_right - chart_left;
    let chart_h = chart_bottom - chart_top;

    let max_count = velocity.iter().map(|v| v.count).max().unwrap_or(1).max(1);
    let n = velocity.len();
    let bar_w = (chart_w / n as f32).max(1.0).min(8.0);

    // Draw bars
    for (i, v) in velocity.iter().enumerate() {
        let x = chart_left + (i as f32 / n as f32) * chart_w;
        let h = (v.count as f32 / max_count as f32) * chart_h;
        let y = chart_bottom - h;
        let color = category_color(&v.dominant_category);
        fill_rect(&mut pixmap, x, y, bar_w, h, color);
    }

    // Red overlay for drought periods
    for drought in &wm.drought_periods {
        // Find start/end indices
        let start_idx = velocity.iter().position(|v| v.date == drought.start_date);
        let end_idx = velocity.iter().position(|v| v.date == drought.end_date);
        if let (Some(si), Some(ei)) = (start_idx, end_idx) {
            let x0 = chart_left + (si as f32 / n as f32) * chart_w;
            let x1 = chart_left + ((ei + 1) as f32 / n as f32) * chart_w;
            fill_rect_alpha(&mut pixmap, x0, chart_top, x1 - x0, chart_h, Color::from_rgba8(255, 0, 0, 255), 0.2);
            // Duration label
            let label = format!("{}d", drought.duration_days);
            let mid_x = (x0 + x1) / 2.0 - text.measure_text(&label, 11.0) / 2.0;
            text.draw_text(&mut pixmap, &label, mid_x, chart_top + 15.0, 11.0, Color::from_rgba8(255, 100, 100, 255));
        }
    }

    // Rolling 7-day average line below main chart
    let avg_top = 790.0f32;
    let avg_bottom = 950.0f32;
    let avg_h = avg_bottom - avg_top;

    text.draw_text(&mut pixmap, "7-day rolling average", 80.0, avg_top - 5.0, 14.0, white());

    let rolling = &wm.rolling_7day_avg;
    if rolling.len() > 1 {
        let max_avg = rolling.iter().map(|r| r.avg).fold(0.0f64, f64::max).max(1.0);

        for i in 1..rolling.len() {
            let x0 = chart_left + ((i - 1) as f32 / n as f32) * chart_w;
            let x1 = chart_left + (i as f32 / n as f32) * chart_w;
            let y0 = avg_bottom - (rolling[i - 1].avg / max_avg) as f32 * avg_h;
            let y1 = avg_bottom - (rolling[i].avg / max_avg) as f32 * avg_h;
            draw_line(&mut pixmap, x0, y0, x1, y1, Color::from_rgba8(66, 133, 244, 200), 1.5);
        }
    }

    // X-axis month labels
    let mut last_month = 0u32;
    for (i, v) in velocity.iter().enumerate() {
        let (_, m, _) = parse_date(&v.date);
        if m != last_month {
            let x = chart_left + (i as f32 / n as f32) * chart_w;
            text.draw_text(&mut pixmap, month_name(m), x, chart_bottom + 15.0, 10.0, dim());
            last_month = m;
        }
    }

    text.draw_text(&mut pixmap, "commit-viz", 40.0, HEIGHT as f32 - 20.0, 10.0, Color::from_rgba8(70, 70, 80, 255));
    save_chart(&pixmap, dir, "03_velocity_drought.png")
}

// ============================================================
// Chart 4: Commit-to-Merge Latency Scatter
// ============================================================
pub fn render_merge_latency_scatter(
    wm: &ChangeFlowMetrics,
    text: &TextRenderer,
    dir: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut pixmap = Pixmap::new(WIDTH, HEIGHT).unwrap();
    pixmap.fill(bg());

    text.draw_text(&mut pixmap, "Commit-to-Merge Latency Scatter", 40.0, 50.0, 28.0, white());
    text.draw_text(&mut pixmap, "How quickly do branch commits get integrated? Dots below yellow (7d) = fast integration.", 40.0, 78.0, 13.0, dim());
    text.draw_text(&mut pixmap, "Above red (30d) or magenta at top (unmerged) = work at risk of going stale or causing conflicts.", 40.0, 94.0, 13.0, dim());

    let entries = &wm.commit_merge_latency;
    if entries.is_empty() {
        text.draw_text(&mut pixmap, "No merge latency data", 40.0, 130.0, 18.0, dim());
        save_chart(&pixmap, dir, "04_merge_scatter.png")?;
        return Ok(());
    }

    let stats = format!(
        "Median merge latency: {:.1}d | Merged within 7d: {:.1}% | Within 30d: {:.1}%",
        wm.merge_median_latency, wm.merge_pct_within_7d, wm.merge_pct_within_30d
    );
    text.draw_text(&mut pixmap, &stats, 40.0, 115.0, 16.0, light());

    let chart_left = 100.0f32;
    let chart_right = WIDTH as f32 - 60.0;
    let chart_top = 145.0f32;
    let chart_bottom = 980.0f32;
    let chart_w = chart_right - chart_left;
    let chart_h = chart_bottom - chart_top;

    // X-axis: date range, Y-axis: log scale of days (0.1 to 365+)
    let log_min = -1.0f32; // log10(0.1)
    let log_max = 2.7f32;  // log10(~500)
    let unmerged_y = chart_top + 15.0; // top band for unmerged

    // Find date range
    let dates: Vec<f64> = entries.iter().map(|e| {
        let (y, m, d) = parse_date(&e.commit_date[..10.min(e.commit_date.len())]);
        y as f64 * 365.25 + m as f64 * 30.44 + d as f64
    }).collect();

    let min_date = dates.iter().cloned().fold(f64::MAX, f64::min);
    let max_date = dates.iter().cloned().fold(f64::MIN, f64::max);
    let date_range = (max_date - min_date).max(1.0);

    // Dashed threshold lines
    let y_7d = chart_bottom - ((7.0f32.log10() - log_min) / (log_max - log_min)) * chart_h;
    let y_30d = chart_bottom - ((30.0f32.log10() - log_min) / (log_max - log_min)) * chart_h;

    draw_dashed_line(&mut pixmap, chart_left, y_7d, chart_right, y_7d,
        Color::from_rgba8(255, 255, 0, 180), 1.5, 8.0);
    text.draw_text(&mut pixmap, "7 days", chart_right - 60.0, y_7d - 5.0, 11.0,
        Color::from_rgba8(255, 255, 0, 200));

    draw_dashed_line(&mut pixmap, chart_left, y_30d, chart_right, y_30d,
        Color::from_rgba8(255, 60, 60, 180), 1.5, 8.0);
    text.draw_text(&mut pixmap, "30 days", chart_right - 65.0, y_30d - 5.0, 11.0,
        Color::from_rgba8(255, 60, 60, 200));

    // Draw dots
    for (i, entry) in entries.iter().enumerate() {
        let x = chart_left + ((dates[i] - min_date) / date_range) as f32 * chart_w;

        let (y, color) = if let Some(days) = entry.days_to_merge {
            let log_days = (days as f32).max(0.1).log10();
            let y = chart_bottom - ((log_days - log_min) / (log_max - log_min)) * chart_h;
            (y.clamp(chart_top, chart_bottom), category_color(&entry.category))
        } else {
            (unmerged_y, magenta())
        };

        let r = (2.0 + (entry.lines_changed as f32).ln().max(0.0) * 1.2).min(10.0);
        fill_circle(&mut pixmap, x, y, r, color);
    }

    // Unmerged label
    text.draw_text(&mut pixmap, "Unmerged", 40.0, unmerged_y + 4.0, 11.0, magenta());

    // Y-axis labels
    for &days in &[0.1f32, 1.0, 7.0, 30.0, 100.0, 365.0] {
        let log_d = days.log10();
        let y = chart_bottom - ((log_d - log_min) / (log_max - log_min)) * chart_h;
        if y > chart_top && y < chart_bottom {
            let label = if days < 1.0 { format!("{:.1}d", days) } else { format!("{}d", days as u32) };
            text.draw_text(&mut pixmap, &label, 40.0, y + 4.0, 10.0, dim());
        }
    }

    text.draw_text(&mut pixmap, "commit-viz", 40.0, HEIGHT as f32 - 20.0, 10.0, Color::from_rgba8(70, 70, 80, 255));
    save_chart(&pixmap, dir, "04_merge_scatter.png")
}

// ============================================================
// Chart 5: Release Cadence Lollipop + Distribution
// ============================================================
pub fn render_release_cadence(
    wm: &ChangeFlowMetrics,
    text: &TextRenderer,
    dir: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut pixmap = Pixmap::new(WIDTH, HEIGHT).unwrap();
    pixmap.fill(bg());

    text.draw_text(&mut pixmap, "Release Cadence & Interval Distribution", 40.0, 50.0, 28.0, white());
    text.draw_text(&mut pixmap, "How predictable is the release rhythm? Green dots within the band are healthy intervals.", 40.0, 78.0, 13.0, dim());
    text.draw_text(&mut pixmap, "Outlier red dots suggest disruptions. A high CV (>0.5) means unpredictable delivery timing.", 40.0, 94.0, 13.0, dim());

    let intervals = &wm.release_intervals;
    if intervals.is_empty() {
        text.draw_text(&mut pixmap, "Not enough releases for analysis", 40.0, 130.0, 18.0, dim());
        save_chart(&pixmap, dir, "05_release_cadence.png")?;
        return Ok(());
    }

    let stats = format!(
        "Mean: {:.1}d | Median: {:.1}d | CV: {:.2} | Longest gap: {:.1}d",
        wm.release_interval_mean, wm.release_interval_median,
        wm.release_interval_cv, wm.release_interval_longest_gap
    );
    text.draw_text(&mut pixmap, &stats, 40.0, 115.0, 16.0, light());

    // Lollipop chart (left 70% of width)
    let lollipop_right = WIDTH as f32 * 0.68;
    let chart_left = 80.0f32;
    let chart_top = 150.0f32;
    let chart_bottom = 950.0f32;
    let chart_h = chart_bottom - chart_top;
    let chart_w = lollipop_right - chart_left;

    let max_days = intervals.iter().map(|r| r.days_since_previous).fold(0.0f64, f64::max).max(1.0);
    let mean = wm.release_interval_mean;
    let stdev = if wm.release_interval_cv > 0.0 { mean * wm.release_interval_cv } else { mean * 0.3 };

    let n = intervals.len();
    let stick_gap = (chart_w / n as f32).min(20.0);

    // Healthy band (mean +/- 1 stdev)
    let band_lo = ((mean - stdev).max(0.0) / max_days) as f32;
    let band_hi = ((mean + stdev) / max_days) as f32;
    let band_y_top = chart_bottom - band_hi.min(1.0) * chart_h;
    let band_y_bot = chart_bottom - band_lo * chart_h;
    fill_rect_alpha(&mut pixmap, chart_left, band_y_top, chart_w, band_y_bot - band_y_top,
        Color::from_rgba8(76, 175, 80, 255), 0.08);
    // Dashed border lines for the healthy band
    draw_dashed_line(&mut pixmap, chart_left, band_y_top, chart_left + chart_w, band_y_top,
        Color::from_rgba8(76, 175, 80, 100), 1.0, 6.0);
    draw_dashed_line(&mut pixmap, chart_left, band_y_bot, chart_left + chart_w, band_y_bot,
        Color::from_rgba8(76, 175, 80, 100), 1.0, 6.0);

    // Draw lollipops
    for (i, interval) in intervals.iter().enumerate() {
        let x = chart_left + (i as f32 + 0.5) * stick_gap;
        if x > lollipop_right { break; }

        let h_frac = (interval.days_since_previous / max_days) as f32;
        let y = chart_bottom - h_frac * chart_h;

        // Stick
        draw_line(&mut pixmap, x, chart_bottom, x, y, Color::from_rgba8(100, 100, 100, 200), 1.5);

        // Dot colored by distance from mean
        let dist = (interval.days_since_previous - mean).abs();
        let color = if dist < stdev {
            heat_color(0.0) // green
        } else if dist < stdev * 2.0 {
            heat_color(0.5) // yellow
        } else {
            heat_color(1.0) // red
        };
        fill_circle(&mut pixmap, x, y, 4.0, color);
    }

    // Mean line
    let mean_y = chart_bottom - (mean / max_days) as f32 * chart_h;
    draw_dashed_line(&mut pixmap, chart_left, mean_y, lollipop_right, mean_y,
        Color::from_rgba8(255, 255, 255, 150), 1.0, 6.0);
    text.draw_text(&mut pixmap, &format!("mean={:.0}d", mean), lollipop_right - 100.0, mean_y - 5.0, 10.0, light());

    // Histogram sidebar (right 28% of width)
    let hist_left = WIDTH as f32 * 0.72;
    let hist_right = WIDTH as f32 - 40.0;
    let hist_w = hist_right - hist_left;

    text.draw_text(&mut pixmap, "Distribution", hist_left, chart_top - 5.0, 16.0, white());

    let bins = &wm.release_interval_distribution;
    if !bins.is_empty() {
        let max_bin = bins.iter().map(|b| b.count).max().unwrap_or(1).max(1);
        let bin_h = 40.0f32;
        let bin_gap = 8.0f32;

        for (i, bin) in bins.iter().enumerate() {
            let y = chart_top + 20.0 + i as f32 * (bin_h + bin_gap);
            let w = (bin.count as f32 / max_bin as f32) * hist_w * 0.7;

            text.draw_text(&mut pixmap, &bin.label, hist_left, y + bin_h / 2.0 + 4.0, 12.0, light());

            let bar_x = hist_left + 70.0;
            fill_rect(&mut pixmap, bar_x, y, w, bin_h, Color::from_rgba8(66, 133, 244, 200));

            text.draw_text(&mut pixmap, &bin.count.to_string(), bar_x + w + 8.0, y + bin_h / 2.0 + 4.0, 12.0, dim());
        }
    }

    text.draw_text(&mut pixmap, "commit-viz", 40.0, HEIGHT as f32 - 20.0, 10.0, Color::from_rgba8(70, 70, 80, 255));
    save_chart(&pixmap, dir, "05_release_cadence.png")
}

// ============================================================
// Chart 6: Work Disposition Donut
// ============================================================
pub fn render_work_disposition_donut(
    wm: &ChangeFlowMetrics,
    text: &TextRenderer,
    dir: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut pixmap = Pixmap::new(WIDTH, HEIGHT).unwrap();
    pixmap.fill(bg());

    text.draw_text(&mut pixmap, "Work Disposition", 40.0, 50.0, 28.0, white());
    text.draw_text(&mut pixmap, "What proportion of work ships quickly vs. slowly vs. not at all? A healthy codebase shows mostly", 40.0, 78.0, 13.0, dim());
    text.draw_text(&mut pixmap, "green (fast-merged). Large yellow or red segments indicate slow review cycles or abandoned work.", 40.0, 94.0, 13.0, dim());

    let wd = &wm.work_disposition;
    let total_lines = wd.fast_merged_lines + wd.slow_merged_lines + wd.unmerged_lines;
    let total_commits = wd.fast_merged_commits + wd.slow_merged_commits + wd.unmerged_commits;

    if total_lines == 0 {
        text.draw_text(&mut pixmap, "No disposition data", 40.0, 130.0, 18.0, dim());
        save_chart(&pixmap, dir, "06_work_disposition.png")?;
        return Ok(());
    }

    // Donut center
    let cx = 480.0f32;
    let cy = 540.0f32;
    let outer_r = 280.0f32;
    let mid_r = 200.0f32;
    let inner_r = 130.0f32;

    // Inner ring: fast/slow/unmerged by lines
    let segments_inner = [
        ("Fast merged (<7d)", wd.fast_merged_lines, Color::from_rgba8(76, 175, 80, 230)),
        ("Slow merged (>7d)", wd.slow_merged_lines, Color::from_rgba8(255, 193, 7, 230)),
        ("Unmerged", wd.unmerged_lines, Color::from_rgba8(244, 67, 54, 230)),
    ];

    let total_f = total_lines as f64;
    let mut angle = -std::f64::consts::FRAC_PI_2; // start at top

    // Draw inner ring arcs
    for &(_, lines, color) in &segments_inner {
        if lines == 0 { continue; }
        let sweep = (lines as f64 / total_f) * std::f64::consts::TAU;
        draw_arc_filled(&mut pixmap, cx, cy, inner_r, mid_r, angle as f32, sweep as f32, color);
        angle += sweep;
    }

    // Outer ring: subdivide by category within each merge-speed segment
    angle = -std::f64::consts::FRAC_PI_2;
    for &(_, lines, base_color) in &segments_inner {
        if lines == 0 { continue; }
        let speed = match base_color.green() as u32 {
            175 => "fast",
            193 => "slow",
            _ => "unmerged",
        };
        let speed_match = match speed {
            "fast" => "fast",
            "slow" => "slow",
            _ => "unmerged",
        };

        // Get sub-segments for this speed
        let sub_segs: Vec<_> = wd.segments.iter()
            .filter(|s| s.merge_speed == speed_match)
            .collect();

        let speed_sweep = (lines as f64 / total_f) * std::f64::consts::TAU;

        if sub_segs.is_empty() {
            draw_arc_filled(&mut pixmap, cx, cy, mid_r + 4.0, outer_r, angle as f32, speed_sweep as f32, base_color);
            angle += speed_sweep;
        } else {
            let speed_total: u32 = sub_segs.iter().map(|s| s.lines_changed).sum();
            let speed_total = speed_total.max(1);
            for seg in &sub_segs {
                let sub_sweep = (seg.lines_changed as f64 / speed_total as f64) * speed_sweep;
                let color = category_color(&seg.category);
                draw_arc_filled(&mut pixmap, cx, cy, mid_r + 4.0, outer_r, angle as f32, sub_sweep as f32, color);
                angle += sub_sweep;
            }
        }
    }

    // Center text
    let total_label = format!("{} commits", total_commits);
    let lines_label = format!("{} lines", total_lines);
    let tw1 = text.measure_text(&total_label, 18.0);
    let tw2 = text.measure_text(&lines_label, 14.0);
    text.draw_text(&mut pixmap, &total_label, cx - tw1 / 2.0, cy - 5.0, 18.0, white());
    text.draw_text(&mut pixmap, &lines_label, cx - tw2 / 2.0, cy + 18.0, 14.0, light());

    // Right panel: detail table
    let table_left = 820.0f32;
    let mut ty = 120.0f32;

    text.draw_text(&mut pixmap, "Breakdown", table_left, ty, 20.0, white());
    ty += 40.0;

    // Header
    text.draw_text(&mut pixmap, "Category", table_left, ty, 13.0, dim());
    text.draw_text(&mut pixmap, "Speed", table_left + 160.0, ty, 13.0, dim());
    text.draw_text(&mut pixmap, "Lines", table_left + 290.0, ty, 13.0, dim());
    text.draw_text(&mut pixmap, "Commits", table_left + 390.0, ty, 13.0, dim());
    text.draw_text(&mut pixmap, "%", table_left + 490.0, ty, 13.0, dim());
    ty += 25.0;

    draw_line(&mut pixmap, table_left, ty - 5.0, WIDTH as f32 - 40.0, ty - 5.0,
        Color::from_rgba8(60, 60, 60, 255), 1.0);

    for seg in &wd.segments {
        if ty > HEIGHT as f32 - 60.0 { break; }
        let pct = seg.lines_changed as f64 / total_f * 100.0;

        let cat_color = category_color(&seg.category);
        fill_rect(&mut pixmap, table_left - 18.0, ty - 10.0, 10.0, 10.0, cat_color);

        text.draw_text(&mut pixmap, &seg.category, table_left, ty, 12.0, light());
        text.draw_text(&mut pixmap, &seg.merge_speed, table_left + 160.0, ty, 12.0, light());
        text.draw_text(&mut pixmap, &seg.lines_changed.to_string(), table_left + 290.0, ty, 12.0, light());
        text.draw_text(&mut pixmap, &seg.commit_count.to_string(), table_left + 390.0, ty, 12.0, light());
        text.draw_text(&mut pixmap, &format!("{:.1}%", pct), table_left + 490.0, ty, 12.0, light());
        ty += 22.0;
    }

    // Summary below table
    ty += 20.0;
    let fast_pct = wd.fast_merged_lines as f64 / total_f * 100.0;
    let slow_pct = wd.slow_merged_lines as f64 / total_f * 100.0;
    let unmerged_pct = wd.unmerged_lines as f64 / total_f * 100.0;

    fill_rect(&mut pixmap, table_left - 18.0, ty - 10.0, 10.0, 10.0, Color::from_rgba8(76, 175, 80, 230));
    text.draw_text(&mut pixmap, &format!("Fast merged (<7d): {:.1}%", fast_pct), table_left, ty, 14.0, light());
    ty += 25.0;
    fill_rect(&mut pixmap, table_left - 18.0, ty - 10.0, 10.0, 10.0, Color::from_rgba8(255, 193, 7, 230));
    text.draw_text(&mut pixmap, &format!("Slow merged (>7d): {:.1}%", slow_pct), table_left, ty, 14.0, light());
    ty += 25.0;
    fill_rect(&mut pixmap, table_left - 18.0, ty - 10.0, 10.0, 10.0, Color::from_rgba8(244, 67, 54, 230));
    text.draw_text(&mut pixmap, &format!("Unmerged: {:.1}%", unmerged_pct), table_left, ty, 14.0, light());

    text.draw_text(&mut pixmap, "commit-viz", 40.0, HEIGHT as f32 - 20.0, 10.0, Color::from_rgba8(70, 70, 80, 255));
    save_chart(&pixmap, dir, "06_work_disposition.png")
}

/// Draw a filled arc segment (donut slice) using line segments approximation
fn draw_arc_filled(
    pixmap: &mut Pixmap,
    cx: f32, cy: f32,
    r_inner: f32, r_outer: f32,
    start_angle: f32, sweep: f32,
    color: Color,
) {
    if sweep.abs() < 0.001 { return; }

    let steps = ((sweep.abs() * 50.0) as usize).max(4);
    let step_angle = sweep / steps as f32;

    let mut paint = Paint::default();
    paint.set_color(color);
    paint.anti_alias = true;

    let mut pb = PathBuilder::new();

    // Outer arc forward
    let a0 = start_angle;
    pb.move_to(cx + a0.cos() * r_outer, cy + a0.sin() * r_outer);
    for i in 1..=steps {
        let a = a0 + i as f32 * step_angle;
        pb.line_to(cx + a.cos() * r_outer, cy + a.sin() * r_outer);
    }

    // Inner arc backward
    for i in (0..=steps).rev() {
        let a = a0 + i as f32 * step_angle;
        pb.line_to(cx + a.cos() * r_inner, cy + a.sin() * r_inner);
    }

    pb.close();
    if let Some(path) = pb.finish() {
        pixmap.fill_path(&path, &paint, FillRule::Winding, Transform::identity(), None);
    }
}

/// Render all 6 change flow charts to the specified directory (parallel)
pub fn render_all(
    wm: &ChangeFlowMetrics,
    dir: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    std::fs::create_dir_all(dir)?;

    // Each chart gets its own TextRenderer since they run in parallel threads
    let renderers: Vec<(&str, Box<dyn Fn(&ChangeFlowMetrics, &TextRenderer, &Path) -> Result<(), Box<dyn std::error::Error>> + Send + Sync>)> = vec![
        ("01_release_heatmap", Box::new(render_commit_to_release_heatmap)),
        ("02_branch_gantt", Box::new(render_branch_lifespan_gantt)),
        ("03_velocity_drought", Box::new(render_velocity_drought)),
        ("04_merge_scatter", Box::new(render_merge_latency_scatter)),
        ("05_release_cadence", Box::new(render_release_cadence)),
        ("06_work_disposition", Box::new(render_work_disposition_donut)),
    ];

    let results: Vec<Result<(), String>> = renderers
        .par_iter()
        .map(|(name, render_fn)| {
            let text = TextRenderer::new();
            render_fn(wm, &text, dir).map_err(|e| format!("Error rendering {}: {}", name, e))
        })
        .collect();

    for result in results {
        if let Err(e) = result {
            return Err(e.into());
        }
    }

    Ok(())
}
