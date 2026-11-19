//! Pure presentation helpers shared by the 2D and 3D views (no web-sys, host-testable):
//! colour map + LUT, grid → texture packing, plan ↔ world mapping, floor picking, iso-contours,
//! wall crossings along a ray, layout fitting and the first-person camera.
use crate::math::{self, M4, V};
use wifi_core::{Band, Material, Plan};

/// dBm colour stops; anything at or below -80 is a dead zone.
pub const STOPS: [(f32, [u8; 3]); 5] = [
    (-50.0, [0, 150, 60]),
    (-60.0, [120, 200, 40]),
    (-67.0, [240, 220, 40]),
    (-70.0, [250, 150, 30]),
    (-80.0, [220, 40, 40]),
];
pub const DEAD: [u8; 3] = [90, 30, 110];
/// Legend / contour levels (dBm, or SINR mapped onto the dBm scale).
pub const LEVELS: [f32; 5] = [-50.0, -60.0, -67.0, -70.0, -80.0];
/// SINR legend values that land on `LEVELS` through `sinr_as_dbm`.
pub const SINR_LEVELS: [f32; 5] = [25.0, 17.0, 11.0, 8.0, 0.0];

pub fn color(dbm: f32) -> [u8; 3] {
    if dbm >= STOPS[0].0 {
        return STOPS[0].1;
    }
    if dbm <= -80.0 {
        return DEAD;
    }
    for w in STOPS.windows(2) {
        let ((a, ca), (b, cb)) = (w[0], w[1]);
        if dbm >= b {
            let t = (a - dbm) / (a - b);
            return [0, 1, 2].map(|i| (ca[i] as f32 + (cb[i] as f32 - ca[i] as f32) * t).round() as u8);
        }
    }
    DEAD
}

/// SINR shown on the same colour scale: 25 dB -> -50, 0 dB -> -80.
pub fn sinr_as_dbm(s: f32) -> f32 {
    -80.0 + s * 1.2
}

/// Value range mapped to 0..1 in the half-float field texture (and covered by the LUT).
pub const PACK_LO: f32 = -100.0;
pub const PACK_HI: f32 = -30.0;
pub const LUT_N: usize = 256;

/// IEEE 754 binary16 bits of `v` (round to nearest; overflow → inf, tiny → subnormal / 0).
pub fn f16_bits(v: f32) -> u16 {
    let x = v.to_bits();
    let sign = ((x >> 16) & 0x8000) as u16;
    if v.is_nan() {
        return sign | 0x7e00;
    }
    let exp = ((x >> 23) & 0xff) as i32 - 127 + 15;
    let mant = x & 0x7f_ffff;
    if exp >= 31 {
        return sign | 0x7c00;
    }
    if exp <= 0 {
        if exp < -10 {
            return sign;
        }
        let m = mant | 0x80_0000;
        let shift = (14 - exp) as u32;
        return sign | ((m + (1 << (shift - 1))) >> shift) as u16;
    }
    // rounding may carry into the exponent, which is still the correct result
    sign | ((((exp as u32) << 10) | (mant >> 13)) + ((mant >> 12) & 1)) as u16
}

/// binary16 bits → f32.
#[cfg_attr(not(test), allow(dead_code))]
pub fn f16_value(h: u16) -> f32 {
    let sign = if h & 0x8000 != 0 { -1.0 } else { 1.0 };
    let mant = (h & 0x3ff) as f32;
    sign * match (h >> 10) & 0x1f {
        0 => mant * 2f32.powi(-24),
        31 if mant == 0.0 => f32::INFINITY,
        31 => f32::NAN,
        e => (1.0 + mant / 1024.0) * 2f32.powi(e as i32 - 15),
    }
}

/// dBm → texel: position 0..1 in `PACK_LO..PACK_HI` (clamped) as binary16.
pub fn pack(dbm: f32) -> u16 {
    f16_bits(((dbm - PACK_LO) / (PACK_HI - PACK_LO)).clamp(0.0, 1.0))
}

