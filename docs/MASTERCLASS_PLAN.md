# Green Circle TD — Commercial Redesign Plan

Status: implementation blueprint. The current playable build is still the
Legacy ruleset; the Campaign systems below are acceptance criteria, not claims
about features already shipped.

The Warcraft III reference supplies the identity: a continuous circular road,
clockwise/counter-clockwise streams, no conventional exit, extreme upgrade
ladders, specialist damage types, and escalating crowd pressure. The commercial
version keeps that identity but must make every important decision legible,
positional, and capable of producing a different run.

## Product target

- A focused 25–30 minute Veteran campaign with optional endless play.
- Easy to understand in the first two encounters; strategically demanding by
  the first commander at encounter 7.
- Six immediately readable tower jobs, expanding into distinct late-game builds.
- Four acts of six normal encounters plus a mechanically different commander.
- A cohesive dark-fantasy battlefield, not a mixture of unrelated asset packs.
- Smooth Edge/Chrome WASM play with scalable visual quality.

The game is not release-ready until every acceptance gate at the end of this
document passes. “More polygons” alone is not a quality gate.

## Decision: the reference is not the main ruleset

The extracted Warcraft III data remains useful as an unlockable **Legacy
Circle** mode. It is not the default campaign. The current build proves why:

- its deterministic winner builds 24 Siege towers out of 32;
- that winner earns about 10 million gold and leaves about 7.9 million unspent;
- most waves are a single enemy type repeated 60-161 times;
- the 5% Immune rule and air-only targeting create purchase checks, not
  interesting tactical counters;
- 131 statistical rungs create many buttons without 131 different decisions;
- global damage/rate/range doctrines strengthen the solved build instead of
  changing how the player thinks.

Commercial Campaign therefore gets a curated balance layer. It keeps the
closed two-way ring, persistent lap pressure, build pads, recognizable tower
families, four-act arc, and extreme late-game spectacle. It replaces the source
economy, wave composition, hard immunities, exposed 131-rung shop, and flat
doctrines. This is the line between respectful inspiration and shipping an old
custom map's problems in a new renderer.

## The USD 5 product

One 25-minute campaign by itself is not enough value. Version 1.0 ships only
when this complete package is present:

- **Circle Campaign:** four acts and 28 authored mixed encounters, 25-30 minutes at
  1x, with three difficulty patterns;
- **Legacy Circle:** the extracted roster and source-wave rules, clearly
  labelled as the chaotic original rather than the recommended first game;
- **Endless:** post-campaign scaling with a local and relay-backed scoreboard;
- **Contracts:** ten score modifiers unlocked by campaign progress, with up to
  three combined in a run;
- **two compact authored pad layouts** on the same ring geometry, one balanced
  and one unlockable layout that changes sector strengths without enlarging the map;
- **three Command abilities**, **six tower jobs**, **twelve specialisations**,
  **twelve doctrine cards**, a bestiary, run history, and achievements;
- save/resume, graphics and audio mixers, remappable keyboard controls,
  readable colour-blind symbols, reduced motion, and UI scale.

The content target is 6-10 hours to clear Veteran, learn all branches, and earn
the first five Contract medals; mastery, Nightmare, and Endless extend it.

## Commercial Campaign ruleset

### Pressure replaces raw headcount

The current `creeps.len() > limit` rule makes a tiny swarm unit as dangerous as
a commander. Campaign uses visible **Ring Pressure**:

```text
unit pressure = rank weight × (1 + 0.20 × min(completed laps, 3))

Swarm 0.35    Standard 1.00    Vanguard 2.50    Commander 15.00
```

Veteran capacity is 100 pressure. Apprentice uses 120 and Nightmare 85.
An additional trait multiplies that unit's weight by 1.25. At 80 the HUD and
battlefield enter a warning state. At 100 a three-second
breach countdown begins; dropping below 100 cancels it. This preserves the
reference game's accumulating circular debt while giving the player one last
readable recovery window. A commander completing its third lap also defeats
the player. Legacy Circle retains the source headcount rule.

### Economy uses bounded, authored wave budgets

Campaign does not calculate income from exponentially scaled effective HP.
Each encounter owns a gold budget, with 45% paid as command income when it
starts and 55% distributed across its units by pressure weight. This limits
snowballing while still rewarding kills.

