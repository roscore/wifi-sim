//! Indoor Wi-Fi coverage *estimate*.
//!
//! Model: log-distance path loss (exponent `n`, FSPL at 1 m as reference) plus the COST 231
//! multi-wall sum along the direct ray, and a dominant-path heuristic (Dijkstra on the grid with
//! per-edge wall costs) that lets signal go around walls through openings. Each cell takes the
//! better of the two. No reflections, no diffraction loss, no furniture/people: it is a rough guide.
//!
//! Coordinates are in grid cells (floats). Cell `(i, j)` covers `[i, i+1) x [j, j+1)`; its centre is
//! `(i + 0.5, j + 0.5)`. `Plan::cell_m` converts cells to metres.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Band {
    #[serde(rename = "2.4")]
    G2_4,
    #[serde(rename = "5")]
    G5,
    #[serde(rename = "6")]
    G6,
}

impl Band {
    pub const ALL: [Band; 3] = [Band::G2_4, Band::G5, Band::G6];

    /// Representative centre frequency of the band.
    pub fn freq_mhz(self) -> f64 {
        match self {
            Band::G2_4 => 2437.0,
            Band::G5 => 5250.0,
            Band::G6 => 6105.0,
        }
    }

    pub fn idx(self) -> usize {
        self as usize
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Material {
    Drywall,
    WoodDoor,
    Glass,
    Brick,
    Concrete,
    Metal,
    Mirror,
}

impl Material {
    pub const ALL: [Material; 7] = [
        Material::Drywall,
        Material::WoodDoor,
        Material::Glass,
        Material::Brick,
        Material::Concrete,
        Material::Metal,
        Material::Mirror,
    ];

    pub fn idx(self) -> usize {
        self as usize
    }
}

/// Attenuation in dB per wall crossing, `db[material][band]`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MaterialTable {
    pub db: [[f64; 3]; 7],
}

impl Default for MaterialTable {
    /// Rough single-wall defaults (2.4 / 5 / 6 GHz) in the range commonly quoted for indoor
    /// planning. Real walls vary a lot; edit them in the UI or calibrate.
    fn default() -> Self {
        MaterialTable {
            db: [
                [3.0, 4.0, 5.0],    // drywall / partition
                [4.0, 6.0, 7.0],    // wooden door
                [2.0, 6.0, 8.0],    // glass / window (low-e coated glass is much worse)
                [8.0, 12.0, 14.0],  // brick
                [12.0, 18.0, 22.0], // concrete / load-bearing
                [26.0, 32.0, 35.0], // metal / elevator shaft
                [6.0, 10.0, 12.0],  // mirror (metal backing)
            ],
        }
    }
}

impl MaterialTable {
    pub fn loss(&self, m: Material, b: Band) -> f64 {
        self.db[m.idx()][b.idx()]
    }

