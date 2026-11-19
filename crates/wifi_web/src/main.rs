//! Browser UI: plain web-sys, a 2D canvas editor and an optional WebGL2 3D view.
//! Everything runs locally; no network requests.

#[allow(dead_code)]
mod gl;
#[allow(dead_code)]
mod math;
mod scene;
mod view;

use scene::{CamMode, Frame, Scene};
use std::{cell::Cell, cell::RefCell, rc::Rc};
use view::{Crossing, LEVELS, SINR_LEVELS};
use wasm_bindgen::{prelude::*, Clamped, JsCast};
use web_sys::{
    Blob, BlobPropertyBag, CanvasRenderingContext2d, DeviceMotionEvent, DeviceOrientationEvent, Document, Element,
    Event, EventTarget, FileReader, HtmlAnchorElement, HtmlCanvasElement, HtmlElement, HtmlImageElement,
    HtmlInputElement, HtmlSelectElement, ImageData, KeyboardEvent, PointerEvent, Url, WheelEvent,
};
use wifi_core::pdr::{calibrate_step, Mark, StepDetector, Walk};
use wifi_core::{
    calibrate, coverage, score, suggest_placement, Ap, Band, Coverage, Material, Measurement, Plan, Wall,
};

const STORE_KEY: &str = "wifi-sim-plan";
const HINT_KEY: &str = "wifi-sim-hint-seen";
const APARTMENT: &str = include_str!("../../../examples/apartment.json");
const OFFICE: &str = include_str!("../../../examples/office.json");
/// Legend bar range (dBm, left → right).
const LEGEND: (f32, f32) = (-85.0, -45.0);

#[rustfmt::skip]
const STRINGS: &[(&str, &str, &str)] = &[
    ("title", "Wi-Fi coverage simulator", "Wi-Fi 커버리지 시뮬레이터"),
    ("estimate", "Estimate only: real signal depends on furniture, people, devices and construction details.", "추정치일 뿐입니다. 실제 신호는 가구, 사람, 기기, 건물 구조에 따라 달라집니다."),
    ("lang", "한국어", "English"),
    ("tool_wall", "Wall", "벽"),
    ("tool_ap", "AP", "AP"),
    ("tool_measure", "Measure", "실측"),
    ("tool_erase", "Eraser", "지우개"),
    ("drywall", "Drywall / partition", "석고보드 / 칸막이"),
    ("wood_door", "Wooden door", "나무 문"),
    ("glass", "Glass / window", "유리 / 창문"),
    ("brick", "Brick", "벽돌"),
    ("concrete", "Concrete / load-bearing", "콘크리트 / 내력벽"),
    ("metal", "Metal / elevator", "금속 / 엘리베이터"),
    ("mirror", "Mirror", "거울"),
    ("undo", "Undo", "실행 취소"),
    ("redo", "Redo", "다시 실행"),
    ("clear", "Clear", "모두 지우기"),
    ("section_ap", "Access points", "AP (공유기 / 메시)"),
    ("hint_ap", "AP tool: tap an empty spot to add, drag to move (also in 3D). Settings below apply to the selected AP (or the next one you add).", "AP 도구: 빈 곳을 탭하면 추가, 드래그하면 이동합니다 (3D에서도 가능). 아래 설정은 선택한 AP(또는 다음에 추가할 AP)에 적용됩니다."),
    ("band", "Band", "대역"),
    ("tx", "TX power (dBm)", "송신 출력 (dBm)"),
    ("gain", "Antenna gain (dBi)", "안테나 이득 (dBi)"),
    ("channel", "Channel", "채널"),
    ("view", "View", "보기"),
    ("view_rssi", "Signal (dBm)", "신호 세기 (dBm)"),
    ("view_sinr", "SINR (same-channel APs)", "SINR (동일 채널 AP)"),
    ("suggest", "Suggest AP placement", "AP 위치 추천"),
    ("section_plan", "Floor plan", "평면도"),
    ("scale", "Metres per cell", "셀 한 칸 (m)"),
    ("gw", "Width (cells)", "가로 (칸)"),
    ("gh", "Height (cells)", "세로 (칸)"),
    ("example", "Example", "예제"),
    ("ex_pick", "Load…", "불러오기…"),
    ("ex_blank", "Blank", "빈 평면"),
    ("ex_apartment", "Sample apartment", "샘플 아파트"),
    ("ex_office", "Sample office", "샘플 사무실"),
    ("section_calib", "Calibration", "보정"),
    ("calib_hint", "Measure tool: tap a spot and enter the dBm you measured there, then press Fit.", "실측 도구: 위치를 탭하고 그곳에서 측정한 dBm 값을 입력한 뒤 '맞추기'를 누르세요."),
    ("exponent", "Path-loss exponent n", "경로손실 지수 n"),
    ("offset", "Offset (dB)", "오프셋 (dB)"),
    ("calibrate", "Fit", "맞추기"),
    ("meas_clear", "Clear points", "측정점 지우기"),
    ("section_mat", "Wall attenuation (dB per wall)", "벽 감쇠 (벽 1개당 dB)"),
    ("section_file", "File & share", "파일 · 공유"),
    ("export", "Export JSON", "JSON 내보내기"),
    ("import", "Import JSON", "JSON 가져오기"),
    ("share", "Share link", "공유 링크"),
    ("share_note", "The plan is packed into the #fragment of the link. Browsers never send the fragment to a server.", "평면도는 링크의 # 뒷부분에 담깁니다. 브라우저는 이 부분을 서버로 보내지 않습니다."),
    ("section_bg", "Background image", "배경 이미지"),
    ("bg_pick", "Choose image", "이미지 선택"),
    ("bg_clear", "Remove", "제거"),
    ("opacity", "Opacity", "불투명도"),
    ("bg_note", "The image stays on this device and is not saved in the plan or link.", "이미지는 이 기기에만 머물며 평면도 파일이나 링크에 저장되지 않습니다."),
    ("status", "{w}×{h} cells = {mw}×{mh} m · computed in {ms} ms", "{w}×{h}칸 = {mw}×{mh} m · 계산 {ms} ms"),
    ("readout", "{x}, {y} m · {dbm} dBm (AP {ap}) · SINR {sinr} dB", "{x}, {y} m · {dbm} dBm (AP {ap}) · SINR {sinr} dB"),
    ("prompt_dbm", "Measured signal here (dBm, e.g. -62):", "이 위치에서 측정한 신호 (dBm, 예: -62):"),
    ("need_points", "Add measurement points first (Measure tool).", "먼저 실측 도구로 측정점을 추가하세요."),
    ("calib_res", "Fitted n = {n}, offset {o} dB. RMSE {b} → {a} dB over {k} point(s).", "보정 결과 n = {n}, 오프셋 {o} dB. RMSE {b} → {a} dB (측정점 {k}개)."),
    ("need_ap", "Add an AP first (AP tool).", "먼저 AP 도구로 AP를 추가하세요."),
    ("suggest_res", "Moved AP {i} to the best grid spot: median {m} dBm, {p}% of area below -70 dBm.", "AP {i}을(를) 최적 위치로 옮겼습니다: 중앙값 {m} dBm, -70 dBm 미만 면적 {p}%."),
    ("bad_plan", "Could not load plan: ", "평면도를 불러올 수 없습니다: "),
    ("share_done", "Link updated in the address bar (and below). Copy it to share.", "주소창(과 아래 칸)의 링크가 갱신되었습니다. 복사해서 공유하세요."),
    ("section_walk", "Walk mode: draw the plan by walking", "걸어서 도면 그리기"),
    ("walk_hint", "On a phone: press Start, stand in a corner, tap Corner, then walk slowly along the wall holding the phone flat and pointing forward. Tap Corner at every corner and Door in doorways; back at the start tap Close loop. Next room: start it by tapping Door in the shared doorway. Straighten snaps walls to right angles. Expect ±5–10 %.", "휴대폰에서: '시작'을 누르고 방 모서리에 서서 '모서리'를 탭한 뒤, 휴대폰을 앞을 향해 평평하게 들고 벽을 따라 천천히 걸으세요. 모서리마다 '모서리', 문 위치에서 '문'을 탭하고, 출발점으로 돌아오면 '루프 닫기'를 누르세요. 다음 방은 이어지는 문 위치에서 '문'을 탭해 시작합니다. '직각 맞추기'는 벽을 직각으로 맞춥니다. 오차는 ±5–10% 정도입니다."),
    ("walk_privacy", "Motion and orientation data are processed in memory on this device and never leave it; only the resulting walls are kept. No GPS / location is used. Browsers cannot read Wi-Fi signal strength: to add dBm, read it from a Wi-Fi analyzer app and type it in.", "움직임·방향 센서 데이터는 이 기기의 메모리에서만 처리되고 밖으로 나가지 않으며, 결과 벽만 남습니다. GPS/위치 정보는 쓰지 않습니다. 브라우저는 Wi-Fi 신호 세기를 읽을 수 없으니, dBm은 Wi-Fi 분석 앱에서 확인해 직접 입력하세요."),
    ("step_len", "Step length (m)", "보폭 (m)"),
    ("walk_start", "Start", "시작"),
    ("walk_calib", "Calibrate step", "보폭 보정"),
    ("walk_corner", "Corner", "모서리"),
    ("walk_door", "Door", "문"),
    ("walk_close", "Close loop", "루프 닫기"),
    ("walk_snap", "Straighten", "직각 맞추기"),
    ("walk_dbm", "dBm here", "여기 dBm"),
    ("walk_undo", "Undo mark", "표시 취소"),
    ("walk_apply", "Use as floor plan", "평면도로 사용"),
    ("walk_stop", "Discard walk", "걷기 버리기"),
    ("walk_need_start", "Press Start first.", "먼저 '시작'을 누르세요."),
    ("walk_no_sensor", "No motion sensors here: use a phone, over HTTPS.", "모션 센서가 없습니다. HTTPS로 접속한 휴대폰에서 사용하세요."),
    ("walk_denied", "Motion sensor permission was denied.", "모션 센서 권한이 거부되었습니다."),
    ("walk_started", "Walking. Stand in a corner and tap Corner to begin a room.", "걷기 시작. 방 모서리에 서서 '모서리'를 탭하세요."),
    ("walk_calib_on", "Walk along a wall of known length, then tap Calibrate step again.", "길이를 아는 벽을 따라 걸은 뒤 '보폭 보정'을 다시 누르세요."),
    ("walk_calib_prompt", "Length you just walked (m):", "방금 걸은 길이 (m):"),
    ("walk_calib_res", "Step length set to {m} m ({n} steps).", "보폭을 {m} m로 설정했습니다 (걸음 {n}개)."),
    ("walk_closed", "Loop closed: {g} m of drift spread over the walls.", "루프를 닫았습니다: 누적 오차 {g} m를 벽들에 나눠 보정했습니다."),
    ("walk_applied", "Walls created. Edit them with the normal tools (drywall inside, concrete for the outer loop).", "벽을 만들었습니다. 일반 도구로 고치세요 (내부는 석고보드, 바깥 루프는 콘크리트)."),
    ("walk_status", "{s} steps · heading {h}° · {r} room(s)", "걸음 {s} · 방향 {h}° · 방 {r}개"),
    ("walk_drift", " · heading drift {d}° on the last wall: tap Straighten or walk more slowly", " · 마지막 벽 방향 오차 {d}°: '직각 맞추기'를 누르거나 더 천천히 걸으세요"),
    ("lg_rssi", "≥ -50 excellent|-60 good|-67 fair|-70 weak|-80 poor|≤ -80 dead zone", "≥ -50 매우 좋음|-60 좋음|-67 보통|-70 약함|-80 나쁨|≤ -80 음영 지역"),
    ("lg_sinr", "≥ 25 dB excellent|17 good|11 fair|8 weak|0 poor|≤ 0 unusable", "≥ 25 dB 매우 좋음|17 좋음|11 보통|8 약함|0 나쁨|≤ 0 사용 불가"),
    ("tip_wall", "Wall: drag to draw (pick a material in Materials)", "벽: 드래그해서 그리기 (재질은 '재질'에서 선택)"),
    ("tip_ap", "AP: tap to add, drag to move (2D and 3D)", "AP: 탭하면 추가, 드래그하면 이동 (2D·3D)"),
    ("tip_measure", "Measure: tap and enter a measured dBm", "실측: 탭하고 측정한 dBm 입력"),
    ("tip_erase", "Eraser: tap a wall, AP or point", "지우개: 벽·AP·측정점을 탭"),
    ("tip_undo", "Undo (Ctrl+Z)", "실행 취소 (Ctrl+Z)"),
    ("tip_redo", "Redo (Ctrl+Shift+Z)", "다시 실행 (Ctrl+Shift+Z)"),
    ("tip_clear", "Clear everything", "모두 지우기"),
    ("tip_panel", "Show / hide settings", "설정 보이기 / 숨기기"),
    ("settings", "Settings", "설정"),
    ("card_mat", "Materials", "재질"),
    ("card_view", "View", "보기"),
    ("cam_persp", "Perspective", "원근"),
    ("cam_top", "Top", "위에서"),
    ("cam_walk", "Walk-through", "걸어 보기"),
    ("rays", "Signal rays", "신호 광선"),
    ("contours", "Contour lines", "등고선"),
    ("no_webgl", "3D needs WebGL2, which is not available in this browser. Staying in 2D.", "3D 보기에는 WebGL2가 필요한데 이 브라우저에서는 쓸 수 없습니다. 2D로 계속합니다."),
    ("walk3d_hint", "WASD / arrow keys to move (Shift = faster), drag to look. Touch: one finger looks, two fingers move.", "WASD / 화살표로 이동 (Shift = 빠르게), 드래그로 둘러보기. 터치: 한 손가락은 보기, 두 손가락은 이동."),
    ("hint_title", "Welcome!", "환영합니다!"),
    ("hint_body", "This is a sample apartment. Drag the glowing AP to watch the coverage update live, draw walls with the Wall tool, or switch to 3D at the top. Settings are on the right.", "샘플 아파트입니다. 빛나는 AP를 드래그하면 커버리지가 실시간으로 바뀌고, 벽 도구로 벽을 그릴 수 있으며, 위쪽에서 3D로 전환할 수 있습니다. 설정은 오른쪽에 있습니다."),
    ("hint_ok", "Got it", "알겠어요"),
    ("rays_hud", "Ray from AP {i}: {n} wall(s), {db} dB", "AP {i}에서의 광선: 벽 {n}개, {db} dB"),
    ("legend_rssi", "Signal (dBm)", "신호 세기 (dBm)"),
    ("legend_sinr", "SINR (dB)", "SINR (dB)"),
    ("legend_ends", "dead zone → excellent", "음영 → 매우 좋음"),
    ("fps", " · {f} fps", " · {f} fps"),
];


