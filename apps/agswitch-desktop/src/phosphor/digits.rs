use gpui::{
    BorderStyle, Bounds, Corners, Edges, Hsla, IntoElement, Pixels, Styled, canvas, point, px,
    quad, rgba, size, transparent_black,
};

use crate::phosphor::palette::PhosphorTheme;

/// 5x7 dot-matrix bitmap glyphs (each row is 5 bits, MSB on left) for Samo/PHOSPHOR instrument readouts.
fn glyph_5x7(ch: char) -> [u8; 7] {
    match ch {
        '0' => [0b01110, 0b10001, 0b10011, 0b10101, 0b11001, 0b10001, 0b01110],
        '1' => [0b00100, 0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110],
        '2' => [0b01110, 0b10001, 0b00001, 0b00110, 0b01000, 0b10000, 0b11111],
        '3' => [0b01110, 0b10001, 0b00001, 0b00110, 0b00001, 0b10001, 0b01110],
        '4' => [0b00010, 0b00110, 0b01010, 0b10010, 0b11111, 0b00010, 0b00010],
        '5' => [0b11111, 0b10000, 0b11110, 0b00001, 0b00001, 0b10001, 0b01110],
        '6' => [0b00110, 0b01000, 0b10000, 0b11110, 0b10001, 0b10001, 0b01110],
        '7' => [0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b01000, 0b01000],
        '8' => [0b01110, 0b10001, 0b10001, 0b01110, 0b10001, 0b10001, 0b01110],
        '9' => [0b01110, 0b10001, 0b10001, 0b01111, 0b00001, 0b00010, 0b01100],
        '.' => [0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b01100, 0b01100],
        '/' => [0b00001, 0b00010, 0b00010, 0b00100, 0b01000, 0b01000, 0b10000],
        '-' => [0b00000, 0b00000, 0b00000, 0b11111, 0b00000, 0b00000, 0b00000],
        ':' => [0b00000, 0b01100, 0b01100, 0b00000, 0b01100, 0b01100, 0b00000],
        'K' | 'k' => [0b10001, 0b10010, 0b10100, 0b11000, 0b10100, 0b10010, 0b10001],
        'M' | 'm' => [0b10001, 0b11011, 0b10101, 0b10101, 0b10001, 0b10001, 0b10001],
        '%' => [0b11001, 0b11010, 0b00010, 0b00100, 0b01000, 0b01011, 0b10011],
        _ => [0; 7],
    }
}

fn glyph_cols(ch: char) -> u8 {
    match ch {
        '.' | ':' => 2,
        ' ' => 2,
        _ => 5,
    }
}

/// Single-canvas GPU-painted 5x7 LED dot-matrix readout (e.g., `56.1`, `6.6`, `2/3`).
/// Zero child `div` nodes; paints every lit and unlit phosphor dot in one GPU quad batch.
pub fn dot_matrix_readout(
    text: &str,
    dot_size: f32,
    gap: f32,
    lit_color: Hsla,
    unlit_color: Hsla,
) -> impl IntoElement + use<> {
    let chars: Vec<char> = text.chars().collect();
    let step = dot_size + gap;
    let char_gap = dot_size * 1.4;

    let mut total_w = 0.0_f32;
    for (idx, &ch) in chars.iter().enumerate() {
        let cols = f32::from(glyph_cols(ch));
        total_w += cols * dot_size + (cols - 1.0).max(0.0) * gap;
        if idx + 1 < chars.len() {
            total_w += char_gap;
        }
    }
    let total_h = 7.0 * dot_size + 6.0 * gap;

    canvas(
        move |_bounds, _window, _cx| chars,
        move |bounds: Bounds<Pixels>, chars: Vec<char>, window, _cx| {
            let ox = bounds.origin.x;
            let oy = bounds.origin.y;
            let radii = Corners::all(px((dot_size * 0.32).max(0.6)));
            let cell = size(px(dot_size), px(dot_size));
            let mut cursor_x = 0.0_f32;

            for &ch in &chars {
                let cols = glyph_cols(ch);
                let bitmap = glyph_5x7(ch);
                let col_offset: u8 = if cols == 2 { 1 } else { 0 };

                for (r_idx, &row_bits) in bitmap.iter().enumerate() {
                    let rf = f32::from(u8::try_from(r_idx).unwrap_or(0));
                    for c in 0..cols {
                        let bit_pos = 4_u8.saturating_sub(c + col_offset);
                        let is_on = ((row_bits >> bit_pos) & 1) == 1;
                        let col = if is_on { lit_color } else { unlit_color };
                        if !is_on && ch == ' ' {
                            continue;
                        }
                        let x = cursor_x + f32::from(c) * step;
                        let y = rf * step;
                        let b = Bounds::new(point(ox + px(x), oy + px(y)), cell);
                        window.paint_quad(quad(
                            b,
                            radii,
                            col,
                            Edges::all(px(0.0)),
                            transparent_black(),
                            BorderStyle::Solid,
                        ));
                    }
                }

                cursor_x +=
                    f32::from(cols) * dot_size + (f32::from(cols) - 1.0).max(0.0) * gap + char_gap;
            }
        },
    )
    .w(px(total_w.max(1.0)))
    .h(px(total_h))
    .flex_shrink_0()
}