```text
encounter gold Gw = round-to-5(120 + 9 × encounter + 30 × act)
act = floor((encounter - 1) / 7)
```

The 28 encounters provide about 8,275 base gold before bounded tempo bonuses.
Rewards are fixed when a unit spawns; laps never increase bounty.

- Starting gold: **600**.
- Base towers cost **170-220**; at least six compositionally distinct two- or
  three-tower openings must reach the first commander in seeded simulation.
- Tier 2 costs **260-320**, the branch tier costs **480-600**, and Apex costs
  **850-1,100**. A completed tower therefore costs roughly 1,760-2,220.
- A Veteran clear earns **8,000-9,500** total gold, fields 8-14 meaningful
  towers, spends 80-95% of earned gold, and ends with at most 15% of net worth
  idle.
- Selling refunds 80% during a calm tail and 65% during active combat, always
  shown on the button and always capped below invested gold. Each defeated boss
  grants one free **Relocation** of an existing tower with three seconds of
  setup, preventing a lesson learned on a boss from ruining the remaining run
  without making initial placement fake.
- Rush can advance only the final eight seconds and is capped at 5% of the
  incoming encounter budget. Clean Sweep is 7%. They remain mutually exclusive.

Gold is displayed in whole, human-scale values. The extracted million-gold
economy remains visible only in Legacy Circle.

### Timing

- Wave 1 waits for the player; there is no opening countdown.
- Normal deployment lasts 28-34 seconds. The next automatic boundary is 46-50
  seconds after the encounter starts.
- Rush unlocks only after the current stream has fully deployed. A second press
  cannot delete or skip queued enemies.
- Boss encounters last 55-75 seconds and act drafts pause the simulation for up
  to 20 seconds.
- A winning 1x run must measure 24-30 minutes. A 2x run must still take 13-17
  minutes. Campaign never exposes 3x.

### Difficulty changes patterns, not the player's income curve

- **Apprentice:** 120 pressure, longer planning windows, fewer simultaneous
  traits, and optional counter hints.
- **Veteran:** 100 pressure and the authored composition/timing baseline.
- **Nightmare:** 85 pressure, earlier mixed formations, extra boss phases, and
  shorter planning windows. It does not secretly reduce bounty or multiply all
  HP by a flat number.

All modes show the same next-three-wave information. Difficulty is never made
by hiding knowledge.

## Core loop

Each encounter repeats a short, clear rhythm:

1. **Read:** preview the next three threat compositions and their counters.
2. **Plan:** build, upgrade, sell, and choose which road section becomes the
   next kill zone.
3. **Commit:** send normally, or call early for bonus gold and overlapping risk.
4. **React:** redirect target priority, trigger one commander ability, or repair
   a weak counter without pausing the action.
5. **Recover:** earn a clean-ring bonus when the road is empty, inspect tower
   contribution, and prepare for the next composition.

Normal encounter starts are 46–50 seconds apart, with 28–34 seconds of visible
deployment and an 8–14 second calm tail in at least 70% of successful Veteran
encounters. Planning overlaps readable cleanup instead of pausing after every
wave; boss encounters can take up to 75 seconds and act drafts pause fully. At
normal speed, a successful run must not finish below 22 minutes or exceed 32
minutes for an experienced player.

## The board must create decisions

Campaign uses 32 authored pads in eight readable sectors, with twelve marked
inner/outer pairs. Legacy Circle may retain the larger pad set. The Campaign
pads create deliberate kill zones, not a decorative grid or tower carpet.

- A turret plinth should visually sit 0.15–0.60 tile from the road edge.
- Pad centres stay at least 2.20 tiles apart and no apex model may overlap.
- No tower, aura, doctrine, or Command may cover more than 35 of the 109.5 road
  tiles; long range cannot erase the board.
- Straight sections favour long-range and line attacks.
- Bends favour splash, bounce, poison, and persistent ground effects.
- Every useful inner pad has one visibly linked outer partner. Two different
  tower jobs on that pair create **Crossfire** when they hit the same target
  within 0.8 seconds: Expose for 2 seconds, with a 3-second per-target cooldown.
  Identical jobs cannot trigger it. The link lights only when the combo is
  currently possible, so this never becomes invisible arithmetic.
