# Green Circle TD: Realistic Campaign

Design owner: Astra. Implementation owner: GPT-5.6 Terra.
Date: 2026-09-11. Status: approved direction from the user; implementation blueprint, not a claim of shipped features.

## The commission

The user wants every graphic and 3D element to become as realistic as practical, a beautiful and exceptionally fun tower defense, and a full campaign taking **at least five hours at 2x**. Astra designs; Terra writes all implementation code. The first three minutes must be compelling, but the game is not a three-minute run. No monetary amount mentioned by the user authorizes buying assets or services.

This document controls this implementation over older roadmap assumptions. In particular, the earlier 25-30-minute campaign, 28-encounter ending, 32 isolated pads, and 90-body ceiling in `docs/MASTERCLASS_PLAN.md` do not satisfy the current request. Reuse useful existing rules and code, not obsolete acceptance targets. Preserve a separate Legacy mode for the extracted 36-wave rules, tower graph, saves and replays; never silently reinterpret an existing save as the new campaign.

Do not describe the result as the world's best game without player evidence. The concrete bar is a coherent, complete, technically verified realistic campaign with meaningful decisions and a proven duration.

## Visual authority and concept limitations

- [Battlefield target](battlefield-target.png): material mood, a dominant battlefield, quiet attached rail, localized effects.
- [Tower material studies](tower-material-studies.png): the primary reference for realistic tower assembly, material separation, wear and silhouettes.
- [Creature material studies](creature-material-studies.png): realistic anatomy and surface targets for recurring enemy rig groups; production meshes must include complete wings even where the study crops them.
- [Encounter authoring specification](ENCOUNTERS.md): formation schedule, trait introductions and commander mechanics for the complete campaign.

These are generated concept images, not finished meshes or gameplay captures. The earlier cartoon-style concept is superseded. Written specifications win over generated inconsistencies: the battlefield mockup is not perfectly square, contains excess foliage, repeats some tower silhouettes, assigns some wrong icons, and invents prices. Do not reproduce those errors. Use the correct Siege mortar and other distinct designs from the tower study, exact runtime prices, exact hotkeys, the complete route, and a true square viewport. Treat the battlefield image as a minimum direction toward realism; creatures must also receive realistic anatomy and animation rather than its residual toy proportions.

## Experience pillars

1. **Command a living battlefield.** Hundreds of bodies move coherently around one comprehensible circuit. Their movement and the holes made by a good defense create the spectacle.
2. **Recognize the cause of success.** An armor breaker exposes a commander; heavy fire exploits it. A slowing tower holds a pack in a mortar's area. A player can identify the useful purchase.
3. **Make a consequential decision every 20-45 real seconds at 2x.** Improve a kill zone, specialize, retarget, prepare a counter, Rush, or use a tactical command. Do not require hundreds of trivial upgrade clicks.
4. **Real objects, readable scale.** Realistic materials, mechanisms, anatomy and motion; selective detail that survives the fixed camera.
5. **A long expedition with safe stopping points.** Chapters provide closure, replenished strategic choices and dependable resume. No requirement to sit for five uninterrupted hours.

## Duration: a testable contract

The complete new campaign has **10 chapters x 60 encounters = 600 encounters**. Each chapter contains 54 normal encounters and six commander encounters, at local encounter numbers 10, 20, 30, 40, 50 and 60. Encounter 60 is the chapter's signature boss finale.

| Encounter | Count | Planned duration at 1x | Earliest legal advance at 1x |
| --- | ---: | ---: | ---: |
| Normal | 540 | 60 simulation seconds | 54 seconds with Rush |
| Commander | 60 | at least 120 simulation seconds | 120 seconds, plus any living objective cleanup |

Scheduled base duration: `540*60 + 60*120 = 39,600s` = **11 hours at 1x / 5h30m at 2x**.
Fastest legal duration: `540*54 + 60*120 = 36,360s` = **10h06m at 1x / 5h03m at 2x**.

These totals exclude optional setup, pauses, drafts, loading, menus and cleanup. They must measure *active unpaused gameplay*, never wall-clock waiting. Every encounter, including the last, must satisfy its authored active duration. Speed is capped at 2x for this campaign. Debug shortcuts and Legacy/endless rules are separate.

