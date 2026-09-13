# Latest user correction: match the UI

## Controlling clarification: tower build buttons at bottom-right

The user clarified again: "for the tower to click to build that tower" and "it is on the right buttom". The immediate requested change is specifically the tower build palette at the BOTTOM-RIGHT, like the reference. Astra's broader top-bar/minimap/full-dock interpretation below is optional design context, not an additional user requirement. Do not delay this concrete fix for a full HUD redesign or introduce a new minimap solely because the earlier text suggested one.

Implement a compact, consistently anchored bottom-right grid of square tower icons. Use a 4-column by 3-row arrangement where it fits the actual roster, with explicit category/page navigation if more slots are required. Include every currently unlocked buildable tower; show locked/unaﬀordable states clearly rather than omitting unexplained choices. Each icon represents its tower, with cost and shortcut where available; hover or keyboard focus shows its name and purpose. Clicking an affordable tower arms its actual placement ghost. Clicking a valid battlefield location builds that exact tower and deducts its cost once. Invalid placement must not spend money. Escape or right-click cancels; preserve the existing repeat-build behavior. Selecting or navigating the palette must never place a tower behind it.

The old tall right-side build-card list must cease being the primary build selector. Place the compact grid in a reserved bottom-right control area, with no tall empty panel above it and no obstruction of legal build locations or the visible route. Keep other HUD elements stable unless a local adjustment is necessary to fit the palette. Use the reference's framed icon-button treatment; this is an in-game tower selector, not another large card catalog. On touch devices use at least 44-CSS-pixel targets and responsive layout without clipping the grid.

Verify the exact flow in the live browser: click a bottom-right tower icon, see the correct ghost, place successfully, check tower identity and gold, cancel, try invalid/unaﬀordable placement, and check palette input isolation. Capture the bottom-right menu both idle and with a tower armed. Deliver this focused UI correction as the next visible change while preserving the rest of the authorized game work.

The user explicitly clarified: "no i mean the UI" after supplying user-combat-effects-reference.png. Despite that file's initial name, this latest request concerns the interface, not combat effects. Open that image and user-primary-visual-reference.png. Do not start a new combat-effects pass based on Astra's mistaken interpretation.

This clarification overrides the earlier desktop right-rail recommendation. The user is rejecting a fundamentally different interface composition, not asking for minor card colors or more rounded panels. Implement an original RTS-style top bar and bottom command dock matching the reference's hierarchy and fantasy material language. Keep existing campaign work, browser support, whole fixed board and responsive requirements.

## Desktop composition

- Thin full-width top resource/menu strip, approximately 30-40 CSS pixels high. Present essential resources, encounter/chapter and pressure compactly; group menu, pause and actual supported speed controls. Avoid large repeated resource cards. Do not reproduce unrelated multiplayer/chat/upkeep controls merely because they exist in the reference.
- Replace the large right-side card rail with a bottom command dock spanning the window. Begin around 160-190 CSS pixels high at 1080p, 132-156 at 768p, then inspect readability. These are starting dimensions, not permission to clip controls or crush the board.
- Left module: an approximately square live minimap, showing the actual route, defense positions and threat locations using a quiet palette. The board camera remains fixed; do not add camera scrolling to make the minimap interactive. It can support inspect/highlight behavior without moving the view. Keep its framing subordinate to combat.
- Center module: selected tower portrait/model icon, name, level/specialization, key combat stats, and a short ability description. When unselected, show useful encounter information and brief build guidance in this same space. Avoid redundant next-wave panels or tall empty boxes. Hide implementation diagnostics from normal play.
- Right module: a compact 4-column by 3-row command grid as a starting arrangement. Use consistent square framed icons for build families, available upgrades, targeting, sell and back according to selection context. Every existing necessary action must remain reachable. Use explicit category/back navigation or pages where the roster exceeds capacity; do not silently omit towers. Mouse hover/focus reveals readable name, cost, shortcut and effect; touch uses inspect then explicit confirmation where needed. Disabled and unaffordable actions need distinct, legible states.
- Keep the action grid spatially stable when selecting and upgrading; do not move every control between clicks. Make upgrade branches recognizable. Selling must not occupy a position that encourages accidental confirmation when an upgrade layout changes.
- Maximize and center the entire square tactical world in the space between top bar and bottom dock. No camera movement, stretching or path cropping. Avoid shrinking the board further with another permanent side panel. Handle aspect-ratio remainder deliberately; it is not a reason to enlarge the dock or distort the square world.

## Visual treatment

Use original dark carved-stone and aged-metal framing, narrow bevels, recessed dark surfaces, muted gold edging and restrained teal highlights. Give panel joins and icon wells consistent depth. Match the reference's cohesive fantasy interface, not a web dashboard with large flat rounded cards. Decorative trim stays shallow so it does not consume the battlefield.

Use the game's final tower portraits/icons consistently. Keep readable typography at normal play size: approximately 13-15 CSS pixels for principal labels, 11-12 for genuinely secondary compact values. Avoid microtext as a way to fit content. Display resource numbers and critical encounter information with strong contrast. Material textures remain quiet behind text. Keep keyboard focus, selected, hovered, active and disabled states distinct without depending only on color.

## Responsive and functional requirements

At narrow widths, retain the same hierarchy but reflow the bottom modules into a compact selected-information strip and expandable action sheet; the minimap can collapse. Essential touch actions remain at least 44 CSS pixels. Short landscape phones require a compact dock/drawer instead of scaling down desktop text. Support safe areas, resize, fullscreen entry/exit and keyboard/touch equivalence. UI input must not place or select towers behind the dock. Preserve save state and the user's active session.

## Acceptance evidence

Implement in the real Rust/egui application, not a standalone HTML mockup or generated screenshot. Compare actual browser captures at 1920x1080 and 1366x768 to the reference for these specific UI properties: thin top strip; bottom dock; left minimap; central selection information; right icon commands; coherent stone/metal framing; large unobstructed battlefield; no giant right rail.

Capture unselected, selected tower, upgrade choices, build placement and unaffordable action states. Verify build, repeat build, cancel, upgrade, targeting, sell, pause, menu and available speed actions after the layout change. Check 390x844 and 844x390 for functional reflow and input isolation. Keep testing actual functionality rather than declaring visual success from compilation alone.

Continue the existing task. This changes the UI design direction; it does not cancel the campaign, graphics or duration requirements. Respect any newer direct user instructions received in your own terminal. Do not claim the UI matches until you have inspected the actual served output against the supplied images.