/// Single-canvas segmented LED bar (Samo hardware instrument style, e.g. 14 blocks or 8 micro blocks).
pub fn segmented_led_bar(
    segments: u8,
    fill_fraction: f32,
    theme: PhosphorTheme,
    dim_col: Hsla,
    seg_w: f32,
    seg_h: f32,
    gap: f32,
) -> impl IntoElement + use<> {
    let segs = segments.max(1);
    let clamped = fill_fraction.clamp(0.0, 1.0);
    let total_w = f32::from(segs) * seg_w + (f32::from(segs) - 1.0).max(0.0) * gap;
    let lut = theme.lut();
    let step = seg_w + gap;

    canvas(
        |_bounds, _window, _cx| (),
        move |bounds: Bounds<Pixels>, (), window, _cx| {
            let ox = bounds.origin.x;
            let oy = bounds.origin.y;
            let radii = Corners::all(px(1.2));
            let cell = size(px(seg_w), px(seg_h));
            let filled_f = clamped * f32::from(segs);

            for i in 0..segs {
                let idx_f = f32::from(i);
                let col = if idx_f + 1.0 <= filled_f {
                    // Subtle thermal ramp across filled segments (brighter toward active tip)
                    let progress = (idx_f + 1.0) / f32::from(segs);
                    let lut_idx = if progress > 0.85 {
                        245
                    } else if progress > 0.6 {
                        215
                    } else {
                        185
                    };
                    lut.led[lut_idx]
                } else if idx_f < filled_f {
                    lut.led[150]
                } else {
                    dim_col
                };

                let b = Bounds::new(point(ox + px(idx_f * step), oy), cell);
                window.paint_quad(quad(
                    b,
                    radii,
                    col,
                    Edges::all(px(0.0)),
                    transparent_black(),
                    BorderStyle::Solid,
                ));
            }
        },
    )
    .w(px(total_w))
    .h(px(seg_h))
    .flex_shrink_0()
}

/// Single-canvas GPU mini telemetry sparkline (animated live pulse trace).
#[allow(dead_code)]
pub fn led_sparkline(
    phase: f32,
    level_fraction: f32,
    theme: PhosphorTheme,
    width_px: f32,
    height_px: f32,
) -> impl IntoElement + use<> {
    let clamped = level_fraction.clamp(0.06, 1.0);
    let lut = theme.lut();
    let base_line: Hsla = rgba(0xFFFF_FF16).into();

    canvas(
        |_bounds, _window, _cx| (),
        move |bounds: Bounds<Pixels>, (), window, _cx| {
            const BARS: u8 = 22;
            let ox = bounds.origin.x;
            let oy = bounds.origin.y;
            let step = width_px / f32::from(BARS);
            let bar_w = (step - 1.8).max(1.5);

            // Hairline baseline
            window.paint_quad(quad(
                Bounds::new(
                    point(ox, oy + px(height_px - 1.0)),
                    size(px(width_px), px(1.0)),
                ),
                Corners::all(px(0.5)),
                base_line,
                Edges::all(px(0.0)),
                transparent_black(),
                BorderStyle::Solid,
            ));

            for i in 0..BARS {
                let fi = f32::from(i);
                let wave = 0.55
                    + 0.28 * (phase * 2.2 + fi * 0.55).sin()
                    + 0.17 * (phase * 3.7 - fi * 0.92).cos();
                let h = (wave.clamp(0.12, 1.0) * clamped * (height_px - 2.0)).max(2.0);
                let is_tip = i + 2 >= BARS;
                let col = if is_tip { lut.led[238] } else { lut.led[180] };
                let b = Bounds::new(
                    point(ox + px(fi * step), oy + px(height_px - h)),
                    size(px(bar_w), px(h)),
                );
                window.paint_quad(quad(
                    b,
                    Corners::all(px(1.0)),
                    col,
                    Edges::all(px(0.0)),
                    transparent_black(),
                    BorderStyle::Solid,
                ));
            }
        },
    )
    .w(px(width_px))
    .h(px(height_px))
    .flex_shrink_0()
}
