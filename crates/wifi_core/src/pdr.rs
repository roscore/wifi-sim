//! Pedestrian dead reckoning (PDR): turn phone motion sensors into wall segments.
//!
//! Step detection on the accelerometer magnitude, fixed step length along the current heading,
//! optional rectilinear snap, per-room loop-closure correction, then path -> walls. Everything is
//! in metres. Only corner/door vertices are kept; raw sensor samples are never stored.
//!
//! Output frame: rotated so the first walked wall points along +x (compass north is irrelevant,
//! and a snapped plan comes out axis-aligned in the editor).

use crate::{Material, Measurement, Plan, Wall, MAX_GRID};
use std::f64::consts::{FRAC_PI_2, PI, TAU};

/// Door opening cut out of the walls at a door mark.
/// ponytail: fixed width, a per-door width field if people walk double doors.
pub const DOOR_M: f64 = 0.9;

fn wrap(a: f64) -> f64 {
    (a + PI).rem_euclid(TAU) - PI
}

/// Peak detector on |acceleration including gravity|.
#[derive(Clone, Debug)]
pub struct StepDetector {
    /// Peak height above the gravity baseline (m/s²).
    pub threshold: f64,
    pub min_interval_s: f64,
    last_t: Option<f64>,
    fast: f64,
    slow: f64,
    armed: bool,
    last_step: f64,
}

impl Default for StepDetector {
    fn default() -> Self {
        StepDetector {
            threshold: 1.0,
            min_interval_s: 0.3,
            last_t: None,
            fast: 0.0,
            slow: 0.0,
            armed: true,
            last_step: f64::NEG_INFINITY,
        }
    }
}

impl StepDetector {
    pub fn new() -> Self {
        Self::default()
    }

