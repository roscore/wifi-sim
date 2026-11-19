//! Small reusable WebGL2 renderer (no app-specific code): procedural meshes with normals,
//! hemisphere + directional Blinn-Phong lighting, checker/grid ground with fog, sky gradient,
//! planar projected shadows (stencil, no double blending), translucent volumes and line edges.
//! Optional per-vertex colours (multiplied with the paint colour) for batched point clouds.
use crate::math::{self, M4, V};
use web_sys::{HtmlCanvasElement, WebGl2RenderingContext as GL, WebGlBuffer, WebGlProgram, WebGlVertexArrayObject};
use wasm_bindgen::JsCast;

const LIT_VS: &str = r#"#version 300 es
layout(location=0) in vec3 a_pos;
layout(location=1) in vec3 a_nrm;
layout(location=2) in vec3 a_col;
uniform mat4 u_vp, u_model, u_post;
out vec3 v_pos, v_nrm, v_col;
void main() {
  v_col = a_col;
  vec4 w = u_model * vec4(a_pos, 1.0);
  v_nrm = transpose(inverse(mat3(u_model))) * a_nrm;
  w = u_post * w;
  v_pos = w.xyz;
  gl_Position = u_vp * w;
}"#;

const LIT_FS: &str = r#"#version 300 es
precision highp float;
in vec3 v_pos, v_nrm, v_col;
uniform vec4 u_color;
uniform vec3 u_eye, u_light, u_sky, u_gnd;
uniform float u_lit, u_glow, u_clip;
out vec4 o;
void main() {
  if (v_pos.y < u_clip) discard;
  vec3 c = u_color.rgb * v_col;
  if (u_lit > 0.5) {
    vec3 n = normalize(v_nrm);
    vec3 v = normalize(u_eye - v_pos);
    if (dot(n, v) < 0.0) n = -n;
    float d = max(dot(n, u_light), 0.0);
    float s = d > 0.0 ? pow(max(dot(n, normalize(u_light + v)), 0.0), 48.0) : 0.0;
    c = c * (mix(u_gnd, u_sky, n.y * 0.5 + 0.5) + d * vec3(0.95, 0.92, 0.85)) + 0.35 * s;
  }
  o = vec4(mix(c, vec3(1.0), u_glow), u_color.a);
}"#;

const GROUND_FS: &str = r#"#version 300 es
precision highp float;
in vec3 v_pos, v_nrm;
uniform vec3 u_eye, u_c1, u_c2, u_line, u_fog;
uniform vec2 u_fogr;
uniform float u_cell;
out vec4 o;
void main() {
  vec2 p = v_pos.xz / u_cell;
  vec3 c = mix(u_c1, u_c2, mod(floor(p.x) + floor(p.y), 2.0));
  vec2 g = abs(fract(p - 0.5) - 0.5) / fwidth(p);
  c = mix(c, u_line, (1.0 - min(min(g.x, g.y), 1.0)) * 0.7);
  vec2 a = abs(v_pos.xz) / fwidth(v_pos.xz);
  c = mix(c, u_line, (1.0 - min(min(a.x, a.y) * 0.5, 1.0)));
  o = vec4(mix(c, u_fog, smoothstep(u_fogr.x, u_fogr.y, length(v_pos - u_eye))), 1.0);
}"#;

const SKY_VS: &str = r#"#version 300 es
out vec2 v_ndc;
void main() {
  v_ndc = vec2(gl_VertexID & 1, gl_VertexID >> 1) * 4.0 - 1.0;
  gl_Position = vec4(v_ndc, 0.9999, 1.0);
}"#;

const SKY_FS: &str = r#"#version 300 es
precision highp float;
in vec2 v_ndc;
uniform mat4 u_inv;
uniform vec3 u_eye, u_top, u_hor;
out vec4 o;
void main() {
  vec4 p = u_inv * vec4(v_ndc, 1.0, 1.0);
  vec3 d = normalize(p.xyz / p.w - u_eye);
  o = vec4(mix(u_hor, u_top, smoothstep(0.0, 0.5, d.y)), 1.0);
}"#;