Normal encounters have several authored reinforcement groups continuing through simulation second 54; Rush becomes available only after the last group has deployed and advances at most the six-second recovery tail. Commander encounters have telegraphed phases and reinforcement groups through second 120; killing an early lieutenant must not skip the remaining authored fight. Their final objective arrives during the last phase, not as an already-dead boss followed by a wait. Rush never discards pending groups, stacks bonus payments or skips an encounter. If the board is cleared temporarily, immediately show the next formation's direction and arrival countdown; avoid long empty waits.

A minimum-duration integration test must run an idealized fast-kill player using every legal Rush at 2x and assert elapsed active real-equivalent time >=18,000s. A second normal successful run should land around 5h30m-6h15m at 2x. The exact difficulty can change, but do not compensate for missing content by adding idle timers, inflated health sponges or unskippable animations.

## Campaign chapters and progression

Each chapter is a 30-40-minute real session at 2x with six ten-encounter sequences. Each sequence teaches or varies a formation before its commander. Use at least three distinct formations in each sequence, 18 genuinely different normal formation templates per chapter, and explicit authored schedules for all 600 encounters. Parameter variation is allowed; copying one wave 600 times is not.

| Chapter | Realistic environment | New tactical demand | Signature boss |
| --- | --- | --- | --- |
| 1. Mosswatch | Damp soil, limestone, fern edges, neutral daylight | Two-way traffic, splash, armor, first air preview | Iron Warden: break three armor plates with focused damage |
| 2. Copperwood | Woodland floor, wet bark, copper fixtures | Swift flanking packs and displaced kill zones | Thorn Marshal: marked vines constrain one shoulder temporarily |
| 3. Floodplain | Dark silt, reed corners, shallow perimeter puddles | Regeneration and convoy escorts | Mire Matriarch: three clearly announced brood releases |
| 4. Quarry | Chalk dust, fractured limestone, iron rails outside lanes | Alternating heavy convoys and light screens | Quarry Colossus: armored front, vulnerable following escorts |
| 5. Cinderworks | Ash soil, black steel, ember pockets away from road | Timing burst between shield cycles | Furnace Regent: visible vent/open/recover cycle |
| 6. High Pass | Gravel, sparse frost, pale cold rock | Mixed flying and ground threats | Storm Talon: two altitude phases with honest target rules |
| 7. Old Observatory | Worn masonry, bronze instruments, moss seams | Shields broken by rapid fire, then precision damage | Veil Astronomer: segmented shield and timed exposure |
| 8. Bonewood | Muted forest earth, bleached fallen wood | Separated regenerative lieutenants and route switching | Pale Shepherd: suppress healing before focused execution |
| 9. Black Bastion | Basalt, oxidized iron, restrained red fissures | Control resistance and simultaneous lane pressure | Obsidian Host: advance warning before a resistant charge |
| 10. The Last Circle | Elements of prior regions united in a natural ruined courtyard | All learned compositions, predictable mastery checks | Circle Tyrant: paired lieutenants, then exposed final commander |

The arena is always a centered square and never scrolls. A chapter may load a new preauthored circuit at its checkpoint, but no encounter moves the camera or deforms the live path. Prefer 4-5 carefully tuned circuit layouts across ten material sets, with distinct routing/shoulder opportunities; terrain palette alone is not a new chapter mechanic. No maze intersections that make travel ambiguous. Use the current compact winding circuit as chapter one's starting layout.

Between chapters, bank campaign accomplishments and offer three explicit choices for the next region. Rebuild the next arena from a chapter starting purse and a bounded carried-investment credit, rather than stacking 600 encounters of uncapped exponential gold. Preserve personal tower specializations and unlocks through a clear loadout system. A chapter checkpoint records both the new loadout and its purse before the first encounter.

Three separate progression layers: local upgrades within a chapter; chapter rewards choosing new tactical options; account records/cosmetics/unlocked challenge variations. Avoid permanent damage grinding that makes the authored five-hour campaign meaningless. New players should not need previous-run stat bonuses to win.

