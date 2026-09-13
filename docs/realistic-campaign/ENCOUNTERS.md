# Full campaign encounter authoring specification

Read with [DESIGN.md](DESIGN.md). All times are simulation seconds, before the selected speed multiplier. This document specifies the authoring structure; Terra implements data and tuning. No application code is supplied here.

## Index and duration contract

Global encounter number is `(chapter-1)*60 + localEncounter`, chapters 1-10, local encounters 1-60. Each local multiple of ten is a commander. Normal deployments have a final group at t=54 and a natural boundary at t=60. Rush can advance only the t=54-60 tail after every group is deployed. Commander schedules have a final combat group at t=120; their objective must be cleared before advancing. Do not offer commander Rush. Exact final-group counting and any catch-up update must preserve these minima.

Every normal encounter contains meaningful reinforcement groups across its whole 54s deployment. Divide the scheduled unit budget into six groups at t=0, 10, 20, 32, 44 and 54, with roughly 15/18/18/18/18/13 percent of its bodies respectively (use conserved deterministic rounding). Within groups, use short packets over 2-4s, except the final group, which may deploy instantaneously or extend the window. If this creates too many overlaps at high difficulty, reduce HP or weight per unit before reducing the intended visible body density. Commander groups arrive at t=0, 18, 38, 60, 82, 104 and 120. Telegraph the final objective before it enters.

## Eighteen normal formation templates

Fractions below are of authored threat budget rather than necessarily headcount. A template cannot introduce a locked trait: substitute its listed basic behavior until the chapter/encounter introduction table permits it. Keep a complete explicit resolved timeline in the final manifest so designers and tests can inspect exactly what will spawn.

| ID | Formation | Composition and direction | Player decision |
| --- | --- | --- | --- |
| 01 | Split scouts | Standard 70%, Swarm 30%; 50/50 directions | Establish both approaches |
| 02 | Brood tide | Swarm 85%, Standard 15%; three visibly separated packets per group | Place splash at a bend |
| 03 | Iron convoy | Armored 35%, Standard 65%; plated leaders before followers | Focus or Expose leaders |
| 04 | Running flank | Swift 35%, Standard 65%; swift direction alternates by group | Retarget a weak side |
| 05 | Broken column | Standard 50%, Swarm 30%, Armored 20%; leaders and rear screen separated | Avoid wasting precision on chaff |
| 06 | Reversal | Standard 60%, Swarm 40%; early groups 70/30, late groups 30/70 | Build balanced long-term coverage |
| 07 | Wing escort | Flying 30%, Standard 70%; altitude separated from ground pack | Prepare mixed targeting |
| 08 | Patient shields | Shielded 25%, Standard 75%; protected units at front | Rapid fire opens a precision window |
| 09 | Last surge | Standard 50%, Swarm 50%; 40% of budget in the final two groups | Reserve command for announced spike |
| 10 | Healing caravan | Regenerator 25%, Armored 20%, Standard 55% | Suppress the healing anchor |
| 11 | Needle flight | Flying 40%, Swift 20%, Standard 40%; stagger both altitudes | Choose focused or flak air branch |
| 12 | Two fronts | Armored 45% clockwise, Swarm 55% counter-clockwise | Different jobs serve different sides |
| 13 | Thin screen | Swarm 30% precedes Standard 40% and Shielded 30% | Clear screen before shield carriers |
| 14 | Recovering pack | Regenerator 35%, Swarm 65%; two clean gaps between columns | Compare burst and sustained damage |
| 15 | Relentless march | Resistant 30%, Armored 25%, Standard 45% | Damage must work without permanent control |
| 16 | Crosswind | Swift 35%, Flying 35%, Standard 30%; direction alternation per group | Use universal support and honest target priorities |
| 17 | Protected rear | Shielded 25%, Regenerator 20%, Standard 55%; important units trail screen | Retarget strongest/last deliberately |
| 18 | Combined rehearsal | Up to four unlocked behaviors, max two on any unit | Preview the next commander's demand |

Normal-index n=0..53 excludes commander encounters. First pass n=0..17 uses templates 01..18 in order. Second pass n=18..35 uses 12,04,10,02,07,15,03,11,06,14,08,16,05,13,01,17,09,18. Third pass n=36..53 uses 06,12,03,14,11,02,15,07,04,13,10,08,16,05,17,01,09,18. These permutations are a production scaffold, not sufficient variety alone: author each chapter's skin, unlocked trait set, group direction timing, leader behavior and difficulty variant in the final data.

Chapter one local encounters 1-4 are explicit hand-authored teaching exceptions: split scouts, brood tide, mild iron convoy, running flank. Keep the first armored group survivable with a reasonable generalist opening and show Expose as an advantage, not an instantaneous loss check. Show the first optional command before encounter four's announced late spike.

## Trait introduction and limits

| Region / local point | First use | Safeguard |
| --- | --- | --- |
| Ch1 E1 | Standard and Swarm | Both directions visible immediately |
| Ch1 E3 | Armored | First convoy has modest mitigation, plenty of forecast |
| Ch1 E4 | Swift | One brief packet, with an available control answer |
| Ch1 E21 | Flying | Forecast from E18; enough income for an air-capable response |
| Ch2 E11 | Shields | First shielded group isolated with visible breakable pips |
| Ch3 E1 | Regenerator | Introduce without simultaneous shields |
| Ch4 E21 | Resistant | Display shortened control clearly; no full immunity surprise |
| Ch5 onward | Two traits on selected elite units | Never more than two; no unit both fully uncounterable and mandatory |

Before unlocking Flying, use the same formation timing with ordinary ground scouts; before Shields, use a durable but unshielded Standard; before Regenerator, use a tougher Standard; before Resistant, use ordinary Armored. Label the actual resolved traits in preview. Do not show a flying icon for a ground substitute.