/// Colours for one look (dark like a physics viewer, or light).
#[derive(Clone, Copy)]
pub struct Theme {
    pub sky_top: V,
    pub horizon: V,
    pub checker: [V; 2],
    pub grid: V,
    pub amb_sky: V,
    pub amb_ground: V,
    pub shadow: f32,
}

pub const DARK: Theme = Theme {
    sky_top: [0.05, 0.07, 0.12],
    horizon: [0.30, 0.34, 0.42],
    checker: [[0.20, 0.22, 0.26], [0.25, 0.27, 0.31]],
    grid: [0.40, 0.43, 0.50],
    amb_sky: [0.40, 0.42, 0.50],
    amb_ground: [0.15, 0.14, 0.13],
    shadow: 0.45,
};

pub const LIGHT: Theme = Theme {
    sky_top: [0.45, 0.62, 0.86],
    horizon: [0.88, 0.91, 0.95],
    checker: [[0.80, 0.81, 0.83], [0.86, 0.87, 0.89]],
    grid: [0.62, 0.64, 0.68],
    amb_sky: [0.55, 0.58, 0.64],
    amb_ground: [0.30, 0.28, 0.26],
    shadow: 0.28,
};

/// CPU-side geometry: positions, optional normals (empty for lines), optional u16 indices.
#[derive(Default)]
pub struct Geo {
    pub pos: Vec<f32>,
    pub nrm: Vec<f32>,
    /// Optional per-vertex RGB (empty = white).
    pub col: Vec<f32>,
    pub idx: Vec<u16>,
    pub lines: bool,
}

impl Geo {
    fn vert(&mut self, p: V, n: V) -> u16 {
        self.pos.extend(p);
        self.nrm.extend(n);
        (self.pos.len() / 3 - 1) as u16
    }
    fn quad(&mut self, a: V, b: V, c: V, d: V) {
        let n = math::norm(math::cross(math::sub(b, a), math::sub(d, a)));
        let i = [a, b, c, d].map(|p| self.vert(p, n));
        self.idx.extend([i[0], i[1], i[2], i[0], i[2], i[3]]);
    }
    fn tri(&mut self, a: V, b: V, c: V) {
        let n = math::norm(math::cross(math::sub(b, a), math::sub(c, a)));
        let i = [a, b, c].map(|p| self.vert(p, n));
        self.idx.extend(i);
    }
}

/// Unit cube centred at the origin (−0.5…0.5).
pub fn cube() -> Geo {
    let mut g = Geo::default();
    let p = |i: usize| [(i & 1) as f32 - 0.5, (i >> 1 & 1) as f32 - 0.5, (i >> 2 & 1) as f32 - 0.5];
    for [a, b, c, d] in [[0, 2, 3, 1], [4, 5, 7, 6], [0, 1, 5, 4], [2, 6, 7, 3], [0, 4, 6, 2], [1, 3, 7, 5]] {
        g.quad(p(a), p(b), p(c), p(d));
    }
    g
}

/// Cylinder along y, radius 0.5, height 1 (y −0.5…0.5), with caps.
pub fn cylinder(n: usize) -> Geo {
    let mut g = Geo::default();
    let at = |i: usize, y: f32| {
        let (s, c) = (i as f32 / n as f32 * std::f32::consts::TAU).sin_cos();
        [0.5 * c, y, 0.5 * s]
    };
    for i in 0..n {
        let (a, b) = (at(i, -0.5), at(i + 1, -0.5));
        let (na, nb) = ([a[0], 0.0, a[2]], [b[0], 0.0, b[2]]);
        let v = [g.vert(a, na), g.vert(b, nb), g.vert(at(i + 1, 0.5), nb), g.vert(at(i, 0.5), na)];
        g.idx.extend([v[0], v[2], v[1], v[0], v[3], v[2]]);
        g.tri([0.0, 0.5, 0.0], at(i + 1, 0.5), at(i, 0.5));
        g.tri([0.0, -0.5, 0.0], at(i, -0.5), at(i + 1, -0.5));
    }
    g
}

