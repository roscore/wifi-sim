//! Browser UI: plain web-sys + one <canvas>. Everything runs locally; no network requests.

use std::{cell::RefCell, rc::Rc};
use wasm_bindgen::{prelude::*, Clamped, JsCast};
use web_sys::{
    Blob, BlobPropertyBag, CanvasRenderingContext2d, Document, Element, Event, EventTarget, FileReader,
    HtmlAnchorElement, HtmlCanvasElement, HtmlElement, HtmlImageElement, HtmlInputElement, HtmlSelectElement,
    ImageData, KeyboardEvent, PointerEvent, Url,
};
use wifi_core::{
    calibrate, coverage, score, suggest_placement, Ap, Band, Coverage, Material, Measurement, Plan, Wall,
};

const STORE_KEY: &str = "wifi-sim-plan";
const APARTMENT: &str = include_str!("../../../examples/apartment.json");
const OFFICE: &str = include_str!("../../../examples/office.json");

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
    ("hint_ap", "AP tool: tap an empty spot to add, drag to move. Settings below apply to the selected AP (or the next one you add).", "AP 도구: 빈 곳을 탭하면 추가, 드래그하면 이동합니다. 아래 설정은 선택한 AP(또는 다음에 추가할 AP)에 적용됩니다."),
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
    ("lg_rssi", "≥ -50 excellent|-60 good|-67 fair|-70 weak|-80 poor|≤ -80 dead zone", "≥ -50 매우 좋음|-60 좋음|-67 보통|-70 약함|-80 나쁨|≤ -80 음영 지역"),
    ("lg_sinr", "≥ 25 dB excellent|17 good|11 fair|8 weak|0 poor|≤ 0 unusable", "≥ 25 dB 매우 좋음|17 좋음|11 보통|8 약함|0 나쁨|≤ 0 사용 불가"),
];

/// dBm colour stops; anything at or below -80 is a dead zone.
const STOPS: [(f32, [u8; 3]); 5] = [
    (-50.0, [0, 150, 60]),
    (-60.0, [120, 200, 40]),
    (-67.0, [240, 220, 40]),
    (-70.0, [250, 150, 30]),
    (-80.0, [220, 40, 40]),
];
const DEAD: [u8; 3] = [90, 30, 110];

fn color(dbm: f32) -> [u8; 3] {
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
            return [0, 1, 2].map(|i| (ca[i] as f32 + (cb[i] as f32 - ca[i] as f32) * t) as u8);
        }
    }
    DEAD
}

/// SINR shown on the same colour scale: 25 dB -> -50, 0 dB -> -80.
fn sinr_as_dbm(s: f32) -> f32 {
    -80.0 + s * 1.2
}

fn wall_style(m: Material) -> (&'static str, f64) {
    match m {
        Material::Drywall => ("#8d969e", 3.0),
        Material::WoodDoor => ("#a0522d", 3.0),
        Material::Glass => ("#29b6f6", 3.0),
        Material::Brick => ("#c0392b", 4.0),
        Material::Concrete => ("#37474f", 5.0),
        Material::Metal => ("#111111", 6.0),
        Material::Mirror => ("#90a4ae", 4.0),
    }
}

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