- Aura towers reward a compact cluster. Stacking uses strongest + half the
  second + quarter of each remaining aura and caps damage/rate at 35%; auras
  never buff themselves.
- Placement previews show real range, affected road length, valid targets, and
  the nearest useful partner—not only a green/red footprint.

Acceptance: move the same purchased roster from a strong kill zone to poor pads
and it must lose at least six waves earlier on Veteran.

The placement ghost must calculate, not guess: road seconds covered in each
direction, expected targets for the next wave, nearby aura recipients, paired
Crossfire job, and model clearance. A pad that offers zero useful coverage for
the chosen tower is disabled with a written reason. This removes trap clicks
without removing placement strategy.

## Six readable tower jobs

The UI presents six jobs first. The reference roster and its unusual named
families remain underneath these jobs instead of confronting a new player with
131 near-duplicate levels.

| Job | Base cost | Base promise | Specialisations | Honest weakness |
| --- | ---: | --- | --- | --- |
| **Hunter** | 180 | accurate single-target fire; repeated hits build Aim | Executioner boss burst / Destroyer anti-armour | dense swarms |
| **Bombard** | 220 | ground splash and heavy impact | Siege shatter / Pyre fire zones | air and swift units |
| **Repeater** | 190 | fast pressure that handles many light targets | Volley multishot / Ricochet status spread | heavy armour |
| **Warden** | 170 | chill and lane control | Frost root / Thornwall displacement | control resistance |
| **Hexer** | 210 | damage over time and suppression | Venom anti-regen / Corruption anti-aura | immediate burst checks |
| **Skyguard** | 190 | strong anti-air with weak emergency ground fire | Flak swarm control / Lancer air-boss focus | armoured ground |

Special Aura, Chaos, Demon, King, and Super weapons become visible milestone
branches or run-defining rewards. They do not occupy equal-looking shop buttons
from wave one.

Every tower has four visual milestones: Foundation, Armed, Empowered, and Apex.
Stat upgrades between milestones remain useful but do not pretend to be a new
model. Branch points show the resulting job, target rules, and one-sentence
trade-off before purchase.

Campaign has exactly four mechanical tiers per placed tower: Foundation,
Armed, one of two Specialisations, and Apex. It does not expose twenty small
linear Siege upgrades. The original family names map under these twelve
specialisations, so the reference remains recognizable without filling the
command panel with duplicates.

Target restrictions avoid dead purchases: Hunter, Warden, and Hexer can engage
both altitudes; Repeater deals 65% to air; Skyguard deals 35% to ground;
Bombard is visibly ground-only. These are soft inefficiencies with one obvious
exception, not surprise zero-damage walls.

## Combat combinations

Depth comes from three readable interactions, not a dictionary of hidden
multipliers.

- **Expose:** Hexer suppression, Breacher impacts, or Crossfire remove a visible
  armour segment; Hunter gains value against the exposed target.
- **Shatter:** a heavy impact on a strongly slowed target causes a small burst,
  giving Warden + Bombard a swarm answer.
- **Ignite:** Fire refreshes damage over time but does not stack infinitely;
  rapid Repeater hits spread an existing ignite to one nearby enemy.
- **Disrupt** is Hexer's role action: it pauses regeneration, Veil shields, and
  commander auras. It is not a fourth damage multiplier.
- Hunter's **Aim** is private tower state, displayed over its target and
  consumed by Deadeye or Breaker. It replaces invisible critical-hit luck.

Only two status effects may dominate a monster’s silhouette at once. Every
effect requires a battlefield icon, a distinct impact response, and a matching
sound family.

Campaign armour is bounded to 0-120 and uses
`damage = raw × 100 / (100 + effective armour)`. Expose removes 20 armour per
stack, maximum two. Slow combines the strongest source plus 25% of the second
and caps at 55% for normal units, 30% for Vanguards, and 15% for commanders.
Roots grant 1.5 seconds of immunity afterward and displacement has a four-tile
lifetime budget. A weaker damage-over-time or slow application may refresh a
stronger effect but can never reduce its strength or remaining damage.

## Enemy language and wave composition

Waves become mixed compositions with six visually obvious threat traits:

