use crate::data::CollectedData;
use crate::text::TextRenderer;
use std::path::Path;
use tiny_skia::{Color, Paint, PathBuilder, Pixmap, Transform};

fn category_color(category: &str) -> Color {
    match category {
        "feature" => Color::from_rgba8(66, 133, 244, 255),
        "bugfix" => Color::from_rgba8(234, 67, 53, 255),
        "release" => Color::from_rgba8(255, 215, 0, 255),
        "refactor" => Color::from_rgba8(171, 71, 188, 255),
        "docs" => Color::from_rgba8(124, 179, 66, 255),
        "ci" => Color::from_rgba8(0, 172, 193, 255),
        "test" => Color::from_rgba8(255, 152, 0, 255),
        _ => Color::from_rgba8(158, 158, 158, 255),
    }
}

pub fn render_report(
    data: &CollectedData,
    output_path: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let width = 1920u32;
    let height = 1080u32;
    let mut pixmap = Pixmap::new(width, height).unwrap();
    pixmap.fill(Color::from_rgba8(25, 25, 30, 255));

    let text = TextRenderer::new();
    let white = Color::from_rgba8(255, 255, 255, 255);
    let light = Color::from_rgba8(200, 200, 200, 255);
    let dim = Color::from_rgba8(140, 140, 140, 255);

    // Header
    let repo_name = &data.metadata.repo;
    text.draw_text(&mut pixmap, "commit-viz Statistics Report", 40.0, 50.0, 28.0, white);
    text.draw_text(&mut pixmap, repo_name, 40.0, 85.0, 20.0, light);

    let date_range = format!(
        "{} to {}",
        if data.metadata.date_range.start.is_empty() { "beginning" } else { &data.metadata.date_range.start },
        if data.metadata.date_range.end.is_empty() { "present" } else { &data.metadata.date_range.end },
    );
    text.draw_text(&mut pixmap, &date_range, 40.0, 115.0, 16.0, dim);

    let stats = match &data.statistics {
        Some(s) => s,
        None => {
            text.draw_text(&mut pixmap, "No statistics available", 40.0, 180.0, 20.0, light);
            pixmap.save_png(output_path)?;
            return Ok(());
        }
    };

    // Summary stats line
    let summary = format!(
        "{} commits | {} authors | {} days | {:.1} commits/week",
        stats.total_commits, stats.unique_authors, stats.date_span_days, stats.commits_per_week
    );
    text.draw_text(&mut pixmap, &summary, 40.0, 150.0, 16.0, light);

    // --- Category bar chart ---
    text.draw_text(&mut pixmap, "Commits by Category", 40.0, 210.0, 20.0, white);

    let categories_ordered = [
        "feature", "bugfix", "release", "refactor", "docs", "ci", "test", "other",
    ];
    let max_count = categories_ordered
        .iter()
        .filter_map(|c| stats.by_category.get(*c))
        .max()
        .copied()
        .unwrap_or(1);

    let bar_area_left = 160.0;
    let bar_area_right = 900.0;
    let bar_height = 24.0;
    let bar_gap = 8.0;
    let mut bar_y = 240.0;

    for cat in &categories_ordered {
        let count = stats.by_category.get(*cat).copied().unwrap_or(0);
        if count == 0 && *cat != "other" {
            bar_y += bar_height + bar_gap;
            continue;
        }
        let pct = if stats.total_commits > 0 {
            count as f32 / stats.total_commits as f32 * 100.0
        } else {
            0.0
        };

        // Label
        text.draw_text(&mut pixmap, cat, 40.0, bar_y + bar_height - 4.0, 14.0, light);

        // Bar
        let bar_width = (count as f32 / max_count as f32) * (bar_area_right - bar_area_left);
        let mut paint = Paint::default();
        paint.set_color(category_color(cat));

        let mut pb = PathBuilder::new();
        pb.move_to(bar_area_left, bar_y);
        pb.line_to(bar_area_left + bar_width, bar_y);
        pb.line_to(bar_area_left + bar_width, bar_y + bar_height);
        pb.line_to(bar_area_left, bar_y + bar_height);
        pb.close();
        if let Some(path) = pb.finish() {
            pixmap.fill_path(
                &path,
                &paint,
                tiny_skia::FillRule::Winding,
                Transform::identity(),
                None,
            );
        }

        // Count + percentage
        let label = format!("{} ({:.1}%)", count, pct);
        text.draw_text(
            &mut pixmap,
            &label,
            bar_area_left + bar_width + 10.0,
            bar_y + bar_height - 4.0,
            13.0,
            dim,
        );

        bar_y += bar_height + bar_gap;
    }

    // --- Release cycle stats ---
    let rc_y = 540.0;
    text.draw_text(&mut pixmap, "Release Cycle Analysis", 40.0, rc_y, 20.0, white);

    let rc = &stats.release_cycles;
    if rc.count >= 2 {
        let lines = [
            format!("Tagged releases: {}", rc.count),
            format!("Mean interval: {:.1} days", rc.mean_days),
            format!("Min interval: {:.0} days", rc.min_days),
            format!("Max interval: {:.0} days", rc.max_days),
            format!("Std deviation: {:.1} days", rc.stdev_days),
        ];
        for (i, line) in lines.iter().enumerate() {
            text.draw_text(
                &mut pixmap,
                line,
                60.0,
                rc_y + 35.0 + i as f32 * 26.0,
                15.0,
                light,
            );
        }
    } else {
        text.draw_text(
            &mut pixmap,
            "Not enough tagged releases for cycle analysis",
            60.0,
            rc_y + 35.0,
            15.0,
            dim,
        );
    }

    // --- Top authors ---
    let auth_x = 1000.0;
    text.draw_text(&mut pixmap, "Top Authors", auth_x, 210.0, 20.0, white);

    let max_author_commits = stats
        .top_authors
        .first()
        .map(|a| a.commits)
        .unwrap_or(1)
        .max(1);

    for (i, author) in stats.top_authors.iter().take(15).enumerate() {
        let y = 245.0 + i as f32 * 28.0;

        // Bar
        let bar_w = (author.commits as f32 / max_author_commits as f32) * 400.0;
        let mut paint = Paint::default();
        paint.set_color(Color::from_rgba8(66, 133, 244, 160));
        let mut pb = PathBuilder::new();
        pb.move_to(auth_x, y);
        pb.line_to(auth_x + bar_w, y);
        pb.line_to(auth_x + bar_w, y + 20.0);
        pb.line_to(auth_x, y + 20.0);
        pb.close();
        if let Some(path) = pb.finish() {
            pixmap.fill_path(
                &path,
                &paint,
                tiny_skia::FillRule::Winding,
                Transform::identity(),
                None,
            );
        }

        // Truncate long author names
        let name = if author.author.len() > 25 {
            format!("{}...", &author.author[..22])
        } else {
            author.author.clone()
        };

        text.draw_text(&mut pixmap, &name, auth_x + 5.0, y + 16.0, 12.0, white);
        text.draw_text(
            &mut pixmap,
            &author.commits.to_string(),
            auth_x + bar_w + 8.0,
            y + 16.0,
            12.0,
            dim,
        );
    }

    // Footer
    text.draw_text(
        &mut pixmap,
        "Generated by commit-viz",
        40.0,
        height as f32 - 30.0,
        12.0,
        dim,
    );

    pixmap.save_png(output_path)?;
    Ok(())
}
