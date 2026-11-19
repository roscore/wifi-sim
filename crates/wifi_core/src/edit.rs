//! Pure wall-editing geometry for the floor-plan editor: joints, snapping, polyline / room
//! construction, openings and box-select hit tests. Units are plan cells, like [`Wall`]; callers
//! convert metres with `Plan::cell_m`. Walls stay plain segments, so the propagation model and the
//! saved-plan format do not change: a "joint" is just endpoints that coincide.

use crate::{Material, Wall};

pub type P = (f64, f64);

/// Endpoints closer than this (cells) form a joint.
pub const JOINT_EPS: f64 = 1e-3;
/// Soft angle snap: directions within this many degrees of a multiple of 45° snap to it.
pub const ANGLE_TOL_DEG: f64 = 8.0;

/// One endpoint of one wall (`b` = the (x2, y2) end).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct End {
    pub wall: usize,
    pub b: bool,
}

fn dist(a: P, b: P) -> f64 {
    (a.0 - b.0).hypot(a.1 - b.1)
}

pub fn end_pos(w: &Wall, b: bool) -> P {
    if b {
        (w.x2, w.y2)
    } else {
        (w.x1, w.y1)
    }
}

pub fn set_end(w: &mut Wall, b: bool, p: P) {
    if b {
        (w.x2, w.y2) = p;
    } else {
        (w.x1, w.y1) = p;
    }
}

pub fn length(w: &Wall) -> f64 {
    (w.x2 - w.x1).hypot(w.y2 - w.y1)
}

pub fn midpoint(w: &Wall) -> P {
    ((w.x1 + w.x2) / 2.0, (w.y1 + w.y2) / 2.0)
}

/// Every wall endpoint within `eps` of `p` (the joint at `p`).
pub fn ends_at(walls: &[Wall], p: P, eps: f64) -> Vec<End> {
    let mut out = Vec::new();
    for (i, w) in walls.iter().enumerate() {
        for b in [false, true] {
            if dist(end_pos(w, b), p) <= eps {
                out.push(End { wall: i, b });
            }
        }
    }
    out
}

/// Moves every listed endpoint to exactly `p` (a joint drag: connected walls stay connected).
pub fn move_ends(walls: &mut [Wall], ends: &[End], p: P) {
    for e in ends {
        if let Some(w) = walls.get_mut(e.wall) {
            set_end(w, e.b, p);
        }
    }
}

/// Closest point of the wall to `p`: (parameter t in 0..=1, point).
pub fn project(w: &Wall, p: P) -> (f64, P) {
    let (dx, dy) = (w.x2 - w.x1, w.y2 - w.y1);
    let l2 = dx * dx + dy * dy;
    let t = if l2 > 0.0 { (((p.0 - w.x1) * dx + (p.1 - w.y1) * dy) / l2).clamp(0.0, 1.0) } else { 0.0 };
    (t, (w.x1 + t * dx, w.y1 + t * dy))
}

pub fn dist_to_wall(w: &Wall, p: P) -> f64 {
    dist(project(w, p).1, p)
}

/// Nearest wall within `tol` of `p`.
pub fn nearest_wall(walls: &[Wall], p: P, tol: f64) -> Option<usize> {
    walls
        .iter()
        .enumerate()
        .map(|(i, w)| (i, dist_to_wall(w, p)))
        .filter(|(_, d)| *d <= tol)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(i, _)| i)
}