| Trait | Read | Counter pressure |
| --- | --- | --- |
| **Swarm** | many small, bright eye-lines | splash, bounce, persistent zones |
| **Armoured** | plates and grey health segments | Expose, heavy impact, chaos |
| **Swift** | lean silhouette and wind trail | slow, root, forward kill zones |
| **Regenerator** | pulsing green wound glow | poison, ignite, suppression |
| **Flying** | elevated shadow and altitude marker | Skyguard and universal chaos |
| **Veiled** | periodic spectral shield | sustained fire, timed burst after shield |

The next-three-wave panel shows proportions such as “60% Swarm / 40% Armoured,”
not a vague single tag. New traits are introduced alone, then combined later.
Surviving laps add speed and pressure but never mint extra bounty. A visible
veteran ring makes that escalation look intentional rather than a movement bug.

Base health grows by `H(e) = H1 × 1.135^(e-1)`, roughly 31× across Campaign,
not the reference's roughly 2,000×. Encounter pressure budget is
`24 + 1.5e + 6×floor((e-1)/7)` and the authored groups must land within 3%.
A normal encounter caps live bodies at 90, one unit carries at most two traits,
and a new trait appears alone before it is mixed. Veiled enemies use ten visible
shield pips: a hit removes at most one pip, breaking the veil opens a three-
second vulnerability, and Repeater therefore opens a window that Hunter can
cash without another immunity cliff.

### Four-act campaign

- **Act I — Establish (encounters 1–7):** teaches direction split, swarm,
  armour,
  air, and one simple combo. Boss: **Iron Warden**, whose armour aura asks the
  player to expose the commander or clear escorts efficiently.
- **Act II — Specialise (encounters 8–14):** mixes swift, regeneration, and air;
  the player must commit to a branch. Boss: **Brood Matriarch**, periodically
  releasing small fliers at telegraphed health thresholds.
- **Act III — Adapt (encounters 15–21):** introduces veiled enemies and control
  resistance, testing a secondary damage plan. Boss: **Veil Queen**, alternating
  shield and vulnerability windows that are clearly telegraphed.
- **Act IV — Master (encounters 22–28):** combines traits without unreadable
  crowds. Boss: **Circle Tyrant**, leading both road directions with linked
  lieutenants; balanced coverage matters more than raw health inflation.

### Authored encounter beats

Campaign encounters are `EncounterDef` timelines containing one to three spawn
groups, not one `WaveDef` copied many times. Percentages below describe pressure
budget, not raw headcount; final counts are tuned by simulation and readability.

| Encounter | Composition and lesson |
| ---: | --- |
| 1 | 70% Standard / 30% Swarm; Repeater and Bombard are both valid openings |
| 2 | Swarm from both directions; teach bends and target priority |
| 3 | 35% Armoured convoy; demonstrate Expose and Hunter value |
| 4 | Swift Standard pack; teach Warden and Command Pulse on a safe spike |
| 5 | Armoured Swarm; first combined counter and real Rush/Clean decision |
| 6 | 30% Flying over ground pressure; forecast it from encounter 3 |
| 7 | **Iron Warden** with plated escorts and breakable armour segments |
| 8 | 30% Regenerator; introduce Hexer without making it mandatory |
| 9 | Swift Swarm; tests the player's first specialisation |
| 10 | Armoured Regenerators; Expose and Disrupt compete for priority |
| 11 | Flying + Swift crossing opposite approaches |
| 12 | Regenerating Swarm; persistent zones and Venom shine |
| 13 | Flying Swarm with a small plated ground screen |
| 14 | **Brood Matriarch**; telegraphed 75/50/25% flying broods |
| 15 | First Veiled group, initially alone and slow |
| 16 | Veiled Swift units; Repeater opens and Hunter cashes the shield window |
| 17 | Armoured convoy screened by Swarm in both directions |
| 18 | Asymmetric 70/30 direction split that changes mid-encounter |
| 19 | Control-resistant Vanguards; damage must work without chain control |
| 20 | Veiled air group over Standard ground pressure |
| 21 | **Veil Queen**; ten shield pips and a four-second vulnerability rhythm |
| 22 | Low-stat all-trait mastery check |
| 23 | Alternating clockwise and counter-clockwise Vanguard packs |
| 24 | Air armada over an Armoured ground convoy |
| 25 | Regenerator escort protecting one high-pressure lieutenant |
| 26 | Veiled Swarm; controlled reveal rather than particle clutter |
| 27 | Twin lieutenants enter opposite directions; final coverage rehearsal |
| 28 | **Circle Tyrant** with two linked lieutenants and suppressible repairs |

