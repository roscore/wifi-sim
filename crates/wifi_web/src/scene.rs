//! 3D view of the plan on top of `gl.rs`: heatmap floor (live texture), extruded walls,
//! AP devices with pulsing rings, optional signal rays, orbit / top / walk-through cameras.
use crate::gl::{self, FieldPaint, Geo, Mesh, Paint, Renderer};
use crate::math::{self, Orbit, OrbitCam, M4, V};
use crate::view::{self, Fps};
use web_sys::{HtmlCanvasElement, WebGlTexture};
use wifi_core::{Material, Plan, Wall};

pub const WALL_H: f32 = 2.4;
/// Height of the AP device body above the floor (m).
pub const AP_H: f32 = 1.15;
const ACCENT: V = [0.30, 0.78, 1.0];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CamMode {
    Perspective,
    Top,
    Walk,
}

pub struct Scene {
    pub r: Renderer,
    cube: Mesh,
    cyl: Mesh,
    sphere: Mesh,
    ring: Mesh,
    opaque: Option<Mesh>,
    glass: Option<Mesh>,
    built: Option<(Vec<Wall>, u64, Vec<usize>)>,
    field: Option<WebGlTexture>,
    lut: WebGlTexture,
    rays: Option<Mesh>,
    pub orbit: OrbitCam,
    pub fps: Fps,
    pub mode: CamMode,
    pub vp: M4,
    pub inv_vp: M4,
    pub eye: V,
}

/// Inputs for one frame that come from the app.
pub struct Frame<'a> {
    pub plan: &'a Plan,
    pub selected: Option<usize>,
    /// Selected walls (highlighted).
    pub sel_walls: &'a [usize],
    pub hover: Option<(f64, f64)>,
    pub drag_ap: Option<usize>,
    /// Hovered AP (glow).
    pub hot_ap: Option<usize>,
    pub time: f64,
    pub has_field: bool,
    pub iso: bool,
    pub rays: bool,
}

/// Appends an oriented box (cube scaled by `size` along the basis `x, y, z`, centred at `c`).
fn push_box(g: &mut Geo, c: V, axes: [V; 3], size: V, col: V) {
    let unit = gl::cube();
    let m = math::basis(axes[0], axes[1], axes[2], size, c);
    for &i in &unit.idx {
        let i = i as usize;
        let p: V = [unit.pos[i * 3], unit.pos[i * 3 + 1], unit.pos[i * 3 + 2]];
        let n: V = [unit.nrm[i * 3], unit.nrm[i * 3 + 1], unit.nrm[i * 3 + 2]];
        g.pos.extend(math::transform(&m, p));
        // axis-aligned unit normals stay normals under a rotation + per-axis scale
        let rn = math::norm(math::add(math::add(math::scale(axes[0], n[0]), math::scale(axes[1], n[1])), math::scale(axes[2], n[2])));
        g.nrm.extend(rn);
        g.col.extend(col);
    }
}

/// Box from `a` to `b` (world, on the floor) with the given thickness and height.
fn push_beam(g: &mut Geo, a: V, b: V, thick: f32, height: f32, col: V) {
    let d = math::sub(b, a);
    let len = math::dot(d, d).sqrt();
    if len < 1e-4 {
        return;
    }
    let x = math::scale(d, 1.0 / len);
    let up = if x[1].abs() > 0.9 { [1.0, 0.0, 0.0] } else { [0.0, 1.0, 0.0] };
    let z = math::norm(math::cross(x, up));
    let y = math::cross(z, x);
    let c = math::add(math::scale(math::add(a, b), 0.5), math::scale(y, height / 2.0));
    push_box(g, c, [x, y, z], [len + thick * 0.999, height, thick], col);
}

/// Extruded height of a wall (m): doors are lower.
pub fn wall_height(m: Material) -> f32 {
    if m == Material::WoodDoor {
        2.1
    } else {
        WALL_H
    }
}

/// Wall solids, split into opaque and glass; selected walls are tinted with the accent colour.
pub fn wall_geo(plan: &Plan, sel: &[usize]) -> (Geo, Geo) {
    let (mut opaque, mut glass) = (Geo::default(), Geo::default());
    for (i, w) in plan.walls.iter().enumerate() {
        let (col, t) = view::material_style(w.material);
        let mut col = view::rgbf(col);
        if sel.contains(&i) {
            col = math::add(math::scale(col, 0.35), math::scale(ACCENT, 0.65));
        }
        let a = view::cell_to_world(plan, w.x1, w.y1, 0.0);
        let b = view::cell_to_world(plan, w.x2, w.y2, 0.0);
        let g = if w.material == Material::Glass { &mut glass } else { &mut opaque };
        push_beam(g, a, b, t as f32, wall_height(w.material), col);
    }
    (opaque, glass)
}