/// Capsule along y: radius 1, straight part from −`half` to `half` (a sphere when `half` = 0).
pub fn capsule(n: usize, half: f32) -> Geo {
    let mut g = Geo::default();
    let rows = n / 2 * 2;
    // Rows rows/2 and rows/2 + 1 are both on the equator, shifted up / down by `half`.
    for r in 0..=rows + 1 {
        let (lat, dy) = if r <= rows / 2 { (r, half) } else { (r - 1, -half) };
        let th = lat as f32 / rows as f32 * std::f32::consts::PI;
        for s in 0..=n {
            let ph = s as f32 / n as f32 * std::f32::consts::TAU;
            let nv = [th.sin() * ph.cos(), th.cos(), th.sin() * ph.sin()];
            g.vert([nv[0], nv[1] + dy, nv[2]], nv);
        }
    }
    let w = n as u16 + 1;
    for r in 0..=rows as u16 {
        for s in 0..n as u16 {
            let (a, b) = (r * w + s, (r + 1) * w + s);
            g.idx.extend([a, a + 1, b, a + 1, b + 1, b]);
        }
    }
    g
}

pub fn sphere(n: usize) -> Geo {
    capsule(n, 0.0)
}

/// Square on y = 0 (x, z in −0.5…0.5), normal up.
pub fn plane() -> Geo {
    fan(&[[-0.5, 0.0, -0.5], [-0.5, 0.0, 0.5], [0.5, 0.0, 0.5], [0.5, 0.0, -0.5]])
}

/// Convex polygon (triangle fan) in any plane; normal from the first triangle.
pub fn fan(pts: &[V]) -> Geo {
    let mut g = Geo::default();
    for i in 1..pts.len().saturating_sub(1) {
        g.tri(pts[0], pts[i], pts[i + 1]);
    }
    g
}

/// Flat ring (annulus) on y = 0 between radii `r0` and `r1`.
pub fn ring(r0: f32, r1: f32, n: usize) -> Geo {
    let mut g = Geo::default();
    for i in 0..=n {
        let (s, c) = (i as f32 / n as f32 * std::f32::consts::TAU).sin_cos();
        g.vert([r0 * c, 0.0, r0 * s], [0.0, 1.0, 0.0]);
        g.vert([r1 * c, 0.0, r1 * s], [0.0, 1.0, 0.0]);
    }
    for i in 0..n as u16 {
        let a = i * 2;
        g.idx.extend([a, a + 1, a + 2, a + 1, a + 3, a + 2]);
    }
    g
}

/// Pyramid from `apex` to the rectangle `corners` (in order around the rim), with far cap.
pub fn frustum(apex: V, corners: &[V; 4]) -> Geo {
    let mut g = Geo::default();
    for i in 0..4 {
        g.tri(apex, corners[i], corners[(i + 1) % 4]);
    }
    g.quad(corners[0], corners[3], corners[2], corners[1]);
    g
}

/// Batch of small octahedra (one per point, half-size `r`, colour per point) as a single
/// non-indexed mesh, so clouds beyond the u16 index range draw in one call. Points stay in order:
/// `draw_first(.., 24 * k)` shows only the first k.
pub fn points(pts: &[V], cols: &[V], r: f32) -> Geo {
    let mut g = Geo::default();
    let ax = [[r, 0., 0.], [0., r, 0.], [0., 0., r]];
    for (p, c) in pts.iter().zip(cols) {
        for oct in 0..8 {
            let s = |k: usize| if oct >> k & 1 == 0 { 1.0 } else { -1.0 };
            let v = [0, 1, 2].map(|k| math::add(*p, math::scale(ax[k], s(k))));
            // Keep counter-clockwise winding as seen from outside.
            let (b, c2) = if s(0) * s(1) * s(2) > 0.0 { (v[1], v[2]) } else { (v[2], v[1]) };
            let n = math::norm([s(0), s(1), s(2)]);
            for q in [v[0], b, c2] {
                g.pos.extend(q);
                g.nrm.extend(n);
                g.col.extend(c);
            }
        }
    }
    g
}

