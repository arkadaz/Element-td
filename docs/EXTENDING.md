# Extending Elemental TD

The code is arranged so the things you are most likely to change are **data**, not
logic. Almost every addition below is a one-place edit.

| I want to... | Edit | Logic changes needed |
| --- | --- | --- |
| Retune a tower | `game/defs.rs` → `TOWERS` | none |
| Add a tower | `game/defs.rs` → `TOWERS` | none |
| Change a tier-3 fork | `game/defs.rs` → that tower's `forks` | none |
| Add a tower effect | `game/defs.rs` → `Special` + 2 match arms | small |
| Add an attack style | `game/defs.rs` → `Delivery` + 1 match arm | small |
| Add a monster | `game/defs.rs` → `Kind` + `kind_for` + `build_waves` | small |
| Retune difficulty | `game/defs.rs` → `build_waves()` | none |
| Redraw the level | `game/board.rs` → `WAYPOINTS` | none |
| Recolour anything | `view.rs` → `theme`, `ui.rs` → `pal` | none |
| Add scenery | `decor.rs` | none |
| Change the lighting | `gfx/shaders/solid.wgsl`, `Renderer` tunables | none |

---

## Add a tower

`TOWERS` is a slice, so there is no count to keep in sync. Append an entry:

```rust
TowerDef {
    id: "harpoon", name: "Harpoon", role: "Puller",
    desc: "Drags the front rank back down the road.",
    dtype: Damage::Physical,
    dmg: 30.0, rate: 0.8, range: 4.0, splash: 0.0, cost: 85,
    delivery: Delivery::Shot { speed: 20.0 },
    specials: &[Special::Knockback { dist: 0.8 }],
    color: [0.60, 0.78, 0.86],
    forks: [ /* two specialisations */ ],
},
```

That is the whole change. It appears in the build palette sorted by cost, gets
tier scaling, a tooltip built from its specials and forks, a minimap colour and a
3D model tinted to `color`.

**Two rules the tests enforce**, so keep them in mind:

1. `role` must be unique — if your new tower does the same job as an existing one,
   the design is weaker for having both.
2. Both forks must actually differ, from the base *and* from each other.

## Design the forks

A fork is the interesting decision in the game, so make it a real trade, not a
bigger number. The existing ones follow one of three shapes:

- **Trade rate for punch** (Marksman vs Repeater, Mortar vs Grapeshot).
- **Trade damage for utility** (Glacier freezes; Rime makes everything else hit harder).
- **Change the mechanism** (Storm chains five times; Overload chains twice and stuns).

`keep_base: true` keeps the base specials and adds the fork's on top;
`keep_base: false` replaces them. `delivery: Some(...)` swaps the attack style
entirely.

## Add a tower effect

1. Add a variant to `Special`.
2. Add a line to `Special::describe` so the tooltip reads correctly.
3. Handle it where it applies:
   - **on hit** (status effects) → `combat::on_hit_specials`
   - **at fire time** (damage modifiers, crits) → `combat::fire`
   - **on kill** (gold, contagion) → `Game::on_creep_died` / `combat::contagion`
   - **passive** (auras, income, interest) → `Game::rebuild_auras`, `tower_income`,
     `interest_rate`

The compiler will point you at every match that needs the new arm. `SpecialSet`
holds six inline, so a tower can carry base + fork specials without allocating.

## Add a monster

1. Add a `Kind` variant, then fill in `armor()`, `radius()` and `tell()`.
2. Add it to the unlock ladder in `kind_for()` — pick the wave where the player
   should first be forced to answer it.
3. Give it a stat line in `build_waves()` (`count`, `hp_mul`, `speed`).
4. If it needs a new behaviour, add the field to `WaveDef` and `Creep`, then handle
   it in `Game::step_creeps` (see `heal`, `shield` and `phasing` for the pattern).
5. Give it a body in `view.rs::creep` — the `match c.kind` arm is where its
   silhouette is decided.

Design note: a monster should punish exactly one lazy habit. If you cannot say
what a new monster forces the player to stop doing, it is decoration.

## Redraw the level

`WAYPOINTS` in `game/board.rs` is the road. Corners are rounded automatically and
the build pads are regenerated from the new shape — pads land on a checkerboard
1.15–2.35 tiles from the road, so a new route just works. `BW`/`BH` set the plot
size; the camera reframes itself.

## Add scenery

`decor.rs` runs once at startup from a fixed seed and hands the renderer a slab of
instances. Add a generator function and call it from `Decor::build`. Use
`is_free(board, p, clearance)` so nothing lands on the road or a pad.

## Visual tuning

`Renderer` exposes tunables:

```rust
bloom_strength   bloom_threshold      // glow
particle_drag    particle_gravity     // debris feel
light_dir        fog                  // mood - light_dir must match LIGHT_DIR in main.rs
shadows_enabled  bloom_enabled        // toggles
```

Camera framing is `CAM_PITCH_DEG` and `CAM_ZOOM` in `main.rs`; `Camera::frame_board`
auto-fits the board at any aspect ratio.

---

## Layering rules

Keep these boundaries and changes stay cheap:

- **`game/`** never mentions wgpu, egui or pixels. It works in tiles and seconds.
- **`view.rs`** is the only place that decides how the board looks.
- **`ui.rs`** only reads game state and calls public `Game` methods — the same ones
  a keyboard shortcut calls. Never mutate game fields directly from a widget.
- **`gfx/`** knows nothing about towers or monsters, only cubes, glows and particles.

## Determinism

`rng.rs` is a seeded xorshift and the simulation runs at a fixed 120 Hz timestep,
so a run is reproducible from its seed. Keep new gameplay randomness going through
`Game::rng`, never `SystemTime` or thread-local RNG — that is what would make
replays or lockstep multiplayer feasible later.
