use std::sync::OnceLock;

use gpui::{Hsla, Rgba};

/// Five thermal phosphor LED color ramps ported from the PHOSPHOR Design System.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PhosphorTheme {
    Red,
    Amber,
    Emerald,
    Cyan,
    White,
}

impl PhosphorTheme {
    pub const ALL: [Self; 5] = [
        Self::Red,
        Self::Amber,
        Self::Emerald,
        Self::Cyan,
        Self::White,
    ];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Red => "Red",
            Self::Amber => "Amber",
            Self::Emerald => "Emerald",
            Self::Cyan => "Cyan",
            Self::White => "White",
        }
    }

    /// Precomputed 256-entry thermal & optical bloom lookup tables (zero per-frame math).
    pub fn lut(self) -> &'static ThemeLut {
        let all = ALL_LUTS.get_or_init(AllThemeLuts::build);
        match self {
            Self::Red => &all.red,
            Self::Amber => &all.amber,
            Self::Emerald => &all.emerald,
            Self::Cyan => &all.cyan,
            Self::White => &all.white,
        }
    }

    /// Primary semantic accent color for hardware toggles and pill tags.
    pub fn accent(self) -> Hsla {
        self.lut().led[220]
    }
}

/// Precomputed 256-step lookup tables for a single `PhosphorTheme`.
pub struct ThemeLut {
    /// Physical LED cell color across intensity `0..=255` (base -> mid @ 0.68 -> hot @ 1.0).
    pub led: [Hsla; 256],
    /// Outer soft optical bloom (`alpha = 0..0.22`).
    pub outer_bloom: [Hsla; 256],
    /// Inner tight optical bloom (`alpha = 0..0.36`).
    pub inner_bloom: [Hsla; 256],
}

struct AllThemeLuts {
    red: ThemeLut,
    amber: ThemeLut,
    emerald: ThemeLut,
    cyan: ThemeLut,
    white: ThemeLut,
}

static ALL_LUTS: OnceLock<AllThemeLuts> = OnceLock::new();

impl AllThemeLuts {
    fn build() -> Self {
        Self {
            red: build_theme_lut([33, 15, 13], [235, 42, 26], [255, 132, 94], [255, 58, 36]),
            amber: build_theme_lut([34, 22, 11], [245, 142, 18], [255, 218, 128], [255, 160, 28]),
            emerald: build_theme_lut([12, 30, 20], [20, 216, 118], [158, 255, 206], [34, 229, 132]),
            cyan: build_theme_lut([12, 26, 33], [20, 200, 236], [172, 245, 255], [34, 211, 238]),
            white: build_theme_lut([26, 26, 29], [220, 224, 232], [255, 255, 255], [238, 242, 255]),
        }
    }
}

fn build_theme_lut(base: [u8; 3], mid: [u8; 3], hot: [u8; 3], glow: [u8; 3]) -> ThemeLut {
    let transparent = Hsla::default();
    let mut led = [transparent; 256];
    let mut outer_bloom = [transparent; 256];
    let mut inner_bloom = [transparent; 256];

    let gr = f32::from(glow[0]) / 255.0;
    let gg = f32::from(glow[1]) / 255.0;
    let gb = f32::from(glow[2]) / 255.0;

    for idx_u8 in 0..=255_u8 {
        let idx = usize::from(idx_u8);
        let intensity = f32::from(idx_u8) / 255.0;

        let (r, g, b) = if intensity < 0.68 {
            let k = intensity / 0.68;
            (
                lerp_u8(base[0], mid[0], k),
                lerp_u8(base[1], mid[1], k),
                lerp_u8(base[2], mid[2], k),
            )
        } else {
            let k = (intensity - 0.68) / 0.32;
            (
                lerp_u8(mid[0], hot[0], k),
                lerp_u8(mid[1], hot[1], k),
                lerp_u8(mid[2], hot[2], k),
            )
        };

        led[idx] = Rgba {
            r: r / 255.0,
            g: g / 255.0,
            b: b / 255.0,
            a: 1.0,
        }
        .into();

        if intensity > 0.25 {
            let glow_alpha = (intensity - 0.25) / 0.75;
            outer_bloom[idx] = Rgba {
                r: gr,
                g: gg,
                b: gb,
                a: glow_alpha * 0.22,
            }
            .into();
            inner_bloom[idx] = Rgba {
                r: gr,
                g: gg,
                b: gb,
                a: glow_alpha * 0.36,
            }
            .into();
        }
    }

    ThemeLut {
        led,
        outer_bloom,
        inner_bloom,
    }
}

fn lerp_u8(a: u8, b: u8, t: f32) -> f32 {
    let af = f32::from(a);
    let bf = f32::from(b);
    af + (bf - af) * t
}

/// Quantize a normalized `0.0..=1.0` float into `0..=255` LUT index without `as` casts.
pub fn quantize_255(value: f32) -> u8 {
    let clamped = value.clamp(0.0, 1.0);
    // Binary search over u8 (8 comparisons, branchless-friendly, zero `as` casts)
    let mut lo: u16 = 0;
    let mut hi: u16 = 255;
    let target = clamped * 255.0;
    while lo < hi {
        let mid = (lo + hi).div_ceil(2);
        if f32::from(mid) <= target {
            lo = mid;
        } else {
            hi = mid.saturating_sub(1);
        }
    }
    u8::try_from(lo).unwrap_or(255)
}
