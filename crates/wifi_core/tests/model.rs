use wifi_core::*;

fn approx(a: f64, b: f64, tol: f64) -> bool {
    (a - b).abs() <= tol
}

fn ap(x: f64, y: f64) -> Ap {
    Ap { x, y, band: Band::G2_4, tx_dbm: 20.0, gain_dbi: 2.0, channel: 1 }
}

fn wall(x1: f64, y1: f64, x2: f64, y2: f64, material: Material) -> Wall {
    Wall { x1, y1, x2, y2, material }
}

#[test]
fn fspl_matches_friis() {
    assert!(approx(fspl_db(1.0, 2400.0), 40.05, 0.05), "{}", fspl_db(1.0, 2400.0));
    assert!(approx(fspl_db(10.0, 5000.0), 66.43, 0.05), "{}", fspl_db(10.0, 5000.0));
    // log-distance with n=2 equals FSPL
    assert!(approx(log_distance_db(10.0, 5000.0, 2.0), fspl_db(10.0, 5000.0), 1e-9));
    // n=3 adds 10 dB per decade more than n=2
    assert!(approx(log_distance_db(10.0, 5000.0, 3.0) - fspl_db(10.0, 5000.0), 10.0, 1e-9));
}

#[test]
fn wall_losses_sum_along_direct_ray() {
    let mut plan = Plan::new(60, 20, 0.1);
    plan.walls = vec![
        wall(10.0, 0.0, 10.0, 20.0, Material::Drywall),
        wall(20.0, 0.0, 20.0, 20.0, Material::Brick),
        wall(30.0, 0.0, 30.0, 20.0, Material::Concrete),
        wall(50.0, 0.0, 50.0, 20.0, Material::Metal), // beyond target: not crossed
    ];
    let t = &plan.materials;
    let expected = t.loss(Material::Drywall, Band::G2_4)
        + t.loss(Material::Brick, Band::G2_4)
        + t.loss(Material::Concrete, Band::G2_4);
    let got = direct_wall_loss(&plan, Band::G2_4, (5.0, 10.0), (40.0, 10.0));
    assert!(approx(got, expected, 1e-9), "got {got} want {expected}");
    // higher band attenuates more
    assert!(direct_wall_loss(&plan, Band::G5, (5.0, 10.0), (40.0, 10.0)) > got);
    // direct RSSI = tx + gain - FSPL - walls
    let a = ap(5.0, 10.0);
    let d_m = 35.0 * 0.1;
    let want = 22.0 - fspl_db(d_m, Band::G2_4.freq_mhz()) - expected;
    assert!(approx(direct_rssi(&plan, &a, 39.5, 9.5), want, 0.2));
}

/// Closed concrete room (40..80 x 40..80) with one door gap in the top wall near the left corner.
fn room_with_door() -> Plan {
    let mut plan = Plan::new(100, 100, 0.1);
    let c = Material::Concrete;
    plan.walls = vec![
        wall(40.0, 40.0, 42.0, 40.0, c), // top wall, gap 42..50
        wall(50.0, 40.0, 80.0, 40.0, c),
        wall(80.0, 40.0, 80.0, 80.0, c),
        wall(80.0, 80.0, 40.0, 80.0, c),
        wall(40.0, 80.0, 40.0, 40.0, c),
    ];
    plan.aps = vec![ap(20.5, 60.5)];
    plan
}

#[test]
fn dominant_path_through_door_beats_wall_heavy_direct_path() {
    let plan = room_with_door();
    let a = &plan.aps[0];
    let (tx, ty) = (75usize, 75usize);
    let (cx, cy) = (tx as f64 + 0.5, ty as f64 + 0.5);
    // the direct ray really crosses the concrete wall
    let walls = direct_wall_loss(&plan, a.band, (a.x, a.y), (cx, cy));
    assert!(walls >= plan.materials.loss(Material::Concrete, a.band) - 1e-9);

    let direct = direct_rssi(&plan, a, cx, cy);
    let dpm = dominant_rssi_map(&plan, a);
    let i = ty * plan.width + tx;
    assert!((dpm[i] as f64) > direct + 3.0, "dpm {} direct {direct}", dpm[i]);

    let cov = coverage(&plan);
    assert!(approx(cov.best[i] as f64, dpm[i] as f64, 1e-3));
}

#[test]
fn closed_room_does_not_leak_through_corners() {
    let mut plan = room_with_door();
    plan.walls[0] = wall(40.0, 40.0, 80.0, 40.0, Material::Concrete); // close the door
    plan.walls.remove(1);
    let a = plan.aps[0].clone();
    let dpm = dominant_rssi_map(&plan, &a);
    for (tx, ty) in [(41usize, 41usize), (60, 60), (78, 78), (41, 78)] {
        let i = ty * plan.width + tx;
        let direct = direct_rssi(&plan, &a, tx as f64 + 0.5, ty as f64 + 0.5);
        assert!((dpm[i] as f64) <= direct + 0.5, "leak at {tx},{ty}: dpm {} direct {direct}", dpm[i]);
    }
}

