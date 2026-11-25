use gpui::{
    BorderStyle, Bounds, Corners, Div, Edges, Hsla, IntoElement, ParentElement, Pixels, Styled,
    canvas, div, point, px, quad, rgba, size, transparent_black,
};

use crate::phosphor::palette::PhosphorTheme;

/// Single-canvas GPU micro-dot grid backdrop (`11px` hardware dot matrix, zero child divs).
pub fn dot_grid_backdrop(dot_color: Hsla, spacing_px: f32) -> impl IntoElement {
    canvas(
        |_bounds, _window, _cx| (),
        move |bounds: Bounds<Pixels>, (), window, _cx| {
            let step = spacing_px.max(6.0);
            let width_px: f32 = bounds.size.width.into();
            let height_px: f32 = bounds.size.height.into();
            if width_px <= 0.0 || height_px <= 0.0 {
                return;
            }
            let dot = size(px(1.5), px(1.5));
            let radii = Corners::all(px(1.0));
            let mut y = step * 0.5;
            while y < height_px {
                let mut x = step * 0.5;
                while x < width_px {
                    let b = Bounds::new(
                        point(bounds.origin.x + px(x), bounds.origin.y + px(y)),
                        dot,
                    );
                    window.paint_quad(quad(
                        b,
                        radii,
                        dot_color,
                        Edges::all(px(0.0)),
                        transparent_black(),
                        BorderStyle::Solid,
                    ));
                    x += step;
                }
                y += step;
            }
        },
    )
    .absolute()
    .inset_0()
}

/// Authentic recessed `#000000` hardware pill wrapping a PHOSPHOR LED matrix + label + optional tag.
pub fn hw_pill(
    led: impl IntoElement,
    label: impl Into<String>,
    tag: Option<(String, Hsla)>,
) -> Div {
    let pill_bg: Hsla = rgba(0x0000_00FF).into();
    let pill_border: Hsla = rgba(0xFFFF_FF18).into();
    let pill_text: Hsla = rgba(0xF6F6_F6FF).into();

    let mut el = div()
        .px(px(13.0))
        .py(px(8.0))
        .rounded(px(999.0))
        .bg(pill_bg)
        .border_1()
        .border_color(pill_border)
        .flex()
        .items_center()
        .gap(px(10.0))
        .child(led)
        .child(
            div()
                .text_size(px(13.5))
                .font_weight(gpui::FontWeight::MEDIUM)
                .text_color(pill_text)
                .child(label.into()),
        );

    if let Some((tag_txt, tag_col)) = tag {
        el = el.child(
            div()
                .ml_auto()
                .text_size(px(10.5))
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_color(tag_col)
                .child(tag_txt),
        );
    }

    el
}

/// Recessed matte-black hardware socket housing for compact 5x5 / 7x7 LED matrices.
pub fn led_housing(led: impl IntoElement, highlight: Option<Hsla>) -> Div {
    let bg: Hsla = rgba(0x0000_00FF).into();
    let default_border: Hsla = rgba(0xFFFF_FF14).into();
    div()
        .p(px(6.0))
        .rounded(px(10.0))
        .bg(bg)
        .border_1()
        .border_color(highlight.unwrap_or(default_border))
        .flex()
        .items_center()
        .justify_center()
        .flex_shrink_0()
        .child(led)
}

/// Glowing tactile hardware toggle switch (visual indicator; wrap with `.id(...).on_click(...)`).
pub fn hardware_switch_track(enabled: bool, theme: PhosphorTheme, light_mode: bool) -> Div {
    let (track_bg, border_col, knob_col): (Hsla, Hsla, Hsla) = if enabled {
        let accent = theme.accent();
        let tint: Hsla = if light_mode {
            match theme {
                PhosphorTheme::Red => rgba(0xFF3B_2826).into(),
                PhosphorTheme::Amber => rgba(0xFF9F_1C26).into(),
                PhosphorTheme::Emerald => rgba(0x10B9_8128).into(),
                PhosphorTheme::Cyan => rgba(0x0284_C726).into(),
                PhosphorTheme::White => rgba(0x1111_1424).into(),
            }
        } else {
            match theme {
                PhosphorTheme::Red => rgba(0xFF3B_2840).into(),
                PhosphorTheme::Amber => rgba(0xFF9F_1C40).into(),
                PhosphorTheme::Emerald => rgba(0x22E5_8438).into(),
                PhosphorTheme::Cyan => rgba(0x22D3_EE38).into(),
                PhosphorTheme::White => rgba(0xFFFF_FF30).into(),
            }
        };
        (tint, accent, accent)
    } else if light_mode {
        (
            rgba(0xE4E4_EAFF).into(),
            rgba(0x0000_0024).into(),
            rgba(0x7171_7AFF).into(),
        )
    } else {
        (
            rgba(0xFFFF_FF10).into(),
            rgba(0xFFFF_FF20).into(),
            rgba(0xA1A1_AAFF).into(),
        )
    };

    let mut track = div()
        .w(px(34.0))
        .h(px(19.0))
        .px(px(2.5))
        .rounded(px(999.0))
        .bg(track_bg)
        .border_1()
        .border_color(border_col)
        .flex()
        .items_center();

    if enabled {
        track = track.justify_end();
    } else {
        track = track.justify_start();
    }

    track.child(div().w(px(13.0)).h(px(13.0)).rounded(px(999.0)).bg(knob_col))
}
