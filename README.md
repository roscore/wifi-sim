# wifi-sim — 집 Wi-Fi 커버리지 시뮬레이터 / Home Wi-Fi coverage simulator

> **추정치일 뿐입니다.** 정확도를 보장하지 않습니다. / **Estimate only.** No accuracy claims.

**▶ 바로 써 보기 / Try it now: https://roscore.github.io/wifi-sim/**  
설치 없이 브라우저에서 바로 실행돼요. 휴대폰에서도 열 수 있어요. / Runs in the browser, no install, works on phones.

[한국어](#한국어) · [English](#english)

---

## 한국어

### 무엇인가요?
브라우저에서 평면도를 그리고 공유기(AP)·메시 노드를 놓으면, 집 안 Wi-Fi 신호 세기(dBm)를
히트맵으로 보여 주는 정적 웹 앱입니다. Rust로 작성해 WebAssembly로 컴파일했고, **모든 계산은
브라우저 안에서만** 이루어집니다. 서버, 분석 도구, 추적기, 외부 CDN/폰트가 없으며 실행 중
네트워크 요청을 하지 않습니다 (Content-Security-Policy로도 막혀 있습니다).

### 화면 구성 (스크린샷 설명)
- **2D 화면**: 어두운 앱 화면 가운데에 평면도 캔버스가 꽉 차게 놓입니다. 초록→노랑→주황→빨강→보라
  히트맵이 부드럽게 보간되어 깔리고, 범례 기준값(-50/-60/-67/-70/-80 dBm)에 흰색 등고선이 그려집니다.
  벽은 재질 색과 실제 두께로, AP는 빛나는 원형 아이콘(번호·대역·채널 라벨)으로 표시됩니다. 커서를 올리면
  담당 AP까지의 점선과 거리(m) 라벨이, 벽을 그리는 동안에는 길이 라벨이 뜹니다.
- **도구 막대**(왼쪽, 아이콘 + 툴팁): 선택, 방, 벽, AP, 실측, 지우개, 실행 취소/다시 실행, 모두 지우기,
  시작 카드 다시 열기.
- **설정 패널**(오른쪽, 접기 가능한 카드): AP · 재질(색 칩 + 감쇠 표) · 보기 · 평면도 · 걷기 모드.
  휴대폰에서는 아래에서 끌어올리는 시트가 됩니다.
- **HUD**(왼쪽 아래): 커서 위치(x, y), dBm, 담당 AP, SINR, 계산 시간(3D에서는 fps 포함).
  **범례**(오른쪽 아래): 얇은 그라데이션 막대와 눈금.
- **3D 화면**(위쪽 2D/3D 전환): 히트맵이 바닥 텍스처로 깔리고(셰이더에서 색 변환, 부드러운 보간),
  벽은 2.4 m 높이로 세워집니다(재질 색, 유리는 반투명, 바닥에 그림자). AP는 받침대 위의 빛나는 기기와
  바닥에서 퍼지는 고리로 보입니다. 3D에서도 AP를 바닥 위로 드래그하면 히트맵이 실시간으로 다시 계산되고,
  커서를 올리면 그 바닥 지점의 dBm이 표시됩니다.
- **카메라**: 원근(드래그 회전, 우클릭/Shift 드래그 이동, 휠·핀치 확대), 위에서, 걸어 보기(1.6 m 눈높이,
  WASD/화살표 + 드래그로 둘러보기, 터치는 한 손가락 보기·두 손가락 이동).
- **신호 광선**(선택): 선택한 AP에서 커서까지 직선을 그리고, 벽을 지날 때마다 그 벽의 감쇠(dB)를 색으로
  표시합니다(HUD에 벽 개수·합계 dB·재질 목록).
- WebGL2를 쓸 수 없는 브라우저에서는 2D로 남고 작은 안내가 뜹니다. 처음 열면 **시작 카드**가 뜹니다:
  (1) 우리 집 그리기(빈 평면도, 크기 m 입력, 기본 12×9 m), (2) 평면도 이미지 따라 그리기, (3) 걸어서
  그리기(휴대폰), (4) 예제 불러오기. 도구 막대의 집 아이콘으로 언제든 다시 열 수 있습니다.

### 그리는 방법
- **방 도구**(빈 평면도의 기본 도구): 사각형을 드래그하면 현재 재질의 벽 4개가 모서리를 공유하며
  만들어집니다. Shift(터치는 두 번째 손가락)를 누르면 정사각형.
- **벽 도구 = 꺾은선**: 클릭으로 시작하고, 클릭할 때마다 이어진 벽이 하나씩 추가됩니다. 더블클릭, Enter,
  또는 시작점 클릭으로 끝내고, Esc는 지금 그리는 구간만 취소합니다. 터치는 점을 탭하고 "완료"를 누릅니다.
  - 그리는 동안 길이(m)가 실시간으로 표시되고, 각도는 0/45/90°로 스냅됩니다(Alt를 누르면 해제).
  - 기존 벽의 끝점(■)과 벽 위(◆)에 약 0.3 m 반경으로 자석처럼 붙습니다. 상단 바에서 0.1 m 격자 스냅을
    켤 수 있습니다.
  - 구간을 그리는 중에 숫자를 입력하면 그 길이(m)로 정확히 놓입니다(Enter로 확정).
- **선택 도구**(그리기를 끝내면 자동으로 전환): 벽을 클릭하면 강조와 끝점 핸들이 보입니다.
  가운데를 끌면 이동, 끝점을 끌면 모양 변경. 끝점이 겹친 곳은 **접합부**라서 함께 움직이므로 방이 닫힌
  채로 유지됩니다(Alt 드래그는 떼어내기). 빈 곳을 드래그하면 여러 벽을 상자 선택합니다.
- **선택한 벽 패널**: 재질 칩, 정확한 길이(m) 입력, 두께 안내, 삭제, "여기서 나누기"(접합부 추가),
  "여기에 문 / 창문"(벽을 0.9 m / 1.2 m 잘라 나무 문 / 유리 구간을 넣음, 전체 길이는 그대로). 여러 개를
  선택하면 재질 변경·이동·복제·삭제를 한 번에 합니다.
- **키보드**: Delete/Backspace 삭제, 화살표 0.1 m 이동(Shift 0.5 m), Ctrl+Z / Ctrl+Y 실행 취소/다시 실행,
  Ctrl+D 복제, Esc 취소/선택 해제. 모든 편집은 실행 취소 한 단계입니다.
- 커서 모양이 할 일을 알려 줍니다(이동 ✥, 모양 변경 ✋, 추가 +). 3D에서도 선택 도구로 벽을 클릭해
  선택하고 바닥을 따라 끌어 옮길 수 있습니다(모서리 변경·상자 선택은 2D에서).
- **이미지 따라 그리기**: 시작 카드에서 이미지를 고르면 그 비율의 빈 평면도가 만들어지고, 실제 거리를
  아는 두 점을 클릭한 뒤 거리(m)를 입력하면 축척이 맞춰집니다. "배경 이미지"의 "축척 맞추기"로 다시 할 수
  있습니다.
- 벽은 계속 단순한 선분으로 저장되므로 예전에 저장한 평면도와 공유 링크도 그대로 열립니다.

### 사용법
1. **방**·**벽** 도구로 벽을 그리고 **선택** 도구로 고칩니다(위 "그리는 방법"). 재질(석고보드, 나무 문,
   유리/창문, 벽돌, 콘크리트/내력벽, 금속/엘리베이터, 거울)은 "재질" 카드의 색 칩으로 고르거나, 선택한 벽
   패널에서 바꿉니다. 문·창문은 선택한 벽의 "여기에 문 / 창문"으로 넣습니다.
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
cargo test --workspace
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

### What it looks like (screenshot description)
- **2D view**: a dark app shell with the floor plan canvas as the hero. The green → yellow → orange →
  red → purple heatmap is smoothly interpolated, with white contour lines at the legend levels
  (-50 / -60 / -67 / -70 / -80 dBm). Walls are drawn in their material colour at their physical
  thickness, APs as glowing round icons labelled with number, band and channel. Hovering shows a dashed
  guide to the serving AP with a distance label; drawing a wall shows its length.
- **Toolbar** (left, icons with tooltips): select, room, wall, AP, measure, eraser, undo / redo, clear,
  and a home button that reopens the Start card.
- **Settings panel** (right, collapsible cards): Access points, Materials (colour chips + attenuation
  table), View, Floor plan, Walk mode. On phones it becomes a bottom sheet.
- **HUD** (bottom left): cursor x, y, dBm, best AP, SINR and compute time (plus fps in 3D).
  **Legend** (bottom right): slim gradient bar with ticks.
- **3D view** (2D / 3D toggle at the top): the live heatmap is the floor texture (colour-mapped in the
  shader, bilinear), walls are extruded to 2.4 m in material colours (glass translucent) and cast
  shadows, APs are glowing devices on small stands with a soft pulsing ring. Drag an AP across the
  floor and the heatmap recomputes live; hovering shows the dBm at that floor point.
- **Cameras**: perspective (drag to orbit, right / Shift drag to pan, wheel or pinch to zoom), top, and
  walk-through (eye height 1.6 m, WASD / arrows + drag to look; touch: one finger looks, two move).
- **Signal rays** (optional): a line from the selected AP to the cursor; every wall it crosses is
  marked and coloured by its loss (the HUD lists the count, total dB and materials).
- Without WebGL2 the app stays in 2D and shows a small notice. On first run a **Start card** opens:
  (1) Draw my home (empty plan, size in metres, default 12 × 9 m), (2) Trace a floor-plan image,
  (3) Walk mode (phone), (4) Load an example. The home button in the toolbar reopens it.

### How to draw
- **Room tool** (default for empty plans): drag a rectangle to get 4 walls of the current material that
  share their corners. Hold Shift (or put a second finger down) for a square.
- **Wall tool = polyline**: click to start, every click adds a joined segment. Double-click, Enter or a
  click on the start point finishes; Esc cancels the current segment. On touch: tap points, then *Done*.
  - While drawing you see the live length in metres; the angle snaps to 0 / 45 / 90° (hold Alt to
    disable).
  - Endpoints (■) and walls (◆) act as magnets within about 0.3 m. An optional 0.1 m grid snap is in the
    drawing bar at the top.
  - Type a number while a segment is being drawn to give it that exact length (Enter confirms).
- **Select tool** (also where you land after finishing a drawing): click a wall to select it (highlight +
  handles). Drag the middle to move it, drag an endpoint handle to reshape it. Coinciding endpoints form a
  **joint**: dragging a joint moves all connected walls, so rooms stay closed (Alt-drag detaches).
  Drag on empty space to box-select several walls.
- **Selected wall panel**: material chips, exact length (m), a thickness hint, Delete, *Split here*
  (adds a joint) and *Add door / window here* (cuts a 0.9 m / 1.2 m opening and fills it with a wooden
  door / glass segment; the total length is unchanged). With several walls selected: change material,
  move, duplicate or delete them together.
- **Keyboard**: Delete / Backspace removes the selection, arrows nudge by 0.1 m (Shift 0.5 m),
  Ctrl+Z / Ctrl+Y undo / redo, Ctrl+D duplicates, Esc cancels or deselects. Every edit is one undo step.
- The cursor tells you what a drag will do (move, reshape, add). In 3D the Select tool picks walls too
  (ray against the wall boxes) and drags them along the floor; reshaping and box select are 2D only.
- **Tracing an image**: pick it from the Start card; you get an empty plan with the image's aspect ratio,
  then click two points whose real distance you know and type it in metres to set the scale. *Set scale*
  in the Background image card repeats this.
- Walls are still stored as plain segments, so older saved plans and share links keep working.

### How to use
1. Draw walls with the **Room** and **Wall** tools and edit them with **Select** (see "How to draw").
   Pick the material (drywall/partition, wooden door, glass/window, brick, concrete/load-bearing,
   metal/elevator, mirror) with the colour chips in the Materials card, or change it in the selected-wall
   panel. Add doors and windows with *Add door / window here* on a selected wall.
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
cargo test --workspace
trunk build --release        # writes dist/ (public_url = /wifi-sim/)
trunk serve --public-url /   # run locally at http://localhost:8080
sh tools/privacy_check.sh    # scan for personal-data patterns
```
Pushing to `main` builds and deploys to GitHub Pages via `.github/workflows/pages.yml`.

### License
MIT — see [LICENSE](LICENSE) (Copyright wifi-sim contributors).
