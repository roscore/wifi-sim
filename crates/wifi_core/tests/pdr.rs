use std::f64::consts::{FRAC_PI_2, PI};
use wifi_core::pdr::*;
use wifi_core::Material;

/// Tiny LCG so traces are reproducible without a `rand` dependency. Uniform in [-1, 1).
struct Lcg(u64);
impl Lcg {
    fn next(&mut self) -> f64 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (self.0 >> 11) as f64 / (1u64 << 52) as f64 - 1.0
    }
}

const HZ: f64 = 50.0;
const STEP_S: f64 = 0.55;

/// Feeds `n` steps (one accel peak each) plus noise; returns steps detected.
fn walk_steps(w: &mut Walk, det: &mut StepDetector, t: &mut f64, rng: &mut Lcg, n: usize, mut heading: impl FnMut(f64) -> f64) -> usize {
    let mut got = 0;
    let (samples, t0) = ((n as f64 * STEP_S * HZ).round() as usize, *t);
    for _ in 0..samples {
        let phase = ((*t - t0) % STEP_S) / STEP_S;
        let a = 9.81 + 2.5 * (2.0 * PI * phase).sin() + 0.8 * rng.next();
        w.set_heading(heading(*t));
        if det.push(*t, a) {
            w.step();
            got += 1;
        }
        *t += 1.0 / HZ;
    }
    got
}

/// Standing still (turning at a corner): gravity + noise only.
fn stand(det: &mut StepDetector, t: &mut f64, rng: &mut Lcg, secs: f64) -> usize {
    let mut got = 0;
    for _ in 0..(secs * HZ) as usize {
        got += det.push(*t, 9.81 + 0.4 * rng.next()) as usize;
        *t += 1.0 / HZ;
    }
    got
}

fn perimeter(pts: &[(f64, f64)]) -> f64 {
    (0..pts.len()).map(|i| {
        let (a, b) = (pts[i], pts[(i + 1) % pts.len()]);
        (b.0 - a.0).hypot(b.1 - a.1)
    }).sum()
}

fn axis_aligned(pts: &[(f64, f64)]) -> bool {
    (0..pts.len()).all(|i| {
        let (a, b) = (pts[i], pts[(i + 1) % pts.len()]);
        (b.0 - a.0).abs() < 1e-9 || (b.1 - a.1).abs() < 1e-9
    })
}

/// Walks a 4 x 3 m rectangle (0.5 m steps: 8/6/8/6) with heading noise, a 30 deg unknown phone
/// offset and gyro drift. Returns the walk standing back at the start, loop not yet closed.
fn square_walk(drift_deg_s: f64) -> Walk {
    let mut rng = Lcg(7);
    let mut det = StepDetector::new();
    let mut w = Walk::new(0.5);
    let mut t = 0.0;
    let offset = 30f64.to_radians();
    stand(&mut det, &mut t, &mut rng, 2.0);
    w.mark(Mark::Corner);
    let mut total = 0;
    for (k, n) in [8, 6, 8, 6].into_iter().enumerate() {
        let side = k as f64 * FRAC_PI_2;
        let mut noise = Lcg(100 + k as u64);
        total += walk_steps(&mut w, &mut det, &mut t, &mut rng, n, |t| {
            side + offset + (drift_deg_s * t).to_radians() + 4f64.to_radians() * noise.next()
        });
        assert_eq!(stand(&mut det, &mut t, &mut rng, 1.0), 0, "no steps while standing");
        if k < 3 {
            w.mark(Mark::Corner);
        }
    }
    assert_eq!(total, 28, "every synthetic step detected exactly once");
    w
}

#[test]
fn step_detector_counts_peaks_and_ignores_noise() {
    let mut rng = Lcg(1);
    let mut det = StepDetector::new();
    let mut w = Walk::new(0.7);
    let mut t = 0.0;
    assert_eq!(stand(&mut det, &mut t, &mut rng, 3.0), 0);
    assert_eq!(walk_steps(&mut w, &mut det, &mut t, &mut rng, 20, |_| 0.0), 20);
    assert!((w.pos().0 - 14.0).abs() < 1e-9 && w.pos().1.abs() < 1e-9, "{:?}", w.pos());
    // a double peak within the min step interval counts once
    let mut det = StepDetector::new();
    let mut t = 0.0;
    stand(&mut det, &mut t, &mut rng, 2.0);
    let mut n = 0;
    for a in [13.0, 9.0, 13.0, 9.81, 9.81, 9.81] {
        n += det.push(t, a) as usize;
        t += 0.05;
    }
    assert_eq!(n, 1);
}