/// Inverse of `pack` (what the shader reconstructs).
#[cfg_attr(not(test), allow(dead_code))]
pub fn unpack(v: u16) -> f32 {
    PACK_LO + (PACK_HI - PACK_LO) * f16_value(v)
}

/// Row-major `w × h` grid of dBm → one half-float texel each, same layout
/// (row 0 = plan y 0 = world z 0). Half floats keep the bilinear surface smooth for iso-lines.
pub fn pack_grid(vals: &[f32], w: usize, h: usize) -> Vec<u16> {
    assert_eq!(vals.len(), w * h, "grid size");
    vals.iter().map(|&v| pack(v)).collect()
}

/// dBm at LUT texel `k` (the shader maps 0..1 onto texel centres 0..LUT_N-1).
pub fn lut_dbm(k: usize) -> f32 {
    PACK_LO + (PACK_HI - PACK_LO) * k as f32 / (LUT_N - 1) as f32
}

/// 256 × 1 RGB lookup table: texel k is `color(lut_dbm(k))`, so the shader matches the 2D view.
pub fn lut() -> Vec<u8> {
    (0..LUT_N).flat_map(|k| color(lut_dbm(k))).collect()
}

/// CSS `linear-gradient` stops for the legend bar spanning `lo..hi` dBm (left → right).
pub fn legend_gradient(lo: f32, hi: f32) -> String {
    let n = 24;
    let stops: Vec<String> = (0..=n)
        .map(|i| {
            let t = i as f32 / n as f32;
            let c = color(lo + (hi - lo) * t);
            format!("rgb({},{},{}) {:.1}%", c[0], c[1], c[2], t * 100.0)
        })
        .collect();
    format!("linear-gradient(90deg,{})", stops.join(","))
}

/// Position (0..1) of a dBm value on the legend bar.
pub fn legend_pos(dbm: f32, lo: f32, hi: f32) -> f32 {
    ((dbm - lo) / (hi - lo)).clamp(0.0, 1.0)
}

/// Display colour (sRGB) and physical thickness (m) per material, shared by 2D, 3D and chips.
pub fn material_style(m: Material) -> ([u8; 3], f64) {
    match m {
        Material::Drywall => ([203, 213, 225], 0.10),
        Material::WoodDoor => ([194, 132, 82], 0.06),
        Material::Glass => ([96, 200, 250], 0.05),
        Material::Brick => ([214, 104, 84], 0.18),
        Material::Concrete => ([148, 160, 178], 0.25),
        Material::Metal => ([110, 118, 132], 0.12),
        Material::Mirror => ([186, 204, 220], 0.06),
    }
}

pub fn hex(c: [u8; 3]) -> String {
    format!("#{:02x}{:02x}{:02x}", c[0], c[1], c[2])
}

pub fn rgbf(c: [u8; 3]) -> V {
    c.map(|v| v as f32 / 255.0)
}

/// Plan cell coordinates → world (x right, y up, z = plan y down), metres.
pub fn cell_to_world(plan: &Plan, x: f64, y: f64, h: f32) -> V {
    [(x * plan.cell_m) as f32, h, (y * plan.cell_m) as f32]
}

/// World point → plan cell coordinates (not clamped).
pub fn world_to_cell(plan: &Plan, p: V) -> (f64, f64) {
    (p[0] as f64 / plan.cell_m, p[2] as f64 / plan.cell_m)
}

/// Canvas pixel (origin top-left) → normalised device coordinates (y up).
pub fn to_ndc(px: f64, py: f64, w: f64, h: f64) -> (f32, f32) {
    ((px / w.max(1.0) * 2.0 - 1.0) as f32, (1.0 - py / h.max(1.0) * 2.0) as f32)
}

/// Screen ray through NDC (x, y) meets the floor y = `height`.
pub fn floor_pick(inv_vp: &M4, x: f32, y: f32, height: f32) -> Option<V> {
    let (o, d) = math::ndc_ray(inv_vp, x, y);
    math::ray_plane(o, d, [0.0, height, 0.0], [0.0, 1.0, 0.0])
}