/// Solid between two matched closed loops (`inner[i]` pairs with `outer[i]`, xz coordinates)
/// extruded along y from 0 to `h`: both caps plus inner and outer walls, flat-shaded.
/// Non-indexed, so loops of any length fit (e.g. gear outlines with many teeth).
pub fn prism(inner: &[[f32; 2]], outer: &[[f32; 2]], h: f32) -> Geo {
    let mut g = Geo::default();
    let mut tri = |a: V, b: V, c: V| {
        let n = math::norm(math::cross(math::sub(b, a), math::sub(c, a)));
        for p in [a, b, c] {
            g.pos.extend(p);
            g.nrm.extend(n);
        }
    };
    let n = inner.len().min(outer.len());
    for i in 0..n {
        let j = (i + 1) % n;
        let at = |p: [f32; 2], y: f32| [p[0], y, p[1]];
        for y in [0.0, h] {
            tri(at(inner[i], y), at(outer[i], y), at(outer[j], y));
            tri(at(inner[i], y), at(outer[j], y), at(inner[j], y));
        }
        for l in [inner, outer] {
            let (a, b, c, d) = (at(l[i], 0.0), at(l[j], 0.0), at(l[j], h), at(l[i], h));
            tri(a, b, c);
            tri(a, c, d);
        }
    }
    g
}

/// Line segments from point pairs.
pub fn lines(pts: &[V]) -> Geo {
    Geo { pos: pts.iter().flatten().copied().collect(), lines: true, ..Geo::default() }
}

/// Closed polyline through `pts`.
pub fn line_loop(pts: &[V]) -> Geo {
    let segs: Vec<V> = (0..pts.len()).flat_map(|i| [pts[i], pts[(i + 1) % pts.len()]]).collect();
    lines(&segs)
}

pub struct Mesh {
    vao: WebGlVertexArrayObject,
    bufs: Vec<WebGlBuffer>,
    count: i32,
    indexed: bool,
    mode: u32,
}

/// How a mesh is shaded.
#[derive(Clone, Copy)]
pub struct Paint {
    pub color: [f32; 4],
    /// Lighting on (false = flat colour, used for lines, overlays and shadows).
    pub lit: bool,
    /// Blend toward white (hover highlight).
    pub glow: f32,
    /// Discard fragments below this world height (e.g. 0 to cut volumes at the ground).
    pub clip: f32,
}

impl Paint {
    pub fn lit(color: [f32; 4]) -> Self {
        Paint { color, lit: true, glow: 0.0, clip: -1e9 }
    }
    pub fn flat(color: [f32; 4]) -> Self {
        Paint { lit: false, ..Paint::lit(color) }
    }
}

pub struct Renderer {
    pub gl: GL,
    lit: WebGlProgram,
    ground: WebGlProgram,
    sky: WebGlProgram,
    empty: WebGlVertexArrayObject,
    plane: Mesh,
    pub theme: Theme,
    /// Unit direction toward the key light.
    pub light: V,
    /// Fog start / end distance for the ground.
    pub fog: [f32; 2],
    /// Ground checker cell size (m).
    pub cell: f32,
}

fn bytes<T: Copy, const N: usize>(v: &[T], f: impl Fn(T) -> [u8; N]) -> Vec<u8> {
    v.iter().flat_map(|x| f(*x)).collect()
}