## Core rules and economy

Keep the closed clockwise/counter-clockwise ring and persistent survivors. Campaign uses visible weighted Ring Pressure, while Legacy keeps the source headcount. Initial tuning baseline: swarm 0.20, standard 0.75, armored/vanguard 2.0, lieutenant 5.0, boss 12.0 pressure; each completed lap adds 15% to that unit's pressure, capped after four laps. Start Veteran capacity at 140, Apprentice at 175 and Nightmare at 120; these are tuning inputs, not values to preserve despite failed playtests. A 3s simulation breach grace at capacity permits an earned recovery. Display current pressure and the breach timer; headcount remains an optional secondary metric.

Normal formations should include roughly 120-240 bodies across their deployment; swarm formations may reach 300. Mixed carryover should produce 150-350 visible bodies routinely and remain playable at 700. Heavy enemies consume more encounter budget. Do not lower visible population just to meet the graphics target. An upper safety cap must queue or resolve spawns explicitly, never silently drop them.

Start chapter one with 600 gold. Baseline six jobs use the existing curated `campaign.rs` vocabulary and 170-220 base costs. The six base jobs are **Hunter, Bombard, Repeater, Warden, Hexer, Skyguard**; show these six clearly in the default campaign roster. The full eleven-root, 24-family roster remains available in Legacy and feeds campaign advanced unlocks. Do not put locked advanced choices into eleven equal-looking day-one buttons. Campaign has Foundation, Armed, Specialization, Apex rather than 20 repetitive stat rungs. Each upgrade explains the new benefit and the cost.

Use bounded, chapter-local encounter budgets rather than source effective-HP bounties. Suggested starting income per chapter: `150 + 5*localEncounter`, with commander reward uplift explicitly in the authored table; pay 40% at deployment and 60% through kills. Distribute kill shares by authored unit contribution and correct rounding so the exact budget is conserved. Starting purse, carry credit, upgrade prices and final income must be tuned together to support 10-20 strategically useful towers and 80-95% spend for successful builds. Do not accept these seed numbers without simulations. Chapter progression increases mechanics and available choices, not just health multipliers.

Rush reward max 5% of incoming normal budget; Clean Sweep 7% of completed budget, mutually exclusive. Refund 80% during recovery and 65% during combat, stated before sale. Grant one optional relocation after each commander; selection and confirmation retain the tower if the destination is invalid. No blind terrain-click snapping.

Use three visible interactions: **Expose** (Hexer/precision support -> armor damage), **Shatter** (strong chill -> heavy hit burst), **Ignite** (persistent fire -> limited spread by rapid fire). Start with existing implementations where they match. Display at most two status cues on one body. Spell colors distinguish the interactions but never recolor the entire creature. Prevent recursive spread, permanent crowd control and unlimited aura stacking. Use the existing curated campaign damage/slow concepts as tuning baselines; keep Legacy math unchanged.

Tactical command: introduce one targeted pulse in the opening chapter, about 25-35s real cooldown at 2x, with two earned alternatives unlocked later (focused exposure, brief defensive control). One active command slot per loadout avoids piano-key combat. Commands assist a reasonable build and cannot independently kill an encounter. Clearly mark radius, cost/cooldown and valid targets, including color-independent shapes.

## The first three minutes

Timing below is approximate real time at 2x after entering play, with setup unconstrained:

| Time | Player experience | Implementation acceptance |
| --- | --- | --- |
| 0-20s | Understand the ring, select two affordable complementary towers, place beside a bend | Short rail guidance, no full-screen tutorial; actual range and valid targets visible |
| 20-50s | First two-way wave; projectiles connect, recoil and impacts sell weight | Multiple enemies visible within 2s of Start; clear first kill and earned gold feedback |
| 50-90s | First useful upgrade and Rush/Clean decision | Enough earned gold for an upgrade or a different third job; explicit trade-off |
| 90-130s | Small armored convoy tests focus and Expose | Forecast before deployment; no surprise zero-damage immunity |
| 130-180s | Swarm/control payoff and a deliberate specialization preview | At least two viable routes to success; visible advantage from a thoughtful placement |