#[test]
fn square_room_closes_within_5_percent_after_correction() {
    let mut w = square_walk(1.0);
    let gap = w.close_loop();
    assert!(gap > 0.5, "drift must open the loop before correction, gap {gap}");
    w.snap = true; // "straighten" works before or after closing: geometry is derived on demand
    let pts: Vec<(f64, f64)> = w.rooms()[0].iter().map(|v| (v.x, v.y)).collect();
    assert_eq!(pts.len(), 4, "closing point merged into the start");
    // output frame: first walked wall along +x, so a snapped room is axis-aligned
    assert!(axis_aligned(&pts), "{pts:?}");
    let err = (perimeter(&pts) - 14.0).abs() / 14.0;
    assert!(err < 0.05, "perimeter {} ({:.1}%)", perimeter(&pts), err * 100.0);
    // sides come out 4 / 3 / 4 / 3
    for (i, want) in [4.0, 3.0, 4.0, 3.0].into_iter().enumerate() {
        let (a, b) = (pts[i], pts[(i + 1) % 4]);
        assert!(((b.0 - a.0).hypot(b.1 - a.1) - want).abs() < 0.2, "side {i}: {pts:?}");
    }
}

#[test]
fn loop_closure_without_snap_still_closes() {
    let mut w = square_walk(0.5);
    let gap = w.close_loop();
    assert!(gap > 0.3, "gap {gap}");
    let pts: Vec<(f64, f64)> = w.rooms()[0].iter().map(|v| (v.x, v.y)).collect();
    let err = (perimeter(&pts) - 14.0).abs() / 14.0;
    assert!(err < 0.05, "perimeter {}", perimeter(&pts));
    // the gap is spread over all walls, not dumped into the closing one
    for (i, want) in [4.0, 3.0, 4.0, 3.0].into_iter().enumerate() {
        let (a, b) = (pts[i], pts[(i + 1) % 4]);
        let got = (b.0 - a.0).hypot(b.1 - a.1);
        assert!((got - want).abs() < 0.05 * want, "side {i}: {got} ({pts:?})");
    }
}

fn rotate(pts: &[(f64, f64)], a: f64) -> Vec<(f64, f64)> {
    let (s, c) = a.sin_cos();
    pts.iter().map(|p| (p.0 * c - p.1 * s, p.0 * s + p.1 * c)).collect()
}

#[test]
fn rectilinear_snap_relative_to_first_segment() {
    let r = 0.3; // first segment direction
    let mut pts = vec![(0.0, 0.0)];
    for (len, deg) in [(4.0, 0.0), (3.0, 86.0), (4.0, 183.0), (2.0, 268.0)] {
        let a = r + f64::to_radians(deg);
        let p = *pts.last().unwrap();
        pts.push((p.0 + len * a.cos(), p.1 + len * a.sin()));
    }
    snap_rectilinear(&mut pts, r);
    let flat = rotate(&pts, -r);
    let want = [(0.0, 0.0), (4.0, 0.0), (4.0, 3.0), (0.0, 3.0), (0.0, 1.0)];
    for (p, q) in flat.iter().zip(want) {
        assert!((p.0 - q.0).abs() < 1e-9 && (p.1 - q.1).abs() < 1e-9, "{flat:?}");
    }
}