fn mat_key(m: Material) -> &'static str {
    ["drywall", "wood_door", "glass", "brick", "concrete", "metal", "mirror"][m.idx()]
}

#[derive(Clone, Copy, PartialEq)]
enum Tool {
    Wall,
    Ap,
    Measure,
    Erase,
}

enum Drag {
    Wall { start: (f64, f64), cur: (f64, f64) },
    Ap { idx: usize, moved: bool },
}

/// Gesture on the 3D canvas.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Drag3 {
    Orbit,
    Pan,
    Look,
    Ap { idx: usize, moved: bool },
    /// Two pointers: pinch-zoom + pan (orbit cams) or move (walk-through).
    Two,
}

struct Ptr {
    id: i32,
    x: f64,
    y: f64,
}

thread_local! {
    static MSG_UNTIL: Cell<f64> = const { Cell::new(0.0) };
}

fn now() -> f64 {
    win().performance().map(|p| p.now()).unwrap_or(0.0)
}

/// Toast message over the canvas, hidden again after a few seconds.
fn toast(s: &str) {
    let m: Element = el("msg");
    m.set_text_content(Some(s));
    let _ = m.class_list().toggle_with_force("show", !s.is_empty());
    MSG_UNTIL.with(|c| c.set(now() + 3500.0 + s.len() as f64 * 40.0));
}

struct App {
    plan: Plan,
    undo: Vec<Plan>,
    redo: Vec<Plan>,
    cov: Coverage,
    tool: Tool,
    material: Material,
    ko: bool,
    selected: Option<usize>,
    drag: Option<Drag>,
    hover: Option<(f64, f64)>,
    sinr_view: bool,
    contours_on: bool,
    rays_on: bool,
    /// Iso-line segments per legend level for the displayed grid (cell coordinates).
    contours: Vec<Vec<[(f32, f32); 2]>>,
    bg: Option<(HtmlImageElement, String)>,
    bg_alpha: f64,
    canvas: HtmlCanvasElement,
    ctx: CanvasRenderingContext2d,
    heat: HtmlCanvasElement,
    heat_ctx: CanvasRenderingContext2d,
    /// canvas pixels per cell
    scale: f64,
    ms: f64,
    walk: Option<WalkUi>,
    walk_canvas: HtmlCanvasElement,
    walk_ctx: CanvasRenderingContext2d,
    // 3D
    canvas3d: HtmlCanvasElement,
    mode3d: bool,
    scene: Option<Scene>,
    gl_failed: bool,
    cam_dims: Option<(usize, usize, u64)>,
    ptrs: Vec<Ptr>,
    drag3: Option<Drag3>,
    down_at: (f64, f64),
    hot_ap: Option<usize>,
    keys: u8,
    /// Coverage needs recomputing (coalesced to one per animation frame).
    dirty_cov: bool,
    last_t: f64,
    fps: (u32, f64, f64),
}

/// Walk-mode state. Sensor samples are consumed as they arrive and never stored.
struct WalkUi {
    walk: Walk,
    det: StepDetector,
    heading: f64,
    /// Step count when "Calibrate step" was started.
    calib: Option<usize>,
}

/// iOS 13+ only grants motion/orientation events after `requestPermission()` from a user tap.
fn request_sensor_permission(denied: &str) {
    for name in ["DeviceMotionEvent", "DeviceOrientationEvent"] {
        let get = |o: &JsValue, k: &str| js_sys::Reflect::get(o, &k.into()).ok();
        let func = |v: Option<JsValue>| v.and_then(|f| f.dyn_into::<js_sys::Function>().ok());
        let Some(ctor) = get(&win(), name) else { continue };
        let Some(req) = func(get(&ctor, "requestPermission")) else { continue };
        let Ok(promise) = req.call0(&ctor) else { continue };
        let Some(then) = func(get(&promise, "then")) else { continue };
        let (d1, d2) = (denied.to_string(), denied.to_string());
        let ok = Closure::once_into_js(move |state: JsValue| {
            if state.as_string().as_deref() != Some("granted") {
                toast(&d1);
            }
        });
        let err = Closure::once_into_js(move |_: JsValue| toast(&d2));
        let _ = then.call2(&promise, &ok, &err);
    }
}

fn win() -> web_sys::Window {
    web_sys::window().unwrap()
}
fn doc() -> Document {
    win().document().unwrap()
}
fn el<T: JsCast>(id: &str) -> T {
    doc().get_element_by_id(id).unwrap_or_else(|| panic!("#{id}")).dyn_into::<T>().unwrap()
}
fn body() -> HtmlElement {
    doc().body().unwrap()
}
fn body_class(c: &str, on: bool) {
    let _ = body().class_list().toggle_with_force(c, on);
}
fn has_body_class(c: &str) -> bool {
    body().class_list().contains(c)
}
fn ctx2d(c: &HtmlCanvasElement) -> CanvasRenderingContext2d {
    c.get_context("2d").unwrap().unwrap().dyn_into().unwrap()
}
fn num(id: &str) -> Option<f64> {
    let v = el::<HtmlInputElement>(id).value_as_number();
    v.is_finite().then_some(v)
}
fn set_num(id: &str, v: f64) {
    el::<HtmlInputElement>(id).set_value_as_number(v);
}
fn select_val(id: &str) -> String {
    el::<HtmlSelectElement>(id).value()
}
fn checked(id: &str) -> bool {
    el::<HtmlInputElement>(id).checked()
}
fn set_active(id: &str, on: bool) {
    let _ = el::<Element>(id).class_list().toggle_with_force("active", on);
}
fn storage() -> Option<web_sys::Storage> {
    win().local_storage().ok().flatten()
}
fn fmt(t: &str, args: &[(&str, String)]) -> String {
    args.iter().fold(t.to_string(), |s, (k, v)| s.replace(&format!("{{{k}}}"), v))
}
fn dist_point_seg(p: (f64, f64), w: &Wall) -> f64 {
    let (dx, dy) = (w.x2 - w.x1, w.y2 - w.y1);
    let l2 = dx * dx + dy * dy;
    let t = if l2 > 0.0 { (((p.0 - w.x1) * dx + (p.1 - w.y1) * dy) / l2).clamp(0.0, 1.0) } else { 0.0 };
    (p.0 - w.x1 - t * dx).hypot(p.1 - w.y1 - t * dy)
}
fn rgba(c: [u8; 3], a: f64) -> String {
    format!("rgba({},{},{},{a})", c[0], c[1], c[2])
}

