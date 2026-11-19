# Task: wifi-sim UI overhaul — game-grade look and a real 3D view

The tool works (wifi_core model, walk mode, AP suggestion) but the page looks like a plain form.
Make it look and feel like a polished simulation app. Keep every existing feature and all
wifi_core tests green; this is a presentation/interaction task, not a model change.
Original code only. No external CDNs, fonts, analytics or runtime network requests (CSP stays).

## Look
- Dark, modern app shell: full-viewport canvas as the hero, tools in a slim floating left toolbar
  (icon + tooltip: wall, AP, measure, eraser, undo/redo, clear), settings in a collapsible right panel
  with grouped cards (Access points, Materials, View, Floor plan, Walk mode). Mobile: bottom sheet.
- Consistent design tokens (CSS variables): background, surface, accent, text, radius, shadow.
  System font stack only. Smooth 150–200 ms transitions. Visible focus states.
- Signal legend as a slim gradient bar with labels, overlaid on the canvas corner.
- HUD overlay on the canvas: cursor readout (x, y, dBm, best AP, SINR), compute time.
- Wall materials as colored chips with a small swatch, current one highlighted.
- Empty/first-run: load the example plan automatically and show a short dismissible hint.

## 3D view (WebGL2, reuse `gl.rs` + `math.rs` copied into `crates/wifi_web/src/` — your own MIT code
from a sibling tool: shaded meshes, directional light, shadow map, orbit/pan/zoom camera)
- Toggle 2D / 3D. In 3D: floor plane textured with the live heatmap (upload the dBm grid as a
  texture, colour-mapped in the shader, smooth bilinear), walls extruded (height ~2.4 m, material
  colour, glass translucent), APs as glowing devices on small stands with a soft pulsing ring.
- Direct manipulation in 3D: drag an AP across the floor (ray–plane pick) and the heatmap
  recomputes live (debounce if needed); hover shows the dBm readout at the floor point.
- Camera presets: perspective, top, walk-through (first person at 1.6 m, WASD/arrow + drag look;
  touch: one-finger look, two-finger move).
- Optional "signal rays" toggle: draw a few lines from the selected AP to the cursor point, colouring
  wall crossings with their loss.
- Fallback: if WebGL2 fails, stay in 2D and show a small notice (2D must still render).

## 2D view
- Keep the editor, but render nicer: antialiased walls with material colours and thickness,
  subtle grid, heatmap with smooth interpolation and optional contour lines at the legend levels,
  AP icons, measurement line with label.

## Quality bar
- 60 fps on a laptop for the default 12×9 m plan in 3D.
- Korean / English toggle still works for every new label.
- `cargo test --workspace`, `trunk build --release`, `sh tools/privacy_check.sh` all pass.
- Add tests for the new pure helpers (colour map, ray–plane pick, grid→texture packing).
- Update README (ko/en) screenshots section with a text description only (no image files needed).

## Delivery
Branch `ui-revamp`, one PR to main titled "UI overhaul: app shell, WebGL2 3D view, live AP drag".
Do not commit this TASK-UI.md to main (it lives on the branch only).