Do not force air-only or rare advanced purchases before their explanation and affordable counter window. A useful opening should work with multiple pairings. Apprentice may pause for hints; Veteran keeps guidance in the rail and the battlefield moving.

## Desktop composition and interaction

One square board plus a 288-320 CSS-point right rail, centered as one composition. The board uses the maximum remaining height after a 48-52-point HUD and 8-12-point margins. Remove the arbitrary 920-point maximum if profiling permits. The rail touches the board; its height exactly equals the board. Side backdrop on ultrawide screens is unavoidable geometry, but no internal gap or empty ground should reduce the playable board. No duplicated minimap.

Use an orthographic or very low-perspective camera, 78-82 degrees down, yaw zero. Keep all outer plots and their tallest models in frame without treating an oversized empty rectangle as content. Rendering, world picking, screen-space overlays and captures must share the exact camera. Wheel, edge hover, WASD and middle drag never pan or zoom gameplay. UI scaling is separate from camera motion.

HUD priority: encounter/chapter, pressure with danger grace, gold, Start/Rush, Pause, speed. Quality, audio and help move to Settings if space is tight. The rail contains next composition and countdown, selected tower/placement context, then the roster. Three future threat previews are available by deliberate expansion/hover, not a permanent repeated dashboard. Campaign six cards can be 2x3, with larger 64-76-point rows; Legacy eleven choices use 2x6 rows of at least 54 points. Derive breakpoints from actual card/dossier heights, not window width alone.

Essential names 13-15 points, role/cost 11-12 minimum, large counters 18-22. Calm off-white text on charcoal/forest; dull brass only as hierarchy, not a glowing outline around every box. Show short unique names. Tower icons are renders of the actual models with matching stages and lighting. Tooltips supply full names, exact target restrictions and next upgrades. Handle long localized text by wrapping or measured layout, never by letting it overflow.

Idle ground has sparse nearly invisible corner marks only where discoverability needs them. While armed, display a faint set of legal shoulders; emphasize only the hovered tile, one model ghost and one fine range ring. Stronger error shape + short text for road, occupied plot, unaffordable or out-of-bounds placement. Clicks on the rail never reach the board. Repeated placement stays armed; Esc/right-click cancels. Confirm an expensive sale through a clear affordance or brief undo, not a permanent modal chain.

## Realism: every graphical category

Realism comes from silhouette, construction, anatomy, motion, surface response and illumination together. Increasing grain, saturation or polygon count alone is not completion. The asset audit must enumerate every live tower stage, enemy archetype, prop, terrain material, effect, icon, cursor, title background and end-state screen, with old source, new source, license/provenance, runtime consumer, LODs and verification image. Every visible old cartoon asset must be replaced or substantially rebuilt; a reskin on top of unchanged toy geometry is insufficient.

### Material and lighting specification

Terrain: damp packed earth with exposed aggregate, sparse stones and ruts following travel; moss and worn grass at the shoulders; soft organic transitions independent of the tile grid. No baked directional shadows in base color. Macro variation establishes natural patches; high-frequency detail is mip-filtered and restrained at tactical scale. Roads remain a stable middle value above the darker moss. Decoration avoids all legal slots, creep silhouettes and shot origins.

Objects: limestone, basalt, old oak, oxidized bronze, blackened iron, ceramic, glass, leather, bone and hide. Assign material roughness deliberately (starting ranges: stone/soil 0.75-0.95, timber 0.55-0.85, worn metal 0.3-0.65, clean glass 0.08-0.2). Use metallic behavior only for metal. Put wear at contact/handling edges, soot near muzzles, water stains under joints. Avoid arbitrary all-over noise. Preserve modeled thickness and supporting joints.

Lighting: broad neutral-warm daylight, stable soft contact shadows, subtle sky fill. Exposure must preserve bright metal and pale bone without crushing dark hides into moss. Bloom belongs only to hot small sources. No depth of field, motion blur, camera shake, chromatic aberration or fog over playable lanes. Chapter color changes must not change the readability hierarchy. Reflective glass can use a controlled cheaper approximation if true refraction fails the performance budget; disclose the compromise in the asset audit.

