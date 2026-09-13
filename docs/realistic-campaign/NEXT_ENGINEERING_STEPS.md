# Senior review: the next implementation steps

The user has rejected the live graphics twice and now explicitly asks Astra to give Terra senior-level implementation guidance. Follow this short execution brief before the larger roadmap. It narrows the next milestone, not the complete authorized goal. Do not discard existing work or start a new design document instead of implementing.

## What the current evidence says

- `rejected-horde-runtime-20260911.png` shows the battle and rail occupying the left portion of a much wider screen, a large unused right area, repeated low-detail blue enemies, tiny old cards, bright large range rings and overlapping damage text.
- The server on 8091 currently serves build `24d033c718ece8c9`; the exact build in the user's already-open tab is unknown. A source change does not change an already-running WASM module. Diagnose this without blaming every graphical failure on caching and without discarding the user's current run.
- Recent work now adds original OBJ replacements for Seed/Siege stages, Warrior/Brute and some scenery. That is useful progress, but it is only part of the roster and has not established the requested visual quality. The current original bake uses `bake_triangles(tris, tris, ...)`, so pose deltas are zero; exporting a mesh has not supplied walking animation.
- The existing packed model pipeline mainly preserves vertex color and normal. Tower drawing uses a whole-object material. Stone, wood and metal therefore cannot acquire correct individual physical behavior merely by assigning them different RGB values. UV/material/animation information discarded by the bake cannot be recovered by extra ground texture noise.
- `ui::board_text` still loops through the queued texts without a screen-space budget. This directly explains the unreadable overlapping numbers. Existing tower/model draw files still need the full work described in the brief.

## Next deliverable: one real browser scene worth reviewing

Produce a reproducible, live Rust/WASM scene with a correctly fitted viewport, a realistic ground/road/edge treatment, two properly made tower families, two properly made enemy types, readable combat and correct UI icons. Show an actual browser capture at 1920x1080 and at the supplied screenshot's 2048x940 size. This is the pipeline proof for the whole game, not permission to stop with a demo.

### 1. Establish which build is running

Record served URL, document viewport, CSS canvas rectangle, canvas backing dimensions, browser DPR, render-target dimensions, selected graphics backend/preset and build identity in diagnostic output. Test a fresh isolated browser session on the exact current dist build; leave the user's active session untouched. If a server points to the wrong folder or an old tab needs reloading, make that distinction explicit. HTTP 200 is not rendering evidence.

Do not leave multiple hung browser smoke processes or listeners competing on the same debug port. Use a unique temporary browser profile/port and bounded startup/capture timeouts, then clean up only the processes started for that check. Investigate a timeout instead of counting it as a pass or waiting indefinitely.

### 2. Fix the live layout before cosmetic detail

Use one root viewport as the source for HUD, board, rail/drawer and picking. Remove stale absolute sizes and independent panel origins. Current `desktop_stage` source centers the composition, so reproduce why the supplied image is left-aligned: compare loaded build, actual egui root rectangle, CSS size, render viewport and resize behavior. Do not guess a different constant.

Preserve an undistorted fully visible tactical world. Maximize its usable height/width, make controls responsive and remove accidental blank gutters. A square cannot fill a wide rectangle by itself: do not stretch the world, crop the route, or claim repainting empty space black is a solution. Full-window app coverage and deliberate aspect-ratio composition must both work. Preserve correct clicks on all edges and after resizing. Follow the phone/tablet/fullscreen requirements in GRAPHICS_CORRECTION.

### 3. Carry real asset information through the complete pipeline

Inspect the Blender/source meshes, export format, bake output, packed vertex/material contract, shader bindings and draw calls as a single path. Make UVs/normal detail and material identity survive from source to runtime where needed. Version any binary contract change and test real round-trip samples. Separate limestone, timber and metal behavior on the same asset using a bounded shared-material scheme, not one metallic shader plus different paint colors. Use matching units for texture scale.

Start with the Seed ballista and Siege mortar. Show actual cord/pivot/barrel construction, correctly located firing origins, and mechanical recoil/return. For creatures, replace the body geometry and supply actual articulation or baked motion; two identical static poses are not locomotion. Use the tower and creature study sheets as construction references. Keep tactical silhouettes clear and optimize LODs after that quality exists.

### 4. Make terrain look like a place without hiding gameplay

The current mustard road and flat green lawn need correct material response and route-following organic edges. Implement dirt/soil aggregate, subtle ruts, moss shoulders, believable roughness and local relief, with a clean path silhouette. Avoid baked lighting in albedo and avoid deriving physically authoritative normals from arbitrary color without inspection. Compare the material at close-up and normal play scale under the actual game light.

Replace the hard outer slab frame and floating round/cube trees with grounded, correctly scaled scenery. Tighten framing against actual geometry. Remove the visible missing-world sky gulf. Do not add huge foliage borders that shrink the board or hide sockets. A smoother primitive canopy is not automatically a realistic tree.

### 5. Remove visual noise with explicit budgets

Implement a stable screen-space combat-text budget and aggregation; ordinary gold earnings can combine, while meaningful crits/objectives take priority. Reject overlapping labels. Budget ordinary health bars even when every enemy is damaged. Cap extra particles separately from actual gameplay objects. Keep core projectiles visible.

Range is one thin restrained outline for the current selection/ghost. Build discovery uses quiet corner hints, with only the hovered tile strong. Make card names/prices comfortably readable and remove redundant lines instead of shrinking fonts. Re-render icons from the final real assets; do not leave cartoon cards next to rebuilt towers.

### 6. Verify and then scale up

Run a bounded deterministic fixture with known purchased towers, active enemies and fixed elapsed time. Capture idle, placement, active 150-body combat and 350/700 stress. Check real browser console, clicks, sound, frame pacing and resize. Produce a same-size before/after comparison and a short checklist of the visible differences. Any item still looking like the rejection image stays open regardless of unit-test success.

Your next milestone update should show the actual served game image, which assets/materials truly changed, measured layout/render dimensions, tests and the exact remaining gaps. It should not be another list of planned improvements. After the pipeline passes, apply it across the complete asset inventory and finish the live five-hour campaign integration. Do not restart campaign-data expansion or generate additional mood images while these concrete visual failures remain unresolved.

Astra supplies design direction; Terra writes all game/tooling code. Continue directly. Ask only about a concrete external blocker that cannot be resolved within existing authorization, and explain its exact cause. Keep the browser-first Rust/WASM requirement and the original full scope.