Iron Warden loses armour segments to heavy impacts and Expose. Brood Matriarch
releases adds only at telegraphed health thresholds, never per hit. Veil Queen's
shield rhythm is shown on the timeline before she reaches a tower. Circle
Tyrant's lieutenants must die within twelve seconds of each other or the survivor
restores 30% of the fallen lieutenant; killing both permanently exposes the
Tyrant. This is the final test of two-direction coverage, target priority, and
the command ability.

## Command progression

The three current flat global buffs are replaced by a deterministic draft after
bosses 7, 14, and 21. Each draft offers one card from three categories:

- **Arsenal:** strengthens burst, armour break, and marks.
- **Tempo:** rewards controlled rushing, attack cadence, and clean sweeps.
- **Formation:** strengthens Crossfire, auras, and road coverage.

There are twelve authored cards, four in each category. Examples are
`First Salvo` (the first heavy hit on a fresh target Exposes), `Clean Engine`
(a Clean Sweep removes 20 seconds from Command cooldown), and `Linked
Batteries` (Crossfire lasts one second longer). Each changes a rule and supports
a build identity; no option is simply “all towers deal 12% more damage.” The
draft seed guarantees three useful choices and never offers a card whose tower
job is absent from the board without marking it as speculative.

The first campaign teaches one active command: **Command Pulse**. The player
selects a Crossfire pair or small road sector; towers there gain 35% attack
speed for six seconds on a 90-second cooldown. It strengthens a planned kill
zone instead of deleting a wave. Two alternative commands unlock later:
`Lockdown` for control and `Null Field` for shields/regen. A command may rescue
one close wave but must not let a counterless or badly placed board clear a
boss.

## Failure must be readable and recoverable

The ring is allowed to look dangerous before it is actually lost. At 60
pressure the HUD names the largest unresolved trait. At 80 it highlights which
kill zone is leaking and recommends a target-mode or upgrade action. At 100 the
three-second Breach countdown creates a final Command/sell/upgrade decision.

The game never grants invisible pity damage. Recovery comes from information,
the visible sell rule, one Relocation per act, fixed kill rewards, and the player's
Command. Once per act at 70 pressure, **Emergency Refit** may pause the action
for six seconds and relocate one tower or sell it at 90%; it spends 20 Command
charge and forfeits that boundary's tempo bonus, so deliberately creating a
crisis is never profitable. After defeat, the summary identifies the first wave where pressure
stayed above 60, the trait that survived, tower idle time, and unspent gold.
Retry offers the same seed and a checkpoint practice option, but medals and
scores require a full run.

## UX required to support the game

- The persistent HUD shows only gold, wave, Ring Pressure, Command cooldown,
  speed, and pause. FPS/instance counters live in a developer overlay.
- The next-three-threat strip uses portraits plus trait icons and counter text.
- The build panel begins with six large job cards; advanced branches appear only
  after selecting or building their parent.
- The selected-tower panel shows current DPS, road coverage, targets, earned
  gold, next branch comparison, and sell value in one scan.
- Holding a compare key overlays before/after range and affected road length.
- End-of-wave feedback names the strongest contribution and the current leak,
  e.g. “Bombard cleared 61% of Swarm; Armoured survivors completed 2 laps.”
- The persistent minimap is removed on the compact arena; a full-board tactical
  view is available on demand. The empty square is not allowed to consume the
  most valuable part of the HUD.
- The bottom console occupies no more than 20% of a 720p playfield and collapses
  while nothing is selected. Minimum body text is 13 px at 100% UI scale.
- Build cards are at least 72 px wide at 720p and never show eleven tiny shop
  icons at once. Controller focus, keyboard focus, hover, selected, affordable,
  and disabled states are visually distinct.
- Tutorial callouts attach to the relevant button/pad and never cover the
  intended click target or active lane.

### Intended first ten minutes

