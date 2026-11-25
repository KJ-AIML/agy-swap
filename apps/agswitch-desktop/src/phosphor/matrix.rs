use std::collections::HashMap;
use std::f32::consts::{FRAC_PI_2, PI, TAU};

use gpui::{
    BorderStyle, Bounds, Corners, Edges, IntoElement, Pixels, Styled, canvas, point, px, quad,
    size, transparent_black,
};

use crate::phosphor::palette::{PhosphorTheme, quantize_255};

/// All 16 procedural & glyph LED matrix animation modes from the PHOSPHOR Design System.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LedMode {
    Analysing,
    Orbital,
    Equalizer,
    Scanner,
    /// Animated sweeping transfer bar, or fixed progress fraction (`Some(0.0..=1.0)`).
    Progress(Option<f32>),
    Syncing,
    Radar,
    Pulse,
    Neural,
    Standby,
    Verified,
    Warning,
    Error,
    Encrypted,
    Verified5,
    Power5,
}

impl LedMode {
    pub const SHOWCASE_MODES: [(&'static str, Self, PhosphorTheme); 12] = [
        ("Analysing", Self::Analysing, PhosphorTheme::Red),
        ("Orbital", Self::Orbital, PhosphorTheme::Red),
        ("Equalizer", Self::Equalizer, PhosphorTheme::Cyan),
        ("Scanner", Self::Scanner, PhosphorTheme::Cyan),
        ("Progress", Self::Progress(None), PhosphorTheme::Amber),
        ("Syncing", Self::Syncing, PhosphorTheme::Cyan),
        ("Radar", Self::Radar, PhosphorTheme::Cyan),
        ("Pulse", Self::Pulse, PhosphorTheme::Emerald),
        ("Neural", Self::Neural, PhosphorTheme::Red),
        ("Verified", Self::Verified, PhosphorTheme::Emerald),
        ("Warning", Self::Warning, PhosphorTheme::Amber),
        ("Encrypted", Self::Encrypted, PhosphorTheme::White),
    ];
}

const GLYPH_VERIFIED_7: [&str; 7] = [
    "0000000", "0000010", "0000110", "0101100", "0111000", "0010000", "0000000",
];
const GLYPH_WARNING_7: [&str; 7] = [
    "0001000", "0001000", "0001000", "0001000", "0000000", "0001000", "0000000",
];
const GLYPH_ERROR_7: [&str; 7] = [
    "0000000", "0100010", "0010100", "0001000", "0010100", "0100010", "0000000",
];
const GLYPH_ENCRYPTED_7: [&str; 7] = [
    "0011100", "0100010", "0100010", "0111110", "0110110", "0111110", "0000000",
];
const GLYPH_CHECK_5: [&str; 5] = ["00001", "00010", "10100", "01000", "00000"];
const GLYPH_POWER_5: [&str; 5] = ["00100", "01110", "10101", "10001", "01110"];

/// Geometry specification for one LED matrix canvas.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LedSpec {
    pub rows: u8,
    pub cols: u8,
    pub trim_corners: bool,
    pub cell_size: f32,
    pub gap: f32,
    pub mode: LedMode,
    pub theme: PhosphorTheme,
    pub phase_offset: f32,
}

impl LedSpec {
    pub fn grid_width(&self) -> f32 {
        let c = f32::from(self.cols);
        c * self.cell_size + (c - 1.0).max(0.0) * self.gap
    }

    pub fn grid_height(&self) -> f32 {
        let r = f32::from(self.rows);
        r * self.cell_size + (r - 1.0).max(0.0) * self.gap
    }

    pub fn compact_5x5(mode: LedMode, theme: PhosphorTheme, phase_offset: f32) -> Self {
        Self {
            rows: 5,
            cols: 5,
            trim_corners: !matches!(mode, LedMode::Verified5 | LedMode::Power5 | LedMode::Syncing),
            cell_size: 3.4,
            gap: 1.3,
            mode,
            theme,
            phase_offset,
        }
    }

    pub fn hero_7x7(mode: LedMode, theme: PhosphorTheme, phase_offset: f32) -> Self {
        Self {
            rows: 7,
            cols: 7,
            trim_corners: !matches!(
                mode,
                LedMode::Scanner | LedMode::Progress(_) | LedMode::Syncing
            ),
            cell_size: 4.4,
            gap: 1.5,
            mode,
            theme,
            phase_offset,
        }
    }
}

