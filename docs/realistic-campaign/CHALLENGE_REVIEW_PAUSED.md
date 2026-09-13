# Challenge review — design only, implementation paused

User feedback on 2026-09-12: the actual game is too easy and not challenging. Terra was explicitly interrupted in the preceding turn. This document records a focused next balance task; it does not authorize restarting Terra or any automatic continuation. No game code was changed during this review.

## Evidence from current production code

- campaign_body_spec in src/game/mod.rs scales basic health by 1 + 0.085 * (chapter - 1) + 0.003 * local encounter. That ranges from 1.003 at the opening to 1.945 at C10E60. Shield/armor/traits also affect difficulty, so this is not a complete effective-health analysis, but the basic growth is modest relative to a persistent expanding defense.
- Campaign starts with 600 gold. resolved_encounter rewards provide 19,050 gold over chapter one if earned, before other grants. finish_campaign_if_ready adds chapter supplies while keeping the existing defense. The code inspected does not reset the tower investment at those transitions. Measure actual purchasing power and damage growth rather than assuming the old bounded-economy design was implemented.
- campaign_pressure_capacity returns fixed 420/360/300 for Classic/Veteran/Nightmare. Weighted lap pressure stops growing after four laps (maximum multiplier 1.6); breach grace is eight simulation seconds. These values may allow weak threats to persist without forcing meaningful action.
- check_end handles Campaign pressure and completion, then returns before the non-Campaign commander lap-limit check. Commander resolution waits for its encounter's survivors. A remaining commander with little weighted pressure does not acquire that lap-based failure consequence. Inspect its complete ability path before deciding which threat to add.
- Difficulty descriptions still discuss Legacy wave-based tightening. Audit the selected mode's actual rules and displayed explanation together. Do not use those descriptions as evidence that Campaign actually tightens its capacity.

These are source-based hypotheses for the user's report, not a completed balance playtest. The user's chosen difficulty, tower composition and encounter were not provided in this turn.

## Bounded next implementation task, if resumed

Limit the next pass to challenge tuning and production-runtime evidence. Do not restart the entire graphics/UI backlog. First reproduce the current campaign with ordinary player purchases, no QA immortality or artificial damage. Run a no-build baseline, an opening-only build followed by no purchases, single-family spam, and at least three mixed builds with different positions. Log encounter reached, pressure percentiles/peaks, losses, surviving laps, banked gold, purchased damage/counter coverage and commander cleanup time. Include free-grass overlap coverage: formerly rare tower clusters can now concentrate much more damage/support.

Set the intended Veteran target: an opening defense should get the player started, but should not survive the first chapter unattended. Introduce an affordable, previewed counter decision before the first commander. Mixed competent builds should remain viable with distinct strengths; there must not be one mandatory exact build order or an untelegraphed immunity that invalidates the player's entire investment. Use measured targets and multiple seeds; do not rig fixtures to force preferred outcomes.

Close the commander consequence gap with an explicit, visible escalating threat tied to its actual mechanics: for example a telegraphed reinforcement pulse and stronger pressure on repeated laps, followed by a clear bounded failure condition. Preserve a recovery opportunity and accessible counter. Apply only in the intended modes and state the rule in the HUD. A hidden timer or another huge health pool is not an adequate boss design.

Reconcile progression power with threat budgets using actual effective damage, coverage, control uptime and incoming effective health per simulation second. Tune the dominant low-cost tower/upgrade or exploitable support combination first where measurements establish it. Check persistent investment and income across chapters; avoid a compounded money advantage that outpaces nearly flat threats. Do not confiscate towers or rewrite existing saves without a designed, disclosed transition. Prefer a versioned new-run balance profile if live saves would otherwise be materially reinterpreted.

Build difficulty through mixed formations, priority targets, armor/air/control tradeoffs and manageable overlapping arrivals. Keep a satisfying visible horde. Lowering pressure capacity alone can cause sudden failure at rapid speeds; any adjustment needs a readable warning and reaction window. Do not make play tedious with uniform HP inflation, automatic counter-spawning based on the player's build, or forced empty waiting.

## Evidence and stopping point

Report current versus tuned results for the same deterministic action scripts, and inspect real browser opening/commander play at the user's selected speed. Validate displayed difficulty, pressure, rewards, bosses, save compatibility and unaffected placement/zoom. Recheck duration only if timings or transition rules change. Human playtesting remains necessary to establish enjoyment.

After this single balance pass and its focused checks, return the playable URL, measured differences and remaining tuning questions, then stop. Do not automatically resume graphics or unrelated backlog work. This task remains unstarted until the paused implementer is explicitly resumed.
