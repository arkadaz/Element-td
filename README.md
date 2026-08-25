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
| `1`–`8` | pick a tower from the build palette |
| Click a pad | build there |
| Click a tower | select it — stats and commands appear in the bottom bar |
| `Shift`+click | build and keep the tower selected |
| Right click / `Esc` | cancel |
| `U` / `S` | upgrade / sell the selected tower |
| `Enter` | call the next wave early (pays 2 gold per second skipped) |
| `Space` | pause · `F` speed 1×/2×/3× · `B` bloom · `H` help |

## How it plays

- **The road never changes.** Placement is about coverage and overlap, not mazing.
- **Armour beats damage type.** Physical bounces off Heavy plate but shreds Warded
  casters; Magic is the mirror; Poison is never resisted and ignores shields. The
  wave preview tells you what is coming one wave ahead.
- **Tier 3 is a fork.** The last upgrade is a choice between two specialisations
  that play differently — Marksman or Repeater, Inferno or Furnace, and so on.
- **Interest.** You earn 5% of the gold in hand every wave, so holding money is a
  real strategy against building immediately.
- **Beacons multiply.** A support tower buffs everything in range, which makes
  tight clusters worth more than the same towers spread thin.

## How it's built

```
src/
  main.rs          app shell, input, the egui↔wgpu render callback
  math.rs          Vec3/Mat4, the diorama camera, ray picking, shadow matrix
  game/
    defs.rs        all balance data: damage types, 8 towers, 9 monsters, 50 waves
    board.rs       the fixed road and the build pads beside it
    mod.rs         entities, wave loop, economy, player actions
    combat.rs      targeting, firing, chains, armour and damage resolution
    fx.rs          particle spawn queue
  gfx/
    mod.rs         pipelines, shadow pass, MSAA, bloom chain, buffers
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

Fourteen tests covering the risky parts: index bookkeeping when monsters die
mid-iteration, splash into a 300-strong pack, shields versus poison, beacon auras
appearing and disappearing, and design invariants (every tower has a distinct
role; both tier-3 forks are genuinely different).

See [`docs/EXTENDING.md`](docs/EXTENDING.md) for how to add towers, monsters and
visual props.
