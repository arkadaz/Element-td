# Release validation

Verified on 2026-09-13: `woodland-command-tempo-20260913.100`.

## Artifact

- WASM: `green_circle_td-5f68857d4c0ebaf7_bg.wasm`
- SHA-256: `8C3667AE8B0A7E40C2FCC6F3859FFE66C054C38E7522B76156D92E565E2CC914`
- Trunk release build succeeded. Generated bundles are local artifacts, not source.

## Checks

- Final pre-merge `cargo test --release --workspace`: 172 passed in total
  (168 game tests and 4 protocol tests), 0 failed, 14 optional diagnostics ignored.
  This run includes the final pending-reward keyboard guard.
- Native game suite: 168 passed, 0 failed, 14 optional diagnostics ignored.
  This includes the full mixed Veteran 600-encounter victory regression, the
  frozen-after-121 defeat gate, Legacy, actual Multi volleys, saves and placement.
- The final pending-reward keyboard guard passed compile and focused layout checks.
- Final desktop WebGPU and actual WebGL2 (`Gl`) fallback passed placement, camera,
  meaningful rendering and save/resume checks with zero runtime errors. Both runs
  identified the exact WASM above.
- Portrait 390x844 and landscape 844x390 passed full smoke/resume before the final
  keyboard guard. The final artifact's reward layout and input were then checked
  directly at those sizes and desktop 1440x900, with screenshots inspected.
- In the isolated pending-reward UI fixture, Escape, Space, U and backdrop input
  preserved paused state, active time, gold and all three choices. Selecting
  Overdrive resumed combat and saved the expected perk ranks.

## Challenge evidence

Legal 2x diagnostics use earned gold and production placement/combat. An 860g
Poison entrance board frozen after encounter 10 now loses at encounter 60 on
Nightmare; a frozen Siege policy loses at encounter 28. Reinvesting Siege/Air and
focused Corruption policies reach encounter 60 on Veteran and Nightmare. The
focused trace records an actual 500g Corruption purchase during encounter 10.

Nightmare non-focused commander fights lasted about 54-63 simulation seconds.
Focused spending reduced several to roughly 7-10 seconds, while encounter 50
remained about 45 seconds. Veteran is more forgiving: the frozen Poison policy
can still clear the first 60 encounters with long boss cleanup.

## Limits and reproduction

These scripts reject reproduced exploits and compare specific policies; they do
not prove every strategy balanced. Browser viewport coverage uses desktop Edge
emulation, not physical-device or all-browser certification. The reward fixture
is synthetic UI evidence, not balance evidence. The authored 36,360-second
schedule is a simulation timing regression, not a five-hour human playtest.

Run `cargo test --locked --release --workspace`, then `trunk serve --release` and
`node tools/browser_smoke.mjs http://127.0.0.1:8080` for the main checks. Use
`GREEN_TD_WINDOW_SIZE=390,844` or `844,390`, `GREEN_TD_FORCE_WEBGL2=1`, and
`GREEN_TD_VERIFY_RESUME=1` for the viewport/fallback/resume variations. Run browser
variants sequentially because the resume harness compares the encounter number.

Superseded agent handoffs and raw local logs were removed from the release source
set during repository cleanup. On the authoring machine they remain recoverable
under ignored `.local/archive/`; this portable record replaces their transient
server addresses, process IDs and temporary screenshot paths.
