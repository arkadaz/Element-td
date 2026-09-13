# Latest rejection: narrow battlefield, flat presentation, unsatisfying play

User supplied rejected-letterboxed-game.png and says the game is not full screen, must look 3D and must be fun to play. Keep the requested tower build icon grid at bottom-right. This correction is about the visible game world and gameplay, not merely browser fullscreen API support.

## Verified cause and controlling layout correction

At approximately 2048x1031, the supplied capture shows a roughly 750-pixel-wide battle image between two huge dark regions. The browser canvas already fills the window. In the inspected src/main.rs CentralPanel path, area is painted PANEL_DEEP and fixed_board_rect(area, false) returns a square used for rendering, camera and input. This deliberately creates the observed empty sides. Do not call it a browser-cache issue or solve it by changing the Full screen button.

Replace that presentation with a continuous 3D scene covering the full available rectangular battlefield viewport above the controls. Retain one square tactical world in world coordinates, but stop requiring its render target to be square. Pass the real viewport aspect to camera fitting and rendering. Draw continuous, grounded surrounding terrain to the camera frustum edges so no dark UI gutters, sky slivers, terrain seams or disconnected picture frame remain. Surrounding terrain is non-buildable scenery, visually quieter than the tactical route. Reject invalid off-board placement cleanly.

The actual defense route must remain maximized, fully visible and easy to interact with. Extending scenery alone is insufficient if a tiny playable board is still lost in it. Reduce oversized bottom panel interiors and consolidate repeated next-threat information as needed to recover play height, while preserving the bottom-right command grid and readable controls. Start with a 150-190 CSS pixel dock on a 1031-pixel-high desktop and inspect rather than treating that as a mandatory magic number. No giant right rail.

No stretching or cropping the route, no free camera movement and no altering campaign topology just to fill a wide rectangle. A square tactical region cannot occupy an entire ultrawide rectangle with those constraints; the correct compromise is the largest useful route within a continuous full-width world, with deliberate quiet scenery at its sides. Be honest about that geometry rather than claiming every pixel is playable.

Use the same actual scene rectangle and camera for render callback, render target, ray picking, tile ghosts, world labels and diagnostic projections. Fit all legal pads, route extents and relevant tower heights, not only ground-plane corners. Recompute on resize, DPI/orientation changes and fullscreen entry/exit. UI clicks remain isolated from world input.

## Make existing 3D visible

Use a fixed oblique perspective showing tower sides, creature bodies and terrain depth. Test around 45-55 degrees above the ground as starting angles, verifying the math's angle convention. Fit the complete route after the change. Avoid a nearly vertical map view, a low horizon view, or exaggerating perspective until rear enemies become unreadable. Keep the camera stable during combat.

Improve scene exposure and lighting at normal play size. The supplied image is dark enough to hide construction and creatures. Show separated lit and shaded planes, grounded contact shadows, plausible cast shadows, and readable material differences. Increase actual model quality and appropriate projected unit size; bloom, artificial tilt or a brighter flat texture cannot substitute. Ground relief must not break logical pathing/picking, and tall foliage must not hide legal build areas. Preserve the textured fantasy direction in the user's references.

Confirm a short live clip shows locomotion, tower aiming/recoil, visible attack travel, impact and death feedback. At fast simulation speeds, keep visual presentation perceivable with bounded render-time effect lifetimes and event aggregation; do not slow damage, fabricate hits or change the requested speed just to make a screenshot. Respect newer direct speed instructions the user may have given your terminal. Verify pause/resume behavior for those presentation effects.

## Make the next play session engaging

Play the actual opening for several minutes and inspect a representative commander, middle chapter and later defense. Record specific dead periods, choices, reward feedback and failure causes. The screenshot's one remaining commander is not by itself proof of poor pacing, but investigate prolonged cleanup, unreachable targets, unnecessary countdown waits and confusing counter requirements.

The opening should quickly allow a useful first purchase, successful attacks and a meaningful second build or upgrade choice. Preview incoming roles early enough for the player to react. Use dense formations and contrasting enemy behaviors from the authored plan rather than uniform streams or HP inflation. Commanders need readable phase cues and actionable responses; meaningful danger can come from supporting formations and objectives as designed, not just a stationary damage sponge. Reward and upgrade feedback must be perceptible at the chosen speed.

Check whether all offered early towers are useful against the upcoming threats. In the supplied image gold is 334 while an Air Tower placement asks for 600; inspect whether this is an intentional persistent ghost after spending or an affordability-selection bug. Explain missing gold clearly, forbid charging on invalid placement, and avoid opaque build failures. Ensure new players can understand the icon grid without reading developer diagnostics.

Keep the long campaign and existing saves intact. Do not claim five hours or exceptional fun from a wave counter, time sum, instantaneous fixture or test flag. Tune observed gameplay and report what is verified.

## Acceptance before the next handback

1. Real browser at the supplied 2048x1031 size: full-width continuous 3D battlefield above the compact controls, complete route visible, bottom-right build grid preserved, no dark unused side panels.
2. Verify 1366x768, 1920x1080, 3440x1440 and phone orientations: no crop/stretch, incorrect picking, hidden controls or world gaps. Name untested device/backend combinations.
3. Same-camera captures of idle, placement, built defense and dense mixed combat plus a short live clip that demonstrates actual depth and attack feedback. Record build identity and served URL.
4. Use actual pointer placement at outer pads, cancel, upgrade, save/resume and resize. Profile a representative mixed horde on named hardware; full-width rendering increases pixel cost, so choose measured render-scale/shadow/particle budgets.
5. Report concrete gameplay changes and observed opening/commander behavior, then continue remaining authorized work. Do not declare visual acceptance based only on Rust tests or HTML canvas dimensions.

Astra provides this design correction; Terra implements all code. This supersedes the earlier square render-rectangle interpretation while preserving the fixed square tactical board. Continue in the existing task rather than starting another plan-only handback.