impl App {
    fn tr<'a>(&self, key: &'a str) -> &'a str {
        STRINGS.iter().find(|s| s.0 == key).map(|s| if self.ko { s.2 } else { s.1 }).unwrap_or(key)
    }

    fn msg(&self, s: &str) {
        toast(s);
    }

    fn snapshot(&mut self) {
        self.undo.push(self.plan.clone());
        if self.undo.len() > 200 {
            self.undo.remove(0);
        }
        self.redo.clear();
    }

    fn set_plan(&mut self, p: Plan) {
        self.snapshot();
        self.plan = p;
        self.selected = None;
        self.after_plan_swap();
    }

    fn after_plan_swap(&mut self) {
        if self.selected.is_some_and(|i| i >= self.plan.aps.len()) {
            self.selected = None;
        }
        self.sync_inputs();
        self.resize();
        self.recompute();
    }

    fn undo(&mut self) {
        if let Some(p) = self.undo.pop() {
            self.redo.push(std::mem::replace(&mut self.plan, p));
            self.after_plan_swap();
        }
    }

    fn redo(&mut self) {
        if let Some(p) = self.redo.pop() {
            self.undo.push(std::mem::replace(&mut self.plan, p));
            self.after_plan_swap();
        }
    }

    fn sync_inputs(&self) {
        set_num("scale", self.plan.cell_m);
        set_num("gw", self.plan.width as f64);
        set_num("gh", self.plan.height as f64);
        set_num("exponent", (self.plan.exponent * 100.0).round() / 100.0);
        set_num("offset", (self.plan.offset_db * 10.0).round() / 10.0);
        if let Some(a) = self.selected.and_then(|i| self.plan.aps.get(i)) {
            el::<HtmlSelectElement>("band").set_value(&a.band.idx().to_string());
            set_num("tx", a.tx_dbm);
            set_num("gain", a.gain_dbi);
            set_num("channel", a.channel as f64);
        }
        self.render_materials();
    }

    fn ap_from_inputs(&self, x: f64, y: f64) -> Ap {
        let band = Band::ALL[select_val("band").parse::<usize>().unwrap_or(1).min(2)];
        Ap {
            x,
            y,
            band,
            tx_dbm: num("tx").unwrap_or(20.0).clamp(-10.0, 36.0),
            gain_dbi: num("gain").unwrap_or(0.0).clamp(-10.0, 20.0),
            channel: num("channel").unwrap_or(1.0).clamp(1.0, 233.0) as u16,
        }
    }

    fn dragging(&self) -> bool {
        matches!(self.drag, Some(Drag::Ap { .. })) || matches!(self.drag3, Some(Drag3::Ap { .. }))
    }

    fn save(&self) {
        if let Some(s) = storage() {
            let _ = s.set_item(STORE_KEY, &self.plan.to_json());
        }
    }

    fn recompute(&mut self) {
        self.dirty_cov = false;
        let t0 = now();
        self.cov = coverage(&self.plan);
        self.ms = now() - t0;
        self.paint_heat();
        if !self.dragging() {
            self.save();
        }
        self.render_status();
        self.render();
        self.render_readout();
    }

    /// Grid shown on the heatmap (dBm, or SINR mapped onto the dBm scale); `None` without APs.
    fn displayed(&self) -> Option<Vec<f32>> {
        if self.plan.aps.is_empty() {
            return None;
        }
        Some(match (&self.cov.sinr, self.sinr_view) {
            (Some(s), true) => s.iter().map(|&v| view::sinr_as_dbm(v)).collect(),
            _ => self.cov.best.clone(),
        })
    }

    /// Writes the per-cell colour image into the offscreen 1-pixel-per-cell canvas, refreshes the
    /// contour cache and the 3D floor texture.
    fn paint_heat(&mut self) {
        let (w, h) = (self.cov.width, self.cov.height);
        let mut buf = vec![0u8; w * h * 4];
        let vals = self.displayed();
        if let Some(vals) = &vals {
            for (k, v) in vals.iter().enumerate() {
                let c = view::color(*v);
                buf[k * 4..k * 4 + 4].copy_from_slice(&[c[0], c[1], c[2], 165]);
            }
        }
        self.heat.set_width(w as u32);
        self.heat.set_height(h as u32);
        let img = ImageData::new_with_u8_clamped_array_and_sh(Clamped(&buf), w as u32, h as u32).unwrap();
        self.heat_ctx.put_image_data(&img, 0.0, 0.0).unwrap();
        self.contours = match (&vals, self.contours_on) {
            (Some(v), true) => LEVELS.iter().map(|&l| view::contours(v, w, h, l)).collect(),
            _ => Vec::new(),
        };
        if let Some(sc) = &mut self.scene {
            match &vals {
                Some(v) => sc.set_field(v, w, h),
                None => sc.set_field(&vec![view::PACK_LO; w * h], w, h),
            }
        }
    }

    fn mobile(&self) -> bool {
        win().inner_width().ok().and_then(|v| v.as_f64()).unwrap_or(1024.0) <= 760.0
    }

    /// Lays the 2D canvas out (plan aspect, centred between the floating chrome) and sizes the 3D canvas.
    fn resize(&mut self) {
        let dpr = win().device_pixel_ratio().max(1.0);
        let stage: HtmlElement = el("stage");
        let (sw, sh) = (stage.client_width().max(200) as f64, stage.client_height().max(200) as f64);
        // the top row wraps on narrow screens: lay out below wherever it ends
        let top = el::<Element>("top").get_bounding_client_rect().bottom().max(40.0);
        let _ = body().style().set_property("--top-h", &format!("{top:.0}px"));
        let (l, r, t, b) = if self.mobile() {
            (12.0, 12.0, top + 64.0, 180.0)
        } else {
            let panel = if has_body_class("panel-open") { 364.0 } else { 16.0 };
            (72.0, panel, top + 12.0, 84.0)
        };
        let (x, y, w, h) =
            view::fit_rect(l, t, (sw - l - r).max(80.0), (sh - t - b).max(80.0), self.plan.width as f64, self.plan.height as f64);
        let st = self.canvas.style();
        for (k, v) in [("left", x), ("top", y), ("width", w), ("height", h)] {
            let _ = st.set_property(k, &format!("{v:.1}px"));
        }
        self.scale = (w * dpr / self.plan.width as f64).max(0.5);
        self.canvas.set_width((self.plan.width as f64 * self.scale).round() as u32);
        self.canvas.set_height((self.plan.height as f64 * self.scale).round() as u32);
        self.canvas3d.set_width((sw * dpr).round() as u32);
        self.canvas3d.set_height((sh * dpr).round() as u32);
        self.render();
    }

    /// Converts canvas pixels to cell units, independent of CSS scaling.
    fn px(&self, css_px: f64) -> f64 {
        let rect = self.canvas.get_bounding_client_rect();
        css_px * self.plan.width as f64 / rect.width().max(1.0)
    }

    fn to_cell(&self, e: &PointerEvent) -> (f64, f64) {
        let r = self.canvas.get_bounding_client_rect();
        let x = (e.client_x() as f64 - r.left()) / r.width().max(1.0) * self.plan.width as f64;
        let y = (e.client_y() as f64 - r.top()) / r.height().max(1.0) * self.plan.height as f64;
        (x.clamp(0.0, self.plan.width as f64), y.clamp(0.0, self.plan.height as f64))
    }

    /// Ray from the selected AP (or AP 1) to the hovered point: (AP index, crossings).
    fn ray(&self) -> Option<(usize, Vec<Crossing>)> {
        let p = self.hover?;
        let i = self.selected.unwrap_or(0);
        let a = self.plan.aps.get(i)?;
        Some((i, view::ray_crossings(&self.plan, a.band, (a.x, a.y), p)))
    }

    fn render(&self) {
        if self.mode3d {
            return;
        }
        let c = &self.ctx;
        let (w, h) = (self.plan.width as f64, self.plan.height as f64);
        let s = self.scale;
        let lw = |px: f64| px * win().device_pixel_ratio().max(1.0) / s;
        let tau = std::f64::consts::TAU;
        c.set_transform(1.0, 0.0, 0.0, 1.0, 0.0, 0.0).unwrap();
        c.set_fill_style_str("#0f1522");
        c.fill_rect(0.0, 0.0, self.canvas.width() as f64, self.canvas.height() as f64);
        c.set_transform(s, 0.0, 0.0, s, 0.0, 0.0).unwrap();

        if let Some((img, _)) = &self.bg {
            if img.complete() && img.natural_width() > 0 {
                c.set_global_alpha(self.bg_alpha);
                let _ = c.draw_image_with_html_image_element_and_dw_and_dh(img, 0.0, 0.0, w, h);
                c.set_global_alpha(1.0);
            }
        }
        // 1-pixel-per-cell image stretched with smoothing = bilinear between cell values
        c.set_image_smoothing_enabled(true);
        let _ = c.draw_image_with_html_canvas_element_and_dw_and_dh(&self.heat, 0.0, 0.0, w, h);

        // subtle 1 m grid, stronger every 5 m
        let step = (1.0 / self.plan.cell_m).round().max(1.0);
        if step * s >= 8.0 {
            for (every, alpha) in [(1.0, 0.045), (5.0, 0.09)] {
                c.set_stroke_style_str(&format!("rgba(255,255,255,{alpha})"));
                c.set_line_width(lw(1.0));
                c.begin_path();
                let st = step * every;
                let mut x = 0.0;
                while x <= w + 1e-9 {
                    c.move_to(x, 0.0);
                    c.line_to(x, h);
                    x += st;
                }
                let mut y = 0.0;
                while y <= h + 1e-9 {
                    c.move_to(0.0, y);
                    c.line_to(w, y);
                    y += st;
                }
                c.stroke();
            }
        }

        // contour lines at the legend levels
        if !self.contours.is_empty() {
            c.set_line_width(lw(1.0));
            for (k, segs) in self.contours.iter().enumerate() {
                let a = if LEVELS[k] == -70.0 { 0.65 } else { 0.32 };
                c.set_stroke_style_str(&format!("rgba(255,255,255,{a})"));
                c.begin_path();
                for sg in segs {
                    c.move_to(sg[0].0 as f64, sg[0].1 as f64);
                    c.line_to(sg[1].0 as f64, sg[1].1 as f64);
                }
                c.stroke();
            }
        }

        // walls: dark casing + material colour at physical thickness
        c.set_line_cap("round");
        let wall = |a: (f64, f64), b: (f64, f64), m: Material, alpha: f64| {
            let (col, t) = view::material_style(m);
            let width = (t / self.plan.cell_m).max(lw(2.5));
            c.set_stroke_style_str(&format!("rgba(4,8,14,{})", 0.7 * alpha));
            c.set_line_width(width + lw(2.0));
            c.begin_path();
            c.move_to(a.0, a.1);
            c.line_to(b.0, b.1);
            c.stroke();
            c.set_stroke_style_str(&rgba(col, if m == Material::Glass { 0.75 } else { 1.0 } * alpha));
            c.set_line_width(width);
            c.stroke();
        };
        for wl in &self.plan.walls {
            wall((wl.x1, wl.y1), (wl.x2, wl.y2), wl.material, 1.0);
        }

        let font = |px: f64, weight: u32| format!("{weight} {}px system-ui, sans-serif", lw(px));
        // rounded label pill centred at (x, y)
        let pill = |text: &str, x: f64, y: f64, fg: &str, bg: &str| {
            c.set_font(&font(11.0, 600));
            let tw = c.measure_text(text).map(|m| m.width()).unwrap_or(0.0);
            let (pw, ph) = (tw + lw(12.0), lw(18.0));
            let (x0, y0, r) = (x - pw / 2.0, y - ph / 2.0, ph / 2.0);
            c.set_fill_style_str(bg);
            c.begin_path();
            let _ = c.arc(x0 + r, y0 + r, r, tau / 4.0, tau * 0.75);
            let _ = c.arc(x0 + pw - r, y0 + r, r, -tau / 4.0, tau / 4.0);
            c.close_path();
            c.fill();
            c.set_fill_style_str(fg);
            c.set_text_align("center");
            c.set_text_baseline("middle");
            let _ = c.fill_text(text, x, y + lw(0.5));
        };

        if let Some(Drag::Wall { start, cur }) = &self.drag {
            wall(*start, *cur, self.material, 0.7);
            let len = (cur.0 - start.0).hypot(cur.1 - start.1) * self.plan.cell_m;
            let mid = ((start.0 + cur.0) / 2.0, (start.1 + cur.1) / 2.0 - lw(16.0));
            pill(&format!("{len:.2} m"), mid.0, mid.1, "#04121c", "rgba(76,199,255,0.95)");
        }

        // signal ray (selected AP → cursor) or a distance guide to the serving AP
        let hover = self.hover.filter(|_| !self.plan.aps.is_empty() && self.drag.is_none());
        if let Some(p) = hover {
            let k = ((p.1 as usize).min(self.plan.height - 1)) * self.plan.width + (p.0 as usize).min(self.plan.width - 1);
            let (ai, crossings) = match (self.rays_on, self.ray()) {
                (true, Some((i, cr))) => (i, Some(cr)),
                _ => (self.cov.best_ap[k] as usize, None),
            };
            if let Some(a) = self.plan.aps.get(ai) {
                let (dx, dy) = (p.0 - a.x, p.1 - a.y);
                match crossings {
                    Some(cr) => {
                        let mut t0 = 0.0;
                        let mut acc = 0.0;
                        c.set_line_width(lw(2.5));
                        for (t, loss) in cr.iter().map(|c| (c.t, c.loss_db)).chain([(1.0, 0.0)]) {
                            let col = if acc > 0.0 { rgba(view::loss_color(acc), 0.95) } else { "rgba(220,240,255,0.9)".into() };
                            c.set_stroke_style_str(&col);
                            c.begin_path();
                            c.move_to(a.x + dx * t0, a.y + dy * t0);
                            c.line_to(a.x + dx * t, a.y + dy * t);
                            c.stroke();
                            acc += loss;
                            t0 = t;
                        }
                        for x in &cr {
                            c.set_fill_style_str(&rgba(view::loss_color(x.loss_db), 1.0));
                            c.begin_path();
                            let _ = c.arc(a.x + dx * x.t, a.y + dy * x.t, lw(4.5), 0.0, tau);
                            c.fill();
                        }
                    }
                    None => {
                        let _ = c.set_line_dash(&js_sys::Array::of2(&lw(5.0).into(), &lw(4.0).into()));
                        c.set_stroke_style_str("rgba(220,240,255,0.55)");
                        c.set_line_width(lw(1.2));
                        c.begin_path();
                        c.move_to(a.x, a.y);
                        c.line_to(p.0, p.1);
                        c.stroke();
                        let _ = c.set_line_dash(&js_sys::Array::new());
                    }
                }
                let d = dx.hypot(dy) * self.plan.cell_m;
                if d > 0.3 {
                    pill(&format!("{d:.1} m"), a.x + dx / 2.0, a.y + dy / 2.0, "#e6ebf2", "rgba(15,21,34,0.85)");
                }
            }
        }

        for m in &self.plan.measurements {
            c.set_fill_style_str("#e6ebf2");
            c.begin_path();
            let _ = c.arc(m.x, m.y, lw(4.5), 0.0, tau);
            c.fill();
            c.set_stroke_style_str("#7c5cff");
            c.set_line_width(lw(2.0));
            c.stroke();
            c.set_stroke_style_str("rgba(230,235,242,0.6)");
            c.set_line_width(lw(1.0));
            c.begin_path();
            c.move_to(m.x, m.y - lw(5.0));
            c.line_to(m.x, m.y - lw(12.0));
            c.stroke();
            pill(&format!("{:.0} dBm", m.dbm), m.x, m.y - lw(20.0), "#ffffff", "rgba(124,92,255,0.92)");
        }

        for (i, a) in self.plan.aps.iter().enumerate() {
            let sel = self.selected == Some(i);
            // glow
            if let Ok(g) = c.create_radial_gradient(a.x, a.y, 0.0, a.x, a.y, lw(30.0)) {
                let _ = g.add_color_stop(0.0, if sel { "rgba(76,199,255,0.55)" } else { "rgba(76,199,255,0.3)" });
                let _ = g.add_color_stop(1.0, "rgba(76,199,255,0)");
                c.set_fill_style_canvas_gradient(&g);
                c.begin_path();
                let _ = c.arc(a.x, a.y, lw(30.0), 0.0, tau);
                c.fill();
            }
            c.set_fill_style_str(if sel { "#4cc7ff" } else { "#e6ebf2" });
            c.set_stroke_style_str(if sel { "#ffffff" } else { "#0f1522" });
            c.set_line_width(lw(2.5));
            c.begin_path();
            let _ = c.arc(a.x, a.y, lw(12.0), 0.0, tau);
            c.fill();
            c.stroke();
            // wifi glyph
            c.set_stroke_style_str("#04121c");
            c.set_line_width(lw(1.8));
            for r in [4.0, 7.5] {
                c.begin_path();
                let _ = c.arc(a.x, a.y + lw(3.5), lw(r), tau * 0.625, tau * 0.875);
                c.stroke();
            }
            c.set_fill_style_str("#04121c");
            c.begin_path();
            let _ = c.arc(a.x, a.y + lw(3.5), lw(1.6), 0.0, tau);
            c.fill();
            let band = ["2.4G", "5G", "6G"][a.band.idx()];
            pill(&format!("{} · {band} ch{}", i + 1, a.channel), a.x, a.y + lw(25.0), "#e6ebf2", "rgba(15,21,34,0.85)");
        }
        if let Some(p) = self.hover {
            c.set_stroke_style_str("rgba(255,255,255,0.85)");
            c.set_line_width(lw(1.5));
            c.begin_path();
            let _ = c.arc(p.0, p.1, lw(6.0), 0.0, tau);
            c.stroke();
        }
    }

    fn render_readout(&self) {
        let text = self.hover.filter(|_| !self.plan.aps.is_empty()).map(|(x, y)| {
            let i = ((y as usize).min(self.plan.height - 1)) * self.plan.width + (x as usize).min(self.plan.width - 1);
            let sinr = self.cov.sinr.as_ref().map(|s| format!("{:.0}", s[i])).unwrap_or_default();
            fmt(
                self.tr("readout"),
                &[
                    ("x", format!("{:.1}", x * self.plan.cell_m)),
                    ("y", format!("{:.1}", y * self.plan.cell_m)),
                    ("dbm", format!("{:.0}", self.cov.best[i])),
                    ("ap", (self.cov.best_ap[i] + 1).to_string()),
                    ("sinr", sinr),
                ],
            )
        });
        el::<Element>("readout").set_text_content(Some(&text.unwrap_or_default()));
        let rays = self.ray().filter(|_| self.rays_on).map(|(i, cr)| {
            let db = cr.iter().map(|c| c.loss_db).sum::<f64>() + 0.0; // no "-0"
            let mats: Vec<&str> = cr.iter().map(|c| self.tr(mat_key(c.material))).collect();
            let mut s = fmt(
                self.tr("rays_hud"),
                &[("i", (i + 1).to_string()), ("n", cr.len().to_string()), ("db", format!("{db:.0}"))],
            );
            if !mats.is_empty() {
                s += &format!(": {}", mats.join(", "));
            }
            s
        });
        el::<Element>("rayinfo").set_text_content(Some(&rays.unwrap_or_default()));
    }

    fn render_legend(&self) {
        let (lo, hi) = LEGEND;
        el::<HtmlElement>("legend-bar").style().set_property("background", &view::legend_gradient(lo, hi)).unwrap();
        let names = self.tr(if self.sinr_view { "lg_sinr" } else { "lg_rssi" });
        let names: Vec<&str> = names.split('|').collect();
        let vals = if self.sinr_view { SINR_LEVELS } else { LEVELS };
        let html: String = LEVELS
            .iter()
            .zip(vals)
            .enumerate()
            .map(|(k, (&l, v))| {
                let pct = view::legend_pos(l, lo, hi) * 100.0;
                let tip = names.get(k).copied().unwrap_or("");
                // -67 / 11 sits too close to its neighbour for a label: tick mark only
                let minor = if k == 2 { " class=\"minor\"" } else { "" };
                format!("<span{minor} style=\"left:{pct:.1}%\" title=\"{v:.0} · {tip}\">{v:.0}</span>")
            })
            .collect();
        el::<Element>("legend-ticks").set_inner_html(&html);
        let title = self.tr(if self.sinr_view { "legend_sinr" } else { "legend_rssi" });
        el::<Element>("legend-name").set_text_content(Some(title));
        el::<Element>("legend-unit").set_text_content(Some(self.tr("legend_ends")));
    }

    fn render_materials(&self) {
        let mut html = String::from("<table><tr><th></th><th>2.4</th><th>5</th><th>6 GHz</th></tr>");
        for m in Material::ALL {
            let col = view::hex(view::material_style(m).0);
            html += &format!("<tr><td><i style=\"background:{col}\"></i>{}</td>", self.tr(mat_key(m)));
            for b in Band::ALL {
                html += &format!(
                    "<td><input type=\"number\" min=\"0\" max=\"80\" step=\"0.5\" data-m=\"{}\" data-b=\"{}\" value=\"{}\" aria-label=\"{} {}\"></td>",
                    m.idx(),
                    b.idx(),
                    self.plan.materials.loss(m, b),
                    self.tr(mat_key(m)),
                    ["2.4 GHz", "5 GHz", "6 GHz"][b.idx()],
                );
            }
            html += "</tr>";
        }
        el::<Element>("materials").set_inner_html(&(html + "</table>"));
        self.render_chips();
    }

    fn render_chips(&self) {
        let html: String = Material::ALL
            .iter()
            .map(|&m| {
                let on = m == self.material;
                format!(
                    "<button class=\"mchip{}\" role=\"radio\" aria-checked=\"{on}\" data-mat=\"{}\"><i style=\"background:{}\"></i>{}</button>",
                    if on { " active" } else { "" },
                    m.idx(),
                    view::hex(view::material_style(m).0),
                    self.tr(mat_key(m)),
                )
            })
            .collect();
        el::<Element>("mat-chips").set_inner_html(&html);
    }

    fn apply_lang(&self) {
        doc().document_element().unwrap().set_attribute("lang", if self.ko { "ko" } else { "en" }).unwrap();
        let nodes = doc().query_selector_all("[data-i18n]").unwrap();
        for i in 0..nodes.length() {
            let e: Element = nodes.get(i).unwrap().dyn_into().unwrap();
            let key = e.get_attribute("data-i18n").unwrap();
            e.set_text_content(Some(self.tr(&key)));
        }
        // tooltips: icon buttons keep their SVG children
        let nodes = doc().query_selector_all("[data-i18n-tip]").unwrap();
        for i in 0..nodes.length() {
            let e: Element = nodes.get(i).unwrap().dyn_into().unwrap();
            let key = e.get_attribute("data-i18n-tip").unwrap();
            let _ = e.set_attribute("data-tip", self.tr(&key));
            let _ = e.set_attribute("aria-label", self.tr(&key));
        }
        el::<Element>("lang").set_text_content(Some(self.tr("lang")));
        doc().set_title(self.tr("title"));
        self.render_legend();
        self.render_materials();
        self.render_status();
        self.render_walk();
        self.render_readout();
    }

    fn render_status(&self) {
        let p = &self.plan;
        let mut status = fmt(
            self.tr("status"),
            &[
                ("w", p.width.to_string()),
                ("h", p.height.to_string()),
                ("mw", format!("{:.1}", p.width as f64 * p.cell_m)),
                ("mh", format!("{:.1}", p.height as f64 * p.cell_m)),
                ("ms", format!("{:.0}", self.ms)),
            ],
        );
        if self.mode3d && self.fps.2 > 0.0 {
            status += &fmt(self.tr("fps"), &[("f", format!("{:.0}", self.fps.2))]);
        }
        el::<Element>("status").set_text_content(Some(&status));
    }

    fn set_material(&mut self, m: Material) {
        self.material = m;
        self.render_chips();
    }

    fn set_tool(&mut self, t: Tool) {
        self.tool = t;
        let nodes = doc().query_selector_all("[data-tool]").unwrap();
        let names = ["wall", "ap", "measure", "erase"];
        for i in 0..nodes.length() {
            let e: Element = nodes.get(i).unwrap().dyn_into().unwrap();
            let on = e.get_attribute("data-tool").as_deref() == Some(names[t as usize]);
            e.class_list().toggle_with_force("active", on).unwrap();
            let _ = e.set_attribute("aria-pressed", if on { "true" } else { "false" });
        }
    }

    fn hit_ap(&self, p: (f64, f64)) -> Option<usize> {
        let tol = self.px(16.0);
        self.plan
            .aps
            .iter()
            .enumerate()
            .map(|(i, a)| (i, (a.x - p.0).hypot(a.y - p.1)))
            .filter(|(_, d)| *d <= tol)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(i, _)| i)
    }

    fn erase_at(&mut self, p: (f64, f64)) {
        let tol = self.px(12.0);
        if let Some(i) = self.hit_ap(p) {
            self.snapshot();
            self.plan.aps.remove(i);
            self.selected = None;
        } else if let Some(i) = self.plan.measurements.iter().position(|m| (m.x - p.0).hypot(m.y - p.1) <= tol) {
            self.snapshot();
            self.plan.measurements.remove(i);
        } else if let Some((i, _)) = self
            .plan
            .walls
            .iter()
            .enumerate()
            .map(|(i, w)| (i, dist_point_seg(p, w)))
            .filter(|(_, d)| *d <= tol)
            .min_by(|a, b| a.1.total_cmp(&b.1))
        {
            self.snapshot();
            self.plan.walls.remove(i);
        } else {
            return;
        }
        self.recompute();
    }

    fn measure_at(&mut self, p: (f64, f64)) {
        let answer = win().prompt_with_message(self.tr("prompt_dbm")).ok().flatten();
        if let Some(dbm) = answer.and_then(|s| s.trim().replace(',', ".").parse::<f64>().ok()) {
            if dbm.is_finite() && (-120.0..=0.0).contains(&dbm) {
                self.snapshot();
                self.plan.measurements.push(Measurement { x: p.0, y: p.1, dbm });
                self.recompute();
            }
        }
    }

    fn pointer_down(&mut self, e: &PointerEvent) {
        let p = self.to_cell(e);
        match self.tool {
            Tool::Wall => {
                let s = (p.0.round(), p.1.round());
                self.drag = Some(Drag::Wall { start: s, cur: s });
            }
            Tool::Ap => {
                self.snapshot();
                let idx = match self.hit_ap(p) {
                    Some(i) => {
                        self.drag = Some(Drag::Ap { idx: i, moved: false });
                        i
                    }
                    None => {
                        let ap = self.ap_from_inputs(p.0, p.1);
                        self.plan.aps.push(ap);
                        self.drag = Some(Drag::Ap { idx: self.plan.aps.len() - 1, moved: true });
                        self.plan.aps.len() - 1
                    }
                };
                self.selected = Some(idx);
                self.sync_inputs();
                self.recompute();
            }
            Tool::Measure => self.measure_at(p),
            Tool::Erase => self.erase_at(p),
        }
    }

    fn pointer_move(&mut self, e: &PointerEvent) {
        let p = self.to_cell(e);
        self.hover = Some(p);
        match &mut self.drag {
            Some(Drag::Wall { start, cur }) => {
                let mut c = (p.0.round(), p.1.round());
                // snap near-axis walls to horizontal / vertical
                if (c.1 - start.1).abs() < 0.2 * (c.0 - start.0).abs() {
                    c.1 = start.1;
                } else if (c.0 - start.0).abs() < 0.2 * (c.1 - start.1).abs() {
                    c.0 = start.0;
                }
                *cur = c;
                self.render();
            }
            Some(Drag::Ap { idx, moved }) => {
                *moved = true;
                let i = *idx;
                self.plan.aps[i].x = p.0.min(self.plan.width as f64 - 0.01);
                self.plan.aps[i].y = p.1.min(self.plan.height as f64 - 0.01);
                self.dirty_cov = true; // recomputed once per animation frame
            }
            None => self.render(),
        }
        self.render_readout();
    }

    fn pointer_up(&mut self) {
        match self.drag.take() {
            Some(Drag::Wall { start, cur }) if start != cur => {
                self.snapshot();
                let material = self.material;
                self.plan.walls.push(Wall { x1: start.0, y1: start.1, x2: cur.0, y2: cur.1, material });
                self.recompute();
            }
            Some(Drag::Ap { moved: false, .. }) => {
                self.undo.pop(); // tap to select only: nothing changed
                self.render();
            }
            Some(Drag::Ap { moved: true, .. }) => self.recompute(),
            _ => self.render(),
        }
    }

    // ---------- 3D ----------

    fn set_mode3d(&mut self, on: bool) {
        if on && self.scene.is_none() {
            if !self.gl_failed {
                self.scene = Scene::new(&self.canvas3d);
            }
            if self.scene.is_none() {
                self.gl_failed = true;
                el::<Element>("notice").class_list().add_1("show").unwrap();
                return self.set_mode3d(false);
            }
            self.cam_dims = None;
            self.paint_heat();
        }
        self.mode3d = on;
        body_class("mode3d", on);
        self.resize();
        set_active("mode-2d", !on);
        set_active("mode-3d", on);
        self.hover = None;
        self.fps = (0, now(), 0.0);
        if on {
            self.sync_camera();
        } else {
            self.render();
        }
        self.render_status();
        self.render_readout();
    }

    /// Re-frames the camera when the plan size changed (first 3D use, new plan).
    fn sync_camera(&mut self) {
        let dims = (self.plan.width, self.plan.height, self.plan.cell_m.to_bits());
        let Some(sc) = &mut self.scene else { return };
        if self.cam_dims != Some(dims) {
            let first = self.cam_dims.is_none();
            sc.preset(&self.plan, sc.mode);
            if first {
                sc.jump();
            }
            self.cam_dims = Some(dims);
        }
    }

    fn set_cam(&mut self, m: CamMode) {
        let Some(sc) = &mut self.scene else { return };
        sc.preset(&self.plan, m);
        set_active("cam-persp", m == CamMode::Perspective);
        set_active("cam-top", m == CamMode::Top);
        set_active("cam-walk", m == CamMode::Walk);
        body_class("walkcam", m == CamMode::Walk);
    }

    fn ndc3(&self, x: f64, y: f64) -> (f32, f32) {
        let r = self.canvas3d.get_bounding_client_rect();
        view::to_ndc(x - r.left(), y - r.top(), r.width(), r.height())
    }

    fn down3(&mut self, e: &PointerEvent) {
        let (x, y) = (e.client_x() as f64, e.client_y() as f64);
        self.ptrs.retain(|p| p.id != e.pointer_id());
        self.ptrs.push(Ptr { id: e.pointer_id(), x, y });
        let Some(sc) = &mut self.scene else { return };
        sc.orbit.held = true;
        if self.ptrs.len() >= 2 {
            // a second finger turns any gesture into pinch / two-finger move (keep a moved AP)
            if let Some(Drag3::Ap { moved: false, .. }) = self.drag3 {
                self.undo.pop();
            }
            self.drag3 = Some(Drag3::Two);
            return;
        }
        self.down_at = (x, y);
        let ndc = self.ndc3(x, y);
        let sc = self.scene.as_ref().unwrap();
        if let Some(i) = sc.pick_ap(&self.plan, ndc).filter(|_| e.button() == 0) {
            self.snapshot();
            self.selected = Some(i);
            self.sync_inputs();
            self.drag3 = Some(Drag3::Ap { idx: i, moved: false });
        } else if e.button() == 2 || e.shift_key() {
            self.drag3 = Some(Drag3::Pan);
        } else if sc.mode == CamMode::Walk {
            self.drag3 = Some(Drag3::Look);
        } else {
            self.drag3 = Some(Drag3::Orbit);
        }
        let _ = self.canvas3d.class_list().add_1("dragging");
    }

    fn move3(&mut self, e: &PointerEvent) {
        let (x, y) = (e.client_x() as f64, e.client_y() as f64);
        let centroid = |ps: &[Ptr]| {
            let n = ps.len().max(1) as f64;
            (ps.iter().map(|p| p.x).sum::<f64>() / n, ps.iter().map(|p| p.y).sum::<f64>() / n)
        };
        let spread = |ps: &[Ptr]| if ps.len() >= 2 { (ps[0].x - ps[1].x).hypot(ps[0].y - ps[1].y) } else { 0.0 };
        let (c0, s0) = (centroid(&self.ptrs), spread(&self.ptrs));
        let prev = self.ptrs.iter().find(|p| p.id == e.pointer_id()).map(|p| (p.x, p.y));
        if let Some(p) = self.ptrs.iter_mut().find(|p| p.id == e.pointer_id()) {
            (p.x, p.y) = (x, y);
        }
        let (dx, dy) = prev.map(|(px, py)| (x - px, y - py)).unwrap_or((0.0, 0.0));
        let ndc = self.ndc3(x, y);
        let Some(sc) = &mut self.scene else { return };
        let pick = view::pick_cell(&self.plan, &sc.inv_vp, ndc.0, ndc.1);
        match self.drag3 {
            Some(Drag3::Ap { idx, .. }) => {
                if let Some(p) = pick {
                    let a = &mut self.plan.aps[idx];
                    a.x = p.0.clamp(0.0, self.plan.width as f64 - 0.01);
                    a.y = p.1.clamp(0.0, self.plan.height as f64 - 0.01);
                    self.drag3 = Some(Drag3::Ap { idx, moved: true });
                    self.dirty_cov = true;
                }
            }
            Some(Drag3::Orbit) => sc.orbit.rotate(-dx as f32 * 0.006, dy as f32 * 0.006),
            Some(Drag3::Look) => sc.fps.look(dx as f32 * 0.004, -dy as f32 * 0.004),
            Some(Drag3::Pan) => pan(sc, dx, dy),
            Some(Drag3::Two) if self.ptrs.len() >= 2 => {
                let (c1, s1) = (centroid(&self.ptrs), spread(&self.ptrs));
                let (cx, cy) = (c1.0 - c0.0, c1.1 - c0.1);
                if sc.mode == CamMode::Walk {
                    let (w, d) = Scene::extent(&self.plan);
                    sc.fps.walk(-cy as f32 * 0.02, cx as f32 * 0.02, (w, d));
                } else {
                    if s0 > 1.0 && s1 > 1.0 {
                        sc.orbit.zoom((s0 / s1) as f32);
                    }
                    pan(sc, cx, cy);
                }
            }
            _ => {}
        }
        self.hover = if matches!(self.drag3, Some(Drag3::Ap { .. })) {
            self.drag3.and_then(|d| match d {
                Drag3::Ap { idx, .. } => self.plan.aps.get(idx).map(|a| (a.x, a.y)),
                _ => None,
            })
        } else {
            pick
        };
        if self.drag3.is_none() {
            let sc = self.scene.as_ref().unwrap();
            self.hot_ap = sc.pick_ap(&self.plan, ndc);
            let _ = self.canvas3d.class_list().toggle_with_force("hot", self.hot_ap.is_some());
        }
        self.render_readout();
    }

    fn up3(&mut self, e: &PointerEvent) {
        self.ptrs.retain(|p| p.id != e.pointer_id());
        if !self.ptrs.is_empty() {
            return;
        }
        let tap = (e.client_x() as f64 - self.down_at.0).hypot(e.client_y() as f64 - self.down_at.1) < 5.0;
        match self.drag3.take() {
            Some(Drag3::Ap { moved: false, .. }) => {
                self.undo.pop();
            }
            Some(Drag3::Ap { moved: true, .. }) => self.recompute(),
            Some(Drag3::Orbit | Drag3::Look) if tap => self.tap3(e),
            _ => {}
        }
        if let Some(sc) = &mut self.scene {
            sc.orbit.held = false;
        }
        let _ = self.canvas3d.class_list().remove_1("dragging");
    }

    /// Tap on the 3D floor: add an AP (AP tool), measure, or erase the nearest AP.
    fn tap3(&mut self, e: &PointerEvent) {
        let ndc = self.ndc3(e.client_x() as f64, e.client_y() as f64);
        let Some(sc) = &self.scene else { return };
        let Some(p) = view::pick_cell(&self.plan, &sc.inv_vp, ndc.0, ndc.1) else { return };
        match self.tool {
            Tool::Ap => {
                self.snapshot();
                let ap = self.ap_from_inputs(p.0.min(self.plan.width as f64 - 0.01), p.1.min(self.plan.height as f64 - 0.01));
                self.plan.aps.push(ap);
                self.selected = Some(self.plan.aps.len() - 1);
                self.sync_inputs();
                self.recompute();
            }
            Tool::Measure => self.measure_at(p),
            Tool::Erase => {
                if let Some(i) = self.scene.as_ref().and_then(|s| s.pick_ap(&self.plan, ndc)) {
                    self.snapshot();
                    self.plan.aps.remove(i);
                    self.selected = None;
                    self.recompute();
                }
            }
            Tool::Wall => {}
        }
    }

    fn frame(&mut self, t: f64) {
        let dt = ((t - self.last_t) / 1000.0).clamp(0.0, 0.1) as f32;
        self.last_t = t;
        if self.dirty_cov {
            self.recompute();
        }
        if MSG_UNTIL.with(|c| c.get()) < t {
            let _ = el::<Element>("msg").class_list().remove_1("show");
        }
        if !self.mode3d {
            return;
        }
        let (w, h) = (self.canvas3d.width() as i32, self.canvas3d.height() as i32);
        let Some(sc) = &mut self.scene else { return };
        if sc.mode == CamMode::Walk && self.keys != 0 {
            let k = |b: u8| if self.keys & b != 0 { 1.0 } else { 0.0 };
            let speed = if self.keys & 16 != 0 { 3.2 } else { 1.6 } * dt;
            let (ew, ed) = Scene::extent(&self.plan);
            sc.fps.walk((k(1) - k(2)) * speed, (k(8) - k(4)) * speed, (ew, ed));
        }
        let drag_ap = match self.drag3 {
            Some(Drag3::Ap { idx, .. }) => Some(idx),
            _ => None,
        };
        let frame = Frame {
            plan: &self.plan,
            selected: self.selected,
            hover: self.hover,
            drag_ap,
            hot_ap: self.hot_ap,
            time: t / 1000.0,
            has_field: !self.plan.aps.is_empty(),
            iso: self.contours_on,
            rays: self.rays_on,
        };
        sc.draw(&frame, w, h, dt);
        self.fps.0 += 1;
        if t - self.fps.1 > 500.0 {
            self.fps.2 = self.fps.0 as f64 * 1000.0 / (t - self.fps.1);
            self.fps = (0, t, self.fps.2);
            self.render_status();
        }
    }

    // ---------- settings ----------

    fn edit_selected_ap(&mut self) {
        let Some(i) = self.selected.filter(|&i| i < self.plan.aps.len()) else { return };
        let (x, y) = (self.plan.aps[i].x, self.plan.aps[i].y);
        let ap = self.ap_from_inputs(x, y);
        if ap != self.plan.aps[i] {
            self.snapshot();
            self.plan.aps[i] = ap;
            self.recompute();
        }
    }

    fn edit_grid(&mut self) {
        let mut p = self.plan.clone();
        p.cell_m = num("scale").unwrap_or(p.cell_m).clamp(0.02, 5.0);
        p.width = num("gw").unwrap_or(p.width as f64).clamp(10.0, 400.0) as usize;
        p.height = num("gh").unwrap_or(p.height as f64).clamp(10.0, 400.0) as usize;
        p.exponent = num("exponent").unwrap_or(p.exponent).clamp(1.0, 8.0);
        p.offset_db = num("offset").unwrap_or(p.offset_db).clamp(-40.0, 40.0);
        if p != self.plan {
            let sel = self.selected;
            self.set_plan(p);
            self.selected = sel;
        }
    }

    fn do_calibrate(&mut self) {
        match calibrate(&self.plan) {
            None => self.msg(self.tr("need_points")),
            Some(c) => {
                self.snapshot();
                self.plan.exponent = c.exponent;
                self.plan.offset_db = c.offset_db;
                self.sync_inputs();
                self.recompute();
                let m = fmt(
                    self.tr("calib_res"),
                    &[
                        ("n", format!("{:.2}", c.exponent)),
                        ("o", format!("{:.1}", c.offset_db)),
                        ("b", format!("{:.1}", c.rmse_before)),
                        ("a", format!("{:.1}", c.rmse_after)),
                        ("k", self.plan.measurements.len().to_string()),
                    ],
                );
                self.msg(&m);
            }
        }
    }

    fn do_suggest(&mut self) {
        if self.plan.aps.is_empty() {
            return self.msg(self.tr("need_ap"));
        }
        let i = self.selected.unwrap_or(0).min(self.plan.aps.len() - 1);
        let s = suggest_placement(&self.plan, i);
        self.snapshot();
        self.plan.aps[i].x = s.x;
        self.plan.aps[i].y = s.y;
        self.selected = Some(i);
        self.recompute();
        let sc = score(&self.cov);
        let m = fmt(
            self.tr("suggest_res"),
            &[
                ("i", (i + 1).to_string()),
                ("m", format!("{:.0}", sc.median_dbm)),
                ("p", format!("{:.0}", sc.frac_below_70 * 100.0)),
            ],
        );
        self.msg(&m);
    }

    fn load_str(&mut self, src: &str) {
        match Plan::from_json(src) {
            Ok(p) => self.set_plan(p),
            Err(e) => self.msg(&format!("{}{e}", self.tr("bad_plan"))),
        }
    }

    fn load_hash(&mut self) -> bool {
        let hash = win().location().hash().unwrap_or_default();
        let Some(s) = hash.strip_prefix("#plan=") else { return false };
        match Plan::from_share_string(s) {
            Ok(p) => {
                self.set_plan(p);
                true
            }
            Err(e) => {
                self.msg(&format!("{}{e}", self.tr("bad_plan")));
                false
            }
        }
    }

    fn share(&self) {
        let url = format!("#plan={}", self.plan.to_share_string());
        let _ = win().history().unwrap().replace_state_with_url(&JsValue::NULL, "", Some(&url));
        el::<HtmlInputElement>("share-out").set_value(&win().location().href().unwrap_or_default());
        el::<HtmlInputElement>("share-out").select();
        self.msg(self.tr("share_done"));
    }

    fn export(&self) {
        let parts = js_sys::Array::of1(&JsValue::from_str(&self.plan.to_json()));
        let bag = BlobPropertyBag::new();
        bag.set_type("application/json");
        let blob = Blob::new_with_str_sequence_and_options(&parts, &bag).unwrap();
        let url = Url::create_object_url_with_blob(&blob).unwrap();
        let a: HtmlAnchorElement = doc().create_element("a").unwrap().dyn_into().unwrap();
        a.set_href(&url);
        a.set_download("wifi-plan.json");
        let body = doc().body().unwrap();
        let _ = body.append_child(&a); // some browsers ignore clicks on detached anchors
        a.click();
        a.remove();
        let _ = Url::revoke_object_url(&url);
    }

    // ---------- walk mode ----------

    fn step_len(&self) -> f64 {
        num("step-len").unwrap_or(0.7).clamp(0.3, 1.2)
    }

    fn walk_start(&mut self) {
        let has = js_sys::Reflect::has(&win(), &"DeviceMotionEvent".into()).unwrap_or(false);
        if !has || !win().is_secure_context() {
            return self.msg(self.tr("walk_no_sensor"));
        }
        request_sensor_permission(self.tr("walk_denied"));
        if self.walk.is_none() {
            self.walk = Some(WalkUi { walk: Walk::new(self.step_len()), det: StepDetector::new(), heading: 0.0, calib: None });
        }
        self.msg(self.tr("walk_started"));
        self.render_walk();
    }

    fn on_motion(&mut self, e: &DeviceMotionEvent) {
        let Some(w) = &mut self.walk else { return };
        let Some(a) = e.acceleration_including_gravity() else { return };
        let (Some(x), Some(y), Some(z)) = (a.x(), a.y(), a.z()) else { return };
        if w.det.push(e.time_stamp() / 1000.0, (x * x + y * y + z * z).sqrt()) {
            w.walk.set_heading(w.heading);
            w.walk.step();
            self.render_walk();
        }
    }

    fn on_orientation(&mut self, e: &DeviceOrientationEvent) {
        let Some(w) = &mut self.walk else { return };
        // alpha (gyro-fused, counter-clockwise) is steadier indoors than the magnetic compass;
        // iOS's clockwise webkitCompassHeading is only the fallback.
        let compass = e.alpha().map(|a| 360.0 - a).or_else(|| {
            js_sys::Reflect::get(e, &"webkitCompassHeading".into()).ok().and_then(|v| v.as_f64())
        });
        if let Some(c) = compass {
            w.heading = (c - 90.0).to_radians(); // compass 0 = screen up (-y)
        }
    }

    /// Runs `f` on the active walk, or asks the user to press Start.
    fn with_walk(&mut self, f: impl FnOnce(&mut WalkUi) -> Option<String>) {
        let Some(w) = &mut self.walk else { return self.msg(self.tr("walk_need_start")) };
        if let Some(m) = f(w) {
            self.msg(&m);
        }
        self.render_walk();
    }

    fn walk_close(&mut self) {
        let t = self.tr("walk_closed");
        self.with_walk(|w| Some(fmt(t, &[("g", format!("{:.2}", w.walk.close_loop()))])));
    }

    fn walk_dbm(&mut self) {
        if self.walk.is_none() {
            return self.msg(self.tr("walk_need_start"));
        }
        let answer = win().prompt_with_message(self.tr("prompt_dbm")).ok().flatten();
        let dbm = answer.and_then(|s| s.trim().replace(',', ".").parse::<f64>().ok());
        if let Some(d) = dbm.filter(|d| (-120.0..=0.0).contains(d)) {
            self.with_walk(|w| {
                w.walk.set_dbm(d);
                None
            });
        }
    }

    fn walk_calib(&mut self) {
        let Some(w) = &mut self.walk else { return self.msg(self.tr("walk_need_start")) };
        let Some(start) = w.calib.take() else {
            w.calib = Some(w.walk.steps);
            return self.msg(self.tr("walk_calib_on"));
        };
        let steps = w.walk.steps - start;
        let answer = win().prompt_with_message(self.tr("walk_calib_prompt")).ok().flatten();
        let len = answer.and_then(|s| s.trim().replace(',', ".").parse::<f64>().ok()).unwrap_or(0.0);
        if let Some(s) = calibrate_step(len, steps).map(|s| s.clamp(0.3, 1.2)) {
            if let Some(w) = &mut self.walk {
                w.walk.step_m = s;
            }
            set_num("step-len", (s * 100.0).round() / 100.0);
            let m = fmt(self.tr("walk_calib_res"), &[("m", format!("{s:.2}")), ("n", steps.to_string())]);
            self.msg(&m);
        }
    }

    fn walk_apply(&mut self) {
        let Some(w) = &self.walk else { return self.msg(self.tr("walk_need_start")) };
        let mut p = w.walk.to_plan(self.plan.cell_m);
        if p.walls.is_empty() {
            return;
        }
        p.name = self.plan.name.clone();
        p.materials = self.plan.materials.clone();
        p.exponent = self.plan.exponent;
        p.offset_db = self.plan.offset_db;
        self.set_plan(p);
        self.msg(self.tr("walk_applied"));
    }

    /// Small live preview of the walk in metres, auto-fitted.
    fn render_walk(&self) {
        let c = &self.walk_ctx;
        let (cw, ch) = (self.walk_canvas.width() as f64, self.walk_canvas.height() as f64);
        c.set_transform(1.0, 0.0, 0.0, 1.0, 0.0, 0.0).unwrap();
        c.set_fill_style_str("#0f1522");
        c.fill_rect(0.0, 0.0, cw, ch);
        el::<Element>("walk-snap")
            .class_list()
            .toggle_with_force("active", self.walk.as_ref().is_some_and(|w| w.walk.snap))
            .unwrap();
        let status: Element = el("walk-status");
        let Some(w) = &self.walk else {
            status.set_text_content(None);
            return;
        };
        let (walls, rooms, cur) = (w.walk.walls(), w.walk.rooms(), w.walk.cursor());
        let pts: Vec<(f64, f64)> = rooms.iter().flatten().map(|v| (v.x, v.y)).chain([cur]).collect();
        let (mut x0, mut y0, mut x1, mut y1) = (cur.0, cur.1, cur.0, cur.1);
        for &(x, y) in &pts {
            (x0, y0, x1, y1) = (x0.min(x), y0.min(y), x1.max(x), y1.max(y));
        }
        let pad = 24.0;
        let s = ((cw - 2.0 * pad) / (x1 - x0).max(2.0)).min((ch - 2.0 * pad) / (y1 - y0).max(2.0));
        let (ox, oy) = ((cw - (x1 - x0) * s) / 2.0, (ch - (y1 - y0) * s) / 2.0);
        let tx = |p: (f64, f64)| (ox + (p.0 - x0) * s, oy + (p.1 - y0) * s);
        c.set_line_cap("round");
        for wl in &walls {
            let (col, t) = view::material_style(wl.material);
            let (a, b) = (tx((wl.x1, wl.y1)), tx((wl.x2, wl.y2)));
            c.set_stroke_style_str(&view::hex(col));
            c.set_line_width(3.0 + t * 20.0);
            c.begin_path();
            c.move_to(a.0, a.1);
            c.line_to(b.0, b.1);
            c.stroke();
        }
        let dot = |p: (f64, f64), r: f64, col: &str| {
            let p = tx(p);
            c.set_fill_style_str(col);
            c.begin_path();
            let _ = c.arc(p.0, p.1, r, 0.0, std::f64::consts::TAU);
            c.fill();
        };
        if let (true, Some(v)) = (w.walk.is_open(), rooms.last().and_then(|r| r.last())) {
            let (a, b) = (tx((v.x, v.y)), tx(cur));
            c.set_stroke_style_str("rgba(76,199,255,0.6)");
            c.set_line_width(2.0);
            c.begin_path();
            c.move_to(a.0, a.1);
            c.line_to(b.0, b.1);
            c.stroke();
        }
        for v in rooms.iter().flatten() {
            let col = match (v.mark, v.dbm.is_some()) {
                (_, true) => "#7c5cff",
                (Mark::Door, _) => "#ff8a65",
                (Mark::Corner, _) => "#e6ebf2",
            };
            dot((v.x, v.y), 5.0, col);
        }
        dot(cur, 8.0, "#4cc7ff");

        let mut text = fmt(
            self.tr("walk_status"),
            &[
                ("s", w.walk.steps.to_string()),
                ("h", format!("{:.0}", (w.heading.to_degrees() + 90.0).rem_euclid(360.0))),
                ("r", rooms.len().to_string()),
            ],
        );
        let drift = w.walk.drift_deg();
        let warn = !w.walk.snap && drift > 15.0;
        if warn {
            text += &fmt(self.tr("walk_drift"), &[("d", format!("{drift:.0}"))]);
        }
        status.set_text_content(Some(&text));
        status.class_list().toggle_with_force("warn", warn).unwrap();
    }
}