### Tower roster: realistic shape grammar

All foundations stay within their logical socket and remain low; upper parts may be taller but cannot block neighboring clicks. Model axes, muzzles and recoil pivots must agree with firing direction. Show four visual milestones through meaningful mechanical additions, not uniform scaling; intermediate Legacy rungs reuse a coherent stage and small investment details.

| Family / campaign role | Primary geometry and realistic construction | Signature movement / effect |
| --- | --- | --- |
| Single / Hunter seed | Oak crossbow, iron pivot, tensioned cord | Draw, release, bow flex |
| Siege / Bombard | Black steel mortar, bronze trunnions, recoil cradle | Barrel kick, soot puff, arcing shell |
| Bouncing / Ricochet | Three reflector arms around a restrained resonator | Brief sequential reflected shot |
| Multi / Repeater-Volley | Fan of four bow arms on one windlass | Staggered releases, distinct muzzles |
| Corruption / Hexer | Copper pressure vessel, teal glass gauge, injector | Valve pulse, narrow corrosive jet |
| Air / Skyguard | Paired elevation arms and slim spear/bolt tubes | Track altitude, elevated launch |
| Chaos | Asymmetric volcanic jaws and a narrow red seam | Short contained discharge |
| Destruction | Squat obsidian heavy mortar with a wide braced mouth | Heavy recoil, local dark-red shock |
| Aura | Three-ring bronze astrolabe, small white core | Slow mechanically linked rotation |
| Damage | Astrolabe with outward faceted brass reflectors | Discrete warm activation pulse |
| Speed | Astrolabe with exposed flywheel and linkages | Faster inner rotation, faint cool pulse |
| Demon | Basalt shrine, realistic horn relief, narrow violet inset | Brief seal opening on execution |
| King | Tall bronze crown lantern on buttressed stone | Stable amber light, measured assembly |
| Slow / Warden | Low stone condenser with thick pipes | Chilled mist confined near base |
| Frost | Upright frosted vanes and braced cold core | Short crystalline strike, no ice dome |
| Poison / Venom | Twin ceramic reservoirs, tubing, pressure head | Droplet arc with short wet impact |
| Critical / Executioner | Long precision bow/rail with sight and counterweight | Deliberate aim, sharp release |
| Troll / rapid branch | Heavy geared repeater, visible heat vents | Winding rhythm accelerates visibly |
| Fire / Pyre | Refractory furnace, fuel pipes, short nozzle | Pressure ignition and local flame |
| OneStrike | Narrow aligned obsidian lens with a shutter | Charge cue and one clear execution hit |
| SuperChaos | Larger supported version of Chaos with radial focusing ribs | One intense, brief focused discharge |
| SuperDestruct | Paired reinforced destruction barrels | Alternating heavy impacts |
| SuperMulti | Two mechanically supported banks of launchers | Controlled grouped volleys |
| SuperBounce | Four reflector tiers around one supported core | Ordered chain accents, capped glow |

Avoid modern toy helicopters, unrelated plastic turrets and giant decorative gemstones. Existing imported models can only remain if rebuilt/retargeted to this coherent construction language. Do not claim the AI reference sheet has supplied usable 3D geometry; mesh production is implementation work.

### Creatures, scenery and animation

Keep the simulation archetype IDs stable where saves require them; provide a visual mapping for every live consumer. Humanoids need plausible joints, weight-bearing feet and believable equipment. Mammoth must read as a mammoth, not the current cow proxy; Bear must not silently remain a wolf proxy. Reptiles need scaled hides and articulated tails, spiders/crabs chitin and credible leg contact, undead bone and cloth, giants/golems weight and articulated stone, winged creatures an identifiable wing cycle and elevated shadow. Machines use the same wood/metal vocabulary as towers. Magical props use plausible housings plus controlled light. Snow, water, foliage and stone assets must share the lighting/material pipeline.