    pub fn set(&mut self, m: Material, b: Band, db: f64) {
        self.db[m.idx()][b.idx()] = db;
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Wall {
    pub x1: f64,
    pub y1: f64,
    pub x2: f64,
    pub y2: f64,
    pub material: Material,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Ap {
    pub x: f64,
    pub y: f64,
    pub band: Band,
    pub tx_dbm: f64,
    pub gain_dbi: f64,
    #[serde(default = "default_channel")]
    pub channel: u16,
}

fn default_channel() -> u16 {
    1
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Measurement {
    pub x: f64,
    pub y: f64,
    pub dbm: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Plan {
    #[serde(default)]
    pub name: String,
    pub width: usize,
    pub height: usize,
    /// Metres per grid cell.
    pub cell_m: f64,
    #[serde(default)]
    pub walls: Vec<Wall>,
    #[serde(default)]
    pub aps: Vec<Ap>,
    #[serde(default)]
    pub materials: MaterialTable,
    /// Path-loss exponent `n` (2 = free space).
    #[serde(default = "default_exponent")]
    pub exponent: f64,
    /// Global calibration offset added to every prediction.
    #[serde(default)]
    pub offset_db: f64,
    #[serde(default)]
    pub measurements: Vec<Measurement>,
}

fn default_exponent() -> f64 {
    2.0
}

pub const MAX_GRID: usize = 400;
const MAX_ITEMS: usize = 5000;
const MAX_SHARE_BYTES: usize = 4 << 20;

impl Plan {
    pub fn new(width: usize, height: usize, cell_m: f64) -> Plan {
        Plan {
            name: String::new(),
            width,
            height,
            cell_m,
            walls: Vec::new(),
            aps: Vec::new(),
            materials: MaterialTable::default(),
            exponent: default_exponent(),
            offset_db: 0.0,
            measurements: Vec::new(),
        }
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("plan serialises")
    }

    /// Parses and validates untrusted JSON (file import, URL fragment).
    pub fn from_json(s: &str) -> Result<Plan, String> {
        let p: Plan = serde_json::from_str(s).map_err(|e| e.to_string())?;
        p.validate()?;
        Ok(p)
    }

    fn validate(&self) -> Result<(), String> {
        let fin = |v: &[f64]| v.iter().all(|x| x.is_finite());
        if !(1..=MAX_GRID).contains(&self.width) || !(1..=MAX_GRID).contains(&self.height) {
            return Err(format!("grid must be 1..={MAX_GRID} cells per side"));
        }
        if !(self.cell_m.is_finite() && self.cell_m > 0.001 && self.cell_m <= 10.0) {
            return Err("cell size out of range".into());
        }
        if self.walls.len() > MAX_ITEMS || self.aps.len() > 64 || self.measurements.len() > MAX_ITEMS {
            return Err("too many items".into());
        }
        let ok = fin(&[self.exponent, self.offset_db])
            && self.materials.db.iter().all(|r| fin(r))
            && self.walls.iter().all(|w| fin(&[w.x1, w.y1, w.x2, w.y2]))
            && self.aps.iter().all(|a| fin(&[a.x, a.y, a.tx_dbm, a.gain_dbi]))
            && self.measurements.iter().all(|m| fin(&[m.x, m.y, m.dbm]));
        if ok {
            Ok(())
        } else {
            Err("non-finite number".into())
        }
    }

    /// Compact URL-safe form for `#plan=...`: deflate + base64url.
    pub fn to_share_string(&self) -> String {
        use base64::Engine;
        let json = serde_json::to_vec(self).expect("plan serialises");
        let z = miniz_oxide::deflate::compress_to_vec(&json, 9);
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(z)
    }

    pub fn from_share_string(s: &str) -> Result<Plan, String> {
        use base64::Engine;
        let z = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(s.trim())
            .map_err(|e| e.to_string())?;
        let json = miniz_oxide::inflate::decompress_to_vec_with_limit(&z, MAX_SHARE_BYTES)
            .map_err(|e| format!("{e:?}"))?;
        Plan::from_json(std::str::from_utf8(&json).map_err(|e| e.to_string())?)
    }
}

/// Free-space path loss (Friis), distance in metres, frequency in MHz.
pub fn fspl_db(d_m: f64, f_mhz: f64) -> f64 {
    20.0 * d_m.log10() + 20.0 * f_mhz.log10() - 27.55
}

/// Log-distance model: FSPL at 1 m plus `10 n log10(d)`.
pub fn log_distance_db(d_m: f64, f_mhz: f64, n: f64) -> f64 {
    fspl_db(1.0, f_mhz) + 10.0 * n * d_m.log10()
}

const MIN_DIST_M: f64 = 0.1;

fn cross(ax: f64, ay: f64, bx: f64, by: f64) -> f64 {
    ax * by - ay * bx
}

/// Segment p-q intersects segment a-b (touching counts, so rays through wall joints are blocked).
fn segments_cross(p: (f64, f64), q: (f64, f64), a: (f64, f64), b: (f64, f64)) -> bool {
    const EPS: f64 = 1e-9;
    let (rx, ry) = (q.0 - p.0, q.1 - p.1);
    let (sx, sy) = (b.0 - a.0, b.1 - a.1);
    let d = cross(rx, ry, sx, sy);
    if d.abs() < 1e-12 {
        return false; // parallel: grazing along a wall is not a crossing
    }
    let (wx, wy) = (a.0 - p.0, a.1 - p.1);
    let t = cross(wx, wy, sx, sy) / d;
    let u = cross(wx, wy, rx, ry) / d;
    (-EPS..=1.0 + EPS).contains(&t) && (-EPS..=1.0 + EPS).contains(&u)
}

/// Sum of wall losses crossed by the straight segment p0-p1 (COST 231 multi-wall term).
pub fn direct_wall_loss(plan: &Plan, band: Band, p0: (f64, f64), p1: (f64, f64)) -> f64 {
    let (lx, hx) = (p0.0.min(p1.0), p0.0.max(p1.0));
    let (ly, hy) = (p0.1.min(p1.1), p0.1.max(p1.1));
    plan.walls
        .iter()
        .filter(|w| w.x1.max(w.x2) >= lx && w.x1.min(w.x2) <= hx && w.y1.max(w.y2) >= ly && w.y1.min(w.y2) <= hy)
        .filter(|w| segments_cross(p0, p1, (w.x1, w.y1), (w.x2, w.y2)))
        .map(|w| plan.materials.loss(w.material, band))
        .sum()
}

fn rssi(plan: &Plan, ap: &Ap, dist_m: f64, walls_db: f64) -> f64 {
    ap.tx_dbm + ap.gain_dbi - log_distance_db(dist_m.max(MIN_DIST_M), ap.band.freq_mhz(), plan.exponent) - walls_db
        + plan.offset_db
}

/// Multi-wall model RSSI (dBm) at point (x, y) in cell units.
pub fn direct_rssi(plan: &Plan, ap: &Ap, x: f64, y: f64) -> f64 {
    let d = (x - ap.x).hypot(y - ap.y) * plan.cell_m;
    rssi(plan, ap, d, direct_wall_loss(plan, ap.band, (ap.x, ap.y), (x, y)))
}

/// Wall loss on the 4 "forward" edges of each cell centre: +x, +y, +x+y, -x+y.
struct Edges {
    w: usize,
    e: Vec<[f32; 4]>,
}

const FWD: [(i64, i64); 4] = [(1, 0), (0, 1), (1, 1), (-1, 1)];

impl Edges {
    fn build(plan: &Plan, band: Band) -> Edges {
        let (w, h) = (plan.width as i64, plan.height as i64);
        let mut e = vec![[0f32; 4]; plan.width * plan.height];
        for wall in &plan.walls {
            let loss = plan.materials.loss(wall.material, band) as f32;
            let clamp = |v: f64, hi: i64| (v as i64).clamp(0, hi - 1);
            let (i0, i1) = (clamp(wall.x1.min(wall.x2).floor() - 1.0, w), clamp(wall.x1.max(wall.x2).ceil() + 1.0, w));
            let (j0, j1) = (clamp(wall.y1.min(wall.y2).floor() - 1.0, h), clamp(wall.y1.max(wall.y2).ceil() + 1.0, h));
            for j in j0..=j1 {
                for i in i0..=i1 {
                    let c = (i as f64 + 0.5, j as f64 + 0.5);
                    for (k, (dx, dy)) in FWD.iter().enumerate() {
                        let n = (c.0 + *dx as f64, c.1 + *dy as f64);
                        if segments_cross(c, n, (wall.x1, wall.y1), (wall.x2, wall.y2)) {
                            e[(j * w + i) as usize][k] += loss;
                        }
                    }
                }
            }
        }
        Edges { w: plan.width, e }
    }

    /// Loss of the step from cell `a` by (dx, dy) to cell `b`.
    fn step(&self, a: usize, b: usize, dx: i64, dy: i64) -> f32 {
        match (dx, dy) {
            (1, 0) => self.e[a][0],
            (-1, 0) => self.e[b][0],
            (0, 1) => self.e[a][1],
            (0, -1) => self.e[b][1],
            (1, 1) => self.e[a][2],
            (-1, -1) => self.e[b][2],
            (-1, 1) => self.e[a][3],
            _ => self.e[b][3], // (1, -1)
        }
    }
}

/// Dijkstra minimises this additive proxy (metres * PROXY + wall dB); the final RSSI is then
/// re-evaluated with the log-distance law on the found path's real length.
/// ponytail: fixed dB/m trade-off, a per-distance weight would track the log law better.
const PROXY_DB_PER_M: f32 = 1.0;

fn dominant_map(plan: &Plan, ap: &Ap, edges: &Edges) -> Vec<f32> {
    let (w, h) = (plan.width, plan.height);
    let n = w * h;
    let mut cost = vec![f32::INFINITY; n];
    let mut len = vec![0f32; n];
    let mut wall = vec![0f32; n];
    let si = (ap.x.floor().max(0.0) as usize).min(w - 1);
    let sj = (ap.y.floor().max(0.0) as usize).min(h - 1);
    let s = sj * w + si;
    cost[s] = 0.0;
    let cm = plan.cell_m as f32;
    // Dial's bucket queue: bucket width = smallest edge cost, so a popped node is always final.
    // Circular, sized to cover the largest single edge cost.
    let q = cm * PROXY_DB_PER_M;
    let max_wall = edges.e.iter().flatten().fold(0f32, |m, &v| m.max(v));
    let nb = ((cm * std::f32::consts::SQRT_2 * PROXY_DB_PER_M + max_wall) / q).ceil() as usize + 2;
    let mut buckets: Vec<Vec<u32>> = vec![Vec::new(); nb];
    buckets[0].push(s as u32);
    let (mut pending, mut b) = (1usize, 0usize);
    while pending > 0 {
        let Some(u) = buckets[b % nb].pop() else {
            b += 1;
            continue;
        };
        pending -= 1;
        let u = u as usize;
        if (cost[u] / q) as usize != b {
            continue; // stale entry, node was improved into an earlier bucket
        }
        let (ui, uj) = ((u % w) as i64, (u / w) as i64);
        for dy in -1i64..=1 {
            for dx in -1i64..=1 {
                let (vi, vj) = (ui + dx, uj + dy);
                if (dx == 0 && dy == 0) || vi < 0 || vj < 0 || vi >= w as i64 || vj >= h as i64 {
                    continue;
                }
                let v = (vj as usize) * edges.w + vi as usize;
                let step_m = if dx != 0 && dy != 0 { cm * std::f32::consts::SQRT_2 } else { cm };
                let wl = edges.step(u, v, dx, dy);
                let c = cost[u] + step_m * PROXY_DB_PER_M + wl;
                if c < cost[v] {
                    cost[v] = c;
                    len[v] = len[u] + step_m;
                    wall[v] = wall[u] + wl;
                    buckets[(c / q) as usize % nb].push(v as u32);
                    pending += 1;
                }
            }
        }
    }
    (0..n)
        .map(|k| {
            let (x, y) = ((k % w) as f64 + 0.5, (k / w) as f64 + 0.5);
            let euclid = (x - ap.x).hypot(y - ap.y) * plan.cell_m;
            rssi(plan, ap, (len[k] as f64).max(euclid), wall[k] as f64) as f32
        })
        .collect()
}

/// Dominant-path RSSI for every cell (row-major, `width * height`).
pub fn dominant_rssi_map(plan: &Plan, ap: &Ap) -> Vec<f32> {
    dominant_map(plan, ap, &Edges::build(plan, ap.band))
}

/// Per-cell best of direct multi-wall and dominant path for one AP.
fn ap_map(plan: &Plan, ap: &Ap, edges: &Edges) -> Vec<f32> {
    let mut m = dominant_map(plan, ap, edges);
    for (k, v) in m.iter_mut().enumerate() {
        let (x, y) = ((k % plan.width) as f64 + 0.5, (k / plan.width) as f64 + 0.5);
        *v = v.max(direct_rssi(plan, ap, x, y) as f32);
    }
    m
}

fn edges_for(plan: &Plan) -> [Option<Edges>; 3] {
    let mut out = [None, None, None];
    for a in &plan.aps {
        if out[a.band.idx()].is_none() {
            out[a.band.idx()] = Some(Edges::build(plan, a.band));
        }
    }
    out
}

pub const NO_SIGNAL_DBM: f32 = -120.0;
pub const NOISE_FLOOR_DBM: f64 = -95.0;

#[derive(Clone, Debug)]
pub struct Coverage {
    pub width: usize,
    pub height: usize,
    /// Strongest RSSI per cell (dBm).
    pub best: Vec<f32>,
    /// Index of the AP giving `best`.
    pub best_ap: Vec<u8>,
    /// SINR (dB) vs other APs on the same band + channel and the noise floor. `None` without APs.
    pub sinr: Option<Vec<f32>>,
}

pub fn coverage(plan: &Plan) -> Coverage {
    let n = plan.width * plan.height;
    let edges = edges_for(plan);
    let maps: Vec<Vec<f32>> = plan
        .aps
        .iter()
        .map(|a| ap_map(plan, a, edges[a.band.idx()].as_ref().unwrap()))
        .collect();
    let mut best = vec![NO_SIGNAL_DBM; n];
    let mut best_ap = vec![0u8; n];
    for (ai, m) in maps.iter().enumerate() {
        for k in 0..n {
            if m[k] > best[k] {
                best[k] = m[k];
                best_ap[k] = ai as u8;
            }
        }
    }
    let mw = |dbm: f64| 10f64.powf(dbm / 10.0);
    let sinr = (!maps.is_empty()).then(|| {
        (0..n)
            .map(|k| {
                let b = &plan.aps[best_ap[k] as usize];
                let interference: f64 = plan
                    .aps
                    .iter()
                    .enumerate()
                    .filter(|(ai, a)| *ai != best_ap[k] as usize && a.band == b.band && a.channel == b.channel)
                    .map(|(ai, _)| mw(maps[ai][k] as f64))
                    .sum();
                (best[k] as f64 - 10.0 * (interference + mw(NOISE_FLOOR_DBM)).log10()) as f32
            })
            .collect()
    });
    Coverage { width: plan.width, height: plan.height, best, best_ap, sinr }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Score {
    pub median_dbm: f64,
    /// Fraction of cells below -70 dBm.
    pub frac_below_70: f64,
    /// Higher is better: median minus 50 dB per unit of weak-area fraction.
    pub score: f64,
}

fn score_of(best: &[f32]) -> Score {
    let mut v = best.to_vec();
    let mid = v.len() / 2;
    let median = *v.select_nth_unstable_by(mid, |a, b| a.total_cmp(b)).1 as f64;
    let frac = best.iter().filter(|&&x| x < -70.0).count() as f64 / best.len().max(1) as f64;
    Score { median_dbm: median, frac_below_70: frac, score: median - 50.0 * frac }
}

pub fn score(cov: &Coverage) -> Score {
    score_of(&cov.best)
}

#[derive(Clone, Copy, Debug)]
pub struct Suggestion {
    pub x: f64,
    pub y: f64,
    pub score: f64,
}

/// Grid search for the best position of AP `idx`, other APs fixed.
/// ponytail: ~12x12 candidates, each a full map; finer search = local refinement around the winner.
pub fn suggest_placement(plan: &Plan, idx: usize) -> Suggestion {
    let edges = edges_for(plan);
    let others: Vec<f32> = plan
        .aps
        .iter()
        .enumerate()
        .filter(|(i, _)| *i != idx)
        .map(|(_, a)| ap_map(plan, a, edges[a.band.idx()].as_ref().unwrap()))
        .fold(vec![NO_SIGNAL_DBM; plan.width * plan.height], |mut acc, m| {
            acc.iter_mut().zip(m).for_each(|(a, b)| *a = a.max(b));
            acc
        });
    let step = (plan.width.max(plan.height) / 12).max(1);
    let mut ap = plan.aps[idx].clone();
    let e = &edges[ap.band.idx()].as_ref().unwrap();
    let mut best = Suggestion { x: ap.x, y: ap.y, score: f64::NEG_INFINITY };
    for j in (step / 2..plan.height).step_by(step) {
        for i in (step / 2..plan.width).step_by(step) {
            ap.x = i as f64 + 0.5;
            ap.y = j as f64 + 0.5;
            let mut m = ap_map(plan, &ap, e);
            m.iter_mut().zip(&others).for_each(|(a, b)| *a = a.max(*b));
            let s = score_of(&m).score;
            if s > best.score {
                best = Suggestion { x: ap.x, y: ap.y, score: s };
            }
        }
    }
    best
}

#[derive(Clone, Copy, Debug)]
pub struct Calibration {
    pub exponent: f64,
    pub offset_db: f64,
    pub rmse_before: f64,
    pub rmse_after: f64,
}

/// Least-squares fit of exponent `n` and offset against `plan.measurements`, using the direct
/// multi-wall model of the strongest AP at each point. Measured = A - n * 10log10(d) + offset is
/// linear in (n, offset). One point (or all at the same distance) fits the offset only.
pub fn calibrate(plan: &Plan) -> Option<Calibration> {
    if plan.measurements.is_empty() || plan.aps.is_empty() {
        return None;
    }
    // per point: (x = 10 log10 d, y = measured - A)
    let pts: Vec<(f64, f64)> = plan
        .measurements
        .iter()
        .map(|m| {
            let ap = plan
                .aps
                .iter()
                .max_by(|a, b| direct_rssi(plan, a, m.x, m.y).total_cmp(&direct_rssi(plan, b, m.x, m.y)))
                .unwrap();
            let d = ((m.x - ap.x).hypot(m.y - ap.y) * plan.cell_m).max(MIN_DIST_M);
            let walls = direct_wall_loss(plan, ap.band, (ap.x, ap.y), (m.x, m.y));
            let a = ap.tx_dbm + ap.gain_dbi - fspl_db(1.0, ap.band.freq_mhz()) - walls;
            (10.0 * d.log10(), m.dbm - a)
        })
        .collect();
    let k = pts.len() as f64;
    let mx = pts.iter().map(|p| p.0).sum::<f64>() / k;
    let my = pts.iter().map(|p| p.1).sum::<f64>() / k;
    let sxx: f64 = pts.iter().map(|p| (p.0 - mx).powi(2)).sum();
    let sxy: f64 = pts.iter().map(|p| (p.0 - mx) * (p.1 - my)).sum();
    let exponent = if sxx > 1e-9 { (-sxy / sxx).clamp(1.0, 8.0) } else { plan.exponent };
    let offset_db = my + exponent * mx;
    let rmse = |n: f64, off: f64| (pts.iter().map(|p| (p.1 - (off - n * p.0)).powi(2)).sum::<f64>() / k).sqrt();
    Some(Calibration {
        exponent,
        offset_db,
        rmse_before: rmse(plan.exponent, plan.offset_db),
        rmse_after: rmse(exponent, offset_db),
    })
}