/// Pan the orbit target by a screen drag (CSS px), "grab the floor" style.
fn pan(sc: &mut Scene, dx: f64, dy: f64) {
    let o = sc.orbit.cur;
    let (sy, cy) = o.yaw.sin_cos();
    let k = o.dist * 0.0016;
    let right = [cy, 0.0, -sy];
    let fwd = [-sy, 0.0, -cy];
    let off = math::add(math::scale(right, -dx as f32 * k), math::scale(fwd, dy as f32 * k));
    sc.orbit.pan(off);
}

type Shared = Rc<RefCell<App>>;

fn on(target: &EventTarget, ev: &str, f: impl FnMut(Event) + 'static) {
    let c = Closure::<dyn FnMut(Event)>::new(f);
    target.add_event_listener_with_callback(ev, c.as_ref().unchecked_ref()).unwrap();
    c.forget();
}

fn on_id(id: &str, ev: &str, app: &Shared, mut f: impl FnMut(&mut App, Event) + 'static) {
    let a = app.clone();
    on(&el::<EventTarget>(id), ev, move |e| f(&mut a.borrow_mut(), e));
}

/// Focus is in a control that uses the keyboard itself (text / number / range input, select).
fn typing() -> bool {
    doc().active_element().is_some_and(|e| match e.tag_name().as_str() {
        "SELECT" | "TEXTAREA" => true,
        "INPUT" => !matches!(e.get_attribute("type").as_deref(), Some("checkbox" | "button" | "file")),
        _ => false,
    })
}

