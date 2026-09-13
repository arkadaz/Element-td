# Terra: continue implementation through playable completion

The user explicitly asks you to resume and gives the existing full scope again. You own all game and tooling code. Astra owns design direction. This is an execution brief, not a request for another plan or another foundation-only final response. Preserve unrelated workspace changes and existing player saves. Continue in your current terminal; do not restart the project or recruit another implementer.

Your last report verified a fresh browser build but explicitly left the live 600-encounter Campaign, material/animation data path and complete realistic scene replacement unfinished. Those are remaining implementation tasks. An intermediate milestone warrants a progress update followed by the next task, not a final answer. If a tool fails, investigate with a bounded retry or another reasonable approach. Stop for a blocker only if you identify the exact missing external input and cannot make meaningful progress on another authorized part. Do not invent completion, hide limitations or loop forever on a failing check.

Read this brief, LIVE_CAMPAIGN_REQUIRED.md, DESIGN.md, ENCOUNTERS.md and GRAPHICS_CORRECTION.md. Use NEXT_ENGINEERING_STEPS.md for the rendering pipeline details. Keep a concise checklist in IMPLEMENTATION_STATUS.md with implemented, verified and outstanding distinguished. Existing status text and screenshots are evidence from particular builds, not permanent descriptions of current code.

## 1. Deliver the actual Campaign runtime next

Finish any in-flight mutation safely, then prioritize this path before more terrain polish:

1. Trace actual menu selection through game creation, update, spawn, reward, save and victory. Record where mode ownership resides. Audit src/menu.rs, src/main.rs, src/game/mod.rs, src/game/campaign.rs, src/game/defs.rs, src/save.rs and src/ui.rs. Search every 36-wave constant, clamp and derived progress value; inspect meaning rather than blanket replacing numbers.
2. Give the new Campaign an explicit identity separate from the 36-wave Legacy mode. The primary Campaign action must construct the live campaign state and encounter one. Continue must load the saved mode, and the HUD must show chapter and encounter from that same state. Keep Legacy separately labeled and compatible with its existing saves.
3. Connect the authored encounter resolver to production spawning. Wave previews, spawn packets, traits, encounter timing, rewards and endings must read the same active encounter. Integrate CampaignState with the live game instead of ticking an unused parallel state. Preserve packet ordering and deterministic randomness across speed changes and save/load.
4. Make each transition explicit: preparation, deployment, active combat/objectives, resolution, reward, chapter choice where applicable, next encounter. Required living enemies/objectives prevent resolution. Rewards and chapter choices execute once. The Campaign cannot enter Legacy victory at 36, truncate at 60, skip the final deployment packet or conclude while the final boss remains alive.
5. Implement the actual formation, trait, commander phase, chapter economy and tower-choice behavior in DESIGN.md and ENCOUNTERS.md. Inspect the existing mechanics before adapting them. The new campaign cannot be the old wave loop with a 600 denominator, repeated enemies with increasing HP or empty time gates.
6. Use the real save envelope and browser persistence. Save mode, chapter/encounter, deployment cursor, enemies/objectives as appropriate to the supported resume point, rewards, tower investment/loadout, RNG, choices, cooldowns and active elapsed time. Make the supported resume behavior visible and accurate. Reload must neither duplicate gold nor erase a purchased specialization. Handle old and unsupported save versions explicitly.

Acceptance: select the new Campaign through the actual browser menu, buy/build/upgrade, fight, use Rush and 2x, save, reload and resume the same campaign. A clearly labeled diagnostic fixture must cross 35->36->37 and 60->61 using the production transition code, and 599->600 must require final objective completion before victory. Also run Legacy regression checks. Report the exact build and evidence; a source diff or detached struct test does not pass this milestone.

## 2. Verify five hours of meaningful play at 2x

The design target is ten chapters and 600 encounters with a fastest planned duration just above five hours at 2x. Schedule arithmetic is not a delivered feature or proof of actual duration.

Build a bounded accelerated simulation harness around the same production update/spawn/transition adapters. Use deterministic player actions and the fastest legal Rush/speed settings, and log encounter entry/exit, packet conservation, remaining objectives, rewards and accumulated active time. Simulated time must advance through real updates; do not assign encounter indices or replace combat resolution with a test-only success flag in the duration run. Separate boundary fixtures from genuine runtime duration evidence.

