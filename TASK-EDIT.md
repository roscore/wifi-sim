# Task: make walls easy to create and edit (wifi-sim, branch ui-revamp)

Feedback from the first real user: "a new user starts with NO walls, and the current wall editing is
far too hard". Today a wall is one drag = one segment; the only edit is the eraser. Fix the editing
model. Keep every existing feature and all tests green. Original code only, no external libs/CDNs.

## Start
- First run opens a "Start" card instead of silently loading the sample apartment:
  (1) Draw my home (empty plan, size field in metres, defaults 12×9 m),
  (2) Trace a floor-plan image (opens the existing image picker, then a 2-click scale calibration:
      click two points, type the real distance in m),
  (3) Walk mode (phone), (4) Load an example. The card can be reopened from the toolbar.

## Drawing
- **Room tool** (new, default for empty plans): drag a rectangle → 4 walls of the current material,
  sharing corners. Shift/second finger = square.
- **Wall tool = polyline**: click to start, each click adds a joined segment, double-click / Enter /
  click the start point to finish, Esc to cancel the current segment. Touch: tap points, "Done" button.
- While drawing: live length label in metres, angle snap (0/45/90°, hold Alt to disable), endpoint and
  on-wall snapping (magnet radius ~0.3 m, show a snap marker), optional grid snap 0.1 m.
- Typing a number while a segment is being drawn sets its exact length (Enter to confirm).

## Editing (new Select tool, also the default after finishing a drawing)
- Click a wall to select it (highlight + handles). Drag the middle to move it, drag an endpoint handle
  to reshape. Endpoints that coincide are a JOINT: dragging a joint moves all connected walls together
  (so rooms stay closed). Alt-drag detaches.
- Selected wall panel: material chips, exact length (m) editable, thickness hint, Delete,
  "Split here" (adds a joint), "Add door/window here" (cuts an opening of 0.9 m / 1.2 m and inserts a
  wooden-door / glass segment).
- Box select (drag on empty space) for multiple walls → delete / change material / move together.
- Keyboard: Delete/Backspace removes selection, arrows nudge 0.1 m (Shift 0.5 m), Ctrl+Z/Y undo/redo,
  Ctrl+D duplicate. Every edit is one undo step.
- Hover cursor tells what will happen (move, reshape, add). Selection works the same in 3D view
  (ray pick on wall boxes; drag along the floor plane).

## Model
- Keep `wifi_core::Wall` as segments (the propagation model does not change). Put joint detection,
  snapping, polyline/room construction, opening insertion and box-select hit tests in a new pure
  module `wifi_core::edit` with unit tests (joint drag keeps connected endpoints equal; opening split
  conserves total length; snap finds the nearest endpoint within radius; room = 4 closed walls).
- Saved plans / share links stay backward compatible.

## Delivery
Commit on the existing branch `ui-revamp` and push; PR #1 updates. Run `cargo test --workspace`,
`trunk build --release`, `sh tools/privacy_check.sh`. Update README (ko/en) "How to draw" section.
Do not commit this file to main.