/// Canvas presses call preventDefault (no focus change), so drop focus from panel controls
/// explicitly: keyboard shortcuts and walk-through keys then reach the window handler.
fn blur_active() {
    if let Some(e) = doc().active_element().and_then(|e| e.dyn_into::<HtmlElement>().ok()) {
        let _ = e.blur();
    }
}

/// Key → walk-through movement bit (1 fwd, 2 back, 4 left, 8 right, 16 run).
fn key_bit(k: &str) -> u8 {
    match k {
        "w" | "arrowup" => 1,
        "s" | "arrowdown" => 2,
        "a" | "arrowleft" => 4,
        "d" | "arrowright" => 8,
        "shift" => 16,
        _ => 0,
    }
}

fn main() {
    let canvas: HtmlCanvasElement = el("plan");
    let heat: HtmlCanvasElement = doc().create_element("canvas").unwrap().dyn_into().unwrap();
    let ko = win().navigator().language().unwrap_or_default().to_lowercase().starts_with("ko");
    let plan = Plan::from_json(APARTMENT).unwrap();
    let app = App {
        cov: coverage(&plan),
        plan,
        undo: Vec::new(),
        redo: Vec::new(),
        tool: Tool::Ap,
        material: Material::Drywall,
        ko,
        selected: None,
        drag: None,
        hover: None,
        sinr_view: false,
        contours_on: true,
        rays_on: false,
        contours: Vec::new(),
        bg: None,
        bg_alpha: 0.5,
        ctx: ctx2d(&canvas),
        canvas,
        heat_ctx: ctx2d(&heat),
        heat,
        scale: 1.0,
        ms: 0.0,
        walk: None,
        walk_ctx: ctx2d(&el("walk-view")),
        walk_canvas: el("walk-view"),
        canvas3d: el("view3d"),
        mode3d: false,
        scene: None,
        gl_failed: false,
        cam_dims: None,
        ptrs: Vec::new(),
        drag3: None,
        down_at: (0.0, 0.0),
        hot_ap: None,
        keys: 0,
        dirty_cov: false,
        last_t: 0.0,
        fps: (0, 0.0, 0.0),
    };
    let app: Shared = Rc::new(RefCell::new(app));

    {
        let mut a = app.borrow_mut();
        if a.mobile() {
            body_class("panel-open", false);
        }
        a.set_tool(Tool::Ap);
        a.apply_lang();
        // first run = no shared link and nothing saved: the sample apartment plus a short hint
        let mut first_run = false;
        if !a.load_hash() {
            let saved = storage().and_then(|s| s.get_item(STORE_KEY).ok().flatten());
            match saved.and_then(|s| Plan::from_json(&s).ok()) {
                Some(p) => a.plan = p,
                None => first_run = true,
            }
            a.after_plan_swap();
        }
        a.undo.clear();
        let seen = storage().and_then(|s| s.get_item(HINT_KEY).ok().flatten()).is_some();
        if first_run && !seen {
            el::<Element>("hint").class_list().add_1("show").unwrap();
        }
    }

    // 2D canvas pointer input (mouse, pen and touch)
    let target: EventTarget = app.borrow().canvas.clone().into();
    let a = app.clone();
    on(&target, "pointerdown", move |e| {
        let e: PointerEvent = e.unchecked_into();
        e.prevent_default();
        blur_active();
        let mut a = a.borrow_mut();
        let _ = a.canvas.set_pointer_capture(e.pointer_id());
        a.pointer_down(&e);
    });
    let a = app.clone();
    on(&target, "pointermove", move |e| a.borrow_mut().pointer_move(&e.unchecked_into()));
    for ev in ["pointerup", "pointercancel"] {
        let a = app.clone();
        on(&target, ev, move |_| a.borrow_mut().pointer_up());
    }
    let a = app.clone();
    on(&target, "pointerleave", move |_| {
        let mut a = a.borrow_mut();
        a.hover = None;
        a.render();
        a.render_readout();
    });

    // 3D canvas: orbit / pan / zoom / look, AP drag, hover readout
    let t3: EventTarget = app.borrow().canvas3d.clone().into();
    let a = app.clone();
    on(&t3, "pointerdown", move |e| {
        let e: PointerEvent = e.unchecked_into();
        e.prevent_default();
        blur_active();
        let mut a = a.borrow_mut();
        let _ = a.canvas3d.set_pointer_capture(e.pointer_id());
        a.down3(&e);
    });
    let a = app.clone();
    on(&t3, "pointermove", move |e| {
        if let Ok(mut a) = a.try_borrow_mut() {
            a.move3(&e.unchecked_into());
        }
    });
    for ev in ["pointerup", "pointercancel"] {
        let a = app.clone();
        on(&t3, ev, move |e| a.borrow_mut().up3(&e.unchecked_into()));
    }
    let a = app.clone();
    on(&t3, "pointerleave", move |_| {
        let mut a = a.borrow_mut();
        if a.drag3.is_none() {
            a.hover = None;
            a.render_readout();
        }
    });
    let a = app.clone();
    on(&t3, "wheel", move |e| {
        let e: WheelEvent = e.unchecked_into();
        e.prevent_default();
        if let Some(sc) = &mut a.borrow_mut().scene {
            let d = e.delta_y() * if e.delta_mode() == 1 { 16.0 } else { 1.0 };
            if sc.mode == CamMode::Walk {
                sc.fps.look(0.0, (-d * 0.002) as f32);
            } else {
                sc.orbit.zoom((d * 0.0015).exp() as f32);
            }
        }
    });
    on(&t3, "contextmenu", |e| e.prevent_default());

    // tool buttons
    let nodes = doc().query_selector_all("[data-tool]").unwrap();
    for i in 0..nodes.length() {
        let e: Element = nodes.get(i).unwrap().dyn_into().unwrap();
        let tool = match e.get_attribute("data-tool").as_deref() {
            Some("ap") => Tool::Ap,
            Some("measure") => Tool::Measure,
            Some("erase") => Tool::Erase,
            _ => Tool::Wall,
        };
        let a = app.clone();
        on(&e, "click", move |_| a.borrow_mut().set_tool(tool));
    }
    on_id("mat-chips", "click", &app, |a, e| {
        let chip = e.target().and_then(|t| t.dyn_into::<Element>().ok()).and_then(|t| t.closest("[data-mat]").ok().flatten());
        if let Some(m) = chip.and_then(|c| c.get_attribute("data-mat")).and_then(|v| v.parse::<usize>().ok()) {
            a.set_material(Material::ALL[m.min(6)]);
            a.set_tool(Tool::Wall);
        }
    });

    on_id("undo", "click", &app, |a, _| a.undo());
    on_id("redo", "click", &app, |a, _| a.redo());
    on_id("clear", "click", &app, |a, _| {
        let mut p = a.plan.clone();
        p.walls.clear();
        p.aps.clear();
        p.measurements.clear();
        a.set_plan(p);
    });
    on_id("lang", "click", &app, |a, _| {
        a.ko = !a.ko;
        a.apply_lang();
        a.resize(); // the top row may wrap differently in the other language
    });
    for id in ["band", "tx", "gain", "channel"] {
        on_id(id, "change", &app, |a, _| a.edit_selected_ap());
    }
    for id in ["scale", "gw", "gh", "exponent", "offset"] {
        on_id(id, "change", &app, |a, _| a.edit_grid());
    }
    on_id("view", "change", &app, |a, _| {
        a.sinr_view = select_val("view") == "sinr";
        a.paint_heat();
        a.render();
        a.render_legend();
    });
    on_id("contours", "change", &app, |a, _| {
        a.contours_on = checked("contours");
        a.paint_heat();
        a.render();
    });
    on_id("rays", "change", &app, |a, _| {
        a.rays_on = checked("rays");
        a.render();
        a.render_readout();
    });
    on_id("mode-2d", "click", &app, |a, _| a.set_mode3d(false));
    on_id("mode-3d", "click", &app, |a, _| a.set_mode3d(true));
    on_id("cam-persp", "click", &app, |a, _| a.set_cam(CamMode::Perspective));
    on_id("cam-top", "click", &app, |a, _| a.set_cam(CamMode::Top));
    on_id("cam-walk", "click", &app, |a, _| a.set_cam(CamMode::Walk));
    for id in ["panel-toggle", "sheet-handle"] {
        on_id(id, "click", &app, |a, _| {
            body_class("panel-open", !has_body_class("panel-open"));
            a.resize();
        });
    }
    on_id("hint-close", "click", &app, |_, _| {
        el::<Element>("hint").class_list().remove_1("show").unwrap();
        if let Some(s) = storage() {
            let _ = s.set_item(HINT_KEY, "1");
        }
    });
    on_id("notice", "click", &app, |_, _| el::<Element>("notice").class_list().remove_1("show").unwrap());
    on_id("suggest", "click", &app, |a, _| a.do_suggest());
    on_id("calibrate", "click", &app, |a, _| a.do_calibrate());
    on_id("meas-clear", "click", &app, |a, _| {
        if !a.plan.measurements.is_empty() {
            a.snapshot();
            a.plan.measurements.clear();
            a.recompute();
        }
    });
    on_id("example", "change", &app, |a, _| {
        let sel: HtmlSelectElement = el("example");
        match sel.value().as_str() {
            "blank" => {
                let p = Plan::new(120, 90, 0.1);
                a.set_plan(p);
            }
            "apartment" => a.load_str(APARTMENT),
            "office" => a.load_str(OFFICE),
            _ => {}
        }
        sel.set_value("");
    });
    on_id("materials", "change", &app, |a, e| {
        let Some(inp) = e.target().and_then(|t| t.dyn_into::<HtmlInputElement>().ok()) else { return };
        let idx = |k: &str| inp.get_attribute(k).and_then(|v| v.parse::<usize>().ok());
        let (Some(m), Some(b)) = (idx("data-m"), idx("data-b")) else { return };
        let v = inp.value_as_number();
        if v.is_finite() && m < 7 && b < 3 {
            a.snapshot();
            a.plan.materials.set(Material::ALL[m], Band::ALL[b], v.clamp(0.0, 80.0));
            a.recompute();
        }
    });

    // file & share
    on_id("export", "click", &app, |a, _| a.export());
    on_id("import-btn", "click", &app, |_, _| el::<HtmlElement>("import").click());
    let a = app.clone();
    on(&el::<EventTarget>("import"), "change", move |_| {
        let input: HtmlInputElement = el("import");
        let Some(file) = input.files().and_then(|f| f.get(0)) else { return };
        let reader = FileReader::new().unwrap();
        let (a2, r2) = (a.clone(), reader.clone());
        let onload = Closure::once_into_js(move || {
            let text = r2.result().ok().and_then(|v| v.as_string()).unwrap_or_default();
            a2.borrow_mut().load_str(&text);
        });
        reader.set_onload(Some(onload.unchecked_ref()));
        let _ = reader.read_as_text(&file);
        input.set_value("");
    });
    on_id("share", "click", &app, |a, _| a.share());
    let a = app.clone();
    on(&win().into(), "hashchange", move |_| {
        a.borrow_mut().load_hash();
    });

    // background image: object URL only, never uploaded or stored
    on_id("bg-btn", "click", &app, |_, _| el::<HtmlElement>("bg-file").click());
    let a = app.clone();
    on(&el::<EventTarget>("bg-file"), "change", move |_| {
        let input: HtmlInputElement = el("bg-file");
        let Some(file) = input.files().and_then(|f| f.get(0)) else { return };
        let Ok(url) = Url::create_object_url_with_blob(&file) else { return };
        let img = HtmlImageElement::new().unwrap();
        let a2 = a.clone();
        let onload = Closure::once_into_js(move || a2.borrow().render());
        img.set_onload(Some(onload.unchecked_ref()));
        img.set_src(&url);
        if let Some((_, old)) = a.borrow_mut().bg.replace((img, url)) {
            let _ = Url::revoke_object_url(&old);
        }
        input.set_value("");
    });
    on_id("bg-clear", "click", &app, |a, _| {
        if let Some((_, url)) = a.bg.take() {
            let _ = Url::revoke_object_url(&url);
        }
        a.render();
    });
    on_id("bg-opacity", "input", &app, |a, _| {
        a.bg_alpha = num("bg-opacity").unwrap_or(0.5);
        a.render();
    });

    // walk mode: sensor events only do work while a walk is active; nothing is stored or sent
    on_id("walk-start", "click", &app, |a, _| a.walk_start());
    on_id("walk-corner", "click", &app, |a, _| a.with_walk(|w| (w.walk.mark(Mark::Corner), None).1));
    on_id("walk-door", "click", &app, |a, _| a.with_walk(|w| (w.walk.mark(Mark::Door), None).1));
    on_id("walk-close", "click", &app, |a, _| a.walk_close());
    on_id("walk-snap", "click", &app, |a, _| a.with_walk(|w| (w.walk.snap = !w.walk.snap, None).1));
    on_id("walk-undo", "click", &app, |a, _| a.with_walk(|w| (w.walk.undo(), None).1));
    on_id("walk-dbm", "click", &app, |a, _| a.walk_dbm());
    on_id("walk-calib", "click", &app, |a, _| a.walk_calib());
    on_id("walk-apply", "click", &app, |a, _| a.walk_apply());
    on_id("walk-stop", "click", &app, |a, _| {
        a.walk = None;
        a.render_walk();
    });
    on_id("step-len", "change", &app, |a, _| {
        let s = a.step_len();
        if let Some(w) = &mut a.walk {
            w.walk.step_m = s;
        }
    });
    // try_borrow: a sensor event must never panic while a prompt() holds the app borrowed
    let a = app.clone();
    on(&win().into(), "devicemotion", move |e| {
        if let Ok(mut a) = a.try_borrow_mut() {
            a.on_motion(e.unchecked_ref());
        }
    });
    let a = app.clone();
    on(&win().into(), "deviceorientation", move |e| {
        if let Ok(mut a) = a.try_borrow_mut() {
            a.on_orientation(e.unchecked_ref());
        }
    });

    let a = app.clone();
    on(&win().into(), "resize", move |_| a.borrow_mut().resize());
    let a = app.clone();
    on(&win().into(), "keydown", move |e| {
        let e: KeyboardEvent = e.unchecked_into();
        if typing() {
            return;
        }
        let Ok(mut a) = a.try_borrow_mut() else { return };
        let key = e.key().to_lowercase();
        let walking = a.mode3d && a.scene.as_ref().is_some_and(|s| s.mode == CamMode::Walk);
        if walking && !(e.ctrl_key() || e.meta_key()) && key_bit(&key) != 0 {
            a.keys |= key_bit(&key);
            e.prevent_default();
            return;
        }
        if !(e.ctrl_key() || e.meta_key()) {
            return;
        }
        match (key.as_str(), e.shift_key()) {
            ("z", false) => a.undo(),
            ("z", true) | ("y", _) => a.redo(),
            _ => return,
        }
        e.prevent_default();
    });
    let a = app.clone();
    on(&win().into(), "keyup", move |e| {
        let e: KeyboardEvent = e.unchecked_into();
        if let Ok(mut a) = a.try_borrow_mut() {
            a.keys &= !key_bit(&e.key().to_lowercase());
        }
    });
    let a = app.clone();
    on(&win().into(), "blur", move |_| {
        if let Ok(mut a) = a.try_borrow_mut() {
            a.keys = 0;
        }
    });

    // animation loop: coalesced recomputes while dragging, 3D frames, toast timeout
    type Tick = Rc<RefCell<Option<Closure<dyn FnMut(f64)>>>>;
    let tick: Tick = Rc::new(RefCell::new(None));
    let (t2, a) = (tick.clone(), app.clone());
    *tick.borrow_mut() = Some(Closure::new(move |t: f64| {
        if let Ok(mut a) = a.try_borrow_mut() {
            a.frame(t);
        }
        if let Some(cb) = t2.borrow().as_ref() {
            let _ = win().request_animation_frame(cb.as_ref().unchecked_ref());
        }
    }));
    let _ = win().request_animation_frame(tick.borrow().as_ref().unwrap().as_ref().unchecked_ref());
}

