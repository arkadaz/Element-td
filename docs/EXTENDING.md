# Changing Green Circle TD

This game is a port, and that changes what "extending" means. Almost every
number in it belongs to `GREEN TD 9.3c PEIN.w3x`, so most of what you might want
to change is not in this repository at all - it is in the map, and the way to
change it is to edit the map and regenerate.

| I want to... | Do this |
| --- | --- |
| Retune a tower, a wave, an ability, the upgrade graph | Edit the map, then `python extract.py && python emit.py` in `tools/` |
| Redraw the extracted terrain | Edit the map's terrain, then `python emit_map.py` |
| Change the compact solo route | Edit `SOLO_LAP` in `tools/emit_map.py`, then regenerate |
| Change the attack-versus-armour table | Edit `war3mapMisc.txt` in the map, then regenerate |
| Support a map version with new fields | `tools/emit.py`, and the row types in `game/greentd_types.rs` |
| Change what a tower or a monster looks like | `view/models.rs` |
| Change the kill bounty | `game/defs.rs` → `bounty_of` (the one invented number) |
| Recolour the board or the HUD | `view/mod.rs` → `theme`, `ui.rs` → `pal` |
| Change the lighting | `gfx/shaders/solid.wgsl`, `gfx/shaders/post.wgsl` |
| Change the camera - how close it starts, how far it zooms | `main.rs` → `CAM_SPAN`, `CAM_SPAN_MIN`, `CAM_PITCH_DEG`; the pan clamp itself is `math.rs` → `Rig` |
| Add scenery | `decor.rs` |

The two generated modules - `game/greentd.rs` and `game/greentd_map.rs` - are
never hand-edited. They carry a header saying so. If you find yourself wanting
to change a number in one of them, the change belongs in the map or in the
emitter that reads it. The compact route is intentional port configuration in
`emit_map.py`'s `SOLO_LAP`, rather than a claim that the source map's outer
ring is a good one-player layout.

---

## The seam between generated data and code

`greentd.rs` holds rows: `TowerLevel`, `WaveRow`, and the `DAMAGE` table.
`greentd_map.rs` holds the terrain grid, the lane and the arena rectangle.
Neither knows anything about the game.

`greentd_types.rs` is the other half: the types those rows are written in, and
the two Warcraft III rules the numbers are meaningless without - `type_mult`,
which indexes the generated table, and `armour_mult`, which is the engine's
`1 / (1 + 0.06 A)` curve. That file is the only place a Warcraft III rule is
implemented rather than extracted, and each one carries a comment saying which
rule it is.

`defs.rs` is the adapter the rest of the game reads through: the shop, the
upgrade graph, `wave_at`, and the economy.

**Adding a field from the map** is therefore three edits in a line: read it in
`tools/emit.py`, add it to the row type in `greentd_types.rs`, write it out in
the emitter's output section. Nothing else needs to know.

---

## The models

`view/models.rs` rebuilds fifty-six Warcraft III models out of eight primitives.
[`Model`] in `greentd_types.rs` is the list; `tools/emit.py` maps the map's own
`.mdl` paths onto it by file stem, and prints anything it could not place.

To add a model: add a variant, map the stems that should use it in `emit.py`'s
`MODELS` table, and write the builder. Three rules the existing ones follow, and
two tests that enforce them:

- **Detail is the job.** Thirty to sixty pieces, not six.
  `no_tower_is_just_a_pile_of_boxes` fails a model that is mostly boxes or built
  from fewer than three kinds of primitive.
- **Colour is material, not identity.** Steel is grey, leather is brown, skin is
  skin. Only cloth, crests, banners and whatever glows take the unit's own
  colour, which is its attack type for a tower and its armour type for a creep.
- **A crowd is not a portrait.** `Pose::fine` is false for anything arriving in
  numbers; put buckles, membranes and additive glows behind it.
  `every_model_has_its_own_silhouette` fails two models built identically.

---

## The board

`game/board.rs` builds everything from `greentd_map.rs`: the lane polyline from
`LAP`, then 164 tile-sized shoulder sockets beside useful parts of that lane.
There is no runtime waypoint list to edit. `VIEW` is what the fixed gameplay
camera frames and `ARENA` bounds the compact solo adaptation.

---

## The tests are the specification

`game/greentd_tests.rs` asserts the extraction is faithful: the family sizes,
the upgrade graph's shape, the shop being exactly the towers nothing upgrades
into, the eight character-literal wave counts, the generated damage table, and
the armour curve. If you change the emitter, these are what tell you whether you
changed it correctly.

`game/tests.rs` asserts the game is playable: that a board with an answer to
everything clears thirty-six waves, that a board with no anti-air does not, that
nothing can wedge a wave open forever. `a_sensible_build_clears_the_campaign` is
the one that fails first when a balance change goes wrong.

Two diagnostics are worth knowing about, and neither runs in CI:

```
cargo test --release a_narrated_playthrough -- --ignored --nocapture
cargo test --release show_the_extracted_roster -- --ignored --nocapture
```

The first plays a whole campaign and prints a line per wave - the ring's peak,
the purse, the board. The second prints all 131 towers with their upgrade edges.
