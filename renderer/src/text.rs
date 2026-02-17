use fontdue::{Font, FontSettings};
use tiny_skia::{Color, Pixmap};

static FONT_DATA: &[u8] = include_bytes!("../assets/Inconsolata-Regular.ttf");

pub struct TextRenderer {
    font: Font,
}

impl TextRenderer {
    pub fn new() -> Self {
        let font = Font::from_bytes(FONT_DATA, FontSettings::default())
            .expect("Failed to load bundled font");
        TextRenderer { font }
    }

    pub fn draw_text(
        &self,
        pixmap: &mut Pixmap,
        text: &str,
        x: f32,
        y: f32,
        size: f32,
        color: Color,
    ) {
        let r = color.red();
        let g = color.green();
        let b = color.blue();
        let a = color.alpha();

        let mut cursor_x = x;
        for ch in text.chars() {
            let (metrics, bitmap) = self.font.rasterize(ch, size);
            if metrics.width == 0 || metrics.height == 0 {
                cursor_x += metrics.advance_width;
                continue;
            }

            let glyph_y = y - metrics.height as f32 - metrics.ymin as f32;

            for gy in 0..metrics.height {
                for gx in 0..metrics.width {
                    let coverage = f32::from(bitmap[gy * metrics.width + gx]) / 255.0;
                    if coverage < 0.01 {
                        continue;
                    }

                    #[allow(clippy::cast_possible_wrap)]
                    let px = (cursor_x + gx as f32) as i32;
                    #[allow(clippy::cast_possible_wrap)]
                    let py = (glyph_y + gy as f32) as i32;

                    #[allow(clippy::cast_possible_wrap)]
                    if px < 0
                        || py < 0
                        || px >= pixmap.width() as i32
                        || py >= pixmap.height() as i32
                    {
                        continue;
                    }

                    let idx = (py as u32 * pixmap.width() + px as u32) as usize * 4;
                    let data = pixmap.data_mut();
                    if idx + 3 >= data.len() {
                        continue;
                    }

                    let alpha = coverage * a;
                    let inv = 1.0 - alpha;
                    // Data is premultiplied RGBA
                    let bg_a = f32::from(data[idx + 3]) / 255.0;
                    data[idx] =
                        ((r * alpha + f32::from(data[idx]) / 255.0 * inv) * 255.0).min(255.0) as u8;
                    data[idx + 1] = ((g * alpha + f32::from(data[idx + 1]) / 255.0 * inv) * 255.0)
                        .min(255.0) as u8;
                    data[idx + 2] = ((b * alpha + f32::from(data[idx + 2]) / 255.0 * inv) * 255.0)
                        .min(255.0) as u8;
                    data[idx + 3] = ((alpha + bg_a * inv) * 255.0).min(255.0) as u8;
                }
            }

            cursor_x += metrics.advance_width;
        }
    }

    pub fn measure_text(&self, text: &str, size: f32) -> f32 {
        text.chars()
            .map(|ch| {
                let (metrics, _) = self.font.rasterize(ch, size);
                metrics.advance_width
            })
            .sum()
    }
}