Nightmare changes packet overlap, direction imbalance and trait combinations within the same duration budget. Apprentice reduces pressure and HP, expands forecasting and offers contextual hints. Neither difficulty hides information or bypasses campaign length. Retain RNG seeds and authoring IDs so a loss is reproducible.

## Five recurring commander classes

Each chapter uses the following five lieutenant mechanics at E10/E20/E30/E40/E50, then its named signature boss at E60. Their material kit and escort formations match the region. Unlock their harder mechanic only when its counter has already been introduced; early chapters use the basic variant.

| Slot | Commander | Phase structure and counter |
| --- | --- | --- |
| E10 | Bulwark | Three plate segments visibly crack at 75/50/25% health; focused heavy/Expose attacks help; escorts punish ignoring the swarm |
| E20 | Hunt Captain | Marks which direction receives the next fast escort packet 4s before arrival; aim/slow placement rewards preparation |
| E30 | Brood Keeper | Three brood releases tied to one-time thresholds, never every hit; late objective phase remains scheduled at t=120 |
| E40 | Ward Keeper | Visible shield charges; repeated hits open a 4s vulnerability. Early locked-shield variant uses a physical guard stance with only mild mitigation |
| E50 | Siphon Marshal | Heals a limited number of escorts in a narrow nearby band; suppression interrupts it. Early chapters use an armor-support version with Expose counterplay |

Commanders do not repeat the exact same encounter six times: phases alter formation geometry, not just colors and health. Early body phases may retreat or break their armor rather than fake death; make the final objective's arrival and identity explicit. Rewards pay once per authored object. Never respawn a defeated phase invisibly just to enforce duration.

## Signature boss behavior details

- **Iron Warden:** break plate segments while escorts traverse both approaches. Phase two exposes joints and accelerates the escort rhythm. Final shield-bearer enters at t=120; clearing it permanently exposes the Warden. No invulnerability unless its visible carrier is alive.
- **Thorn Marshal:** telegraph two already-empty shoulder tiles with roots; briefly prevents new construction there, never deletes a purchased tower. The player may use other plots or kill a visible root bearer. No route blockage.
- **Mire Matriarch:** 75/50/25% health broods, each trigger once. Final brood's visible nest bearer enters at t=120. DoT and splash have complementary value. Adds count toward authored budget.
- **Quarry Colossus:** plate-bearing vanguard screens fragile support carts; damage to carts removes shield segments from the Colossus. No camera-relative front/back hit detection requirement unless reliably implemented.
- **Furnace Regent:** vent cycle 8s closed / 4s exposed. Closed reduces damage but does not nullify all towers. A heat gauge forecasts exposure; late pressure wave tests saved command timing.
- **Storm Talon:** ground-perched escort objective and flying Talon alternate focus; forecast each altitude phase. At least two affordable air-capable jobs can win. No targetless time for the entire defense.
- **Veil Astronomer:** ten shield pips, max one removed per independent attack, then a 4s exposure window. Corruption can suppress a nearby charger. Define multi-projectile behavior explicitly to prevent accidental instant break exploits.
- **Pale Shepherd:** two visible lieutenants on opposing streams, with capped regeneration. First death leaves a 12s warning before partial revival; killing the healer disables revival. Both deliberate split fire and suppress-then-focus strategies must work.
- **Obsidian Host:** control-resistant charge begins after a 4s telegraph; charge has limited duration and affects speed/pressure rather than teleporting past defenses. Gap after the charge rewards prepared burst.
- **Circle Tyrant:** recaps plates, split lieutenants and a final exposed core, in separately telegraphed phases. No simultaneous pile of every mechanic. Final t=120 objective unlocks the core; full campaign victory requires that core and any required escorts dead.

## Boss fairness and schedule feasibility

Each boss has real targets to fight throughout reinforcement windows, meaningful counters that are already available, and at least two viable strategies at appropriate investment. Avoid long total invulnerability. If a strong defense clears an early phase, show the next incoming formation within a few seconds through the authored reinforcement plan; fill actual encounter content rather than pausing the clock in an empty arena.

Initial boss health is a tuning input relative to that chapter's expected sustained DPS, not an arbitrary 120s health sponge. A competent build must have spare capacity to handle escorts and a dangerous but recoverable late spike. Commander counts and their min duration must remain unchanged when balancing HP or reinforcement volumes.

## Progression cadence inside a chapter

- Every encounter: choose whether to spend and whether to Rush.
- Every five encounters: one brief contextual tactical observation or loadout-preview offer; do not pause automatically.
- Every ten encounters: commander reward, one relocation credit (cap two banked), one of three mutually exclusive chapter perks.
- Every twenty encounters: a specialization opportunity with an explicit trade-off rather than an unconditional multiplier.
- Every sixty encounters: checkpoint, region accomplishment, new loadout option and next arena preparation.

Perks should alter a recognizable job: longer narrow precision coverage but slower retargeting; wider splash but less single-target damage; more control duration but reduced base damage. Avoid endless stacking of generic +damage. Give three candidate perks from different roles and let the player decline; never disable their only air counter due to RNG.

## Required authoring outputs

Terra must produce an inspectable campaign manifest for all 600 resolved encounters with chapter/local/global IDs, duration, groups, counts, traits, directions, reward totals, mechanic IDs and telegraph times. Generate a human-readable schedule report from the same data. Check missing IDs, duplicate rewards, mismatched preview vs runtime, impossible counter introduction, schedule truncation, and total minimum duration.

Automated content checks must supplement, not replace, observations from playing the opening, a middle chapter and the finale. Report repeated-feeling patterns honestly and iterate them before claiming the full campaign achieves the intended variety.
