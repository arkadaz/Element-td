# Extending Elemental TD

The code is arranged so the things you are most likely to change are **data**, not
logic. Almost every addition below is a one-place edit.

| I want to... | Edit | Logic changes needed |
| --- | --- | --- |
| Retune a tower | `game/defs.rs` → `TOWERS` | none |
| Retune an element's reach | `game/defs.rs` → `FREE_TIERS`, `TIER_PER_ESSENCE` | none |
| Change the draft schedule | `game/defs.rs` → `ESSENCE_WAVES` | none |
| Add a tower effect | `game/defs.rs` → `Special` + 2 match arms | small |
| Add an attack style | `game/defs.rs` → `Delivery` + 1 match arm | small |
| Add a monster | `game/defs.rs` → `Kind` + `kind_for` + `shape_of` | small |
| Retune difficulty | `game/defs.rs` → `HP_EXP` | none |
| Redraw the level | `game/board.rs` → `WAYPOINTS` | none |
| Recolour anything | `view/mod.rs` → `theme`, `ui.rs` → `pal` | none |
| Add scenery | `decor.rs` | none |
| Change the lighting | `gfx/shaders/solid.wgsl`, `Renderer` tunables | none |

---

## The roster is not a list, it is a lattice

There are exactly twenty-one towers because there are six elements: one **pure**
tower per element, and one **dual** tower per unordered pair. `pure_index` and
`dual_index` compute positions from the declaration order rather than searching
it, and `the_roster_is_exactly_six_pures_and_every_pair` asserts every pair maps
to a distinct tower, findable from either side.

So you do not add a twenty-second tower. You either **retune an existing one**,
or you add a **seventh element** - which is seven new towers (one pure, six
duals) and a column in the armour table, and is a design decision rather than an
edit.

## Retune a tower

```rust
TowerDef {
    id: "abyss", name: "Abyss", role: "Pull",
    desc: "Drags them back down the road they just walked.",
    elem: (Water, Some(Dark)),
    dtype: Damage::Magic, targets: Targets::Both,
    dmg: 27.0, rate: 0.80, range: 3.5, splash: 0.45, cost: 195,
    delivery: Delivery::Shot { speed: 16.0 },
    specials: &[Special::Pull { dist: 0.85 }, Special::Slow { amt: 0.20, dur: 1.5 }],
    color: [0.44, 0.42, 0.86],
},
```

`elem` decides everything about availability: which essences unlock it, how far
it upgrades, which pips the build card shows, and which base and crown its model
is assembled from. Nothing else needs to know.

**Three rules the tests enforce**, so keep them in mind:

1. `role` must be unique. If your change makes a tower do the same job as
   another, the design is weaker for having both.
2. No two attackers may share a damage type, a delivery *and* an effect set.
3. Every element must unlock at least one tower that can shoot at the air,
   or drafting it is a coin toss on whether the run can answer the sky.

## Two things that must stay true of any new effect

Both of these have shipped as bugs, more than once, and both end a run by hanging
it rather than by losing it.

- **Displacement must be bounded per monster, not per second.** A cooldown alone
  does not stop a wave being pinned in place: a shove of 0.85 tiles every 0.75
  seconds beats a slowed monster's walking speed, so it never arrives, never
  dies and never leaks. Spend from `Creep::push_left`, via `combat::push_back`.
- **Nothing may scale a *distance* by the damage curve.** `Pull` briefly used
  `dist * scale.sqrt()`, which at level eight dragged a monster six tiles a hit.
  Tiles of road do not get longer as a tower gets stronger.

The balance harness asserts that no wave takes more than 200 seconds, which is
what catches the next one of these.

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
holds six inline, so a tower carries its whole effect set without allocating.

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