| Time | Player experience |
| --- | --- |
| 0:00-0:30 | Menu reaches playable Campaign in two clicks; Veteran is clearly recommended |
| 0:30-1:15 | Unlimited planning, six readable jobs, preview of encounters 1-3, and two suggested openings rather than one forced answer |
| 1:15-2:30 | First shots teach range, targeting, gold, and Ring Pressure with no modal text |
| 2:30-3:30 | Two-way Swarm makes the player notice bends and paired pads |
| 3:30-4:30 | Armoured units introduce Expose; the upgrade comparison shows why it helps |
| 4:30-5:30 | Swift units teach Warden/priority and provide a safe Command Pulse moment |
| 5:30-6:30 | Armoured Swarm creates the first real Rush versus Clean Sweep choice |
| 6:30-7:30 | Flying is forecast and remains a soft counter, never a surprise zero-damage wall |
| 7:30-9:00 | Iron Warden is the first boss, with one telegraphed armour-aura mechanic |
| 9:00-10:00 | Contribution recap, first rule-changing draft, and Act II's Regenerator preview |

By minute ten the player has built 5-8 towers, chosen at least one
specialisation, used target priority once, seen pressure rise and recover, and
has resolved the first boss objective. If a first-time tester cannot explain
those five ideas without opening Help, onboarding fails.

## Game-feel contract

Every weapon family needs a complete response chain:

`aim -> anticipation -> recoil -> projectile/trail -> impact -> monster reaction -> death`

- Projectiles originate from the visible barrel, not the tower centre.
- Heavy shots kick the barrel and camera subtly; rapid shots animate mechanisms.
- Impacts use material-specific sparks, dust, bloodless magical residue, decals,
  and brief local light—not only bright spheres and rings.
- Monsters lean into turns, keep feet near the road, stagger on heavy hits, and
  use real locomotion, hurt, ability, and death clips.
- Audio has authored tower families, monster voices, impacts, ambience, music,
  and buses for master/music/effects. Browser audio unlock is explained once.

## Visual and technical dependency

High-detail downloaded models cannot be imported responsibly until the runtime
preserves their authored materials. The asset format and renderer need:

- UVs, tangents, material/submesh indices, and multiple animation frames.
- Base colour, normal, roughness, metallic, AO, and emissive inputs.
- Compressed texture arrays or atlases, mesh LODs, and browser payload budgets.
- Natural material colours; tower-family identity is a restrained emissive or
  paint accent rather than a full cyan/purple/tan wash.
- Blended terrain layers, authored road shoulders, decals, height variation,
  clustered foliage, and softer contact shadows.

Meshy assets may be used only when marked free with a commercial-compatible
licence, with source URL and licence stored in `assets/CREDITS.md`. Account
access is limited to downloads; no generation credits, purchase, upload, or
account change is permitted.

## Replay and mastery

Replayability comes from authored constraints, not permanent stat grinding.
Campaign completion unlocks ten Contracts such as mirrored spawn bias, costly
selling, commander-heavy waves, reduced pad count, and faster Veil cycles.
Players may combine up to three; each combination has a visible score multiplier
and medal threshold. Nothing sold for USD 5 depends on daily online access.

Run history stores seed, difficulty, Contracts, chosen branches/doctrines,
duration, peak pressure, spend ratio, and final score. The summary can replay a
compact pressure graph and copy the seed. Unlocks add sidegrades, cosmetics,
practice tools, and alternative Commands—never permanent damage that makes the
same difficulty easier merely because it was played longer.

## Delivery gates

### Gate 0 — browser correctness

- Edge shows a non-black, non-flat gameplay frame after Start.
- Automated screenshots require visible board pixels and at least three
  luminance bands; a changed PNG alone is not a pass.
- Ten-minute Edge run and wave-35 stress run have no device loss or black frame.
- WebGPU and WebGL2 fallback are tested separately.

### Gate 1 — strategic proof

- At least three substantially different Veteran strategies—Hunter/Hexer,
  Bombard/Warden, and Repeater/Skyguard—each clear at least 16 of 20 fixed seeds
  without sharing more than half their tower spend.
- Poor placement using the same purchases loses at least six waves earlier.
- No family supplies more than 45% of total damage across all winning plans.
- The current cheapest-upgrade/Siege-heavy planner must lose by encounter 20 after
  campaign migration; a random shallow build loses; a sensible beginner build
  clears Apprentice.
- Veteran winners spend 75–95% of earned gold and experience meaningful pressure
  without spending most of the run near the flood cap.
- Veteran winners spend 20-45% of combat time at 60-80 pressure, 2-12% above
  80, and less than 1% at the cap. A winner that never reaches 60 is too easy;
  one parked above 80 is an unreadable zombie run.
