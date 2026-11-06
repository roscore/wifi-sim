# wifi-sim — 집 Wi-Fi 커버리지 시뮬레이터 / Home Wi-Fi coverage simulator

> **추정치일 뿐입니다.** 정확도를 보장하지 않습니다. / **Estimate only.** No accuracy claims.

[한국어](#한국어) · [English](#english)

---

## 한국어

### 무엇인가요?
브라우저에서 평면도를 그리고 공유기(AP)·메시 노드를 놓으면, 집 안 Wi-Fi 신호 세기(dBm)를
히트맵으로 보여 주는 정적 웹 앱입니다. Rust로 작성해 WebAssembly로 컴파일했고, **모든 계산은
브라우저 안에서만** 이루어집니다. 서버, 분석 도구, 추적기, 외부 CDN/폰트가 없으며 실행 중
네트워크 요청을 하지 않습니다 (Content-Security-Policy로도 막혀 있습니다).

*스크린샷 자리: 샘플 2룸 아파트 평면 위에 초록–노랑–빨강 히트맵이 표시된 화면.*

### 사용법
1. **벽** 도구로 드래그해 벽을 그립니다. 재질(석고보드, 나무 문, 유리/창문, 벽돌, 콘크리트/내력벽,
   금속/엘리베이터, 거울)을 먼저 고르세요. 거의 수평/수직인 선은 자동으로 맞춰집니다.
   문틈·개구부는 벽을 비워 두거나 "나무 문" 재질로 그리면 됩니다.
2. **AP** 도구로 빈 곳을 탭하면 AP가 추가되고, 드래그하면 이동합니다. 대역(2.4/5/6 GHz),
   송신 출력(dBm), 안테나 이득(dBi), 채널은 선택한 AP에 적용됩니다.
3. 히트맵 범례: ≥ -50 매우 좋음 … ≤ -80 음영 지역. 마우스를 올리면(터치는 드래그) 해당 위치의
   신호·담당 AP·SINR이 표시됩니다. "보기"에서 동일 채널 AP 간 **SINR**을 볼 수 있습니다.
4. **AP 위치 추천**: 선택한 AP(없으면 1번)를 격자 탐색으로 옮겨 신호 중앙값은 높이고 -70 dBm 미만
   면적은 줄이는 위치를 찾습니다 (다른 AP는 고정).
5. **보정**(선택): **실측** 도구로 위치를 탭하고 실제로 측정한 dBm을 입력한 뒤 "맞추기"를 누르면
   경로손실 지수 n과 오프셋을 최소제곱으로 맞추고 RMSE(전 → 후)를 보여 줍니다.
6. 벽 감쇠 표(dB)는 재질·대역별로 직접 고칠 수 있으며 평면도와 함께 저장됩니다.
7. **파일·공유**: JSON 내보내기/가져오기, "공유 링크"는 평면도를 압축해 URL의 `#plan=...`
   프래그먼트에 넣습니다. 프래그먼트는 브라우저가 서버로 보내지 않습니다.
8. **배경 이미지**: 평면도 이미지를 깔고 불투명도를 조절할 수 있습니다. 이미지는 기기 안에서만
   (object URL) 쓰이며 업로드되거나 파일/링크에 저장되지 않습니다.
9. 실행 취소/다시 실행(Ctrl+Z / Ctrl+Shift+Z, Ctrl+Y), 지우개, 모두 지우기. 한국어/English 전환
   (기본값은 브라우저 언어). 작업 중인 평면도는 이 브라우저의 localStorage에만 저장됩니다.

예제(`examples/`)는 모두 가상의 평면입니다: 샘플 2룸 아파트, 샘플 사무실.

### 걸어서 도면 그리기 (Walk mode)
휴대폰의 모션 센서(가속도계 + 방향 센서)로 보행자 추측 항법(PDR)을 해서 벽을 만듭니다.
1. HTTPS로 연 페이지에서 **시작**을 누릅니다 (iOS는 모션 센서 권한을 물어봅니다).
   필요하면 **보폭 보정**: 길이를 아는 벽을 따라 걷고 다시 눌러 길이(m)를 입력하면 보폭 = 길이 ÷ 걸음 수.
   기본 보폭은 0.7 m입니다.
2. 방 모서리에 서서 **모서리**를 탭하고, 휴대폰을 앞을 향해 평평하게 든 채 벽을 따라 **천천히** 걷습니다.
   모서리마다 **모서리**, 문 위치에서 **문**을 탭합니다 (문 위치에는 0.9 m 틈이 남습니다).
3. 출발점에 돌아오면 **루프 닫기**: 누적 오차(출발점과의 차이)를 벽들에 나눠 보정합니다.
   **직각 맞추기**는 모든 벽을 첫 벽 기준 0/90/180/270°로 맞춥니다 (대부분의 집은 직각). 마지막 벽 방향이
   15° 넘게 어긋나면 방향 오차 경고가 뜹니다.
4. 다음 방은 이어지는 문 위치에서 **문**을 탭해 시작하고 같은 벽을 따라 계속 걸으세요. 가장 가까운 기존 문에
   맞춰 붙습니다.
5. 선택: 모서리/문에서 **여기 dBm**으로 Wi-Fi 분석 앱에서 읽은 dBm을 입력하면 보정용 측정점이 됩니다.
   **브라우저는 Wi-Fi 신호 세기를 읽을 수 없습니다.**
6. **평면도로 사용**: 가장 큰 닫힌 루프는 바깥 벽(콘크리트), 나머지는 석고보드로 변환됩니다. 이후 일반
   편집 도구로 고치세요 (실행 취소 가능, 기존 AP는 지워집니다).

개인정보: GPS/위치 정보는 쓰지 않으며 네트워크 요청도 없습니다. 센서 값은 메모리에서 바로 처리되고 버려지며,
모서리/문 좌표와 결과 벽만 남습니다 (일반 평면도와 같은 localStorage/내보내기).

정확도 한계: **±5–10% 정도를 예상하세요.** 벽을 따라 천천히, 일정한 보폭으로 걸을 때 가장 잘 됩니다.
보폭 차이(뛰기, 짧은 걸음), 휴대폰을 돌리거나 흔드는 것, 철골·가전 근처의 방향 센서 교란이 오차를 키웁니다.
경로는 벽에서 몸 너비만큼 떨어져 있으니 방이 약간 작게 나오고, 붙어 있는 두 방의 공유 벽은 두 줄로 생길 수
있습니다. 루프 닫기는 출발점과의 차이만 없애며 축척(보폭) 오차는 고치지 못합니다.

### 모델 설명
- **경로 손실**: 로그-거리 모델 `PL(d) = FSPL(1 m, f) + 10·n·log10(d)` (n = 2이면 자유공간,
  ITU-R P.1238 형태). 대역 중심 주파수 2437 / 5250 / 6105 MHz.
- **다중 벽 모델 (COST 231 MWM)**: AP→지점 직선이 가로지르는 벽마다 재질·대역별 감쇠를 더합니다.
- **지배 경로(Dominant Path) 휴리스틱**: 격자(8방향) 위에서 칸 사이를 지날 때 벽 감쇠를 비용으로 하는
  다익스트라로 문틈·개구부를 돌아가는 경로를 찾고, 그 경로의 실제 길이로 로그-거리 손실을 다시
  계산합니다. 각 칸은 직선 MWM과 지배 경로 중 더 강한 값을 씁니다.
- **여러 AP**: 칸마다 가장 강한 RSSI. SINR = 신호 − 10·log10(같은 대역·채널 다른 AP 전력 합 + 잡음
  -95 dBm).
- **보정**: 측정값 = A − n·10·log10(d) + 오프셋 이 (n, 오프셋)에 대해 선형이므로 닫힌 형태 최소제곱
  (측정점 1개면 오프셋만). 직선 MWM 항으로만 맞춥니다.

### 한계
반사·회절 손실·다중 경로·가구·사람·층간 전파·안테나 방향성·기기 차이를 모델링하지 않습니다.
벽 감쇠 기본값은 대략적인 일반값입니다. 격자 경로는 실제보다 약간 길고(최대 약 8%), 문 모서리를 도는
회절 손실이 없어 돌아가는 경로는 낙관적으로 나올 수 있습니다. 결과는 **배치 감을 잡는 용도**로만
쓰세요.

### 로컬 빌드
```sh
rustup target add wasm32-unknown-unknown
cargo install trunk --locked
cargo test -p wifi_core
trunk build --release        # dist/ 생성 (public_url = /wifi-sim/)
trunk serve --public-url /   # 로컬 실행: http://localhost:8080
sh tools/privacy_check.sh    # 개인정보 패턴 검사
```
`main` 에 푸시하면 GitHub Actions(`.github/workflows/pages.yml`)가 빌드해 GitHub Pages에 배포합니다.

### 라이선스
MIT — [LICENSE](LICENSE) 참고 (Copyright wifi-sim contributors).

---

## English

### What is it?
A static single-page web app: draw a floor plan, place routers / mesh nodes, and see an estimated
Wi-Fi signal (dBm) heatmap. Written in Rust, compiled to WebAssembly, and **computed entirely in your
browser**. No server, analytics, trackers, external CDNs or fonts, and no network requests at runtime
(also enforced by a Content-Security-Policy).

*Screenshot placeholder: a sample 2-room apartment plan with a green–yellow–red heatmap.*

### How to use
1. **Wall** tool: drag to draw a wall. Pick a material first (drywall/partition, wooden door,
   glass/window, brick, concrete/load-bearing, metal/elevator, mirror). Near-horizontal/vertical lines
   snap. Leave a gap (or draw a wooden door) for doors and openings.
2. **AP** tool: tap an empty spot to add an AP, drag to move it. Band (2.4 / 5 / 6 GHz), TX power (dBm),
   antenna gain (dBi) and channel apply to the selected AP.
3. Legend: ≥ -50 excellent … ≤ -80 dead zone. Hover (or drag on touch) to read signal, serving AP and
   SINR at a point. Switch **View** to SINR for same-channel interference.
4. **Suggest AP placement**: grid search that moves the selected AP (or AP 1), others fixed, to raise
   the median signal and shrink the area below -70 dBm.
5. **Calibration** (optional): with the **Measure** tool tap a spot and type the dBm you measured; press
   *Fit* to least-squares fit the path-loss exponent n and an offset; RMSE before → after is shown.
6. The wall attenuation table (dB per material and band) is editable and saved with the plan.
7. **File & share**: export/import JSON. *Share link* packs the plan (deflate + base64url) into the
   `#plan=...` URL fragment, which browsers never send to a server.
8. **Background image**: load a floor plan picture locally (object URL) with an opacity slider. It is
   never uploaded and not stored in files or links.
9. Undo/redo (Ctrl+Z / Ctrl+Shift+Z, Ctrl+Y), eraser, clear. Korean/English toggle (defaults to the
   browser language). The current plan is kept only in this browser's localStorage.

All examples in `examples/` are synthetic: a sample 2-room apartment and a sample office.

### Walk mode (draw the plan by walking)
Pedestrian dead reckoning (PDR) from the phone's motion sensors (accelerometer + orientation).
1. Open the page over HTTPS and press **Start** (iOS asks for motion-sensor permission). Optionally
   **Calibrate step**: walk along a wall of known length, tap again and type the length; step length =
   length ÷ steps. Default step length is 0.7 m.
2. Stand in a corner, tap **Corner**, then walk **slowly** along the wall holding the phone flat and pointing
   forward. Tap **Corner** at every corner and **Door** in doorways (a 0.9 m gap is left there).
3. Back at the start, tap **Close loop**: the accumulated drift (distance from the start point) is spread
   over the walls. **Straighten** snaps every wall to 0/90/180/270° relative to the first one (most homes
   are rectilinear). A heading-drift warning appears when the last wall is more than 15° off.
4. Start the next room by tapping **Door** in the shared doorway, then walk on along that same wall; it is
   joined to the nearest earlier door.
5. Optional: **dBm here** at a corner/door stores a dBm you read from a Wi-Fi analyzer app as a calibration
   point. **Browsers cannot read Wi-Fi signal strength.**
6. **Use as floor plan**: the largest closed loop becomes the outer wall (concrete), the rest drywall. Edit
   the result with the normal tools (undoable; existing APs are removed).

Privacy: no GPS / Geolocation and no network requests. Sensor samples are processed in memory and
discarded; only corner/door positions and the resulting walls are kept (same localStorage/export as the
editor).

Accuracy limits: **expect ±5–10 %.** Works best walking slowly and evenly along walls. Uneven steps,
turning or shaking the phone, and compass disturbance near steel or appliances make it worse. The path
runs a body-width away from the wall, so rooms come out slightly small and a wall shared by two walked
rooms may appear twice. Loop closure removes the end-point gap only; it cannot fix a wrong step length
(scale).

### Model
- **Path loss**: log-distance `PL(d) = FSPL(1 m, f) + 10·n·log10(d)` (n = 2 is free space; ITU-R
  P.1238-style). Band centre frequencies 2437 / 5250 / 6105 MHz.
- **Multi-wall model (COST 231 MWM)**: sum of per-material, per-band losses of every wall crossed by
  the straight AP→point line.
- **Dominant-path heuristic**: Dijkstra on the 8-connected grid where each step costs distance plus
  the loss of any wall it crosses, so signal can route through door gaps and openings; the found
  path's real length is then fed back into the log-distance law. Each cell takes the stronger of
  direct MWM and dominant path.
- **Multiple APs**: strongest RSSI per cell. SINR = signal − 10·log10(sum of other APs on the same band
  and channel + noise floor of -95 dBm).
- **Calibration**: measured = A − n·10·log10(d) + offset is linear in (n, offset), so a closed-form
  least-squares fit (offset only with a single point). Fitted on the direct MWM terms.

Performance: a 200×200 grid with 30 walls recomputes in ~30 ms per AP natively (release build, x86-64);
WebAssembly is typically somewhat slower, and the app shows the actual compute time under the canvas.

### Limitations
No reflections, diffraction loss, multipath, furniture, people, floors/ceilings, antenna patterns or
client device differences. Default wall losses are rough typical values. Grid paths are slightly
longer than real ones (up to ~8 %), and bending around door edges carries no diffraction penalty, so
detour paths can be optimistic. Use the result to get a feel for placement, not as a measurement.

### Local build
```sh
rustup target add wasm32-unknown-unknown
cargo install trunk --locked
cargo test -p wifi_core
trunk build --release        # writes dist/ (public_url = /wifi-sim/)
trunk serve --public-url /   # run locally at http://localhost:8080
sh tools/privacy_check.sh    # scan for personal-data patterns
```
Pushing to `main` builds and deploys to GitHub Pages via `.github/workflows/pages.yml`.

### License
MIT — see [LICENSE](LICENSE) (Copyright wifi-sim contributors).