#[derive(Debug, Clone)]
struct LedCell {
    r: u8,
    c: u8,
    corner: bool,
    dist: f32,
    angle: f32,
    is_outer_ring: bool,
    x: f32,
    y: f32,
    current: f32,
    target: f32,
    seed: f32,
}

/// Retained physics state for one LED matrix (allocated once, stepped in-place).
#[derive(Debug, Clone)]
struct LedMatrixState {
    rows: u8,
    cols: u8,
    trim_corners: bool,
    cell_size: f32,
    gap: f32,
    mode: LedMode,
    theme: PhosphorTheme,
    phase_offset: f32,
    cells: Vec<LedCell>,
    last_touched_frame: u64,
}

impl LedMatrixState {
    fn from_spec(spec: LedSpec, frame: u64) -> Self {
        let rows = spec.rows.max(1);
        let cols = spec.cols.max(1);
        let cx = (f32::from(cols) - 1.0) * 0.5;
        let cy = (f32::from(rows) - 1.0) * 0.5;
        let max_r = cx.min(cy);

        let total = usize::from(rows) * usize::from(cols);
        let mut cells = Vec::with_capacity(total);

        for r in 0..rows {
            for c in 0..cols {
                let corner = spec.trim_corners
                    && ((r == 0 && c == 0)
                        || (r == 0 && c == cols.saturating_sub(1))
                        || (r == rows.saturating_sub(1) && c == 0)
                        || (r == rows.saturating_sub(1) && c == cols.saturating_sub(1)));

                let dx = f32::from(c) - cx;
                let dy = f32::from(r) - cy;
                let dist = dx.hypot(dy);
                let mut angle = dy.atan2(dx) + FRAC_PI_2;
                if angle < 0.0 {
                    angle += TAU;
                }

                let is_outer_ring = !corner
                    && if rows == 7 && cols == 7 {
                        (2.65..=3.35).contains(&dist)
                    } else {
                        dist >= max_r - 0.35 && dist <= max_r + 0.55
                    };

                let seed = (f32::from(r) * 12.9898 + f32::from(c) * 78.233 + spec.phase_offset)
                    .sin()
                    * 43_758.547;

                cells.push(LedCell {
                    r,
                    c,
                    corner,
                    dist,
                    angle,
                    is_outer_ring,
                    x: f32::from(c) * (spec.cell_size + spec.gap),
                    y: f32::from(r) * (spec.cell_size + spec.gap),
                    current: 0.0,
                    target: 0.0,
                    seed,
                });
            }
        }

        Self {
            rows,
            cols,
            trim_corners: spec.trim_corners,
            cell_size: spec.cell_size,
            gap: spec.gap,
            mode: spec.mode,
            theme: spec.theme,
            phase_offset: spec.phase_offset,
            cells,
            last_touched_frame: frame,
        }
    }

    fn matches_geometry(&self, spec: &LedSpec) -> bool {
        self.rows == spec.rows
            && self.cols == spec.cols
            && self.trim_corners == spec.trim_corners
            && (self.cell_size - spec.cell_size).abs() < 0.01
            && (self.gap - spec.gap).abs() < 0.01
    }

    fn step(&mut self, global_time: f32, dt: f32) {
        self.compute_targets(global_time);
        let attack = 1.0 - (-dt * 28.0).exp();
        let decay = 1.0 - (-dt * 11.0).exp();

        for cell in &mut self.cells {
            if cell.corner {
                continue;
            }
            let rate = if cell.target > cell.current {
                attack
            } else {
                decay
            };
            cell.current += (cell.target - cell.current) * rate;
        }
    }