/// Nearest endpoint within `tol` of `p`, ignoring walls for which `skip` is true.
pub fn nearest_end(walls: &[Wall], p: P, tol: f64, skip: impl Fn(usize) -> bool) -> Option<(End, P)> {
    let mut best: Option<(End, P, f64)> = None;
    for (i, w) in walls.iter().enumerate().filter(|(i, _)| !skip(*i)) {
        for b in [false, true] {
            let q = end_pos(w, b);
            let d = dist(q, p);
            if d <= tol && best.is_none_or(|(_, _, bd)| d < bd) {
                best = Some((End { wall: i, b }, q, d));
            }
        }
    }
    best.map(|(e, q, _)| (e, q))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SnapKind {
    None,
    Grid,
    Angle,
    OnWall,
    Endpoint,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Snap {
    pub p: P,
    pub kind: SnapKind,
}

#[derive(Clone, Copy, Debug)]
pub struct SnapOpts {
    /// Magnet radius for endpoints and walls (cells).
    pub radius: f64,
    /// Grid step (cells), `None` = off.
    pub grid: Option<f64>,
    /// Start of the segment being drawn: enables the 0/45/90° angle snap and length rounding.
    pub from: Option<P>,
    pub angle: bool,
}

fn round_to(v: f64, step: f64) -> f64 {
    (v / step).round() * step
}

/// Snaps `dir` (from `from` to `raw`) to the nearest multiple of 45° if within the tolerance.
fn angle_snap(from: P, raw: P) -> Option<(f64, f64)> {
    let (dx, dy) = (raw.0 - from.0, raw.1 - from.1);
    if dx.hypot(dy) < 1e-9 {
        return None;
    }
    let step = std::f64::consts::FRAC_PI_4;
    let a = dy.atan2(dx);
    let k = round_to(a, step);
    ((a - k).abs() <= ANGLE_TOL_DEG.to_radians()).then(|| (k.cos(), k.sin()))
}

/// Intersection of the ray `o + s d` (s ≥ 0) with wall `w`.
fn ray_wall(o: P, d: (f64, f64), w: &Wall) -> Option<P> {
    let (sx, sy) = (w.x2 - w.x1, w.y2 - w.y1);
    let den = d.0 * sy - d.1 * sx;
    if den.abs() < 1e-12 {
        return None;
    }
    let (wx, wy) = (w.x1 - o.0, w.y1 - o.1);
    let s = (wx * sy - wy * sx) / den;
    let u = (wx * d.1 - wy * d.0) / den;
    (s >= 0.0 && (-1e-9..=1.0 + 1e-9).contains(&u)).then(|| (o.0 + s * d.0, o.1 + s * d.1))
}

/// Snaps a pointer position. Priority: an endpoint in the magnet radius, then a point on a wall
/// (kept on the snapped angle when there is one), then the 0/45/90° angle (length rounded to the
/// grid), then the grid. Walls with `skip(i)` are ignored (the ones being dragged).
pub fn snap(walls: &[Wall], raw: P, o: &SnapOpts, skip: impl Fn(usize) -> bool) -> Snap {
    if let Some((_, q)) = nearest_end(walls, raw, o.radius, &skip) {
        return Snap { p: q, kind: SnapKind::Endpoint };
    }
    let dir = o.from.filter(|_| o.angle).and_then(|f| angle_snap(f, raw).map(|d| (f, d)));
    let on_wall = walls
        .iter()
        .enumerate()
        .filter(|(i, _)| !skip(*i))
        .map(|(_, w)| w)
        .filter(|w| dist_to_wall(w, raw) <= o.radius);
    match dir {
        Some((f, d)) => {
            let hit = on_wall
                .filter_map(|w| ray_wall(f, d, w))
                .filter(|q| dist(*q, raw) <= o.radius)
                .min_by(|a, b| dist(*a, raw).total_cmp(&dist(*b, raw)));
            if let Some(q) = hit {
                return Snap { p: q, kind: SnapKind::OnWall };
            }
            let mut len = (raw.0 - f.0) * d.0 + (raw.1 - f.1) * d.1;
            if let Some(g) = o.grid {
                len = round_to(len, g);
            }
            Snap { p: (f.0 + len * d.0, f.1 + len * d.1), kind: SnapKind::Angle }
        }
        None => {
            let hit = on_wall.map(|w| project(w, raw).1).min_by(|a, b| dist(*a, raw).total_cmp(&dist(*b, raw)));
            if let Some(q) = hit {
                return Snap { p: q, kind: SnapKind::OnWall };
            }
            match o.grid {
                Some(g) => Snap { p: (round_to(raw.0, g), round_to(raw.1, g)), kind: SnapKind::Grid },
                None => Snap { p: raw, kind: SnapKind::None },
            }
        }
    }
}

/// Point at distance `len` from `from` towards `to` (typed exact length).
pub fn at_length(from: P, to: P, len: f64) -> P {
    let d = dist(from, to);
    let (ux, uy) = if d > 1e-9 { ((to.0 - from.0) / d, (to.1 - from.1) / d) } else { (1.0, 0.0) };
    (from.0 + ux * len, from.1 + uy * len)
}

/// Joined segments through `pts`; consecutive duplicates are skipped (no zero-length walls).
pub fn polyline(pts: &[P], material: Material) -> Vec<Wall> {
    pts.windows(2)
        .filter(|s| dist(s[0], s[1]) > JOINT_EPS)
        .map(|s| Wall { x1: s[0].0, y1: s[0].1, x2: s[1].0, y2: s[1].1, material })
        .collect()
}

/// Opposite corner of a drag from `a` to `b`, made square (side = the longer drag axis).
pub fn square_corner(a: P, b: P) -> P {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let s = dx.abs().max(dy.abs());
    (a.0 + s * if dx < 0.0 { -1.0 } else { 1.0 }, a.1 + s * if dy < 0.0 { -1.0 } else { 1.0 })
}

/// Rectangle room with corners `a` and `b`: 4 walls sharing corners, clockwise from `a`.
/// Empty if the rectangle is degenerate.
pub fn room(a: P, b: P, material: Material) -> Vec<Wall> {
    if (a.0 - b.0).abs() <= JOINT_EPS || (a.1 - b.1).abs() <= JOINT_EPS {
        return Vec::new();
    }
    polyline(&[a, (b.0, a.1), b, (a.0, b.1), a], material)
}

/// Splits wall `i` at the projection of `p` into two joined walls (indices `i`, `i + 1`).
/// `None` if the split point is within `min` of either end.
pub fn split(walls: &mut Vec<Wall>, i: usize, p: P, min: f64) -> Option<[usize; 2]> {
    let w = walls.get(i)?.clone();
    let (t, q) = project(&w, p);
    let l = length(&w);
    if t * l < min || (1.0 - t) * l < min {
        return None;
    }
    let mut a = w.clone();
    let mut b = w;
    set_end(&mut a, true, q);
    set_end(&mut b, false, q);
    walls.splice(i..=i, [a, b]);
    Some([i, i + 1])
}

/// Cuts an opening `width` long centred at the projection of `center` on wall `i` and fills it
/// with a `material` segment (door / window). The opening is kept inside the wall; a wall shorter
/// than the opening just changes material. Returns the index of the opening segment.
pub fn insert_opening(walls: &mut Vec<Wall>, i: usize, center: P, width: f64, material: Material) -> Option<usize> {
    let w = walls.get(i)?.clone();
    let l = length(&w);
    if l <= width + 2.0 * JOINT_EPS {
        walls[i].material = material;
        return Some(i);
    }
    let (t, _) = project(&w, center);
    let s0 = (t * l - width / 2.0).clamp(0.0, l - width);
    let s1 = s0 + width;
    let at = |s: f64| (w.x1 + (w.x2 - w.x1) * s / l, w.y1 + (w.y2 - w.y1) * s / l);
    // pieces that would be shorter than JOINT_EPS are dropped and the opening reuses the exact
    // original end, so the joint there survives
    let (left, right) = (s0 > JOINT_EPS, l - s1 > JOINT_EPS);
    let start = if left { at(s0) } else { (w.x1, w.y1) };
    let end = if right { at(s1) } else { (w.x2, w.y2) };
    let mut parts = Vec::with_capacity(3);
    if left {
        parts.push(Wall { x1: w.x1, y1: w.y1, x2: start.0, y2: start.1, material: w.material });
    }
    parts.push(Wall { x1: start.0, y1: start.1, x2: end.0, y2: end.1, material });
    if right {
        parts.push(Wall { x1: end.0, y1: end.1, x2: w.x2, y2: w.y2, material: w.material });
    }
    let opening = i + left as usize;
    walls.splice(i..=i, parts);
    Some(opening)
}

/// Sets wall `i` to `len`, keeping its first end and moving the joint at its second end along
/// the wall direction (connected walls follow).
pub fn set_length(walls: &mut [Wall], i: usize, len: f64, eps: f64) {
    let Some(w) = walls.get(i) else { return };
    if length(w) < 1e-9 || !(len > JOINT_EPS) {
        return;
    }
    let (a, b) = (end_pos(w, false), end_pos(w, true));
    let ends = ends_at(walls, b, eps);
    move_ends(walls, &ends, at_length(a, b, len));
}

/// Translates the selected walls by `d`. Unless `detach`, endpoints of other walls that were
/// joined to a selected wall move too (so rooms stay closed: neighbours stretch).
pub fn move_walls(walls: &mut [Wall], sel: &[usize], d: P, eps: f64, detach: bool) {
    let moved: Vec<P> = sel
        .iter()
        .filter_map(|&i| walls.get(i))
        .flat_map(|w| [end_pos(w, false), end_pos(w, true)])
        .collect();
    for (i, w) in walls.iter_mut().enumerate() {
        let selected = sel.contains(&i);
        for b in [false, true] {
            let p = end_pos(w, b);
            if selected || (!detach && moved.iter().any(|q| dist(*q, p) <= eps)) {
                set_end(w, b, (p.0 + d.0, p.1 + d.1));
            }
        }
    }
}

/// Limits `d` so the selected walls stay inside `0..=w × 0..=h`.
pub fn clamp_delta(walls: &[Wall], sel: &[usize], d: P, w: f64, h: f64) -> P {
    let pts: Vec<P> = sel.iter().filter_map(|&i| walls.get(i)).flat_map(|w| [end_pos(w, false), end_pos(w, true)]).collect();
    if pts.is_empty() {
        return d;
    }
    let (x0, x1) = pts.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), p| (a.min(p.0), b.max(p.0)));
    let (y0, y1) = pts.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), p| (a.min(p.1), b.max(p.1)));
    let c = |v: f64, lo: f64, hi: f64| if lo > hi { v } else { v.clamp(lo, hi) };
    (c(d.0, -x0, w - x1), c(d.1, -y0, h - y1))
}

