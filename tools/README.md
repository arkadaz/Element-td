# Extracting Green Circle TD

`src/game/greentd.rs` is generated from `GREEN TD 9.3c PEIN.w3x` by these three
scripts. The map itself is gitignored - it is not ours to redistribute - so put
your own copy beside the repo and run:

```
cd tools && python emit.py     # writes ../src/game/greentd.rs
```

| file | what it does |
| --- | --- |
| `mpq.py` | Reads the MPQ archive a `.w3x` is. Implements the hash/block table decryption and PKWARE "implode" decompression, neither of which `mpyq` supports - every file inside a Warcraft III map is encrypted, so without them you get "Encryption is not supported yet" and nothing else. |
| `w3obj.py` | Parses Warcraft III object data (`.w3u` units, `.w3a` abilities): a version header, then original and custom tables of objects, each a list of field-id/type/value modifications. |
| `emit.py` | Joins the two, converts to this game's units, and writes the Rust tables. |

## What is taken, and what is not

Taken verbatim: every tower's name, gold cost, damage, cooldown, splash radius,
attack type, targets-allowed and abilities; every wave's creep, count, health,
armour value and armour type.

Converted: **range only**. Warcraft III measures distance in world units at 128
to a tile, so ranges are divided by 128. The map is 96x96 tiles holding eight
arenas of roughly twenty tiles each, which is about the size of this board's
circuit - so a 900-range tower reaches seven tiles here exactly as it does
there.

Not taken: the eight-player layout. This is one arena, single player.

## Two quirks worth knowing

They are the map's, not the extraction's, and `greentd_tests.rs` pins them so a
change gets noticed:

- **Siege Tower 1 costs 100 gold and Siege Tower 2 costs 50.** The second rung
  of the ladder is cheaper than the first.
- **Poison Towers 5 and 6 both cost 500.** Same price, more damage.

The ladders are ordered by the map's own numbering, not by price, because the
price is not monotonic and the numbering is what the upgrade button follows.
