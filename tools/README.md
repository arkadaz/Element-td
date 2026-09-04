# Extracting Green Circle TD

Two of the game's data modules are generated, and neither is ever hand-edited:

| generated | holds | read from |
| --- | --- | --- |
| `src/game/greentd.rs` | 131 towers, the upgrade graph, 36 waves | `war3map.w3u`, `war3map.w3a`, `Scripts\war3map.j` |
| `src/game/greentd_map.rs` | the terrain grid and the player's lane | `war3map.w3e`, `Scripts\war3map.j` |

The source is **`GREEN TD 9.3c PEIN.w3x`**. It is gitignored, because it is not
ours to redistribute, so put your own copy in the repository root under that
exact name. Then, from this directory:

```
python extract.py      # unpacks the archive into tools/greentd/
python emit.py         # writes ../src/game/greentd.rs
python emit_map.py     # writes ../src/game/greentd_map.rs
```

`emit.py` reads the unpacked files, so `extract.py` has to run first.
`emit_map.py` opens the archive itself and does not need either, but running
all three in that order is what you want after a change of map version. The
unpacked files are not checked in.

Why the game is arranged this way, and what the extracted numbers mean, is in
[`../docs/DESIGN.md`](../docs/DESIGN.md). This file is about getting them out.

## The five scripts

| file | what it does |
| --- | --- |
| `mpq.py` | Reads the MPQ archive a `.w3x` is: hash and block table decryption, sector tables, zlib, bzip2 and the PKWARE "implode" decompressor. `mpyq` stops at "Encryption is not supported yet", which is every file inside a Warcraft III map, so this is written out rather than depended on. |
| `w3obj.py` | Parses Warcraft III object data. A version header, then an "original" and a "custom" table of objects, each object a base id, a new id and a list of field-id/type/value modifications. Levelled files (`.w3a` abilities) carry a level and a data pointer on every modification and unlevelled ones (`.w3u` units) do not, which is the only difference in the format. |
| `extract.py` | Pulls the nine files the emitters might want out of the archive. Two of them, `war3map.w3t` and a root-level `war3map.j`, are not in this map; the script says so and carries on. The trigger script lives at `Scripts\war3map.j` and is written out as `script.j`. |
| `emit.py` | Joins units to abilities to triggers, converts to this game's units, and writes the roster, the upgrade graph and the wave table. |
| `emit_map.py` | Bakes the terrain texture grid and traces the route the Red player's creeps walk, out of the map's own regions. |

## What each unpacked file is for

| file | used for |
| --- | --- |
| `war3map.w3u` | Units. Every tower and every wave's creep: name, gold, point value, attack dice, cooldown, range, splash, attack type, targets allowed, model path, scale, health, armour value, armour type, movement speed and movement type. |
| `war3map.w3a` | Abilities. Crit, multishot, the bouncing glaive, poison, slows, the two auras, immolation, roots, the outright kill and armour stripping. These live in a separate file from the units that carry them, which makes them the easiest thing in the extraction to lose silently. |
| `war3map.w3e` | Terrain. The ground texture and cliff level at every corner of the field. |
| `Scripts\war3map.j` | The trigger script: which creep each wave sends and how many, the gap before each wave, the wave banners, the four regions that define the lane, the thousand starting gold and the loss condition. |
| `war3map.w3i` | Map info. Unpacked for reference; nothing reads it. |
| `war3map.doo` | Doodads. Unpacked for reference; the scenery in `src/decor.rs` is placed by rule rather than copied. |
| `war3map.wts` | The string table object data can point into. Unpacked, but this map stores its names literally in `war3map.w3u`, so nothing needs it. |

## What is taken, and what is not

**Taken verbatim.** Every tower's name, gold cost, refund, damage, cooldown,
splash radius, attack type, targets allowed, model and scale, its abilities'
own numbers, and the upgrade graph exactly as `uupt` wires it. Every wave's
creep, count, health, armour value, armour type, speed and banner. The terrain
grid. The four regions the lane is traced through. A thousand starting gold,
and the seven hundred living creeps that end the run.

**Converted: distances and speeds only.** Warcraft III measures both in world
units at 128 to a tile, so everything spatial is divided by 128 - ranges,
splash radii, aura radii, and movement speed into tiles per second. A 900 range
tower reaches seven tiles here exactly as it does there.

**Not generated, but taken.** The attack-versus-armour table lives in
`war3mapMisc.txt`, which this map rewrites wholesale; it is six lines that
apply to the whole game rather than to any one tower, so it is written into
`greentd_types::type_mult` instead of emitted. The same file's
`UpgradeRefundRate=1.0` is why selling pays back nearly everything sunk in.

**Not taken.** The eight-player layout: the whole 96 by 96 field is drawn, but
only one arena is played. The models: nothing here loads a `.mdl`, so `emit.py`
maps each model path onto the archetype it reads as and `src/view/` rebuilds it
out of primitives. The missile art and per-tower missile speeds, which are
decoration. And the kill bounties, which are not in the map file at all - see
`bounty_of` in `src/game/defs.rs` for what replaces them and why.

## The quirks the extraction preserves

These are the map's, not the extraction's, and `src/game/greentd_tests.rs` pins
each one so a change gets noticed.

- **Eight waves write their creep count as a JASS character literal.** `set
  udg_integer14='}'` is a hundred and twenty-five, and waves 7, 11, 12, 16, 22,
  25, 28 and 36 are all written that way. Matching only `udg_integer14=(\d+)`
  reads them as nothing at all, which is exactly how they came out first time:
  eight silent waves scattered through the campaign.
- **Five rungs cost no more than the rung below.** Siege Tower 1 costs 100 gold
  and Siege Tower 2 costs 50; Multi 1 is 400 and Multi 2 is 300; Air 1 is 600
  and Air 2 is 360; Poison 5 and 6 are both 500; Multi 9 and 10 are both 20,000.
  Ladders are ordered by the map's own numbering, never by price, because the
  price is not monotonic and the numbering is what the upgrade button follows.
- **The five airborne waves are set to `hover`, not `fly`.** In Warcraft III
  that leaves them targetable from the ground. Every one of them is a
  gyrocopter, a phoenix, a harpy, a frost wyrm or a bronze dragon, and the map
  gives the Air Tower ten rungs and nothing else to shoot at, so they fly here.
- **The names are the map's, spelling and all**: "Supper chaos tower",
  "lllidan Evil", "Ereder Sorcerer". Correcting them would make the roster
  harder to check against the map, which is the only thing it can be checked
  against.

## Checking a re-extraction

`emit.py` prints its own totals - it should say 131 towers, 36 waves and 11
shop roots - and names any model path it could not place. `emit_map.py` prints
the grid size, the corridor tile count and the traced lane.

Then the assertions:

```
cargo test --release greentd_tests
```

Those check the shape of the extraction rather than the balance: the family
sizes, that every family is numbered without gaps, that the long ladders climb,
that the six branches off the ten gold seed cannot be bought any other way, that
every hundred-thousand gold tower deals Chaos, that Immune arrives every fifth
wave, and that no wave sends nothing.

Two diagnostics print what was actually extracted:

```
cargo test --release show_the_extracted_roster -- --ignored --nocapture
cargo test --release show_the_board -- --ignored --nocapture
```