impl Renderer {
    /// WebGL2 context with antialias + stencil, or `None` if unavailable.
    pub fn new(canvas: &HtmlCanvasElement) -> Option<Self> {
        let attrs = web_sys::WebGlContextAttributes::new();
        attrs.set_antialias(true);
        attrs.set_stencil(true);
        let gl: GL = canvas.get_context_with_context_options("webgl2", &attrs).ok()??.dyn_into().ok()?;
        let lit = program(&gl, LIT_VS, LIT_FS)?;
        let ground = program(&gl, LIT_VS, GROUND_FS)?;
        let sky = program(&gl, SKY_VS, SKY_FS)?;
        let empty = gl.create_vertex_array()?;
        let r = Renderer {
            plane: upload(&gl, &plane())?,
            gl,
            lit,
            ground,
            sky,
            empty,
            theme: DARK,
            light: math::norm([0.45, 0.9, -0.35]),
            fog: [20.0, 80.0],
            cell: 0.5,
        };
        r.gl.enable(GL::DEPTH_TEST);
        r.gl.enable(GL::BLEND);
        r.gl.blend_func(GL::SRC_ALPHA, GL::ONE_MINUS_SRC_ALPHA);
        Some(r)
    }

    pub fn mesh(&self, g: &Geo) -> Option<Mesh> {
        upload(&self.gl, g)
    }

    pub fn free(&self, m: Mesh) {
        for b in &m.bufs {
            self.gl.delete_buffer(Some(b));
        }
        self.gl.delete_vertex_array(Some(&m.vao));
    }

    /// Clear, then draw sky and ground. `viewport` (x, y, w, h in device px) letterboxes the scene;
    /// the area outside stays black.
    pub fn begin(&self, view: &M4, proj: &M4, eye: V, viewport: [i32; 4]) {
        let gl = &self.gl;
        let t = self.theme;
        gl.disable(GL::SCISSOR_TEST);
        gl.clear_color(0.0, 0.0, 0.0, 1.0);
        gl.clear_stencil(0);
        gl.clear(GL::COLOR_BUFFER_BIT | GL::DEPTH_BUFFER_BIT | GL::STENCIL_BUFFER_BIT);
        let [x, y, w, h] = viewport;
        gl.viewport(x, y, w, h);
        gl.scissor(x, y, w, h);
        gl.enable(GL::SCISSOR_TEST);
        let vp = math::mul(proj, view);
        let inv = math::invert(&vp).unwrap_or(math::IDENTITY);

        gl.use_program(Some(&self.sky));
        gl.depth_mask(false);
        self.u_m4(&self.sky, "u_inv", &inv);
        self.u_v3(&self.sky, "u_eye", eye);
        self.u_v3(&self.sky, "u_top", t.sky_top);
        self.u_v3(&self.sky, "u_hor", t.horizon);
        gl.bind_vertex_array(Some(&self.empty));
        gl.draw_arrays(GL::TRIANGLES, 0, 3);

        let p = &self.ground;
        gl.use_program(Some(p));
        gl.depth_mask(true);
        self.u_m4(p, "u_vp", &vp);
        self.u_m4(p, "u_model", &math::basis([1., 0., 0.], [0., 1., 0.], [0., 0., 1.], [1000.0; 3], [eye[0], 0.0, eye[2]]));
        self.u_m4(p, "u_post", &math::IDENTITY);
        self.u_v3(p, "u_eye", eye);
        self.u_v3(p, "u_c1", t.checker[0]);
        self.u_v3(p, "u_c2", t.checker[1]);
        self.u_v3(p, "u_line", t.grid);
        self.u_v3(p, "u_fog", t.horizon);
        gl.uniform2f(gl.get_uniform_location(p, "u_fogr").as_ref(), self.fog[0], self.fog[1]);
        gl.uniform1f(gl.get_uniform_location(p, "u_cell").as_ref(), self.cell);
        // Push the ground back so overlays and shadows lying on y = 0 win the depth test.
        gl.enable(GL::POLYGON_OFFSET_FILL);
        gl.polygon_offset(2.0, 2.0);
        self.raw(&self.plane, self.plane.count);
        gl.disable(GL::POLYGON_OFFSET_FILL);

        gl.use_program(Some(&self.lit));
        self.u_m4(&self.lit, "u_vp", &vp);
        self.u_v3(&self.lit, "u_eye", eye);
        self.u_v3(&self.lit, "u_light", self.light);
        self.u_v3(&self.lit, "u_sky", t.amb_sky);
        self.u_v3(&self.lit, "u_gnd", t.amb_ground);
    }