/// Floor pick straight to plan cells, `None` off the plan.
pub fn pick_cell(plan: &Plan, inv_vp: &M4, x: f32, y: f32) -> Option<(f64, f64)> {
    let c = world_to_cell(plan, floor_pick(inv_vp, x, y, 0.0)?);
    let inside = (0.0..=plan.width as f64).contains(&c.0) && (0.0..=plan.height as f64).contains(&c.1);
    inside.then_some(c)
}

/// Iso-line segments of a row-major grid sampled at cell centres (marching squares),
/// in cell coordinates.
pub fn contours(vals: &[f32], w: usize, h: usize, level: f32) -> Vec<[(f32, f32); 2]> {
    let mut out = Vec::new();
    if w < 2 || h < 2 {
        return out;
    }
    let at = |i: usize, j: usize| vals[j * w + i];
    for j in 0..h - 1 {
        for i in 0..w - 1 {
            // corners: 0 = (i, j), 1 = (i+1, j), 2 = (i+1, j+1), 3 = (i, j+1)
            let c = [at(i, j), at(i + 1, j), at(i + 1, j + 1), at(i, j + 1)];
            let p = [(i, j), (i + 1, j), (i + 1, j + 1), (i, j + 1)].map(|(a, b)| (a as f32 + 0.5, b as f32 + 0.5));
            let mut pts = Vec::with_capacity(4);
            for e in 0..4 {
                let (a, b) = (e, (e + 1) % 4);
                if (c[a] >= level) != (c[b] >= level) {
                    let t = (level - c[a]) / (c[b] - c[a]);
                    pts.push((p[a].0 + (p[b].0 - p[a].0) * t, p[a].1 + (p[b].1 - p[a].1) * t));
                }
            }
            match pts.len() {
                2 => out.push([pts[0], pts[1]]),
                // saddle: pair edges by the centre value
                4 => {
                    let centre = c.iter().sum::<f32>() / 4.0;
                    if (centre >= level) == (c[0] >= level) {
                        out.push([pts[0], pts[1]]);
                        out.push([pts[2], pts[3]]);
                    } else {
                        out.push([pts[0], pts[3]]);
                        out.push([pts[1], pts[2]]);
                    }
                }
                _ => {}
            }
        }
    }
    out
}

/// One wall crossed by a straight ray.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Crossing {
    /// Fraction along the ray (0 = start, 1 = end).
    pub t: f64,
    pub material: Material,
    pub loss_db: f64,
}

/// Walls crossed by p0 → p1, sorted along the ray, with their per-band loss.
pub fn ray_crossings(plan: &Plan, band: Band, p0: (f64, f64), p1: (f64, f64)) -> Vec<Crossing> {
    let cross = |ax: f64, ay: f64, bx: f64, by: f64| ax * by - ay * bx;
    let (rx, ry) = (p1.0 - p0.0, p1.1 - p0.1);
    let mut out: Vec<Crossing> = plan
        .walls
        .iter()
        .filter_map(|w| {
            let (sx, sy) = (w.x2 - w.x1, w.y2 - w.y1);
            let d = cross(rx, ry, sx, sy);
            if d.abs() < 1e-12 {
                return None;
            }
            let (qx, qy) = (w.x1 - p0.0, w.y1 - p0.1);
            let t = cross(qx, qy, sx, sy) / d;
            let u = cross(qx, qy, rx, ry) / d;
            const E: f64 = 1e-9;
            ((-E..=1.0 + E).contains(&t) && (-E..=1.0 + E).contains(&u)).then(|| Crossing {
                t: t.clamp(0.0, 1.0),
                material: w.material,
                loss_db: plan.materials.loss(w.material, band),
            })
        })
        .collect();
    out.sort_by(|a, b| a.t.total_cmp(&b.t));
    out
}

/// Colour for one wall crossing by its loss: yellow (small) → red (large) → magenta (≥ 30 dB).
pub fn loss_color(db: f64) -> [u8; 3] {
    let t = (db / 30.0).clamp(0.0, 1.0) as f32;
    let lerp = |a: f32, b: f32, t: f32| (a + (b - a) * t).round() as u8;
    if t < 0.5 {
        let k = t * 2.0;
        [lerp(250.0, 245.0, k), lerp(220.0, 90.0, k), lerp(60.0, 40.0, k)]
    } else {
        let k = (t - 0.5) * 2.0;
        [lerp(245.0, 220.0, k), lerp(90.0, 40.0, k), lerp(40.0, 200.0, k)]
    }
}