fn bits(p: &Plan) -> u64 {
    p.cell_m.to_bits() ^ ((p.width as u64) << 32) ^ p.height as u64
}

impl Scene {
    pub fn new(canvas: &HtmlCanvasElement) -> Option<Scene> {
        let mut r = Renderer::new(canvas)?;
        r.theme = gl::Theme { shadow: 0.3, ..gl::DARK };
        let lut = r.lut_texture(&view::lut())?;
        let o = Orbit { target: [0.0; 3], yaw: 0.5, pitch: 0.85, dist: 15.0 };
        Some(Scene {
            cube: r.mesh(&gl::cube())?,
            cyl: r.mesh(&gl::cylinder(24))?,
            sphere: r.mesh(&gl::sphere(16))?,
            ring: r.mesh(&gl::ring(0.92, 1.0, 64))?,
            r,
            opaque: None,
            glass: None,
            built: None,
            field: None,
            lut,
            rays: None,
            orbit: OrbitCam::new(o),
            fps: Fps { pos: [1.0, Fps::EYE, 1.0], yaw: 0.0, pitch: 0.0 },
            mode: CamMode::Perspective,
            vp: math::IDENTITY,
            inv_vp: math::IDENTITY,
            eye: [0.0; 3],
        })
    }

    /// Plan extent in metres (x, z).
    pub fn extent(plan: &Plan) -> (f32, f32) {
        ((plan.width as f64 * plan.cell_m) as f32, (plan.height as f64 * plan.cell_m) as f32)
    }

    /// Camera preset (glides there). Walk starts at the plan centre looking "up".
    pub fn preset(&mut self, plan: &Plan, mode: CamMode) {
        let (w, d) = Self::extent(plan);
        let target = [w / 2.0, 0.0, d / 2.0];
        let size = w.max(d);
        self.mode = mode;
        match mode {
            CamMode::Perspective => self.orbit.set_goal(Orbit { target, yaw: 0.45, pitch: 0.8, dist: size * 1.15 }),
            CamMode::Top => self.orbit.set_goal(Orbit { target, yaw: 0.0, pitch: math::PITCH_MAX, dist: size * 1.2 }),
            CamMode::Walk => {
                // stand a metre from the first AP toward the middle, looking at the middle
                let ap = plan.aps.first().map(|a| view::cell_to_world(plan, a.x, a.y, 0.0)).unwrap_or(target);
                let (dx, dz) = (target[0] - ap[0], target[2] - ap[2]);
                let len = dx.hypot(dz);
                let (ux, uz) = if len > 0.5 { (dx / len, dz / len) } else { (0.0, -1.0) };
                let pos = [(ap[0] + ux).clamp(0.0, w), Fps::EYE, (ap[2] + uz).clamp(0.0, d)];
                // forward = (−sin yaw, −cos yaw) on the floor
                self.fps = Fps { pos, yaw: (-ux).atan2(-uz), pitch: -0.12 };
            }
        }
        self.r.fog = [size * 2.0, size * 6.0];
    }

    pub fn jump(&mut self) {
        self.orbit.cur = self.orbit.goal;
    }

    /// Upload the displayed grid (dBm or mapped SINR) as the floor texture.
    pub fn set_field(&mut self, vals: &[f32], w: usize, h: usize) {
        let data = view::pack_grid(vals, w, h);
        self.field = self.r.field_texture(self.field.take(), w, h, &data);
    }

    fn sync_walls(&mut self, plan: &Plan, sel: &[usize]) {
        if self.built.as_ref().is_some_and(|(w, b, s)| *w == plan.walls && *b == bits(plan) && s == sel) {
            return;
        }
        for m in [self.opaque.take(), self.glass.take()].into_iter().flatten() {
            self.r.free(m);
        }
        let (o, g) = wall_geo(plan, sel);
        self.opaque = (!o.pos.is_empty()).then(|| self.r.mesh(&o)).flatten();
        self.glass = (!g.pos.is_empty()).then(|| self.r.mesh(&g)).flatten();
        self.built = Some((plan.walls.clone(), bits(plan), sel.to_vec()));
    }

