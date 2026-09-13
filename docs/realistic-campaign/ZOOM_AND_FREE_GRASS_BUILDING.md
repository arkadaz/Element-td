# Latest user override: zoom and free placement on grass

User: "and i can zoom and place tower in any glass too". In context, glass means grass. This explicitly replaces earlier no-camera-zoom and preset-build-pad restrictions. Preserve the bottom-right tower icon selector, full-viewport 3D direction and existing campaign work. Do not keep obsolete fixed-camera rules in tests or code merely because earlier Astra briefs required them.

## Zoom interaction

Implement smooth bounded mouse-wheel/trackpad zoom over the world and pinch zoom on touch. Anchor the zoom to the ground under the pointer or pinch midpoint so the selected location stays stable. Use actual camera projection and current viewport for both rendering and picking. Minimum zoom provides a fit-to-map overview; maximum zoom should expose useful model/placement detail without camera clipping or unusably narrow context. Provide a visible reset-view control and an accessible keyboard equivalent. When zoomed in, permit deliberate drag panning so every part of the map remains reachable; bound it to the authored world. Do not move the camera merely when the cursor reaches an edge.

Distinguish touch pan/pinch from placement taps; a gesture must not accidentally build a tower. On desktop, use an unambiguous pan gesture such as middle-button drag, preserving left-click build and right-click cancel. Zooming over the command grid, menus or tooltips must not scroll the world. Camera movement must not change simulation speed, path positions, wave timing or save progression. Use an overview camera only as the initial/reset pose, not as a camera forced anew each frame.

## Free grass placement

The desired flow is bottom-right tower icon -> ghost follows pointer on grass -> click to build at that position. Remove preset socket/pad membership as a placement requirement. Grass must support continuous world-space placement; a fine invisible quantization for deterministic storage is acceptable only if it does not feel like the old coarse socket grid. Do not show a permanent build grid across the terrain.

Define one authoritative buildability query used by ghost feedback, actual purchase, keyboard/touch placement and save validation. Validate the whole tower footprint against world bounds, the actual road corridor, water/rocks/solid scenery and existing tower footprints. Decorative low grass is not a blocker. Use a small clear spacing margin, not huge invisible exclusion zones. Define road clearance from route geometry, not only a ground-color pixel. Prevent invalid spending and duplicate click purchases. Show a concise reason for invalid placement and a clear affordability state.

Any grass presented as accessible build land should behave consistently. The new full-width world must not surround the player with plausible empty grass that secretly rejects all towers just because it was previously called scenery. Extend legal placement bounds with the authored accessible terrain where needed; otherwise show an unmistakable world boundary or physically blocked terrain. Do not let the current arbitrary square UI rectangle define legal world placement. Preserve the authored route and pressure rules; this is not a request for player-created pathfinding mazes.

Audit tower storage and every assumption about pad IDs, socket occupancy, nearest-pad picking, range checks, hover selection, spatial indexing, selling, upgrading, save/load and fixtures. Store actual world positions and footprints. Migrate existing placed towers from their previous pad positions without losing upgrades, costs or identity. Existing saves must still load. Range/targeting queries and projectile origins must use the same world positions. Remove early-outs that assume all towers occupy a predefined band beside the route. An out-of-range tower can be allowed with clear range feedback; do not silently pretend its grass tile is invalid.

## Acceptance in the real browser

- Build at three clear grass locations that were not old sockets, including one in expanded accessible terrain if present. Verify exact tower position, one cost deduction, selection, upgrade, sell and save/resume.
- Reject road overlap, tower overlap, a solid obstacle, outside-world placement and unaffordable purchases with accurate feedback and no cost deduction.
- Zoom toward a chosen grass point, build there accurately, pan to another area, reset overview and verify tower/world positions are unchanged. Check viewport resize/fullscreen and DPR changes.
- Pinch and drag on touch without accidental purchases; tap to place intentionally. Keep command buttons usable and prevent UI input from reaching the world.
- Demonstrate the flow in a short real-browser clip, not only static images or layout tests. Run relevant placement/save regression checks and a representative dense scene for picking and performance.

This is the newest controlling requirement. Integrate it with FULL_VIEWPORT_3D_CORRECTION.md, replacing that document's no-zoom/free-camera prohibition and older pad-only acceptance assumptions. Keep all other game, graphics, browser and campaign requirements. Terra implements the code; continue the existing task.
