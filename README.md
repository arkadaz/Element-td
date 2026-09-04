# Green Circle TD

A playable port of the Warcraft III custom map **GREEN TD 9.3c PEIN.w3x**,
written in **pure Rust** and rendered with wgpu. It runs in the browser and on
the desktop from the same code.

The Warcraft III map remains the content foundation: 131 towers and their full
upgrade graph, 36 waves, abilities, armour rules and starting economy are
extracted by [`tools/`](tools/README.md). The standalone edition layers a
compact solo arena, visible difficulty rules, tempo rewards, command upgrades,
endless play and run ratings over that foundation.

- **Rendering**: [`wgpu`](https://wgpu.rs) - WebGPU in the browser, automatic
  WebGL2 fallback, native Vulkan or DX12 on the desktop.
- **Look**: a compact forest battlefield with staged CC0 towers, command-card
  portraits rendered from those exact 3D models, triplanar albedo/normal ground
  detail, shadowed daylight, MSAA and restrained bloom at the top preset.
- **UI**: [`egui`](https://github.com/emilk/egui), also pure Rust. A carved RTS
  console separates the minimap, selected-defense dossier and 4x3 command card.
- **Sound**: original Web Audio cues for commands, waves, bosses and throttled
  weapon impacts; no borrowed samples and no combat-noise pile-up.
- **Models**: 44 staged Quaternius combat turrets, 56 Poly Pizza enemy/world
  models, and Kenney Nature Kit scenery, baked into one runtime mesh pack.

What was taken, what had to be worked out, and the one number that had to be
invented are written up in [`docs/DESIGN.md`](docs/DESIGN.md).

## Run it

### Web

```bash
cargo install trunk                # once, if you don't have it
rustup target add wasm32-unknown-unknown
trunk serve --release --port 8080  # then open http://127.0.0.1:8080
```

`trunk build --release` writes a static site to `dist/` that you can host
anywhere, GitHub Pages included. [`docs/DEPLOY.md`](docs/DEPLOY.md) covers the
container that serves both the site and the lobby.

### Native

```bash
cargo run --release
```

Same code, same shaders, real backtraces. In debug builds `G` grants gold and
`T` scatters towers, for looking at something quickly.

## Controls

| Input | Action |
| --- | --- |
| `1`-`9`, `0`, `-` | pick any of the eleven command-card slots |
| Click a plot | build there |
| Click a tower | select it; its stats and commands appear in the bottom bar |
| `Shift`+click | build and keep the same tower selected for the next plot |
| Right click / `Esc` | cancel |
| `U` / `S` | upgrade / sell the selected tower |
| `Enter` | rush after the current stream deploys (pays for time skipped) |
| `Space` | pause |
| `F` | speed 1x / 2x in campaign; 3x unlocks in endless |
| `B` | cycle the quality preset |
| `H` | help |
| Mouse at top/bottom edge or WASD/arrows | pan the camera |
| Middle-drag / mouse wheel | pan / zoom the battlefield |

## How it plays

- **The lane splits around a loop, and there are no lives.** Each creep takes
  the clockwise or counter-clockwise branch at the source and keeps circling,
  so nothing ever leaks. What you defend is a *rate*. Anything your towers cannot
  kill comes round again, and the lane fills up. Capacity is **700** on Classic;
  Veteran contracts from **450 to 280** after wave 9, and Nightmare from
  **400 to 200** after wave 7.
- **Waves are streams.** A wave's whole count - sixty to a hundred and sixty
  creeps, mostly - arrives evenly across forty-five seconds, on the map's own
  clock, whether the last wave is dead or not. Throughput is what kills you, not
  any one creep.
- **Eleven towers can be bought; there are 131.** The other 120 are reached by
  upgrading, and upgrading is a graph rather than a ladder. The ten gold Single
  shot Tower is a *seed*: it becomes a Slow, Poison, Critical, Troll or Fire
  Tower, or a One-Strike Kill Tower, and none of those six can be bought at any
  price. The Aura Tower forks into Damage or Speed at every rung. The King Tower
  opens the four Super towers, a hundred thousand gold each.
- **Armour is a number, not a class.** It climbs to 700, which on Warcraft III's
  curve leaves 2.3% of a hit. That is why the ladders end in six figures of
  damage, and why a tower two rungs behind the wave is not slightly weak but
  useless.
- **Every fifth wave is Immune** and takes five percent from everything except
  Chaos and Hero damage. Chaos, Destruction, Troll and the One-Strike Kill Tower
  exist for those waves; a board with none of them is doing a twentieth of its
  damage every fifth wave, and what it fails to kill stays on the lane.
- **Five waves fly.** Nine of the ten Air Tower rungs can hit nothing but the
  air, and Siege, Chaos and Destruction can never touch it - which is most of
  the splash damage in the game.
- **Build beside the fight.** Fifty-six inner/outer shoulder pads follow the
  circuit at 1.4–2.8 tiles from the road with at least 2.2 tiles between
  centres. Towers never overlap, decorative dead ground is not presented as a
  fake choice, and each placement commits valuable lane coverage.
- **Tempo is a decision, not a skip button.** Rush unlocks after the current
  stream has fully entered, then pays a bonus for stacking the next stream on
  every survivor. Waiting for the timer while keeping the ring empty pays a
  Clean Sweep bonus instead. Campaign speed is capped at 2x; 3x unlocks after
  victory for endless play.
- **Hard modes evolve during the run.** Vanguards resist control, surviving
  enemies accelerate across their first four laps, and waves 10, 20 and 30
  pause for a permanent damage, attack-speed or range command upgrade.
- **Boss banners have one commander.** The commander has eight times the wave
  health and repairs nearby escorts for 0.8% health per second. Strongest
  targeting focuses it; Corruption shuts the repair down for two seconds.
- **Runs have an ending worth comparing.** Victory requires an empty ring after
  wave 36, then reports a difficulty-weighted score and command rating. Endless
  mode remains available after the campaign.

## How it's built

```
src/
  main.rs          app shell, input, the egui-to-wgpu render callback
  math.rs          Vec3/Mat4, the fixed camera, ray picking, shadow matrix
  game/
    greentd.rs     GENERATED: 131 towers, the upgrade graph, 36 waves
    greentd_map.rs GENERATED: the map's terrain grid and the player's lane
    greentd_types.rs  the types those two are written in, and the Warcraft III
                      rules they depend on: the attack table and the armour curve
    defs.rs        the layer the game reads the tables through, plus the economy
    board.rs       the compact circuit and build plots around it
    mod.rs         entities, difficulty, tempo, doctrines and player actions
    combat.rs      targeting, firing, splash, bounces, auras, damage resolution
    fx.rs          particle spawn queue
  gfx/
    mod.rs         pipelines, shadow pass, MSAA, bloom chain, buffers
    draw.rs        the drawing vocabulary (cubes, bars, glows, rings)
    mesh.rs        primitives plus the baked CC0 model pack
    shaders/*.wgsl solid / shadow / billboard / post
  view/            game state to 3D scene: towers.rs, monsters.rs, models.rs
  decor.rs         baked forest scenery, cliffs, portal and ground details
  audio.rs         original browser SFX synthesis and combat mix throttling
  ui.rs            onboarding, threat intel, minimap, command card and modals
  menu.rs, net.rs  title screen and the scoreboard-only lobby
  save.rs          resuming a run
  rng.rs           deterministic xorshift
tools/             the extractor: MPQ reader, object-data parser, emitters
```

The two `greentd*.rs` files are generated by `tools/emit.py` and
`tools/emit_map.py` and are never hand-edited. To regenerate them from a
different map version, see [`tools/README.md`](tools/README.md).

### Why it's fast

- **Bucketed instancing across the board.** Terrain, scenery, towers, creeps and
  shots are grouped by their shared mesh and material. A busy frame is a bounded
  set of mesh-bucket draws, not one draw call per object.
- **Two render passes** at the Performance preset, three at Balanced, six at
  Ultra. Pass count, not instance count, is what decides whether this runs in a
  browser - on a packed board the simulation and the draw-list build together
  cost well under a tenth of a millisecond, and `bench_tests.rs` keeps that true.
- **Static geometry is uploaded once.** Terrain, scenery and the corridors never
  change, so they are never rebuilt.
- **Particles never touch the CPU after spawn.** Each stores only its spawn
  state and the vertex shader solves position from elapsed time under drag and
  gravity. The buffer is a ring, so spawning is one `write_buffer`.
- **A creep's position is one number** - how far along the loop it has walked -
  so there is no pathfinding at all, however many are circling.
- **Spatial hash for targeting**, so towers only test nearby creeps.
- **Fixed 120 Hz timestep**, so fast-forward and a slow frame behave identically -
  and on the desktop the frame rate is held to a cap rather than rendering
  frames the display will never show.

## Tests

```bash
cargo test --release
```

The suite currently contains 138 tests: 125 automated checks and 13 deliberate
rendering/diagnostic captures.

**The extraction is faithful** (`game/greentd_tests.rs`). The family sizes, the
numbering, that the long ladders climb, that the six branches off the ten gold
seed cannot be bought any other way, that every hundred-thousand gold tower
deals Chaos, that Immune arrives every fifth wave, that no wave sends nothing,
and that the abilities carry the map's own numbers - a 30% crit for four times
damage on Critical 1, fifteen points of armour stripped by Corruption 1, ten
targets at once on the Super Multi.

**The rules are Warcraft III's.** Immune waves fall only to Chaos and Hero
damage; armour reduces by `1/(1+0.06A)`; Siege, Chaos and Destruction cannot
touch the air at any rung, and the first Air Tower cannot touch the ground.

**The risky parts hold.** Same-frame kills stay index-stable until every splash
and multishot resolves; projectiles and effects have browser-safe ceilings;
towers can be sold from under shots; auras appear and disappear safely; saves
never rebuild a board the game would refuse; the HUD survives every window
size; wave 35 remains inside every live combat/visual queue; and no wall of
rooting towers can pin a wave in place forever.

**It is a game.** `a_sensible_build_clears_the_campaign` plays all thirty-six
Classic waves, while `a_sensible_build_can_master_veteran` proves the intended
Veteran campaign remains beatable when the player reads counters and uses the
three command drafts.
`a_ground_only_board_drowns_in_the_air` builds sixty maxed Siege towers and has
to lose every flyer of the first air wave. `kill_money_keeps_pace_with_the_roster`
checks that the one invented number still buys the roster it has to buy.

Two diagnostics print what was extracted, rather than asserting anything:

```bash
cargo test --release show_the_extracted_roster -- --ignored --nocapture
cargo test --release show_the_board -- --ignored --nocapture
```

## Where the numbers came from

`GREEN TD 9.3c PEIN.w3x` is not redistributed here - it is not ours to give
away. The generated tables are checked in, so nothing is needed to build or play
the game. Put your own copy of the map in the repository root if you want to
re-extract them.