    /// AP index under the screen ray (device body or its floor footprint).
    pub fn pick_ap(&self, plan: &Plan, ndc: (f32, f32)) -> Option<usize> {
        let (o, d) = math::ndc_ray(&self.inv_vp, ndc.0, ndc.1);
        let floor = view::floor_pick(&self.inv_vp, ndc.0, ndc.1, 0.0);
        plan.aps
            .iter()
            .enumerate()
            .filter_map(|(i, a)| {
                let c = view::cell_to_world(plan, a.x, a.y, AP_H);
                if let Some(t) = math::ray_sphere(o, d, c, 0.4) {
                    return Some((i, t));
                }
                let f = floor?;
                let dist = (f[0] - c[0]).hypot(f[2] - c[2]);
                (dist < 0.45).then(|| (i, math::dot(math::sub(f, o), d) + 100.0))
            })
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(i, _)| i)
    }

    fn rays_geo(plan: &Plan, f: &Frame, from: usize, to: (f64, f64)) -> Geo {
        let mut g = Geo::default();
        let ap = &plan.aps[from];
        let a = view::cell_to_world(plan, ap.x, ap.y, AP_H);
        let b = view::cell_to_world(plan, to.0, to.1, 0.05);
        let cr = view::ray_crossings(plan, ap.band, (ap.x, ap.y), to);
        let sel = f.selected.unwrap_or(0) == from;
        let thick = if sel { 0.035 } else { 0.018 };
        let lerp = |t: f32| math::add(a, math::scale(math::sub(b, a), t));
        let mut t0 = 0.0f32;
        let mut acc = 0.0;
        let mut col = if sel { [0.85, 0.95, 1.0] } else { [0.45, 0.55, 0.65] };
        for c in cr.iter().map(|c| (c.t as f32, c.loss_db)).chain([(1.0, 0.0)]) {
            let seg_c = if acc > 0.0 && sel { view::rgbf(view::loss_color(acc)) } else { col };
            push_beam(&mut g, lerp(t0), lerp(c.0), thick, thick, seg_c);
            if c.0 < 1.0 && sel {
                let p = lerp(c.0);
                let k = view::rgbf(view::loss_color(c.1));
                push_box(&mut g, p, [[1., 0., 0.], [0., 1., 0.], [0., 0., 1.]], [0.12, 0.12, 0.12], k);
            }
            acc += c.1;
            t0 = c.0;
            if !sel {
                col = math::scale(col, 0.9);
            }
        }
        g
    }

    /// Draw one frame into a `w × h` device-pixel viewport.
    pub fn draw(&mut self, f: &Frame, w: i32, h: i32, dt: f32) -> bool {
        let plan = f.plan;
        self.sync_walls(plan, f.sel_walls);
        let moving = if self.mode == CamMode::Walk { false } else { self.orbit.step(dt) };
        let aspect = w as f32 / h.max(1) as f32;
        let (view, eye) = match self.mode {
            CamMode::Walk => (self.fps.view(), self.fps.pos),
            _ => {
                let o = self.orbit.cur;
                (math::look_at(o.eye(), o.target, [0.0, 1.0, 0.0]), o.eye())
            }
        };
        let fov = if self.mode == CamMode::Walk { 1.15 } else { 0.85 };
        let proj = math::perspective(fov, aspect, 0.05, 600.0);
        self.vp = math::mul(&proj, &view);
        self.inv_vp = math::invert(&self.vp).unwrap_or(math::IDENTITY);
        self.eye = eye;
        let r = &self.r;
        r.begin(&view, &proj, eye, [0, 0, w, h]);

        let (pw, pd) = Self::extent(plan);
        if let Some(t) = &self.field {
            r.draw_field(
                [0.0, 0.0, pw, pd],
                t,
                &self.lut,
                FieldPaint {
                    base: [0.11, 0.13, 0.17],
                    alpha: if f.has_field { 0.92 } else { 0.0 },
                    range: [view::PACK_LO, view::PACK_HI],
                    levels: view::LEVELS,
                    iso: if f.iso && f.has_field { 1.0 } else { 0.0 },
                },
            );
        }

        // AP model matrices: stand, body, two antennas, LED
        let id = [[1., 0., 0.], [0., 1., 0.], [0., 0., 1.]];
        let ap_parts = |x: f32, z: f32| -> [(M4, u8); 5] {
            let b = |s: V, t: V| math::basis(id[0], id[1], id[2], s, t);
            [
                (b([0.05, AP_H - 0.04, 0.05], [x, (AP_H - 0.04) / 2.0, z]), 0),
                (b([0.34, 0.08, 0.34], [x, AP_H, z]), 1),
                (b([0.025, 0.26, 0.025], [x - 0.12, AP_H + 0.15, z]), 2),
                (b([0.025, 0.26, 0.025], [x + 0.12, AP_H + 0.15, z]), 2),
                (b([0.05, 0.05, 0.05], [x, AP_H + 0.05, z]), 3),
            ]
        };
        let aps: Vec<(usize, V)> =
            plan.aps.iter().enumerate().map(|(i, a)| (i, view::cell_to_world(plan, a.x, a.y, 0.0))).collect();

        if let Some(m) = &self.opaque {
            r.shadow(m, &math::IDENTITY);
        }
        for (_, p) in &aps {
            for (m, k) in ap_parts(p[0], p[2]).iter().take(4) {
                r.shadow(if k % 2 == 1 { &self.cyl } else { &self.cube }, m);
            }
        }
        if let Some(m) = &self.opaque {
            r.draw(m, &math::IDENTITY, Paint::lit([1.0; 4]));
        }
        for (i, p) in &aps {
            let sel = f.selected == Some(*i);
            let hot = f.hot_ap == Some(*i) || f.drag_ap == Some(*i);
            let glow = if hot { 0.3 } else if sel { 0.12 } else { 0.0 };
            for (m, k) in ap_parts(p[0], p[2]) {
                let (mesh, col): (&Mesh, [f32; 4]) = match k {
                    0 => (&self.cyl, [0.55, 0.60, 0.68, 1.0]),
                    1 => (&self.cyl, if sel { [0.86, 0.95, 1.0, 1.0] } else { [0.93, 0.95, 0.97, 1.0] }),
                    2 => (&self.cube, [0.20, 0.23, 0.28, 1.0]),
                    _ => (&self.sphere, [ACCENT[0], ACCENT[1], ACCENT[2], 1.0]),
                };
                let mut paint = Paint::lit(col);
                paint.glow = if k == 3 { 0.5 + 0.3 * (f.time * 4.0).sin().abs() as f32 } else { glow };
                r.draw(mesh, &m, paint);
            }
        }

        // signal rays from every AP to the cursor (selected one bright, with wall crossings)
        if let Some(m) = self.rays.take() {
            self.r.free(m);
        }
        let r = &self.r;
        if let (true, Some(hv)) = (f.rays, f.hover) {
            let mut g = Geo::default();
            for i in 0..plan.aps.len() {
                let gi = Self::rays_geo(plan, f, i, hv);
                g.pos.extend(gi.pos);
                g.nrm.extend(gi.nrm);
                g.col.extend(gi.col);
            }
            if !g.pos.is_empty() {
                self.rays = r.mesh(&g);
            }
        }
        if let Some(m) = &self.rays {
            let mut p = Paint::flat([1.0; 4]);
            p.glow = 0.1;
            r.draw(m, &math::IDENTITY, p);
        }

        // translucent last: glass, rings, cursor marker
        if let Some(m) = &self.glass {
            r.draw(m, &math::IDENTITY, Paint::lit([1.0, 1.0, 1.0, 0.32]));
        }
        for (i, p) in &aps {
            let sel = f.selected == Some(*i);
            for k in 0..2 {
                let ph = ((f.time * 0.6 + k as f64 * 0.5 + *i as f64 * 0.17).fract()) as f32;
                let s = 0.3 + ph * 2.4;
                let a = (1.0 - ph).powf(1.5) * if sel { 0.75 } else { 0.45 };
                let m = math::basis(id[0], id[1], id[2], [s, 1.0, s], [p[0], 0.02 + k as f32 * 0.001, p[2]]);
                let mut paint = Paint::flat([ACCENT[0], ACCENT[1], ACCENT[2], a]);
                paint.glow = 0.2;
                r.draw(&self.ring, &m, paint);
            }
            // soft halo around the LED
            let halo = math::basis(id[0], id[1], id[2], [0.22; 3], [p[0], AP_H + 0.05, p[2]]);
            let pulse = 0.18 + 0.1 * (f.time * 3.0 + *i as f64).sin() as f32;
            r.draw(&self.sphere, &halo, Paint::flat([ACCENT[0], ACCENT[1], ACCENT[2], pulse]));
        }
        if let Some(hv) = f.hover.filter(|_| f.drag_ap.is_none()) {
            let p = view::cell_to_world(plan, hv.0, hv.1, 0.03);
            let m = math::basis(id[0], id[1], id[2], [0.22, 1.0, 0.22], p);
            r.draw(&self.ring, &m, Paint::flat([1.0, 1.0, 1.0, 0.85]));
        }
        moving
    }
}