- Omitting one role remains winnable through a more expensive soft counter;
  omitting two complementary answers fails a relevant boss or mixed wave.
- Command Pulse changes the outcome of a close encounter but cannot rescue a
  zero-counter boss test or a deliberately poor placement test.
- Simulated 1x completion is 24-30 minutes and 2x is 13-17 minutes; neither
  repeated Send input nor a low frame rate shortens the encounter schedule.
- Exhaustive build/upgrade/branch/sell walks contain no positive-gold cycle;
  every refund is capped by invested gold and laps never change a unit's bounty.
- Applying a weaker poison, ignite, or slow can never truncate a stronger
  effect; delayed kills retain source attribution.
- A projectile snapshots attack type, target layer, splash, and riders when it
  fires, so selling its tower cannot turn a ground shot into an air hit.
- Crossfire, Expose, Shatter, Aim, control, Command contribution, and aura
  stacking all have automated cap/infinite-stall tests.

### Gate 2 — content and readability

- Every enemy name matches its model and every gameplay trait has a visual read.
- Every tower branch has a recognisable silhouette and its icon is rendered from
  the exact in-game model/material.
- No model overlaps a neighbouring pad, clips below terrain, faces backward, or
  skates around the circuit.
- Every boss mechanic is telegraphed before it causes damage or repair.

### Gate 3 — commercial presentation

- 1080p High targets 60 fps on a defined mid-range desktop; Low holds 30 fps on
  integrated graphics. Frame times, not only CPU instance counts, are recorded.
- Initial compressed browser download target is under 30 MB; optional high-res
  art can stream after the menu.
- UI remains readable at 1280×720, 1920×1080, ultrawide, and 150% display scale.
- Body text is at least 14 CSS px, critical controls are at least 44×44 CSS px,
  and critical contrast meets WCAG AA with symbols as well as colour.
- Build, upgrade, targeting, Send, pause, settings, and confirmation are fully
  keyboard/controller operable with visible focus; browser zoom works to 200%.
- The web build exposes named semantic controls instead of an empty-canvas
  accessibility tree, and no critical rule exists only in a hover tooltip.
- Music, ambience, UI, tower, monster, impact, and boss audio all have separate
  volume control and no clipping under a full wave.

### Gate 4 — player and product proof

- Eight of ten new players start encounter 1 without Help, and seven defeat the
  first commander on Apprentice on their first attempt.
- After ten minutes, eight of ten can explain Ring Pressure, forecast icons,
  tower specialisation, and one combo in their own words.
- Median first Veteran defeat lands in act II or III, never in the tutorial and
  never only on the final wave.
- Five blind Veteran testers produce at least three distinct successful build
  identities; no required opening is purchased by all five.
- Campaign, Legacy, Endless, ten Contracts, three Commands, twelve branches,
  run history, bestiary, settings, save/resume, and accessibility options are
  present before the game is advertised as a USD 5 release.
- A clean browser profile can start playing in under 15 seconds on the target
  broadband connection, and the first interaction explains audio unlock once.

## Implementation order

1. Freeze Legacy Circle data and introduce separate Campaign definitions; do
   not keep mutating extracted tables to serve both products.
2. Implement Ring Pressure, normalized economy, six base jobs, and the first
   seven-encounter act in a headless simulation before touching more content.
3. Prove two different openings, poor-placement failure, bounded spend, and the
   6-8 minute first-act timing with tests.
4. Replace the HUD with the six-card/three-threat interaction model and test it
   at 720p, 1080p, ultrawide, UI scale, mouse, keyboard, and controller.
5. Add the PBR/animation asset format and build one coherent vertical slice:
   one road sector, six jobs, six traits, Iron Warden, complete VFX/audio/UI.
6. Blind-playtest that vertical slice. Fix comprehension, strategy, and feel
   before producing acts II-IV.
7. Implement the remaining encounter timelines, bosses, doctrine cards,
   Commands, Contracts, and product progression against simulation gates.
8. Replace remaining art, run Edge/WebGL stress and frame-time tests, compress
   payloads, then complete commercial QA and store assets.

No full-content production begins until the vertical slice looks and plays like
the target. That prevents another large batch of assets from preserving the
wrong visual language.
