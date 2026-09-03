# Elemental TD

A 3D tower defense for the browser, written in **pure Rust**. Monsters walk a
fixed road; you build towers on the stone pads beside it and try to survive 50 waves.

- **Rendering**: [`wgpu`](https://wgpu.rs) → **WebGPU** in the browser, automatic
  **WebGL2** fallback, native Vulkan/DX12 on the desktop.
- **Look**: real perspective camera, shadow-mapped key light, 4× MSAA, HDR bloom,
  distance fog. ~180 fps at 1080p in release.
- **UI**: [`egui`](https://github.com/emilk/egui) — also pure Rust. No HTML or JS for game chrome.
- **No art assets.** Every tree, tower and monster is built from one instanced cube.

The design — why each tower and monster exists — is written up in
[`docs/DESIGN.md`](docs/DESIGN.md).

## Run it

### Web

```bash
cargo install trunk                # once, if you don't have it
rustup target add wasm32-unknown-unknown
trunk serve --release --port 8080  # then open http://127.0.0.1:8080
```

`trunk build --release` writes a static site to `dist/` that you can host anywhere,
GitHub Pages included.

### Native

```bash
cargo run --release
```

Same code, same shaders, real backtraces. In debug builds `G` grants gold and `T`
scatters towers, for quick playtesting.

## Controls

| Input | Action |
| --- | --- |
| `1`–`9` | pick a tower from the build palette |
| Click a pad | build there |
| Click a tower | select it — stats and commands appear in the bottom bar |
| `Shift`+click | build and keep the tower selected |
| Right click / `Esc` | cancel |
| `U` / `S` | upgrade / sell the selected tower |
| `Enter` | call the next wave early (pays 2 gold per second skipped) |
| `Space` | pause · `F` speed 1×/2×/3× · `B` bloom · `H` help |

## How it plays

- **The road is a circle, and there are no lives.** Monsters walk a closed
  circuit with no exit, so nothing ever gets past you - what you defend is a
  *rate*. Anything your towers cannot kill comes round again, and again, and the
  ring fills up. You lose when more than 320 are circling at once. Waves arrive
  on a fixed 42-second clock whether the last one is dead or not, and there is no
  build phase.
- **Waves are streams, not bursts.** A wave is a hundred-odd monsters arriving
  one at a time across its whole period. Area damage, chains and multishot beat
  one enormous hit, because what kills you is throughput.
- **You draft elements, not towers.** Twenty times over the campaign you pick one
  of six elements from three offered. Each element unlocks its own tower, and
  each *pair* of elements you hold unlocks the tower between them — six pure
  towers and fifteen duals, twenty-one in all. No two runs get the same roster.
- **Depth or breadth, never both.** Every essence of an element raises the level
  ceiling of every tower using it by one, and a dual tower reads whichever of its
  two elements you hold fewer of. Six of one element maxes its pure tower; six of
  each of two maxes the dual between them. Twenty essences do not stretch far.
- **Armour beats damage type.** Physical bounces off Plated but shreds Warded;
  Magic is the mirror and the only real answer to Ethereal; Fire loves an
  unarmoured crowd and barely warms a ghost; Toxic is never resisted and ignores
  shields. The wave preview tells you what is coming one wave ahead.
- **Five towers cannot shoot upwards.** Something on your board has to answer the
  air, and from wave 45 every escorted wave crosses the layers.
- **Interest, and calling early.** You earn 5% of the gold in hand every wave, and
  sending a wave early pays 2 gold a second — the only speed control that is also
  a decision.
- **Groves multiply.** A support tower buffs everything in range, which makes
  tight clusters worth more than the same towers spread thin.

## How it's built

```
src/
  main.rs          app shell, input, the egui↔wgpu render callback
  math.rs          Vec3/Mat4, the diorama camera, ray picking, shadow matrix
  game/
    defs.rs        all balance data: elements, 21 towers, 14 monsters, 80 waves
    board.rs       the closed circuit and the 116 build pads around it
    mod.rs         entities, wave loop, economy, player actions
    combat.rs      targeting, firing, chains, armour and damage resolution
    fx.rs          particle spawn queue
  gfx/
    mod.rs         pipelines, shadow pass, MSAA, bloom chain, buffers
                   (two render passes at the cheap preset, six at Ultra)
    draw.rs        the drawing vocabulary (cubes, bars, glows, rings)
    shaders/*.wgsl solid / shadow / billboard / post
  decor.rs         static set dressing: trees, rocks, fences, lamps, cliffs
  view.rs          game state → 3D scene (all visual decisions)
  ui.rs            resource strip, scoreboard, minimap, command card, palette
  rng.rs           deterministic xorshift
```

### Why it's fast

- **One instanced draw for the whole board.** Terrain, trees, towers, monsters and
  shots are all the same unit cube with per-instance transform and colour. A busy
  frame is ~4 draw calls, not thousands.
- **Particles never touch the CPU after spawn.** Each stores only spawn state; the
  vertex shader solves position from elapsed time under drag and gravity. The
  buffer is a fixed ring, so spawning is one `write_buffer`.
- **Static decor is built once** and blitted into the frame as a slab.
- **A monster's position is one number** — distance along the road — so there is no
  pathfinding cost at all, however many are on screen.
- **Spatial hash for targeting**, so towers only test nearby monsters.
- **Fixed-capacity everything.** No allocation on the hot path; the draw list is
  double-buffered with the render callback by swapping, not copying.
- **Fixed 120 Hz timestep**, so 3× speed and a slow frame behave identically.

## Tests

```bash
cargo test
```

Sixty-three tests covering the risky parts: index bookkeeping when monsters die
mid-iteration, splash into a 300-strong pack, shields versus toxic damage, auras
appearing and disappearing, saves that must never rebuild a board the game would
refuse, HUD layout at every window size, and design invariants (every tower has a
distinct role; every element pair maps to exactly one tower; no wave can be
pinned in place forever).

The balance is a test too: `a_sensible_build_clears_the_campaign` plays a whole
eighty-wave run with a deliberately unsophisticated bot and checks it wins with
the ring 80% full at its worst, while `two_elements_are_not_enough_however_deep_they_go`
and `ignoring_the_air_loses_the_run` check that the builds which *should* lose
actually do. The air layer gets a controlled experiment rather than a whole run:
one heavy ground board, held fixed, fed six ground waves and then six air waves.
It finishes the first with **zero** monsters circling and the second with
**360** - which is the layer split, demonstrated rather than asserted.

See [`docs/EXTENDING.md`](docs/EXTENDING.md) for how to add towers, monsters and
visual props.