/// Largest rectangle with aspect `aw:ah` that fits in `w × h`, centred: (x, y, w, h).
pub fn fit_rect(x: f64, y: f64, w: f64, h: f64, aw: f64, ah: f64) -> (f64, f64, f64, f64) {
    let (w, h) = (w.max(1.0), h.max(1.0));
    let s = (w / aw.max(1e-9)).min(h / ah.max(1e-9));
    let (fw, fh) = (aw * s, ah * s);
    (x + (w - fw) / 2.0, y + (h - fh) / 2.0, fw, fh)
}

/// First-person camera on the floor plan (eye height fixed). yaw 0 looks toward −z (plan "up").
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Fps {
    pub pos: V,
    pub yaw: f32,
    pub pitch: f32,
}

impl Fps {
    pub const EYE: f32 = 1.6;

    pub fn forward(&self) -> V {
        let (sy, cy) = self.yaw.sin_cos();
        let (sp, cp) = self.pitch.sin_cos();
        [-sy * cp, sp, -cy * cp]
    }
    /// Horizontal right vector.
    pub fn right(&self) -> V {
        let (sy, cy) = self.yaw.sin_cos();
        [cy, 0.0, -sy]
    }
    pub fn view(&self) -> M4 {
        math::look_at(self.pos, math::add(self.pos, self.forward()), [0.0, 1.0, 0.0])
    }
    /// Turn by a look delta (rad); positive `dyaw` turns right, positive `dpitch` looks up.
    pub fn look(&mut self, dyaw: f32, dpitch: f32) {
        self.yaw -= dyaw;
        self.pitch = (self.pitch + dpitch).clamp(-1.4, 1.4);
    }
    /// Walk `fwd` metres forward and `side` metres right on the floor, clamped to `max` (x, z).
    pub fn walk(&mut self, fwd: f32, side: f32, max: (f32, f32)) {
        let (sy, cy) = self.yaw.sin_cos();
        let r = self.right();
        self.pos[0] = (self.pos[0] - sy * fwd + r[0] * side).clamp(0.0, max.0);
        self.pos[2] = (self.pos[2] - cy * fwd + r[2] * side).clamp(0.0, max.1);
        self.pos[1] = Self::EYE;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wifi_core::{Ap, Wall};

    fn close(a: f32, b: f32, tol: f32) {
        assert!((a - b).abs() < tol, "{a} vs {b}");
    }

    #[test]
    fn colour_map_hits_stops_and_dead_zone() {
        for (db, c) in STOPS {
            if db > -80.0 {
                assert_eq!(color(db), c, "{db}");
            }
        }
        assert_eq!(color(-30.0), STOPS[0].1);
        assert_eq!(color(-80.0), DEAD);
        assert_eq!(color(-110.0), DEAD);
        // halfway between -50 and -60
        assert_eq!(color(-55.0), [60, 175, 50]);
        for (s, l) in SINR_LEVELS.iter().zip(LEVELS) {
            close(sinr_as_dbm(*s), l, 0.5);
        }
    }

    #[test]
    fn lut_matches_colour_map() {
        let t = lut();
        assert_eq!(t.len(), LUT_N * 3);
        assert_eq!(lut_dbm(0), PACK_LO);
        assert_eq!(lut_dbm(LUT_N - 1), PACK_HI);
        for db in [-95.0, -79.0, -68.5, -61.0, -45.0] {
            // texel nearest to the value, as the shader's lookup picks it
            let k = (f16_value(pack(db)) * (LUT_N - 1) as f32).round() as usize;
            assert_eq!(&t[k * 3..k * 3 + 3], &color(lut_dbm(k)));
            let exact = color(db);
            for i in 0..3 {
                assert!((t[k * 3 + i] as i32 - exact[i] as i32).abs() <= 8, "{db}");
            }
        }
    }

    #[test]
    fn half_floats() {
        assert_eq!(f16_bits(0.0), 0);
        assert_eq!(f16_bits(1.0), 0x3c00);
        assert_eq!(f16_bits(0.5), 0x3800);
        assert_eq!(f16_bits(-2.0), 0xc000);
        assert_eq!(f16_bits(65504.0), 0x7bff);
        assert_eq!(f16_bits(1e6), 0x7c00);
        assert_eq!(f16_bits(2f32.powi(-24)), 1, "smallest subnormal");
        assert!(f16_value(f16_bits(f32::NAN)).is_nan());
        for v in [0.0, 1e-5, 0.1234, 0.33333, 0.5, 0.75, 0.999, 1.0] {
            let back = f16_value(f16_bits(v));
            assert!((back - v).abs() <= v.abs() / 2048.0 + 1e-7, "{v} -> {back}");
        }
    }

    #[test]
    fn grid_packing_round_trips_and_keeps_layout() {
        assert_eq!(pack(-200.0), 0);
        assert_eq!(pack(0.0), 0x3c00);
        for db in [-99.0, -80.0, -67.0, -50.0, -31.0] {
            close(unpack(pack(db)), db, 0.05); // half-float: ~0.03 dB steps at worst
        }
        // 3 × 2 grid: row-major, row 0 first
        let g = pack_grid(&[-100.0, -65.0, -30.0, -30.0, -65.0, -100.0], 3, 2);
        assert_eq!(g, vec![0, 0x3800, 0x3c00, 0x3c00, 0x3800, 0]);
    }

    #[test]
    #[should_panic(expected = "grid size")]
    fn grid_packing_rejects_wrong_size() {
        pack_grid(&[0.0; 5], 3, 2);
    }

    #[test]
    fn ray_plane_pick_lands_on_projected_floor_point() {
        let plan = Plan::new(120, 90, 0.1);
        let vp = math::mul(
            &math::perspective(0.9, 1.4, 0.05, 200.0),
            &math::look_at([6.0, 9.0, 14.0], [6.0, 0.0, 4.5], [0.0, 1.0, 0.0]),
        );
        let inv = math::invert(&vp).unwrap();
        let target = cell_to_world(&plan, 30.0, 70.0, 0.0);
        let s = math::transform(&vp, target);
        let hit = floor_pick(&inv, s[0], s[1], 0.0).unwrap();
        for i in 0..3 {
            close(hit[i], target[i], 1e-3);
        }
        let c = pick_cell(&plan, &inv, s[0], s[1]).unwrap();
        assert!((c.0 - 30.0).abs() < 0.05 && (c.1 - 70.0).abs() < 0.05, "{c:?}");
        // looking at the sky: no floor hit
        let top = math::transform(&vp, [6.0, 50.0, -40.0]);
        assert!(floor_pick(&inv, top[0], top[1], 0.0).is_none());
        // pixel ↔ NDC
        assert_eq!(to_ndc(0.0, 0.0, 200.0, 100.0), (-1.0, 1.0));
        assert_eq!(to_ndc(100.0, 50.0, 200.0, 100.0), (0.0, 0.0));
    }

    #[test]
    fn top_view_is_not_mirrored() {
        // Top preset: camera above, screen right = +x (plan right), screen down = +z (plan down).
        let o = math::Orbit { target: [6.0, 0.0, 4.5], yaw: 0.0, pitch: math::PITCH_MAX, dist: 15.0 };
        let vp = math::mul(&math::perspective(0.9, 1.0, 0.05, 100.0), &math::look_at(o.eye(), o.target, [0.0, 1.0, 0.0]));
        let c = math::transform(&vp, [6.0, 0.0, 4.5]);
        let r = math::transform(&vp, [8.0, 0.0, 4.5]);
        let d = math::transform(&vp, [6.0, 0.0, 6.5]);
        assert!(r[0] > c[0] + 0.05, "right");
        assert!(d[1] < c[1] - 0.05, "down");
    }

    #[test]
    fn contours_of_radial_field_follow_the_circle() {
        let (w, h) = (40, 40);
        let vals: Vec<f32> = (0..w * h)
            .map(|k| {
                let (x, y) = ((k % w) as f32 + 0.5 - 20.0, (k / w) as f32 + 0.5 - 20.0);
                -(x * x + y * y).sqrt()
            })
            .collect();
        let segs = contours(&vals, w, h, -10.0);
        assert!(segs.len() > 40, "{}", segs.len());
        for s in &segs {
            for p in s {
                close(((p.0 - 20.0).powi(2) + (p.1 - 20.0).powi(2)).sqrt(), 10.0, 0.3);
            }
        }
        assert!(contours(&vals, w, h, 5.0).is_empty(), "level above everything");
        assert!(contours(&[1.0], 1, 1, 0.5).is_empty());
    }

    #[test]
    fn ray_crossings_sorted_with_loss() {
        let mut plan = Plan::new(100, 100, 0.1);
        let wall = |x: f64, m| Wall { x1: x, y1: 0.0, x2: x, y2: 100.0, material: m };
        plan.walls = vec![wall(70.0, Material::Concrete), wall(30.0, Material::Drywall), wall(110.0, Material::Metal)];
        plan.aps.push(Ap { x: 10.0, y: 50.0, band: Band::G5, tx_dbm: 20.0, gain_dbi: 0.0, channel: 36 });
        let c = ray_crossings(&plan, Band::G5, (10.0, 50.0), (90.0, 50.0));
        assert_eq!(c.len(), 2);
        assert_eq!(c[0].material, Material::Drywall);
        close(c[0].t as f32, 0.25, 1e-6);
        assert_eq!(c[1].loss_db, plan.materials.loss(Material::Concrete, Band::G5));
        let total: f64 = c.iter().map(|x| x.loss_db).sum();
        assert_eq!(total, wifi_core::direct_wall_loss(&plan, Band::G5, (10.0, 50.0), (90.0, 50.0)));
        assert!(ray_crossings(&plan, Band::G5, (10.0, 50.0), (20.0, 50.0)).is_empty());
        assert_ne!(loss_color(3.0), loss_color(30.0));
    }

    #[test]
    fn fit_rect_keeps_aspect_and_centres() {
        let (x, y, w, h) = fit_rect(10.0, 20.0, 400.0, 100.0, 4.0, 3.0);
        close((w / h) as f32, 4.0 / 3.0, 1e-6);
        assert_eq!(h, 100.0);
        close((x + w / 2.0) as f32, 210.0, 1e-6);
        assert_eq!(y, 20.0);
    }

    #[test]
    fn first_person_walks_where_it_looks() {
        let mut f = Fps { pos: [5.0, Fps::EYE, 5.0], yaw: 0.0, pitch: 0.0 };
        f.walk(1.0, 0.0, (10.0, 10.0));
        close(f.pos[2], 4.0, 1e-6); // forward = −z
        f.walk(0.0, 1.0, (10.0, 10.0));
        close(f.pos[0], 6.0, 1e-6); // right = +x
        f.look(std::f32::consts::FRAC_PI_2, 0.0); // turn right 90°: now facing +x
        let d = f.forward();
        close(d[0], 1.0, 1e-5);
        f.walk(100.0, 0.0, (10.0, 10.0));
        close(f.pos[0], 10.0, 1e-6); // clamped to the plan
        f.look(0.0, 10.0);
        close(f.pitch, 1.4, 1e-6);
        // the view matrix puts the look direction at screen centre
        f.pitch = 0.2;
        let vp = math::mul(&math::perspective(1.0, 1.0, 0.05, 50.0), &f.view());
        let c = math::transform(&vp, math::add(f.pos, math::scale(f.forward(), 3.0)));
        close(c[0], 0.0, 1e-4);
        close(c[1], 0.0, 1e-4);
    }

    #[test]
    fn legend_helpers() {
        let g = legend_gradient(-85.0, -45.0);
        assert!(g.starts_with("linear-gradient(90deg,rgb("));
        assert!(g.ends_with("100.0%)"));
        close(legend_pos(-65.0, -85.0, -45.0), 0.5, 1e-6);
        assert_eq!(hex([255, 0, 16]), "#ff0010");
    }
}