    fn compute_targets(&mut self, global_time: f32) {
        let t = global_time + self.phase_offset;
        let rows_f = f32::from(self.rows);
        let cols_f = f32::from(self.cols);

        match self.mode {
            LedMode::Analysing => {
                let head = (t * 2.85).rem_euclid(TAU);
                let trail = PI * 1.18;
                let inner_pulse = ((t * 2.15).sin() + 1.0) * 0.5;

                for cell in &mut self.cells {
                    if cell.corner {
                        continue;
                    }
                    let behind = trailing_angle(head, cell.angle);
                    let mut val = 0.0_f32;

                    if cell.is_outer_ring {
                        if behind < trail {
                            let progress = 1.0 - behind / trail;
                            val = progress.powf(1.85);
                            if behind < 0.42 {
                                val = (val * 1.18).min(1.0);
                            }
                        } else if behind > TAU - 0.22 {
                            val = (1.0 - (TAU - behind) / 0.22) * 0.45;
                        }
                    } else if self.rows == 7 && cell.dist > 1.75 && cell.dist < 2.65 {
                        let prox = behind.min(TAU - behind);
                        let glint = (-(prox / 0.52).powi(2)).exp();
                        let harm =
                            0.5 + 0.5 * (t * 3.8 + f32::from(cell.r) * 1.4 + f32::from(cell.c) * 0.9).sin();
                        let gate = if inner_pulse > 0.42 {
                            (inner_pulse - 0.42) / 0.58
                        } else {
                            0.0
                        };
                        if cell.dist > 2.05 && glint > 0.35 && harm > 0.55 {
                            val = glint * gate * (0.45 + 0.55 * harm);
                        }
                    }
                    cell.target = val.clamp(0.0, 1.0);
                }
            }
            LedMode::Orbital => {
                let head = (t * 2.9).rem_euclid(TAU);
                let trail = PI * 1.3;
                for cell in &mut self.cells {
                    if cell.corner {
                        continue;
                    }
                    cell.target = if cell.is_outer_ring {
                        let behind = trailing_angle(head, cell.angle);
                        if behind < trail {
                            (1.0 - behind / trail).powf(1.85)
                        } else {
                            0.0
                        }
                    } else {
                        0.0
                    };
                }
            }
            LedMode::Equalizer => {
                let cols_usize = usize::from(self.cols);
                let denom = (rows_f - 1.0).max(1.0);
                for c in 0..self.cols {
                    let cf = f32::from(c);
                    let wave =
                        0.48 + 0.32 * (t * 5.2 + cf * 0.72).sin() + 0.20 * (t * 8.1 - cf * 1.15).cos();
                    let bar_h = wave.clamp(0.15, 1.0) * rows_f;
                    for r in 0..self.rows {
                        let idx = usize::from(r) * cols_usize + usize::from(c);
                        if let Some(cell) = self.cells.get_mut(idx) {
                            if cell.corner {
                                continue;
                            }
                            let from_bottom = f32::from(self.rows.saturating_sub(1).saturating_sub(r));
                            cell.target = if from_bottom < bar_h - 0.35 {
                                0.65 + 0.35 * (from_bottom / denom)
                            } else if from_bottom < bar_h + 0.35 {
                                0.45
                            } else {
                                0.0
                            };
                        }
                    }
                }
            }
            LedMode::Scanner => {
                let pos = (((t * 2.4).sin() + 1.0) * 0.5) * (cols_f - 1.0).max(0.0);
                let mid_r = (rows_f - 1.0) * 0.5;
                let r_scale = (rows_f * 0.85).max(1.0);
                for cell in &mut self.cells {
                    if cell.corner {
                        continue;
                    }
                    let dist_x = (f32::from(cell.c) - pos).abs();
                    let center_bias = 1.0 - (f32::from(cell.r) - mid_r).abs() / r_scale;
                    cell.target = ((-(dist_x / 2.2).powi(2)).exp() * center_bias).clamp(0.0, 1.0);
                }
            }
            LedMode::Progress(fixed_pct) => {
                let (pct, is_flash) = match fixed_pct {
                    Some(p) => (p.clamp(0.0, 1.0), false),
                    None => {
                        let cycle = (t * 0.32).rem_euclid(1.25);
                        (cycle.min(1.0), cycle > 1.0 && cycle < 1.18)
                    }
                };
                let filled_cols = pct * cols_f;
                let lead_shimmer = ((t * 3.0).sin() + 1.0) * 0.08;

                for cell in &mut self.cells {
                    if cell.corner {
                        continue;
                    }
                    let cf = f32::from(cell.c);
                    if is_flash {
                        cell.target = 0.95;
                    } else if cf + 1.0 <= filled_cols {
                        let lead_boost =
                            (-((filled_cols - cf) / 2.5).powi(2)).exp() * (0.36 + lead_shimmer);
                        cell.target = (0.58 + lead_boost).min(1.0);
                    } else if cf < filled_cols {
                        cell.target = ((filled_cols - cf) * 0.95).clamp(0.0, 1.0);
                    } else {
                        cell.target = 0.0;
                    }
                }
            }
            LedMode::Syncing => {
                let center_col = (cols_f - 1.0) * 0.5;
                for cell in &mut self.cells {
                    if cell.corner {
                        continue;
                    }
                    let phase =
                        (f32::from(cell.r) + (f32::from(cell.c) - center_col).abs() * 0.9 + t * 5.2)
                            .rem_euclid(3.2);
                    cell.target = if phase < 1.05 { 1.0 - phase / 1.05 } else { 0.0 };
                }
            }
            LedMode::Radar => {
                let head = (t * 2.5).rem_euclid(TAU);
                let sweep = PI * 0.9;
                for cell in &mut self.cells {
                    if cell.corner {
                        continue;
                    }
                    if cell.dist < 0.25 {
                        cell.target = 0.45 + 0.25 * (t * 5.0).sin();
                        continue;
                    }
                    let behind = trailing_angle(head, cell.angle);
                    cell.target = if behind < sweep {
                        (1.0 - behind / sweep).powi(2) * (0.55 + 0.45 * (cell.dist / 3.16).min(1.0))
                    } else {
                        0.0
                    };
                }
            }
            LedMode::Pulse => {
                for cell in &mut self.cells {
                    if cell.corner {
                        continue;
                    }
                    let wave = (((cell.dist * 1.2 - t * 3.2).sin() + 1.0) * 0.5).powi(3);
                    cell.target = wave.clamp(0.0, 1.0);
                }
            }
            LedMode::Neural => {
                for cell in &mut self.cells {
                    if cell.corner {
                        continue;
                    }
                    let n1 = (t * 2.5 + cell.seed * 0.01).sin();
                    let n2 = (t * 3.6 - f32::from(cell.r) * 0.9 + f32::from(cell.c) * 1.15).cos();
                    let combined = (n1 * 0.55 + n2 * 0.45 + 1.0) * 0.5;
                    cell.target = if combined > 0.5 {
                        ((combined - 0.5) / 0.5).powf(1.45)
                    } else {
                        0.0
                    };
                }
            }
            LedMode::Standby => {
                let breathe = 0.30 + 0.65 * (((t * 2.2).sin() + 1.0) * 0.5);
                for cell in &mut self.cells {
                    if cell.corner {
                        continue;
                    }
                    cell.target = if cell.dist < 0.8 {
                        breathe
                    } else if cell.dist < 1.5 {
                        breathe * 0.22
                    } else {
                        0.0
                    };
                }
            }
            LedMode::Verified | LedMode::Warning | LedMode::Error | LedMode::Encrypted => {
                let bitmap = match self.mode {
                    LedMode::Verified => &GLYPH_VERIFIED_7,
                    LedMode::Warning => &GLYPH_WARNING_7,
                    LedMode::Error => &GLYPH_ERROR_7,
                    _ => &GLYPH_ENCRYPTED_7,
                };
                let sheen_pos = (t * 4.2).rem_euclid(11.0) - 2.0;
                let scan_y = (t * 3.4).rem_euclid(9.0) - 1.0;
                let fault_pulse = if matches!(self.mode, LedMode::Error) {
                    if (t * 7.5).sin() > -0.15 { 1.0 } else { 0.25 }
                } else {
                    1.0
                };
                let warn_pulse = if matches!(self.mode, LedMode::Warning) {
                    0.62 + 0.38 * (((t * 4.5).sin() + 1.0) * 0.5)
                } else {
                    1.0
                };

                for cell in &mut self.cells {
                    if cell.corner {
                        continue;
                    }
                    let on = glyph_bit_7(bitmap, cell.r, cell.c);
                    if on {
                        let mut val = 0.78 * fault_pulse * warn_pulse;
                        if matches!(self.mode, LedMode::Verified) {
                            let sheen = (-((f32::from(cell.c) - sheen_pos) / 1.1).powi(2)).exp();
                            val = (0.72 + sheen * 0.35).min(1.0);
                        } else if matches!(self.mode, LedMode::Encrypted) {
                            let scan = (-((f32::from(cell.r) - scan_y) / 1.0).powi(2)).exp();
                            val = (0.68 + scan * 0.35).min(1.0);
                        }
                        cell.target = val;
                    } else {
                        cell.target = 0.0;
                    }
                }
            }
            LedMode::Verified5 | LedMode::Power5 => {
                let bitmap = if matches!(self.mode, LedMode::Verified5) {
                    &GLYPH_CHECK_5
                } else {
                    &GLYPH_POWER_5
                };
                let breathe = 0.72 + 0.28 * (((t * 3.1).sin() + 1.0) * 0.5);
                for cell in &mut self.cells {
                    if cell.corner {
                        continue;
                    }
                    cell.target = if glyph_bit_5(bitmap, cell.r, cell.c) {
                        breathe
                    } else {
                        0.0
                    };
                }
            }
        }
    }