    /// Draw a mesh. Opaque (alpha 1) writes depth; translucent does not, so draw it last.
    pub fn draw(&self, m: &Mesh, model: &M4, p: Paint) {
        self.draw_post(m, model, &math::IDENTITY, p);
    }

    /// Planar shadow of a mesh on y = 0. Stencil keeps overlapping shadows from darkening twice.
    pub fn shadow(&self, m: &Mesh, model: &M4) {
        let gl = &self.gl;
        gl.enable(GL::STENCIL_TEST);
        gl.stencil_func(GL::EQUAL, 0, 0xff);
        gl.stencil_op(GL::KEEP, GL::KEEP, GL::INCR);
        let post = math::mul(&math::translate([0.0, 0.001, 0.0]), &math::shadow_matrix(math::scale(self.light, -1.0)));
        self.draw_post(m, model, &post, Paint::flat([0.0, 0.0, 0.0, self.theme.shadow]));
        gl.disable(GL::STENCIL_TEST);
    }

    /// Like `draw`, but only the first `n` vertices (indices if indexed), e.g. a trail growing over time.
    pub fn draw_first(&self, m: &Mesh, model: &M4, p: Paint, n: i32) {
        self.draw_n(m, model, &math::IDENTITY, p, n.clamp(0, m.count));
    }

    fn draw_post(&self, m: &Mesh, model: &M4, post: &M4, p: Paint) {
        self.draw_n(m, model, post, p, m.count);
    }

    fn draw_n(&self, m: &Mesh, model: &M4, post: &M4, p: Paint, n: i32) {
        let (gl, prog) = (&self.gl, &self.lit);
        self.u_m4(prog, "u_model", model);
        self.u_m4(prog, "u_post", post);
        gl.uniform4fv_with_f32_array(gl.get_uniform_location(prog, "u_color").as_ref(), &p.color);
        gl.uniform1f(gl.get_uniform_location(prog, "u_lit").as_ref(), if p.lit { 1.0 } else { 0.0 });
        gl.uniform1f(gl.get_uniform_location(prog, "u_glow").as_ref(), p.glow);
        gl.uniform1f(gl.get_uniform_location(prog, "u_clip").as_ref(), p.clip);
        gl.depth_mask(p.color[3] >= 1.0);
        self.raw(m, n);
        gl.depth_mask(true);
    }

    fn raw(&self, m: &Mesh, n: i32) {
        self.gl.bind_vertex_array(Some(&m.vao));
        if m.indexed {
            self.gl.draw_elements_with_i32(m.mode, n, GL::UNSIGNED_SHORT, 0);
        } else {
            self.gl.draw_arrays(m.mode, 0, n);
        }
    }

    // ponytail: uniform locations looked up per draw; cache them if a scene grows past ~100 draws.
    fn u_m4(&self, p: &WebGlProgram, name: &str, m: &M4) {
        self.gl.uniform_matrix4fv_with_f32_array(self.gl.get_uniform_location(p, name).as_ref(), false, m);
    }
    fn u_v3(&self, p: &WebGlProgram, name: &str, v: V) {
        self.gl.uniform3fv_with_f32_array(self.gl.get_uniform_location(p, name).as_ref(), &v);
    }
}