    /// Feeds one sample (time in seconds, magnitude in m/s²); true when a step is detected.
    /// Filters are time-constant based, so 50 Hz and 200 Hz sensors behave the same.
    pub fn push(&mut self, t_s: f64, accel: f64) -> bool {
        if !accel.is_finite() || !t_s.is_finite() {
            return false;
        }
        let Some(lt) = self.last_t.replace(t_s) else {
            self.fast = accel;
            self.slow = accel;
            return false;
        };
        let dt = (t_s - lt).clamp(0.0, 0.2);
        self.fast += (accel - self.fast) * dt / (0.05 + dt); // low-pass ~3 Hz
        self.slow += (accel - self.slow) * dt / (1.5 + dt); // gravity baseline
        let s = self.fast - self.slow;
        if s < 0.0 {
            self.armed = true;
        } else if self.armed && s > self.threshold && t_s - self.last_step >= self.min_interval_s {
            self.armed = false;
            self.last_step = t_s;
            return true;
        }
        false
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mark {
    Corner,
    Door,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Vertex {
    pub x: f64,
    pub y: f64,
    pub mark: Mark,
    /// Wi-Fi signal the user measured here with another app (browsers cannot read it).
    pub dbm: Option<f64>,
}

#[derive(Clone, Debug)]
struct Room {
    /// Raw dead-reckoned vertices; `pts[0]` is where the room was started.
    pts: Vec<Vertex>,
    /// Raw position where "close loop" was tapped (should be back at `pts[0]`).
    end: Option<(f64, f64)>,
    /// Earlier (room, vertex) door this room started from.
    anchor: Option<(usize, usize)>,
}

#[derive(Clone, Debug)]
pub struct Walk {
    pub step_m: f64,
    /// Snap every wall to 0/90/180/270° relative to the first wall.
    pub snap: bool,
    rooms: Vec<Room>,
    pos: (f64, f64),
    heading: f64,
    pub steps: usize,
}

impl Walk {
    pub fn new(step_m: f64) -> Walk {
        Walk { step_m, snap: false, rooms: Vec::new(), pos: (0.0, 0.0), heading: 0.0, steps: 0 }
    }

    /// Heading in radians; plan y points down, so increasing heading turns clockwise on screen.
    pub fn set_heading(&mut self, h: f64) {
        if h.is_finite() {
            self.heading = h;
        }
    }

    pub fn step(&mut self) {
        self.pos.0 += self.step_m * self.heading.cos();
        self.pos.1 += self.step_m * self.heading.sin();
        self.steps += 1;
    }

    /// Raw dead-reckoned position.
    pub fn pos(&self) -> (f64, f64) {
        self.pos
    }

    /// A room has been started and not closed yet.
    pub fn is_open(&self) -> bool {
        self.rooms.last().is_some_and(|r| r.end.is_none())
    }

    /// Marks a corner or door at the current position. With no open room this starts a new one;
    /// a new room started on a door is joined to the nearest door of an earlier room.
    pub fn mark(&mut self, mark: Mark) {
        let v = Vertex { x: self.pos.0, y: self.pos.1, mark, dbm: None };
        if self.is_open() {
            self.rooms.last_mut().unwrap().pts.push(v);
            return;
        }
        let anchor = (mark == Mark::Door)
            .then(|| {
                self.rooms
                    .iter()
                    .enumerate()
                    .flat_map(|(ri, r)| r.pts.iter().enumerate().map(move |(vi, d)| (ri, vi, d)))
                    .filter(|(_, _, d)| d.mark == Mark::Door)
                    .min_by(|a, b| {
                        let d = |p: &Vertex| (p.x - v.x).hypot(p.y - v.y);
                        d(a.2).total_cmp(&d(b.2))
                    })
                    .map(|(ri, vi, _)| (ri, vi))
            })
            .flatten();
        self.rooms.push(Room { pts: vec![v], end: None, anchor });
    }

    /// Attaches a measured dBm to the most recent mark.
    pub fn set_dbm(&mut self, dbm: f64) {
        if let Some(v) = self.rooms.last_mut().and_then(|r| r.pts.last_mut()) {
            v.dbm = Some(dbm);
        }
    }

    /// Closes the open room here and returns the raw drift gap (m) that the correction removes.
    /// Needs at least 2 marks (3 corners with the closing point).
    pub fn close_loop(&mut self) -> f64 {
        let pos = self.pos;
        match self.rooms.last_mut() {
            Some(r) if r.end.is_none() && r.pts.len() >= 2 => {
                r.end = Some(pos);
                (pos.0 - r.pts[0].x).hypot(pos.1 - r.pts[0].y)
            }
            _ => 0.0,
        }
    }

    /// Removes the last mark (or reopens / drops the last room).
    pub fn undo(&mut self) {
        let Some(r) = self.rooms.last_mut() else { return };
        if r.end.take().is_none() {
            r.pts.pop();
            if r.pts.is_empty() {
                self.rooms.pop();
            }
        }
    }

    /// Direction of the first walked wall (raw frame).
    fn ref_heading(&self) -> Option<f64> {
        let r = self.rooms.first()?;
        let a = (r.pts[0].x, r.pts[0].y);
        let b = r.pts.get(1).map(|v| (v.x, v.y)).or(r.end)?;
        Some((b.1 - a.1).atan2(b.0 - a.0))
    }

    /// Angle (deg) between the last finished wall and the nearest right angle to the first wall:
    /// large values mean the heading drifted (or the room really is not rectilinear).
    pub fn drift_deg(&self) -> f64 {
        let (Some(r0), Some(room)) = (self.ref_heading(), self.rooms.last()) else { return 0.0 };
        let mut p: Vec<(f64, f64)> = room.pts.iter().map(|v| (v.x, v.y)).collect();
        p.extend(room.end);
        match p.len() {
            0 | 1 => 0.0,
            n => {
                let (a, b) = (p[n - 2], p[n - 1]);
                let rel = wrap((b.1 - a.1).atan2(b.0 - a.0) - r0);
                (rel - (rel / FRAC_PI_2).round() * FRAC_PI_2).abs().to_degrees()
            }
        }
    }

    fn frame(&self) -> impl Fn((f64, f64)) -> (f64, f64) {
        let (s, c) = (-self.ref_heading().unwrap_or(0.0)).sin_cos();
        move |p| (p.0 * c - p.1 * s, p.0 * s + p.1 * c)
    }

    /// Corrected rooms in the output frame: snapped (if enabled), loop-closed, joined at doors.
    pub fn rooms(&self) -> Vec<Vec<Vertex>> {
        let rot = self.frame();
        let mut out: Vec<Vec<Vertex>> = Vec::new();
        for r in &self.rooms {
            let mut pts: Vec<(f64, f64)> = r.pts.iter().map(|v| rot((v.x, v.y))).chain(r.end.map(&rot)).collect();
            if self.snap {
                snap_rectilinear(&mut pts, 0.0);
            }
            if r.end.is_some() {
                close_loop(&mut pts);
            }
            let shift = r
                .anchor
                .map(|(ri, vi)| (out[ri][vi].x - pts[0].0, out[ri][vi].y - pts[0].1))
                .unwrap_or((0.0, 0.0));
            out.push(
                r.pts
                    .iter()
                    .zip(pts)
                    .map(|(v, p)| Vertex { x: p.0 + shift.0, y: p.1 + shift.1, ..v.clone() })
                    .collect(),
            );
        }
        out
    }

    /// Where the walker is now, in the output frame (for a live preview).
    pub fn cursor(&self) -> (f64, f64) {
        let rot = self.frame();
        let p = rot(self.pos);
        match (self.rooms.last(), self.rooms().last().and_then(|r| r.last().cloned())) {
            (Some(r), Some(d)) if r.end.is_none() => {
                let last = r.pts.last().unwrap();
                let l = rot((last.x, last.y));
                (d.x + p.0 - l.0, d.y + p.1 - l.1)
            }
            _ => p,
        }
    }

    /// Walls in metres. The largest closed room is the outer loop (concrete); the rest drywall.
    /// A door mark cuts `DOOR_M` out of the wall(s) meeting at it.
    pub fn walls(&self) -> Vec<Wall> {
        let rooms = self.rooms();
        let area = |r: &[Vertex]| {
            (0..r.len()).map(|i| {
                let (a, b) = (&r[i], &r[(i + 1) % r.len()]);
                a.x * b.y - b.x * a.y
            }).sum::<f64>().abs() / 2.0
        };
        let outer = (0..rooms.len())
            .filter(|&i| self.rooms[i].end.is_some())
            .max_by(|&a, &b| area(&rooms[a]).total_cmp(&area(&rooms[b])));
        let mut walls = Vec::new();
        for (ri, r) in rooms.iter().enumerate() {
            let material = if Some(ri) == outer { Material::Concrete } else { Material::Drywall };
            let n = r.len();
            let segs = if self.rooms[ri].end.is_some() { n } else { n.saturating_sub(1) };
            for i in 0..segs {
                let (a, b) = (&r[i], &r[(i + 1) % n]);
                let len = (b.x - a.x).hypot(b.y - a.y);
                let cut = |v: &Vertex| if v.mark == Mark::Door { DOOR_M / 2.0 } else { 0.0 };
                let (t0, t1) = (cut(a) / len, 1.0 - cut(b) / len);
                if len < 1e-9 || t1 - t0 < 1e-6 {
                    continue;
                }
                let at = |t: f64| (a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t);
                let (p, q) = (at(t0), at(t1));
                walls.push(Wall { x1: p.0, y1: p.1, x2: q.0, y2: q.1, material });
            }
        }
        walls
    }

    /// New plan (grid units) holding the walls and any dBm the user typed at marks. Cell size is
    /// grown if the walk does not fit in `MAX_GRID` cells; 1 m margin on every side.
    pub fn to_plan(&self, cell_m: f64) -> Plan {
        let walls = self.walls();
        let meas: Vec<(f64, f64, f64)> =
            self.rooms().iter().flatten().filter_map(|v| v.dbm.map(|d| (v.x, v.y, d))).collect();
        let pts: Vec<(f64, f64)> = walls
            .iter()
            .flat_map(|w| [(w.x1, w.y1), (w.x2, w.y2)])
            .chain(meas.iter().map(|m| (m.0, m.1)))
            .collect();
        let (mut x0, mut y0, mut x1, mut y1) = (0.0f64, 0.0f64, 0.0f64, 0.0f64);
        if let Some(&(x, y)) = pts.first() {
            (x0, y0, x1, y1) = (x, y, x, y);
        }
        for &(x, y) in &pts {
            (x0, y0, x1, y1) = (x0.min(x), y0.min(y), x1.max(x), y1.max(y));
        }
        const MARGIN_M: f64 = 1.0;
        let span = (x1 - x0).max(y1 - y0) + 2.0 * MARGIN_M;
        let cell = cell_m.max(span / (MAX_GRID - 1) as f64);
        let cells = |m: f64| ((m + 2.0 * MARGIN_M) / cell).ceil().clamp(10.0, MAX_GRID as f64) as usize;
        let mut plan = Plan::new(cells(x1 - x0), cells(y1 - y0), cell);
        let g = |x: f64, y: f64| ((x - x0 + MARGIN_M) / cell, (y - y0 + MARGIN_M) / cell);
        plan.walls = walls
            .into_iter()
            .map(|w| {
                let ((ax, ay), (bx, by)) = (g(w.x1, w.y1), g(w.x2, w.y2));
                Wall { x1: ax, y1: ay, x2: bx, y2: by, material: w.material }
            })
            .collect();
        plan.measurements = meas
            .into_iter()
            .map(|(x, y, dbm)| {
                let (x, y) = g(x, y);
                Measurement { x, y, dbm }
            })
            .collect();
        plan
    }
}

/// Snaps each segment of the polyline to the nearest multiple of 90° from `ref_angle`, keeping
/// its length, and re-chains from `pts[0]`.
pub fn snap_rectilinear(pts: &mut [(f64, f64)], ref_angle: f64) {
    let mut prev = pts.first().copied().unwrap_or_default();
    let mut at = prev;
    for p in pts.iter_mut().skip(1) {
        let (dx, dy) = (p.0 - prev.0, p.1 - prev.1);
        let rel = wrap(dy.atan2(dx) - ref_angle);
        let a = ref_angle + (rel / FRAC_PI_2).round() * FRAC_PI_2;
        let len = dx.hypot(dy);
        // exact zeros on the axes so snapped walls stay exactly horizontal / vertical
        let (s, c) = a.sin_cos();
        let clean = |v: f64| if v.abs() < 1e-12 { 0.0 } else { v };
        prev = *p;
        at = (at.0 + len * clean(c), at.1 + len * clean(s));
        *p = at;
    }
}

/// Loop closure: the last point should coincide with the first. The gap is spread over the
/// segments per axis, in proportion to each segment's extent on that axis, so axis-aligned walls
/// stay axis-aligned. The duplicate closing point is dropped.
/// ponytail: per-axis proportional split, a least-squares graph optimiser if walks get long.
fn close_loop(pts: &mut Vec<(f64, f64)>) {
    if pts.len() < 2 {
        return;
    }
    let d: Vec<(f64, f64)> = pts.windows(2).map(|w| (w[1].0 - w[0].0, w[1].1 - w[0].1)).collect();
    let (first, last) = (pts[0], pts[pts.len() - 1]);
    let (ex, ey) = (last.0 - first.0, last.1 - first.1);
    let sx: f64 = d.iter().map(|v| v.0.abs()).sum();
    let sy: f64 = d.iter().map(|v| v.1.abs()).sum();
    let mut at = first;
    for (i, v) in d.iter().enumerate() {
        at.0 += v.0 - if sx > 0.0 { ex * v.0.abs() / sx } else { 0.0 };
        at.1 += v.1 - if sy > 0.0 { ey * v.1.abs() / sy } else { 0.0 };
        pts[i + 1] = at;
    }
    pts.pop();
}

/// Step length from "walk N steps along a known length".
pub fn calibrate_step(known_m: f64, steps: usize) -> Option<f64> {
    (known_m > 0.0 && known_m.is_finite() && steps > 0).then(|| known_m / steps as f64)
}