    /// Snapshot compact render quads (with cardinal optical neighbor bleed pre-applied).
    fn snapshot_cells(&self) -> LedRenderPayload {
        let rows = usize::from(self.rows);
        let cols = usize::from(self.cols);
        let mut out = Vec::with_capacity(self.cells.len());

        for cell in &self.cells {
            if cell.corner {
                continue;
            }
            let r = usize::from(cell.r);
            let c = usize::from(cell.c);
            let mut neighbor_energy = 0.0_f32;

            if r > 0
                && let Some(n) = self.cells.get((r - 1) * cols + c)
            {
                neighbor_energy += n.current;
            }
            if r + 1 < rows
                && let Some(n) = self.cells.get((r + 1) * cols + c)
            {
                neighbor_energy += n.current;
            }
            if c > 0
                && let Some(n) = self.cells.get(r * cols + (c - 1))
            {
                neighbor_energy += n.current;
            }
            if c + 1 < cols
                && let Some(n) = self.cells.get(r * cols + (c + 1))
            {
                neighbor_energy += n.current;
            }

            let bleed = (neighbor_energy * 0.045).min(0.16);
            let final_intensity = cell.current.max(bleed).min(1.0);

            out.push(PaintedCell {
                x: cell.x,
                y: cell.y,
                led_lut_idx: quantize_255(final_intensity),
                bloom_lut_idx: if cell.current >= 0.25 {
                    quantize_255(cell.current)
                } else {
                    0
                },
            });
        }

        LedRenderPayload {
            theme: self.theme,
            cell_size: self.cell_size,
            cells: out,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct PaintedCell {
    x: f32,
    y: f32,
    led_lut_idx: u8,
    bloom_lut_idx: u8,
}

#[derive(Debug, Clone)]
pub struct LedRenderPayload {
    theme: PhosphorTheme,
    cell_size: f32,
    cells: Vec<PaintedCell>,
}

/// Shared retained pool of all visible PHOSPHOR LED matrices.
/// One shared 30 FPS clock advances every active matrix in a tight cache-friendly loop,
/// then GPUI paints each matrix as a SINGLE `canvas` node (zero child `div` diffing!).
#[derive(Debug, Default)]
pub struct PhosphorPool {
    matrices: HashMap<String, LedMatrixState>,
    elapsed: f32,
    frame_counter: u64,
    pub paused: bool,
}

impl PhosphorPool {
    pub fn new() -> Self {
        Self::default()
    }

    /// Advance every retained matrix in one batch (`dt` clamped to 50ms for stability).
    pub fn tick(&mut self, dt_secs: f32) {
        if self.paused {
            return;
        }
        let dt = dt_secs.clamp(0.001, 0.05);
        self.elapsed += dt;
        self.frame_counter = self.frame_counter.saturating_add(1);

        // Evict stale matrices that haven't been rendered for > 90 frames (~3s)
        let cutoff = self.frame_counter.saturating_sub(90);
        self.matrices.retain(|_, m| m.last_touched_frame >= cutoff);

        for matrix in self.matrices.values_mut() {
            matrix.step(self.elapsed, dt);
        }
    }

    /// Fetch or initialize a matrix by key and return its compact GPU quad payload.
    pub fn render_matrix(&mut self, key: &str, spec: LedSpec) -> impl IntoElement + use<> {
        let frame = self.frame_counter;
        let entry = self
            .matrices
            .entry(key.to_owned())
            .or_insert_with(|| {
                let mut m = LedMatrixState::from_spec(spec, frame);
                m.step(self.elapsed, 0.033);
                m
            });

        if !entry.matches_geometry(&spec) {
            let mut rebuilt = LedMatrixState::from_spec(spec, frame);
            rebuilt.step(self.elapsed, 0.033);
            *entry = rebuilt;
        } else {
            entry.mode = spec.mode;
            entry.theme = spec.theme;
            entry.last_touched_frame = frame;
        }

        let payload = entry.snapshot_cells();
        let w = spec.grid_width();
        let h = spec.grid_height();

        canvas(
            move |_bounds, _window, _cx| payload,
            move |bounds: Bounds<Pixels>, data: LedRenderPayload, window, _cx| {
                let lut = data.theme.lut();
                let s = data.cell_size;
                let radius = (s * 0.28).max(1.0);
                let ox = bounds.origin.x;
                let oy = bounds.origin.y;

                // Pass 1: Dual soft optical bloom quads (zero shadowBlur overhead)
                for cell in &data.cells {
                    if cell.bloom_lut_idx == 0 {
                        continue;
                    }
                    let idx = usize::from(cell.bloom_lut_idx);
                    let intensity = f32::from(cell.bloom_lut_idx) / 255.0;

                    let outer_exp = s * (0.48 + intensity * 0.32);
                    let outer_bounds = Bounds::new(
                        point(ox + px(cell.x - outer_exp), oy + px(cell.y - outer_exp)),
                        size(px(s + outer_exp * 2.0), px(s + outer_exp * 2.0)),
                    );
                    window.paint_quad(quad(
                        outer_bounds,
                        Corners::all(px(radius + outer_exp * 0.7)),
                        lut.outer_bloom[idx],
                        Edges::all(px(0.0)),
                        transparent_black(),
                        BorderStyle::Solid,
                    ));

                    let inner_exp = s * (0.20 + intensity * 0.16);
                    let inner_bounds = Bounds::new(
                        point(ox + px(cell.x - inner_exp), oy + px(cell.y - inner_exp)),
                        size(px(s + inner_exp * 2.0), px(s + inner_exp * 2.0)),
                    );
                    window.paint_quad(quad(
                        inner_bounds,
                        Corners::all(px(radius + inner_exp * 0.5)),
                        lut.inner_bloom[idx],
                        Edges::all(px(0.0)),
                        transparent_black(),
                        BorderStyle::Solid,
                    ));
                }

                // Pass 2: Physical rounded-square LED cells from 256-entry thermal LUT
                for cell in &data.cells {
                    let idx = usize::from(cell.led_lut_idx);
                    let cell_bounds = Bounds::new(
                        point(ox + px(cell.x), oy + px(cell.y)),
                        size(px(s), px(s)),
                    );
                    window.paint_quad(quad(
                        cell_bounds,
                        Corners::all(px(radius)),
                        lut.led[idx],
                        Edges::all(px(0.0)),
                        transparent_black(),
                        BorderStyle::Solid,
                    ));
                }
            },
        )
        .w(px(w))
        .h(px(h))
        .flex_shrink_0()
    }
}

fn trailing_angle(head: f32, cell: f32) -> f32 {
    (head - cell).rem_euclid(TAU)
}

fn glyph_bit_7(bitmap: &[&str; 7], r: u8, c: u8) -> bool {
    bitmap
        .get(usize::from(r))
        .and_then(|row| row.as_bytes().get(usize::from(c)))
        .is_some_and(|&b| b == b'1')
}

fn glyph_bit_5(bitmap: &[&str; 5], r: u8, c: u8) -> bool {
    bitmap
        .get(usize::from(r))
        .and_then(|row| row.as_bytes().get(usize::from(c)))
        .is_some_and(|&b| b == b'1')
}