Require at least 18,000 seconds of active real-equivalent gameplay at 2x on the fastest legal campaign route. Exclude menus, pauses, loading, inactive-tab stalls and idle padding. Prevent speed-dependent duplicate updates and lost packets. If battles are over early and the player waits for a timer, redesign deployment pressure and decision cadence; do not simply lock the next-wave button. Check the pacing across the whole campaign so five hours does not mean five hours of the same decision. Report what the harness proves and what still requires human playtesting.

Acceptance: production-runtime duration and transition logs, meaningful opening/middle/finale play checks, no runaway economy or unbeatable growth caused by carrying Legacy scaling through 600 encounters. Automated tests cannot prove that this is the world's most fun tower defense; improve concrete observed pacing and choices instead of making that claim.

## 3. Complete the realistic visual pipeline and roster

Carry geometry, material identity, useful texture coordinates/detail and animation information from authored sources through export, packed data, shader bindings and draw calls. Version any changed binary format. A single metallic material with different vertex colors cannot represent stone, oak, iron, skin and moss correctly. Identical rest and animated meshes do not produce walking animation.

Establish two finished tower families and two animated creatures in the live browser with convincing material separation, scale, contact shadows, readable silhouettes, firing origins and recoil/locomotion. Then apply the working pipeline to every playable tower stage, enemy family, boss, prop and corresponding UI icon. Track the entire inventory so remaining toy assets are visible as unfinished work. Use the supplied material-study PNGs as reference; they are not screenshots of implemented assets.

Finish soil paths, organic moss shoulders, grounded scenery and restrained lighting. Eliminate the flat mustard road, toy trees and missing-world border visible in the rejected screenshots. At normal play scale the path, towers, boss tells and enemy roles must remain easier to identify than decorative detail. Inspect actual gameplay images after integration, not only asset close-ups.

## 4. Finish layout and readable hordes across browser sizes

Use the actual viewport as the shared source for layout, rendering and input coordinates. Keep the entire fixed square tactical world visible and undistorted with no camera movement. Maximize the board and use a restrained desktop rail; adapt controls to a bottom drawer on narrow screens. Remove accidental unused canvas regions and clipping. On wide aspect ratios a square board cannot cover every pixel: use balanced intentional framing, not stretching, cropping or a huge extra panel.

Verify at 390x844, 844x390, 768x1024, 1366x768, 1920x1080, 2048x940 and 3440x1440, with DPR 1 and 2 represented. Check resizing, orientation, browser fullscreen where supported, normal viewport fallback, safe areas and pointer/touch selection near every board edge. Small-device controls need practical touch targets and readable names/prices. Do not infer native mobile browser compatibility solely from desktop viewport emulation; label untested browser/device combinations.

Range outlines appear only for the active selection or placement ghost. Keep build hints quiet. Enforce screen-space damage-text aggregation, ordinary health-bar and particle budgets without reducing actual enemy simulation or hiding important boss tells. Show idle, placement and active combat; profile reproducible 150-body gameplay and 350/700-body stress fixtures on named hardware/backend. Measure frame time and memory, select practical quality fallbacks and report limitations. A render cap is not permission to silently drop enemies or rewards.

## 5. Ship the tested browser build and close the checklist

Run appropriate Rust tests and release WASM build after the integrated changes. Test WebGPU and available WebGL2 fallback explicitly; list any backend not tested. In an isolated browser verify loaded build identity, canvas CSS/backing dimensions, input, console, sound after user gesture, campaign selection, transitions and save/resume. Use bounded browser automation and unique debug ports/profiles. Clean up only your own test processes; do not accumulate hung smoke browsers or stop the user's active game.

Serve the tested output from one clearly identified local URL and document which URL/build the player should open. Do not silently reload the user's current run. Capture real browser screenshots with their viewport, encounter and fixture status. Update IMPLEMENTATION_STATUS.md with direct evidence and remaining gaps.

Completion requires the playable campaign, duration evidence, finished asset coverage, responsive browser presentation and validation above. Continue from one verified milestone to the next in this same task. If a genuine external blocker forces an incomplete handback, state the precise blocker and all unfinished requirements plainly; do not present that handback as completion.