#[test]
fn calibration_recovers_exponent_and_offset() {
    let mut plan = room_with_door();
    plan.walls.clear();
    let a = plan.aps[0].clone();
    // synthetic "measurements" from n = 3.1, offset = -4 dB
    let mut truth = plan.clone();
    truth.exponent = 3.1;
    truth.offset_db = -4.0;
    plan.measurements = [(30.5, 60.5), (50.5, 20.5), (80.5, 90.5), (95.5, 10.5), (22.5, 62.5)]
        .iter()
        .map(|&(x, y)| Measurement { x, y, dbm: direct_rssi(&truth, &a, x, y) })
        .collect();
    let fit = calibrate(&plan).expect("fit");
    assert!(approx(fit.exponent, 3.1, 1e-6), "{fit:?}");
    assert!(approx(fit.offset_db, -4.0, 1e-6), "{fit:?}");
    assert!(fit.rmse_after < 1e-6 && fit.rmse_before > 1.0, "{fit:?}");
    // single point: offset-only fit keeps exponent
    plan.measurements.truncate(1);
    let fit1 = calibrate(&plan).expect("fit1");
    assert!(approx(fit1.exponent, plan.exponent, 1e-12));
    assert!(fit1.rmse_after < 1e-6);
    plan.measurements.clear();
    assert!(calibrate(&plan).is_none());
}

#[test]
fn plan_json_round_trip() {
    let mut plan = room_with_door();
    plan.materials.set(Material::Glass, Band::G6, 9.5);
    plan.measurements.push(Measurement { x: 1.0, y: 2.0, dbm: -61.0 });
    let json = plan.to_json();
    let back = Plan::from_json(&json).unwrap();
    assert_eq!(plan, back);
    assert!(Plan::from_json("{not json").is_err());
}

#[test]
fn url_share_round_trip() {
    let plan = room_with_door();
    let s = plan.to_share_string();
    assert!(s.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'), "{s}");
    assert!(s.len() < plan.to_json().len(), "compressed should be shorter");
    assert_eq!(Plan::from_share_string(&s).unwrap(), plan);
    assert!(Plan::from_share_string("!!!").is_err());
}

#[test]
fn examples_parse() {
    for src in [include_str!("../../../examples/apartment.json"), include_str!("../../../examples/office.json")] {
        let p = Plan::from_json(src).unwrap();
        assert!(!p.walls.is_empty() && !p.aps.is_empty());
        let cov = coverage(&p);
        assert_eq!(cov.best.len(), p.width * p.height);
    }
}

#[test]
fn placement_suggestion_prefers_centre_of_empty_plan() {
    let mut plan = Plan::new(60, 60, 0.25);
    plan.aps = vec![ap(1.5, 1.5)];
    let s = suggest_placement(&plan, 0);
    assert!((s.x - 30.0).abs() < 12.0 && (s.y - 30.0).abs() < 12.0, "{s:?}");
    let before = score(&coverage(&plan));
    assert!(s.score > before.score, "{s:?} vs {before:?}");
}

#[test]
fn sinr_drops_with_co_channel_ap() {
    let mut plan = Plan::new(80, 40, 0.1);
    plan.aps = vec![ap(10.5, 20.5)];
    let one = coverage(&plan);
    assert!(one.sinr.is_some());
    plan.aps.push(ap(70.5, 20.5));
    let two = coverage(&plan);
    let i = 20 * 80 + 40; // midpoint: equal signals => SINR ~ 0 dB
    assert!(two.sinr.as_ref().unwrap()[i] < 1.0);
    assert!(one.sinr.as_ref().unwrap()[i] > 20.0);
    // different channel => no interference
    plan.aps[1].channel = 11;
    let three = coverage(&plan);
    assert!(three.sinr.as_ref().unwrap()[i] > 20.0);
}

#[test]
fn grid_200x200_recompute_is_fast() {
    let mut plan = Plan::new(200, 200, 0.1);
    for k in 0..30 {
        let o = 6.0 * k as f64 + 5.0;
        plan.walls.push(wall(o, 0.0, o, 120.0 + (k % 5) as f64 * 10.0, Material::Brick));
    }
    plan.aps = vec![ap(100.5, 100.5), Ap { band: Band::G5, ..ap(20.5, 180.5) }];
    let t = std::time::Instant::now();
    let cov = coverage(&plan);
    let ms = t.elapsed().as_secs_f64() * 1000.0;
    eprintln!("200x200, 30 walls, 2 APs: {ms:.1} ms (native, {} build)", if cfg!(debug_assertions) { "debug" } else { "release" });
    assert_eq!(cov.best.len(), 40_000);
    assert!(ms < if cfg!(debug_assertions) { 5000.0 } else { 300.0 });
}