/// Appends copies of the selected walls offset by `d`; returns the new indices.
pub fn duplicate(walls: &mut Vec<Wall>, sel: &[usize], d: P) -> Vec<usize> {
    let copies: Vec<Wall> = sel
        .iter()
        .filter_map(|&i| walls.get(i))
        .map(|w| Wall { x1: w.x1 + d.0, y1: w.y1 + d.1, x2: w.x2 + d.0, y2: w.y2 + d.1, material: w.material })
        .collect();
    let start = walls.len();
    walls.extend(copies);
    (start..walls.len()).collect()
}

/// Removes the selected walls (any order, duplicates ignored).
pub fn delete(walls: &mut Vec<Wall>, sel: &[usize]) {
    let mut s: Vec<usize> = sel.iter().copied().filter(|&i| i < walls.len()).collect();
    s.sort_unstable();
    s.dedup();
    for i in s.into_iter().rev() {
        walls.remove(i);
    }
}

/// Segment a-b touches the axis-aligned rectangle (Liang–Barsky clip).
fn seg_hits_rect(a: P, b: P, lo: P, hi: P) -> bool {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let (mut t0, mut t1) = (0.0f64, 1.0f64);
    for (p, q) in [(-dx, a.0 - lo.0), (dx, hi.0 - a.0), (-dy, a.1 - lo.1), (dy, hi.1 - a.1)] {
        if p.abs() < 1e-12 {
            if q < 0.0 {
                return false;
            }
        } else {
            let r = q / p;
            if p < 0.0 {
                t0 = t0.max(r);
            } else {
                t1 = t1.min(r);
            }
            if t0 > t1 {
                return false;
            }
        }
    }
    true
}