fn upload(gl: &GL, g: &Geo) -> Option<Mesh> {
    let vao = gl.create_vertex_array()?;
    gl.bind_vertex_array(Some(&vao));
    let mut bufs = vec![];
    for (loc, data) in [(0, &g.pos), (1, &g.nrm), (2, &g.col)] {
        if data.is_empty() {
            gl.disable_vertex_attrib_array(loc);
            let d = if loc == 2 { [1.0; 3] } else { [0.0, 1.0, 0.0] };
            gl.vertex_attrib3f(loc, d[0], d[1], d[2]);
            continue;
        }
        let b = gl.create_buffer()?;
        gl.bind_buffer(GL::ARRAY_BUFFER, Some(&b));
        gl.buffer_data_with_u8_array(GL::ARRAY_BUFFER, &bytes(data, f32::to_le_bytes), GL::STATIC_DRAW);
        gl.enable_vertex_attrib_array(loc);
        gl.vertex_attrib_pointer_with_i32(loc, 3, GL::FLOAT, false, 0, 0);
        bufs.push(b);
    }
    let indexed = !g.idx.is_empty();
    if indexed {
        let b = gl.create_buffer()?;
        gl.bind_buffer(GL::ELEMENT_ARRAY_BUFFER, Some(&b));
        gl.buffer_data_with_u8_array(GL::ELEMENT_ARRAY_BUFFER, &bytes(&g.idx, u16::to_le_bytes), GL::STATIC_DRAW);
        bufs.push(b);
    }
    gl.bind_vertex_array(None);
    let count = if indexed { g.idx.len() } else { g.pos.len() / 3 } as i32;
    Some(Mesh { vao, bufs, count, indexed, mode: if g.lines { GL::LINES } else { GL::TRIANGLES } })
}

fn program(gl: &GL, vs: &str, fs: &str) -> Option<WebGlProgram> {
    let p = gl.create_program()?;
    for (kind, src) in [(GL::VERTEX_SHADER, vs), (GL::FRAGMENT_SHADER, fs)] {
        let s = gl.create_shader(kind)?;
        gl.shader_source(&s, src);
        gl.compile_shader(&s);
        if !gl.get_shader_parameter(&s, GL::COMPILE_STATUS).as_bool().unwrap_or(false) {
            web_sys::console::error_1(&gl.get_shader_info_log(&s).unwrap_or_default().into());
            return None;
        }
        gl.attach_shader(&p, &s);
    }
    gl.link_program(&p);
    gl.get_program_parameter(&p, GL::LINK_STATUS).as_bool().unwrap_or(false).then_some(p)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn points_batch_is_flat_and_closed() {
        let g = points(&[[0.0; 3], [1.0, 2.0, 3.0]], &[[1.0, 0.0, 0.0], [0.0, 0.5, 1.0]], 0.1);
        assert!(g.idx.is_empty() && !g.lines);
        assert_eq!(g.pos.len(), 2 * 24 * 3);
        assert_eq!(g.nrm.len(), g.pos.len());
        assert_eq!(g.col.len(), g.pos.len());
        assert_eq!(&g.col[24 * 3..24 * 3 + 3], &[0.0, 0.5, 1.0]);
        // Every face points away from its octahedron's centre, with winding matching the normal.
        for t in 0..16 {
            let v = |k: usize| -> V { std::array::from_fn(|i| g.pos[(t * 3 + k) * 3 + i]) };
            let c = if t < 8 { [0.0; 3] } else { [1.0, 2.0, 3.0] };
            let n: V = std::array::from_fn(|i| g.nrm[t * 9 + i]);
            let wind = math::cross(math::sub(v(1), v(0)), math::sub(v(2), v(0)));
            assert!(math::dot(n, math::sub(v(0), c)) > 0.0);
            assert!(math::dot(wind, n) > 0.0, "triangle {t}");
        }
    }

    #[test]
    fn prism_caps_and_walls() {
        let sq = |r: f32| [[r, 0.0], [0.0, r], [-r, 0.0], [0.0, -r]];
        let g = prism(&sq(1.0), &sq(2.0), 0.5);
        assert!(g.idx.is_empty() && !g.lines);
        // 4 segments × (2 caps + 2 walls) × 2 triangles × 3 vertices.
        assert_eq!(g.pos.len(), 4 * 8 * 3 * 3);
        assert_eq!(g.nrm.len(), g.pos.len());
        assert!(g.pos.chunks(3).all(|p| p[1] == 0.0 || p[1] == 0.5));
        for n in g.nrm.chunks(3) {
            assert!((n[0] * n[0] + n[1] * n[1] + n[2] * n[2] - 1.0).abs() < 1e-5);
        }
    }
}