#[cfg(test)]
mod tests {
    use super::*;

    const HTML: &str = include_str!("../../../index.html");
    const SRC: &str = include_str!("main.rs");

    fn attr_values<'a>(src: &'a str, attr: &str) -> Vec<&'a str> {
        let pat = format!("{attr}=\"");
        src.match_indices(&pat).map(|(i, _)| {
            let rest = &src[i + pat.len()..];
            &rest[..rest.find('"').unwrap()]
        }).collect()
    }

    #[test]
    fn every_label_has_both_languages() {
        let keys: Vec<&str> =
            attr_values(HTML, "data-i18n").into_iter().chain(attr_values(HTML, "data-i18n-tip")).collect();
        assert!(keys.len() > 60, "{}", keys.len());
        for k in keys {
            let s = STRINGS.iter().find(|s| s.0 == k).unwrap_or_else(|| panic!("missing string {k}"));
            assert!(!s.1.is_empty() && !s.2.is_empty(), "{k}");
        }
        for s in STRINGS {
            assert!(!s.1.is_empty() && !s.2.is_empty(), "{}", s.0);
            assert_eq!(STRINGS.iter().filter(|t| t.0 == s.0).count(), 1, "duplicate {}", s.0);
        }
        for m in Material::ALL {
            assert!(STRINGS.iter().any(|s| s.0 == mat_key(m)));
        }
    }

    #[test]
    fn every_element_id_used_by_the_code_exists() {
        // el() unwraps and release builds abort on panic: a missing id would kill the app.
        let ids = attr_values(HTML, "id");
        let mut used = Vec::new();
        for pat in ["on_id(\"", "el(\"", ">(\"", "set_active(\"", "checked(\"", "num(\"", "select_val(\""] {
            for (i, _) in SRC.match_indices(pat) {
                if pat == ">(\"" && !SRC[i.saturating_sub(40)..i].contains("el::<") {
                    continue;
                }
                let rest = &SRC[i + pat.len()..];
                used.push(&rest[..rest.find('"').unwrap()]);
            }
        }
        assert!(used.len() > 40, "{}", used.len());
        for id in used {
            if id == "{id}" || id.is_empty() || id.contains(' ') {
                continue;
            }
            assert!(ids.contains(&id), "index.html lacks #{id}");
        }
    }

    #[test]
    fn walk_keys() {
        assert_eq!(key_bit("w") | key_bit("arrowright"), 9);
        assert_eq!(key_bit("x"), 0);
    }
}