/// Walls touching the box with corners `a` and `b` (box select).
pub fn in_box(walls: &[Wall], a: P, b: P) -> Vec<usize> {
    let lo = (a.0.min(b.0), a.1.min(b.1));
    let hi = (a.0.max(b.0), a.1.max(b.1));
    walls
        .iter()
        .enumerate()
        .filter(|(_, w)| seg_hits_rect(end_pos(w, false), end_pos(w, true), lo, hi))
        .map(|(i, _)| i)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use Material::*;

    fn w(x1: f64, y1: f64, x2: f64, y2: f64) -> Wall {
        Wall { x1, y1, x2, y2, material: Drywall }
    }
    fn close(a: P, b: P) -> bool {
        dist(a, b) < 1e-9
    }
    fn total(ws: &[Wall]) -> f64 {
        ws.iter().map(length).sum()
    }
    /// Every endpoint coincides with exactly one other endpoint.
    fn closed(ws: &[Wall]) -> bool {
        ws.iter().all(|x| [false, true].iter().all(|&b| ends_at(ws, end_pos(x, b), JOINT_EPS).len() == 2))
    }

    #[test]
    fn room_is_four_closed_walls() {
        let r = room((10.0, 20.0), (50.0, 40.0), Concrete);
        assert_eq!(r.len(), 4);
        assert!(closed(&r));
        assert!(r.iter().all(|x| x.material == Concrete));
        assert!((total(&r) - 120.0).abs() < 1e-9);
        // corners in any drag direction, and squares
        assert_eq!(room((50.0, 40.0), (10.0, 20.0), Brick).len(), 4);
        let s = square_corner((0.0, 0.0), (-30.0, 12.0));
        assert_eq!(s, (-30.0, 30.0));
        assert!(room((5.0, 5.0), (5.0, 30.0), Drywall).is_empty(), "degenerate");
    }

    #[test]
    fn joint_drag_keeps_connected_endpoints_equal() {
        let mut ws = room((0.0, 0.0), (40.0, 30.0), Drywall);
        ws.push(w(60.0, 0.0, 90.0, 0.0)); // unrelated
        let ends = ends_at(&ws, (40.0, 0.0), JOINT_EPS);
        assert_eq!(ends.len(), 2);
        move_ends(&mut ws, &ends, (47.5, -3.25));
        assert!(close(end_pos(&ws[0], true), (47.5, -3.25)));
        assert!(close(end_pos(&ws[1], false), (47.5, -3.25)));
        assert!(closed(&ws[..4]));
        assert_eq!(ws[4], w(60.0, 0.0, 90.0, 0.0));
    }

    #[test]
    fn moving_a_wall_stretches_its_neighbours() {
        let mut ws = room((0.0, 0.0), (40.0, 30.0), Drywall);
        let orig = ws.clone();
        move_walls(&mut ws, &[1], (5.0, 0.0), JOINT_EPS, false); // right wall
        assert!(closed(&ws));
        assert!(close(end_pos(&ws[1], false), (45.0, 0.0)));
        assert!(close(end_pos(&ws[0], false), (0.0, 0.0)), "far ends stay");
        // detached: only the wall itself moves, the room opens
        let mut ws = orig;
        move_walls(&mut ws, &[1], (5.0, 0.0), JOINT_EPS, true);
        assert!(!closed(&ws));
        assert!(close(end_pos(&ws[0], true), (40.0, 0.0)));
        // clamping keeps the selection on the plan
        assert_eq!(clamp_delta(&ws, &[0], (-50.0, 0.0), 100.0, 100.0), (0.0, 0.0));
    }

    #[test]
    fn snap_finds_nearest_endpoint_within_radius() {
        let ws = vec![w(10.0, 10.0, 30.0, 10.0), w(30.5, 11.0, 30.0, 40.0)];
        let o = SnapOpts { radius: 3.0, grid: None, from: None, angle: false };
        let s = snap(&ws, (31.0, 11.2), &o, |_| false);
        assert_eq!(s, Snap { p: (30.5, 11.0), kind: SnapKind::Endpoint }); // nearer than (30, 10)
        let s = snap(&ws, (29.0, 9.5), &o, |_| false);
        assert_eq!(s.p, (30.0, 10.0));
        // outside the radius of every endpoint: lands on the wall
        let s = snap(&ws, (20.0, 12.0), &o, |_| false);
        assert_eq!(s, Snap { p: (20.0, 10.0), kind: SnapKind::OnWall });
        // nothing near: raw, or the grid
        assert_eq!(snap(&ws, (60.0, 60.0), &o, |_| false).kind, SnapKind::None);
        let g = SnapOpts { grid: Some(2.0), ..o };
        assert_eq!(snap(&ws, (60.9, 59.2), &g, |_| false).p, (60.0, 60.0));
        // skipped walls are not magnets
        assert_eq!(snap(&ws, (11.0, 9.0), &o, |i| i == 0).kind, SnapKind::None);
    }

    #[test]
    fn angle_snap_and_typed_length() {
        let o = SnapOpts { radius: 1.0, grid: Some(1.0), from: Some((0.0, 0.0)), angle: true };
        let s = snap(&[], (20.0, 1.5), &o, |_| false); // ~4° off horizontal
        assert_eq!(s.kind, SnapKind::Angle);
        assert!(close(s.p, (20.0, 0.0)));
        let s = snap(&[], (10.0, 10.9), &o, |_| false); // ~47°
        assert!((s.p.0 - s.p.1).abs() < 1e-9, "{s:?}");
        let s = snap(&[], (10.0, 5.0), &o, |_| false); // 26.6°: free
        assert_ne!(s.kind, SnapKind::Angle);
        let off = SnapOpts { angle: false, grid: None, ..o };
        assert_eq!(snap(&[], (20.0, 1.5), &off, |_| false).p, (20.0, 1.5));
        // the snapped ray meets a wall inside the magnet radius
        let ws = vec![w(15.0, -10.0, 15.0, 10.0)];
        let s = snap(&ws, (15.5, 0.6), &o, |_| false);
        assert_eq!(s.kind, SnapKind::OnWall);
        assert!(close(s.p, (15.0, 0.0)));
        assert!(close(at_length((1.0, 1.0), (1.0, 9.0), 3.5), (1.0, 4.5)));
    }

    #[test]
    fn polyline_joins_segments() {
        let pts = [(0.0, 0.0), (10.0, 0.0), (10.0, 0.0), (10.0, 5.0), (0.0, 0.0)];
        let ws = polyline(&pts, Glass);
        assert_eq!(ws.len(), 3, "duplicate point skipped");
        for k in 1..ws.len() {
            assert!(close(end_pos(&ws[k - 1], true), end_pos(&ws[k], false)));
        }
        assert!(closed(&ws));
    }

    #[test]
    fn opening_split_conserves_total_length() {
        let mut ws = room((0.0, 0.0), (50.0, 30.0), Concrete);
        let before = total(&ws);
        let i = insert_opening(&mut ws, 0, (20.0, 1.0), 9.0, WoodDoor).unwrap();
        assert_eq!(ws.len(), 6);
        assert_eq!(ws[i].material, WoodDoor);
        assert!((length(&ws[i]) - 9.0).abs() < 1e-9);
        assert!(close(end_pos(&ws[i], false), (15.5, 0.0)));
        assert!((total(&ws) - before).abs() < 1e-9);
        assert!(closed(&ws), "all pieces stay joined");
        // near an end: the opening is pushed inside the wall, no zero-length piece
        let j = insert_opening(&mut ws, 2 + 2, (49.0, 29.0), 12.0, Glass).unwrap(); // bottom wall
        assert_eq!(ws[j].material, Glass);
        assert!(ws.iter().all(|x| length(x) > JOINT_EPS));
        assert!((total(&ws) - before).abs() < 1e-9);
        assert!(closed(&ws));
        // shorter than the opening: material change only
        let mut short = vec![w(0.0, 0.0, 5.0, 0.0)];
        assert_eq!(insert_opening(&mut short, 0, (2.0, 0.0), 9.0, WoodDoor), Some(0));
        assert_eq!(short.len(), 1);
        assert_eq!(short[0].material, WoodDoor);
    }

    #[test]
    fn split_adds_a_joint() {
        let mut ws = vec![w(0.0, 0.0, 20.0, 0.0)];
        assert_eq!(split(&mut ws, 0, (5.0, 3.0), 0.5), Some([0, 1]));
        assert!(close(end_pos(&ws[0], true), (5.0, 0.0)));
        assert_eq!(ends_at(&ws, (5.0, 0.0), JOINT_EPS).len(), 2);
        assert_eq!(split(&mut ws, 0, (0.2, 0.0), 0.5), None, "too close to an end");
    }

    #[test]
    fn set_length_moves_the_far_joint() {
        let mut ws = room((0.0, 0.0), (40.0, 30.0), Drywall);
        set_length(&mut ws, 0, 55.0, JOINT_EPS);
        assert!(close(end_pos(&ws[0], true), (55.0, 0.0)));
        assert!(closed(&ws));
        assert!((length(&ws[0]) - 55.0).abs() < 1e-9);
    }

    #[test]
    fn box_select_hits_touching_walls() {
        let ws = vec![w(0.0, 0.0, 10.0, 0.0), w(20.0, -5.0, 20.0, 5.0), w(30.0, 30.0, 40.0, 40.0), w(-5.0, 8.0, 50.0, 8.0)];
        assert_eq!(in_box(&ws, (15.0, -1.0), (25.0, 1.0)), vec![1]); // crosses, no endpoint inside
        assert_eq!(in_box(&ws, (25.0, 10.0), (-1.0, -1.0)), vec![0, 1, 3]);
        assert!(in_box(&ws, (41.0, 41.0), (45.0, 45.0)).is_empty());
    }

    #[test]
    fn duplicate_and_delete() {
        let mut ws = room((0.0, 0.0), (10.0, 10.0), Drywall);
        let new = duplicate(&mut ws, &[0, 2], (5.0, 5.0));
        assert_eq!(new, vec![4, 5]);
        assert_eq!(ws[4], w(5.0, 5.0, 15.0, 5.0));
        delete(&mut ws, &[5, 0, 0, 99]);
        assert_eq!(ws.len(), 4);
        assert_eq!(ws[3], w(5.0, 5.0, 15.0, 5.0));
    }
}
