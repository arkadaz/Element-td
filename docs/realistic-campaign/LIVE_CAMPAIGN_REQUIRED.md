# The playable 36-wave limit is a delivery failure

Latest direct user feedback: the actual game still has only 36 waves and does not satisfy the five-hour duration or gameplay requirement. This requirement is not deferred or optional while graphics are improved. Both live campaign completion and graphics quality are necessary before delivery.

The 5h03m figure is currently a design/data-schedule calculation. It must not be described as verified playable campaign duration until the actual application runs that campaign from start to finish. A disconnected `CampaignState` or manifest test is insufficient.

## Complete the runtime path

1. Provide an explicit new Campaign entry in the actual browser menu. Show its chapter/encounter progress and estimated duration. Keep the old 36-wave mode separately and honestly labeled Legacy; preserve old saves. Do not silently resume Legacy when the user chooses the new campaign.
2. Wire the authored campaign definitions/state into the real Game update loop, spawn queues, creature traits, rewards, tower jobs/specializations, pressure, commander objectives, chapter transitions and HUD. Preview, spawn and reward code must read the same active encounter source.
3. Remove the Legacy 36-wave ending from the new campaign path. Locate every `N_WAVES`, `CAMPAIGN_WAVES`, `last_wave`, wave-index clamp, victory condition, menu/save default and HUD denominator. Keep those constants where they legitimately belong to Legacy. Changing one constant to 600 is not integration.
4. Implement chapter purse/loadout progression and choices, boss phase behavior and meaningful formation variations from DESIGN.md and ENCOUNTERS.md. Six hundred copies of the old wave or an endlessly growing health multiplier will not satisfy the brief.
5. Persist and resume the actual live campaign, including current chapter/encounter, deployment cursor, paid rewards, RNG, command cooldowns, upgrades and elapsed active time. Test the real save envelope rather than only serializing a detached campaign struct.
6. Complete the entire final encounter and objective cleanup before campaign victory. Chapter completion is not campaign completion. No victory at Legacy wave 36; no silent switch into an unlabeled endless mode.

## Evidence required

- Real browser: choose Campaign, play its first encounters, use Rush and 2x, save and resume the same live run. Verify actual new rules, not just updated labels.
- Bounded test fixture plus actual browser captures: cross encounters 35 -> 36 -> 37 without victory; cross chapter boundary 60 -> 61 with correct progression and persistence; cross 599 -> 600 and resolve the final objective before victory. Diagnostic fixtures must be clearly labeled as fixtures rather than organic full playthroughs.
- An accelerated test of the production simulation must use the same runtime adapters, spawning and transition conditions as the app. The fastest legal all-Rush route at 2x must consume at least 18,000 seconds of active real-equivalent gameplay time. Menus, loading, pause, inactive tabs and intentional idle padding do not count. A sum of definition durations alone cannot prove this.
- Assert conservation of all spawned groups/rewards across Rush, save/load and chapter transitions. Catch loop skips, hardcoded limits and duplicate payouts.
- Observe the opening, a middle chapter and finale for actual distinct decisions and encounter variety. Report gameplay tuning still needed honestly. Unit tests do not prove that the game is exceptionally fun.

## Implementation order clarification

The graphics milestone in NEXT_ENGINEERING_STEPS.md is meant to establish the rendering pipeline, not to leave the campaign disconnected indefinitely. Finish a concrete milestone, show real evidence, then integrate the live campaign path and propagate graphics quality through all assets. Do not wrap up after another foundation-only delivery. Retain browser-first Rust/WASM, responsive full-device layout, realistic assets and all previous requirements.

Your next progress report must explicitly distinguish: playable Campaign integration, playable Legacy, schedule-only data, verified duration, and remaining work. Never imply that the user can play five hours merely because a duration arithmetic test passed.