Use 8-12 reusable animation/rig archetypes with species-specific meshes and gait parameters. At minimum: walk/run, idle or held, hit reaction and short death. Bake enough motion samples or GPU animation to avoid the current two-pose morph appearing rubbery. Randomize phase deterministically and tie ground gait to movement speed; no sliding on turns or during stun. Bosses get their own anticipation/phase/recovery movements. Corpses and debris fade fast enough to protect lane readability.

Twenty-four tower families can share structural material kits; enemies can share rig/texture atlases. Reuse must not erase silhouette identity. Provide LODs for every repeated model and a close-up contact sheet at production scale plus the real tactical scale.

### Horde presentation and combat effects

Two offset travel streams with seeded intra-lane variation and small longitudinal stagger. Formations are packets and convoys with authored breathing gaps, not straight copies spawned on exactly one point. Keep lanes inside road bounds, including turns and large units. Do not add physical collision that deadlocks the deterministic loop. Formation offsets are visual unless a gameplay change is deliberate and tested.

Budget visible ordinary health bars (about 16) with stable spatial selection and overlap rejection; prioritize selected/targeted objectives and bosses. Apply the same budget when every enemy is below half health. Use a compact rail boss bar for a commander where appropriate. Aggregate nearby kill-gold events and cap visible floating events around 12-24, prioritizing meaningful crits and rewards. Storage limits of 512 do not constitute visual limits.

Effects have anticipation, travel, impact and recovery. A mortar has smoke and falling dirt near the actual impact, bolts retain brief directional trails, chains draw only their active links, acid leaves small wet marks, frost uses brief crystalline contact and fire uses localized flames. At 700 enemies, preserve core projectiles/impact feedback while reducing extra sparks/smoke. Ground markings for bosses have explicit timing and must differ from placement range. No whole-board flashing or sound pile-up.

Audio is part of the realism: material-specific build clanks, recoil/launch and impact separated in time, restrained creature reactions, region ambience and a low-fatigue music bed with pressure-responsive intensity. Master/SFX/music/ambience controls, persistent mute, bounded voice counts and no startling unthrottled boss spikes. Verify browser autoplay unlock and actual output. Current native Audio is a no-op: either implement native sound too or clearly identify native as a limited target; do not present silent native parity as finished.

## Technical implementation boundaries for Terra

Preserve the existing Rust/wgpu/egui stack unless a demonstrated blocker requires a discussed change. Review the workspace's pre-existing uncommitted work before editing. Generated `greentd.rs` and `greentd_map.rs` change through their emitters. Keep rendering independent of simulation randomness.

The current packed mesh carries positions, normals, colors and two pose offsets, but lacks normal asset UV/tangent/material-index data. For credible new assets, extend the versioned bake/runtime contract as needed to support UVs, tangent-space normal detail, material/roughness/metalness and animation/LOD selection. Validate file headers/strides/attribute bindings and retain or intentionally migrate old assets. A generic triplanar grain over vertex-colored toy assets is not the realism deliverable. Consider shared material atlases and texture arrays to preserve instancing. Inspect actual capacities rather than hardcoding assumptions from the concept art.

Use existing reproducible source downloads only when their actual license and visual fit are verified; realistic replacements need source records. Blender was not available on PATH during planning, so verify available authoring tools before choosing a pipeline. Automated mesh construction or proper asset conversion is acceptable if it meets the studies; do not promise offline hand-sculpting that never occurs. Do not purchase packs or subscriptions without explicit authorization. A missing licensed model is a stated blocker, not permission to insert an unrelated placeholder and mark the asset complete.

Start performance budgets (measure and adjust quality, not gameplay): common enemy LOD about 500-1,500 triangles, closer/boss LOD higher, tower 2k-8k depending on count, shared 1K materials with selective 2K assets; keep draw calls bounded through instancing. Target 60fps at 1080p Balanced with 350 units on a named mainstream GPU; p95 frame <=16.7ms, and a documented crowded 700-unit target with no simulation omissions. Performance preset may reduce samples, particle density and texture resolution; it must preserve all enemies, decisions and readable status cues. No blanket guarantee for every integrated GPU or resolution.