struct App {
    plan: Plan,
    undo: Vec<Plan>,
    redo: Vec<Plan>,
    cov: Coverage,
    tool: Tool,
    ko: bool,
    selected: Option<usize>,
    drag: Option<Drag>,
    hover: Option<(f64, f64)>,
    sinr_view: bool,
    bg: Option<(HtmlImageElement, String)>,
    bg_alpha: f64,
    canvas: HtmlCanvasElement,
    ctx: CanvasRenderingContext2d,
    heat: HtmlCanvasElement,
    heat_ctx: CanvasRenderingContext2d,
    /// canvas pixels per cell
    scale: f64,
    ms: f64,
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

impl App {
    fn tr<'a>(&self, key: &'a str) -> &'a str {
        STRINGS.iter().find(|s| s.0 == key).map(|s| if self.ko { s.2 } else { s.1 }).unwrap_or(key)
    }

    fn msg(&self, s: &str) {
        el::<Element>("msg").set_text_content(Some(s));
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

    fn recompute(&mut self) {
        let perf = win().performance().unwrap();
        let t0 = perf.now();
        self.cov = coverage(&self.plan);
        self.ms = perf.now() - t0;
        self.paint_heat();
        if let Some(s) = storage() {
            let _ = s.set_item(STORE_KEY, &self.plan.to_json());
        }
        self.render_status();
        self.render();
    }

    /// Writes the per-cell colour image into the offscreen 1-pixel-per-cell canvas.
    fn paint_heat(&self) {
        let (w, h) = (self.cov.width, self.cov.height);
        let mut buf = vec![0u8; w * h * 4];
        if !self.plan.aps.is_empty() {
            let vals: Vec<f32> = match (&self.cov.sinr, self.sinr_view) {
                (Some(s), true) => s.iter().map(|&v| sinr_as_dbm(v)).collect(),
                _ => self.cov.best.clone(),
            };
            for (k, v) in vals.iter().enumerate() {
                let c = color(*v);
                buf[k * 4..k * 4 + 4].copy_from_slice(&[c[0], c[1], c[2], 150]);
            }
        }
        self.heat.set_width(w as u32);
        self.heat.set_height(h as u32);
        let img = ImageData::new_with_u8_clamped_array_and_sh(Clamped(&buf), w as u32, h as u32).unwrap();
        self.heat_ctx.put_image_data(&img, 0.0, 0.0).unwrap();
    }

    fn resize(&mut self) {
        let dpr = win().device_pixel_ratio().max(1.0);
        let css_w = el::<HtmlElement>("stage").client_width().max(200) as f64 - 14.0;
        self.scale = (css_w * dpr / self.plan.width as f64).max(1.0);
        self.canvas.set_width((self.plan.width as f64 * self.scale).round() as u32);
        self.canvas.set_height((self.plan.height as f64 * self.scale).round() as u32);
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

    fn render(&self) {
        let c = &self.ctx;
        let (w, h) = (self.plan.width as f64, self.plan.height as f64);
        let s = self.scale;
        let lw = |px: f64| px * win().device_pixel_ratio().max(1.0) / s;
        c.set_transform(1.0, 0.0, 0.0, 1.0, 0.0, 0.0).unwrap();
        c.set_fill_style_str("#ffffff");
        c.fill_rect(0.0, 0.0, self.canvas.width() as f64, self.canvas.height() as f64);
        c.set_transform(s, 0.0, 0.0, s, 0.0, 0.0).unwrap();

        if let Some((img, _)) = &self.bg {
            if img.complete() && img.natural_width() > 0 {
                c.set_global_alpha(self.bg_alpha);
                let _ = c.draw_image_with_html_image_element_and_dw_and_dh(img, 0.0, 0.0, w, h);
                c.set_global_alpha(1.0);
            }
        }
        c.set_image_smoothing_enabled(true);
        let _ = c.draw_image_with_html_canvas_element_and_dw_and_dh(&self.heat, 0.0, 0.0, w, h);

        // 1 m grid
        let step = (1.0 / self.plan.cell_m).round().max(1.0);
        if step * s >= 8.0 {
            c.set_stroke_style_str("rgba(0,0,0,0.08)");
            c.set_line_width(lw(1.0));
            c.begin_path();
            let mut x = 0.0;
            while x <= w {
                c.move_to(x, 0.0);
                c.line_to(x, h);
                x += step;
            }
            let mut y = 0.0;
            while y <= h {
                c.move_to(0.0, y);
                c.line_to(w, y);
                y += step;
            }
            c.stroke();
        }

        c.set_line_cap("round");
        let line = |a: (f64, f64), b: (f64, f64), m: Material| {
            let (col, px) = wall_style(m);
            c.set_stroke_style_str(col);
            c.set_line_width(lw(px));
            c.begin_path();
            c.move_to(a.0, a.1);
            c.line_to(b.0, b.1);
            c.stroke();
        };
        for wl in &self.plan.walls {
            line((wl.x1, wl.y1), (wl.x2, wl.y2), wl.material);
        }
        if let Some(Drag::Wall { start, cur }) = &self.drag {
            c.set_global_alpha(0.6);
            line(*start, *cur, self.material());
            c.set_global_alpha(1.0);
        }

        let font = |px: f64| format!("bold {}px system-ui, sans-serif", lw(px));
        c.set_text_align("center");
        c.set_text_baseline("middle");
        for m in &self.plan.measurements {
            c.set_fill_style_str("#0d47a1");
            c.begin_path();
            let _ = c.arc(m.x, m.y, lw(4.0), 0.0, std::f64::consts::TAU);
            c.fill();
            c.set_font(&font(11.0));
            let _ = c.fill_text(&format!("{:.0}", m.dbm), m.x, m.y - lw(11.0));
        }
        for (i, a) in self.plan.aps.iter().enumerate() {
            let sel = self.selected == Some(i);
            c.set_fill_style_str("#ffffff");
            c.set_stroke_style_str(if sel { "#1565c0" } else { "#1d2329" });
            c.set_line_width(lw(if sel { 4.0 } else { 2.0 }));
            c.begin_path();
            let _ = c.arc(a.x, a.y, lw(11.0), 0.0, std::f64::consts::TAU);
            c.fill();
            c.stroke();
            c.set_fill_style_str("#1d2329");
            c.set_font(&font(12.0));
            let _ = c.fill_text(&(i + 1).to_string(), a.x, a.y);
            c.set_font(&font(10.0));
            let band = ["2.4G", "5G", "6G"][a.band.idx()];
            let _ = c.fill_text(&format!("{band} ch{}", a.channel), a.x, a.y + lw(20.0));
        }
        if let Some(p) = self.hover {
            c.set_stroke_style_str("rgba(0,0,0,0.6)");
            c.set_line_width(lw(1.0));
            c.begin_path();
            let _ = c.arc(p.0, p.1, lw(6.0), 0.0, std::f64::consts::TAU);
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
    }

    fn render_legend(&self) {
        let labels = self.tr(if self.sinr_view { "lg_sinr" } else { "lg_rssi" });
        let cols = STOPS.iter().map(|s| s.1).chain([DEAD]);
        let html: String = labels
            .split('|')
            .zip(cols)
            .map(|(l, c)| format!("<span><i style=\"background:rgb({},{},{})\"></i>{l}</span>", c[0], c[1], c[2]))
            .collect();
        el::<Element>("legend").set_inner_html(&html);
    }

    fn render_materials(&self) {
        let mut html = String::from("<table><tr><th></th><th>2.4</th><th>5</th><th>6 GHz</th></tr>");
        for m in Material::ALL {
            html += &format!("<tr><td>{}</td>", self.tr(mat_key(m)));
            for b in Band::ALL {
                html += &format!(
                    "<td><input type=\"number\" min=\"0\" max=\"80\" step=\"0.5\" data-m=\"{}\" data-b=\"{}\" value=\"{}\"></td>",
                    m.idx(),
                    b.idx(),
                    self.plan.materials.loss(m, b)
                );
            }
            html += "</tr>";
        }
        el::<Element>("materials").set_inner_html(&(html + "</table>"));
    }

    fn apply_lang(&self) {
        doc().document_element().unwrap().set_attribute("lang", if self.ko { "ko" } else { "en" }).unwrap();
        let nodes = doc().query_selector_all("[data-i18n]").unwrap();
        for i in 0..nodes.length() {
            let e: Element = nodes.get(i).unwrap().dyn_into().unwrap();
            let key = e.get_attribute("data-i18n").unwrap();
            e.set_text_content(Some(self.tr(&key)));
        }
        el::<Element>("lang").set_text_content(Some(self.tr("lang")));
        doc().set_title(self.tr("title"));
        self.render_legend();
        self.render_materials();
        self.render_status();
    }

    fn render_status(&self) {
        let p = &self.plan;
        let status = fmt(
            self.tr("status"),
            &[
                ("w", p.width.to_string()),
                ("h", p.height.to_string()),
                ("mw", format!("{:.1}", p.width as f64 * p.cell_m)),
                ("mh", format!("{:.1}", p.height as f64 * p.cell_m)),
                ("ms", format!("{:.0}", self.ms)),
            ],
        );
        el::<Element>("status").set_text_content(Some(&status));
    }

    fn material(&self) -> Material {
        Material::ALL[select_val("material").parse::<usize>().unwrap_or(0).min(6)]
    }

    fn set_tool(&mut self, t: Tool) {
        self.tool = t;
        let nodes = doc().query_selector_all("[data-tool]").unwrap();
        let names = ["wall", "ap", "measure", "erase"];
        for i in 0..nodes.length() {
            let e: Element = nodes.get(i).unwrap().dyn_into().unwrap();
            let on = e.get_attribute("data-tool").as_deref() == Some(names[t as usize]);
            e.class_list().toggle_with_force("active", on).unwrap();
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
            Tool::Measure => {
                let answer = win().prompt_with_message(self.tr("prompt_dbm")).ok().flatten();
                if let Some(dbm) = answer.and_then(|s| s.trim().replace(',', ".").parse::<f64>().ok()) {
                    if dbm.is_finite() && (-120.0..=0.0).contains(&dbm) {
                        self.snapshot();
                        self.plan.measurements.push(Measurement { x: p.0, y: p.1, dbm });
                        self.recompute();
                    }
                }
            }
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
                self.recompute();
            }
            None => self.render(),
        }
        self.render_readout();
    }

    fn pointer_up(&mut self) {
        match self.drag.take() {
            Some(Drag::Wall { start, cur }) if start != cur => {
                self.snapshot();
                let material = self.material();
                self.plan.walls.push(Wall { x1: start.0, y1: start.1, x2: cur.0, y2: cur.1, material });
                self.recompute();
            }
            Some(Drag::Ap { moved: false, .. }) => {
                self.undo.pop(); // tap to select only: nothing changed
                self.render();
            }
            _ => self.render(),
        }
    }

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
        tool: Tool::Wall,
        ko,
        selected: None,
        drag: None,
        hover: None,
        sinr_view: false,
        bg: None,
        bg_alpha: 0.5,
        ctx: ctx2d(&canvas),
        canvas,
        heat_ctx: ctx2d(&heat),
        heat,
        scale: 1.0,
        ms: 0.0,
    };
    let app: Shared = Rc::new(RefCell::new(app));

    {
        let mut a = app.borrow_mut();
        a.set_tool(Tool::Wall);
        a.apply_lang();
        if !a.load_hash() {
            let saved = storage().and_then(|s| s.get_item(STORE_KEY).ok().flatten());
            if let Some(p) = saved.and_then(|s| Plan::from_json(&s).ok()) {
                a.plan = p;
            }
            a.after_plan_swap();
        }
        a.undo.clear();
    }

    // canvas pointer input (mouse, pen and touch)
    let target: EventTarget = app.borrow().canvas.clone().into();
    let a = app.clone();
    on(&target, "pointerdown", move |e| {
        let e: PointerEvent = e.unchecked_into();
        e.prevent_default();
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
        a.render_readout();
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

    let a = app.clone();
    on(&win().into(), "resize", move |_| a.borrow_mut().resize());
    let a = app.clone();
    on(&win().into(), "keydown", move |e| {
        let e: KeyboardEvent = e.unchecked_into();
        let in_field = doc().active_element().is_some_and(|el| el.tag_name() == "INPUT");
        if in_field || !(e.ctrl_key() || e.meta_key()) {
            return;
        }
        match (e.key().to_lowercase().as_str(), e.shift_key()) {
            ("z", false) => a.borrow_mut().undo(),
            ("z", true) | ("y", _) => a.borrow_mut().redo(),
            _ => return,
        }
        e.prevent_default();
    });
}