#[test]
fn door_leaves_gap_in_wall() {
    let mut w = Walk::new(0.5);
    w.set_heading(0.0);
    w.mark(Mark::Corner);
    for _ in 0..3 {
        w.step(); // 1.5 m
    }
    w.mark(Mark::Door);
    for _ in 0..5 {
        w.step(); // 4 m
    }
    for (h, n) in [(FRAC_PI_2, 6), (PI, 8), (-FRAC_PI_2, 6)] {
        w.mark(Mark::Corner);
        w.set_heading(h);
        for _ in 0..n {
            w.step();
        }
    }
    w.close_loop();
    let walls = w.walls();
    let bottom: Vec<_> = walls.iter().filter(|s| s.y1.abs() < 1e-9 && s.y2.abs() < 1e-9).collect();
    assert_eq!(bottom.len(), 2, "{walls:?}");
    let mut xs: Vec<(f64, f64)> = bottom.iter().map(|s| (s.x1.min(s.x2), s.x1.max(s.x2))).collect();
    xs.sort_by(|a, b| a.0.total_cmp(&b.0));
    let gap = DOOR_M / 2.0;
    assert!((xs[0].0 - 0.0).abs() < 1e-9 && (xs[0].1 - (1.5 - gap)).abs() < 1e-9, "{xs:?}");
    assert!((xs[1].0 - (1.5 + gap)).abs() < 1e-9 && (xs[1].1 - 4.0).abs() < 1e-9, "{xs:?}");
    // the only loop is the outer one
    assert!(walls.iter().all(|s| s.material == Material::Concrete));
}

#[test]
fn rooms_join_at_doors_and_outer_loop_is_concrete() {
    // outer 4 x 3 with a door at (2, 3) on the far wall, then a 2 x 2 room starting at that door
    let mut w = Walk::new(1.0);
    let leg = |w: &mut Walk, h: f64, n: usize| {
        w.set_heading(h);
        (0..n).for_each(|_| w.step());
    };
    w.mark(Mark::Corner);
    leg(&mut w, 0.0, 4);
    w.mark(Mark::Corner);
    leg(&mut w, FRAC_PI_2, 3);
    w.mark(Mark::Corner);
    leg(&mut w, PI, 2);
    w.mark(Mark::Door);
    leg(&mut w, PI, 2);
    w.mark(Mark::Corner);
    leg(&mut w, -FRAC_PI_2, 3);
    w.close_loop();
    // dead reckoning is ~1 m off by the time room 2 starts at the door: anchoring must remove it
    leg(&mut w, 0.0, 2);
    leg(&mut w, FRAC_PI_2, 3);
    w.set_heading(0.3);
    w.step();
    w.mark(Mark::Door);
    w.set_dbm(-55.0);
    leg(&mut w, FRAC_PI_2, 2);
    w.mark(Mark::Corner);
    leg(&mut w, 0.0, 2);
    w.mark(Mark::Corner);
    leg(&mut w, -FRAC_PI_2, 2);
    w.mark(Mark::Corner);
    leg(&mut w, PI, 2);
    w.close_loop();
    let rooms = w.rooms();
    assert_eq!(rooms.len(), 2);
    assert!((rooms[1][0].x - 2.0).abs() < 1e-9 && (rooms[1][0].y - 3.0).abs() < 1e-9, "{:?}", rooms[1][0]);
    let walls = w.walls();
    assert_eq!(walls.iter().filter(|s| s.material == Material::Concrete).count(), 5, "{walls:?}");
    assert_eq!(walls.iter().filter(|s| s.material == Material::Drywall).count(), 4, "{walls:?}");
    let plan = w.to_plan(0.1);
    assert_eq!(plan.walls.len(), 9);
    assert_eq!(plan.measurements.len(), 1);
    assert_eq!(plan.measurements[0].dbm, -55.0);
    // walls fit inside the grid with a margin
    for s in &plan.walls {
        for v in [s.x1, s.x2] {
            assert!(v > 0.0 && v < plan.width as f64);
        }
        for v in [s.y1, s.y2] {
            assert!(v > 0.0 && v < plan.height as f64);
        }
    }
}

#[test]
fn drift_warning_and_undo() {
    let mut w = Walk::new(1.0);
    w.mark(Mark::Corner);
    w.step();
    w.mark(Mark::Corner);
    w.set_heading(FRAC_PI_2 + 20f64.to_radians()); // a right turn that drifted 20 deg
    w.step();
    w.mark(Mark::Corner);
    assert!((w.drift_deg() - 20.0).abs() < 1e-9, "{}", w.drift_deg());
    w.undo();
    assert_eq!(w.rooms()[0].len(), 2);
    assert!(w.drift_deg() < 1e-9);
    w.undo();
    w.undo();
    assert!(w.rooms().is_empty());
}

#[test]
fn step_length_calibration() {
    assert_eq!(calibrate_step(10.0, 16), Some(0.625));
    assert_eq!(calibrate_step(10.0, 0), None);
    assert_eq!(calibrate_step(-1.0, 5), None);
}