Record total browser frame time, CPU simulation/build time, GPU or graphics timing where available, draw calls, geometry, resident textures, asset download size, and input latency. Run the long campaign in an accelerated deterministic simulation and a real-browser endurance sample; bounded containers, disposal and resource reuse must prevent five-hour growth. Aim for <100ms action response. Do not report CPU benchmark comments as measured browser performance.

## Saves and long-session ergonomics

Versioned run state must include campaign/chapter/encounter identity, elapsed active simulation time, speed, encounter timeline cursor, queued groups, RNG state, live enemies/projectiles if saving mid-combat, towers/branches/investment, purse, rewards already granted, pressure and command cooldowns. Save at chapter boundaries, encounter boundaries and an explicit Save and Quit action. If a mid-combat save is not yet exact, pause and clearly offer the last checkpoint instead of pretending the live state is preserved. Maintain a verified previous save backup and detect corrupt/version-mismatched saves.

Resume must never duplicate income, rewind commander health as an exploit, skip undeployed bodies or advance idle-tab time. Hidden tabs pause by default; returning explains that state. Show chapter progress, expected remaining time at chosen speed, and a clear safe stopping point. Victory requires all 600 encounters and final living objectives resolved. Chapter victory is visibly different from campaign completion. Endless unlocks after the real ending, not at the old wave 36.

## Delivery sequence and acceptance gates

1. **Baseline and inventory.** Reconcile the already-modified workspace. Run current relevant tests and capture actual current gameplay. Create asset audit and a progress checklist linked to this document. Confirm the previously fixed responsive breakpoint and removed desktop guidance overlay remain fixed. The exploratory `capture_the_interface` run made two opening captures then was stopped after a prolonged run; it was not a passing completed verification.
2. **Realistic vertical slice.** Implement the actual material/mesh pipeline, chapter-one ground, Seed/Hunter, Siege/Bombard, one humanoid, one armored creature, their animations/effects, correct icons and rail. Compare actual screenshots against both references. This gate establishes quality for all remaining assets; it is not permission to stop after two towers.
3. **Entire visual inventory.** Complete all active tower families/stages, creatures, props, terrain sets, UI/title/icons and effects. Produce generated inventory-driven contact sheets; no unmapped old cartoon consumers. Verify physically coherent materials at close-up and tactical scale.
4. **Campaign rules and full schedule.** Wire the existing staged campaign module into the real game with explicit new mode/save IDs. Implement all 600 encounter schedules, progression, boss phases, ring pressure, bounded economy, pacing and duration contract. Keep Legacy playable. Run duration, conservation, persistence, target, anti-exploit and balance tests.
5. **Polish and endurance.** Verify six diverse opening plans; swarm/armor/air/control builds; no single tower spam solves every chapter; stronger positioning materially improves outcomes. Test fail/recover/retry, pause/2x/Rush/save/load, long-session growth and exact final ending. Do not claim fun solely from automated winning builds: document playthrough observations and remaining human tuning needs.
6. **Final usable build.** Build browser distribution and native target where supported; run real Chromium/WebGPU smoke, actual input and sound checks, and capture finished results. Report what changed, tests performed, timings, save compatibility, remaining limitations and where to play. Do not publish externally or buy assets implicitly.

Required visual states: opening, repeated placement, invalid placement, each branch selection, 150/350/700 enemies including mass-damaged bodies, commander phases, danger, chapter transition, save/resume, and final victory. Required layouts: 1280x720, 1366x768, 1920x1080, 2560x1440, the minimum desktop breakpoint, 125%/150% UI scaling, Performance and Balanced; check Ultra for bloom washout.

Hard fail conditions: screenshot-only redesign instead of live meshes; merely recolored old geometry; canvas black on real browser; clipped or unreadable commands; camera movement; decorative ground replacing useful board space; wrong icon/model mapping; silent spawn loss; unbounded overlay/effect accumulation; old wave-36 campaign ending; fastest 2x route below five hours; corrupt resume; copied repetitive filler advertised as hundreds of authored encounters.

Implementation ownership now passes to Terra. Astra supplied design documents and visual concepts, not game implementation code.
