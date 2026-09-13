# Green Circle TD - the current Legacy design, and where it came from

> **Implementation status:** this document describes the playable extracted
> Legacy ruleset. The curated commercial Campaign is specified separately in
> [`MASTERCLASS_PLAN.md`](MASTERCLASS_PLAN.md); its 28 encounters, six jobs,
> Ring Pressure, bounded economy, and 32-pad layouts are not yet represented by
> the tables below.

This game begins with `GREEN TD 9.3c PEIN.w3x`, a Warcraft III custom map. Its
131-tower roster, upgrade graph, combat numbers and 36 waves are the mechanical
foundation. The standalone edition then makes explicit product decisions the
eight-player source never needed: a compact solo arena, three difficulty
rulesets, readable threat forecasting, tempo rewards, hard-mode command drafts,
run ratings and endless play.

The map is the specification for extracted content. This document records both
that faithful base and the clearly labelled standalone systems layered over it.

How the numbers are got out is in [`../tools/README.md`](../tools/README.md).

---

## 1. The source

Green Circle TD is an eight-player Warcraft III map: a 96 by 96 field cut into
eight identical arenas by three-tile corridors, with a spawn box in every corner
and on every edge. Each player defends one arena against their own stream of
creeps, which walk a corridor and never leave it. There are no lives and no
exit. The map's own leaderboard states the loss condition in five words:

> When enemies > 700, game over.

The port is one arena, single player. Its compact runtime board preserves the
source identity while the roster, waves, purse, and Legacy loss condition stay
faithful to the map.

---

## 2. What was taken verbatim

| | |
| --- | --- |
| **131 towers**, in 24 families | name, gold cost, refund, damage, cooldown, range, splash radius, attack type, targets allowed, model, scale |
| **the upgrade graph** | every edge, exactly as the map's `uupt` field wires it |
| **every ability** | crit chance and multiplier, multishot count, the bouncing glaive, poison damage and slow and duration, the standing slow, both auras and their radii, immolation, root chance, the outright kill chance, armour stripping, the Troll's self-frenzy |
| **36 waves** | creep, count, health, armour value, armour type, movement speed, whether it flies, and the coloured banner the map prints - Air, Immune, Hero, Boss |
| **the wave clock** | the map's own `PolledWait` at the top of each wave trigger: fifty seconds through the first act, forty-five from wave 11 |
| **the spawn window** | `450./count` on a tenth-second timer, so a wave's whole count arrives evenly across forty-five seconds however many there are |
| **the terrain** | the ground texture and cliff level at every corner of the field, out of `war3map.w3e` |
| **the lane** | traced through the four regions the map orders the Red player's creeps along |
| **1000 starting gold** | `SetPlayerStateBJ(GetEnumPlayer(), PLAYER_STATE_RESOURCE_GOLD, 1000)` |
| **700 living creeps** | `return(udg_integer11>700)`, the map's whole lose condition |

Eleven of the 131 towers can be bought. The other 120 are reached by upgrading,
and a shop tower is defined as exactly one that nothing upgrades into, which is
computed rather than listed.

The cheapest is a **ten gold Single shot Tower**, and it is a seed: it becomes a
Slow, Poison, Critical, Troll or Fire Tower, or the forty-thousand gold
One-Strike Kill Tower, and none of those six families can be bought at any
price. The **Aura Tower** offers Damage or Speed at every rung, both free, and
lets you cross back. The **King Tower** opens the four Super towers, a hundred
thousand gold each. That is the whole shape of the roster: eleven doors, and a
graph behind them.

---

## 3. What had to be understood

The map's numbers are meaningless on their own. Two of Warcraft III's rules sit
underneath them, one of which this map rewrites wholesale, and four things about
the map's own data are not written down anywhere in it.

### 3.1 The attack table, which the map throws away

Warcraft III ships a seven-by-seven table of attack types against armour types:
Piercing shreds unarmoured and bounces off fortified, Magic beats heavy armour,
and so on. This map replaces all of it, in `war3mapMisc.txt`:

```text
DamageBonusNormal=1.00,1.00,1.00,1.00,1.00,1.00,0.05,1.00
DamageBonusPierce=1.00,1.00,1.00,1.00,1.00,1.00,0.05,1.00
DamageBonusSiege =1.00,1.00,1.00,1.00,1.00,1.00,0.05,1.00
DamageBonusMagic =1.00,1.00,1.00,1.00,1.00,1.00,0.05,1.00
DamageBonusSpells=1.00,1.00,1.00,1.00,1.00,1.00,0.05,1.00
DamageBonusHero  =100.00,100.00,100.00,100.00,100.00,100.00,100.00,100.00
```

The seventh column is Divine, which the map's waves call **Immune**. So:

- every ordinary attack does **full damage to everything**, and **five percent**
  to an Immune wave;
- **Chaos** is not in the file, because Warcraft III hard-codes it at 1.0 and no
  file can change it, so Chaos does full damage to Immune as well;
- **Hero** damage is multiplied by **a hundred**, against anything at all.

There is no rock, paper, scissors in this map. There is armour, and there is
every fifth wave. That single table is why the roster looks the way it does: the
Chaos and Destruction families, the Troll Tower, the four Super towers and the
One-Strike Kill Tower all exist because they are the only answers to Immune, and
pricing them as though attack types were a counter system would misprice the
whole game.

It lives in `greentd_types::type_mult`, written out rather than generated,
because it is six lines that apply to the whole game rather than data belonging
to any one unit.

### 3.2 Armour is a value, not a class

The armour *type* decides only what is resisted. The armour *number* is the real
defence, and Warcraft III's curve is unchanged here: each point is worth six
percent of a point, stacking with diminishing returns, so armour approaches
immunity without ever reaching it.

```
taken = 1 / (1 + 0.06 * armour)
```

The campaign runs that number from 0 on wave 1 to **700 on wave 33**, which
takes 2.3% of a hit. Wave 36 carries 200 armour and is Immune as well, so a
Siege Tower lands under four parts in a thousand of the number on its card,
while a Chaos tower lands seven and a half parts in a hundred.

That is the whole reason the ladders climb into six figures of damage. A tower
dealing 39,999 a shot is not absurd; it is a tower built to put a few hundred
damage through 200 armour.

### 3.3 Upgrading is a graph

A tower defence's upgrade path is usually a ladder, and modelling this map as
ladders quietly loses three of the most interesting decisions in it. `uupt` is a
list, not a value: the Single shot Tower names six successors, the Aura Tower
three, the King Tower five. `TowerLevel::upgrades` is therefore a slice, and a
tower with more than one way up takes over the command card rather than being
served by an "upgrade" button that could only pick one.

The desktop console follows the same proven RTS hierarchy without copying the
reference game's artwork: a square minimap, a selected-defense dossier with
threat forecast, and a fixed 4x3 command grid. The icon in that grid and the
portrait in the dossier are rendered from the same staged 3D assembly as the
tower on the field, so the shop never promises a different silhouette.

### 3.4 There is no family field

The map has no notion of a tower family. It has a naming convention it keeps
rigidly - every rung of a ladder starts with the same words - and the family is
read off the longest matching name prefix. That is also where the rung number
comes from: the digits at the tail of the name, or "Perfect" meaning the top.
Sorting by price instead would put five rungs in the wrong place across four
different ladders, because the prices are not monotonic.

### 3.5 The texture grid is the level

The pathing map says the whole source field is walkable. Its terrain is still
preserved as the **ground texture**: corridors are painted in rock and
everything else is grass or dirt. For the standalone port, however, the source
map's huge outer ring is not the playable layout. `emit_map.py` keeps the
extracted grid but emits a compact winding `SOLO_LAP`; `board::is_corridor`
derives the actual solo route from that lane so every road, socket and camera
boundary agrees.

### 3.6 The lane splits in both directions

The source map orders Red through the spawn box and into the north-west
junction. Its trigger then rolls a fifty-fifty branch: one creep is sent
clockwise and the next may go counter-clockwise around the same circuit. The
compact solo board preserves that branch on a winding circuit and offsets the
streams onto opposite sides of its lane, so they pass instead of occupying the
same line.

A creep's progress is still one scalar plus a direction sign. Completed laps
wrap onto the same closed circuit, which keeps cumulative pressure intact while
making tower coverage on both approaches strategically important.

---

## 4. Standalone economy and progression

**The kill bounty is the first standalone economy rule.**

Warcraft III keeps its bounty formula in the game's own gameplay constants
rather than in the map file, and this map leaves them at their defaults. Those
defaults are footman-sized. They are meaningless against a tower that costs a
hundred thousand gold, and there is nothing in the archive to read instead.

So a kill pays a fixed share of what it took to kill: the creep's health scaled
by its armour value, divided down.

```rust
pub fn bounty_of(w: &WaveDef) -> u32 {
    (w.payable_hp() / 350.0).round().max(1.0) as u32
}
```

`payable_hp` deliberately ignores the Immune multiplier. The player is paid for
the toughness of the creep, not for the toughness of having brought the wrong
attack type - otherwise the Immune waves would pay twenty times what they should
and the answer to them would fund itself.

Keeping the bounty on the same exponential as the roster is the point. It means
there is no hand-authored payout table to drift out of step with the extraction
when the map is re-read. Across the campaign it pays about **3.7 million gold**,
against 68,730 for a maxed Siege ladder, and
`kill_money_keeps_pace_with_the_roster` checks it stays between one board of
thirty such ladders and forty boards of them - loose bounds, because the point
is to catch a bounty that has come adrift from the roster, not to pin a balance
number nobody tuned.

Three payouts are expressed in terms of it:

- **A stipend of twelve kills' worth when a wave is called.** On a circuit no
  wave ever ends, so there is no "wave cleared" moment to pay at. Without it a
  board that falls behind can never buy its way back out, and one bad wave
  quietly decides the whole run.
- **Four gold a second for calling a wave early.** Rush unlocks after the
  current stream has fully entered, then begins the new stream on top of all
  survivors. It remains a pressure/reward decision without letting Enter skip
  the authored deployment time.
- **Four kills' bounty for a Clean Sweep.** Letting the clock expire with no
  previous-wave survivor alive rewards control. Calling early earns Rush gold;
  waiting earns Clean Sweep gold; the player cannot claim both for one boundary.

Veteran and Nightmare also stop before waves 10, 20 and 30 for one permanent
command upgrade: +12% global damage, +10% global attack speed, or +0.6 global
range per rank. Three choices across a campaign are enough to define a build
without turning the game into an inventory system.

Past wave 36 the run may continue: health climbs 35% a wave and armour by 20.
The roster does not climb with it, because the top of every ladder has already
been bought by then, so endless always ends. How far is the score.

Campaign speed cycles between 1x and 2x. The 3x setting unlocks only in endless,
after the player has already completed the authored run. Even on 2x, the full
deployment windows keep every difficulty above ten minutes before build pauses,
targeting decisions and any time spent recovering from pressure.

---

## 5. The quirks, kept rather than corrected

A port that tidies its source is no longer a port. Each of these is pinned by an
assertion in `src/game/greentd_tests.rs`.

- **Eight waves write their creep count as a JASS character literal.** `set
  udg_integer14='}'` is a hundred and twenty-five. Waves 7, 11, 12, 16, 22, 25,
  28 and 36 are all written that way, and reading only the decimal form makes
  them send nothing at all - which is how they first came out, eight silent
  waves nobody noticed for a while.
- **Siege Tower 1 costs 100 gold and Siege Tower 2 costs 50.** Four other rungs
  do the same: Multi 1 to 2 drops from 400 to 300, Air 1 to 2 from 600 to 360,
  and Poison 5 and 6 and Multi 9 and 10 are pairs at the same price. The upgrade
  button follows the map's numbering, so the ladder is ordered by that and never
  by cost.
- **All twenty "Perfect" towers refund about half of what was sunk in them.**
  Every rung below them refunds the lot, because the map's refund figure is
  cumulative and `UpgradeRefundRate=1.0`: Siege Tower 2 costs 50 and refunds
  150, the whole ladder up to it. The top of a path is deliberately a one-way
  purchase.
- **The five airborne waves are set to `hover`, not `fly`.** In Warcraft III
  hovering leaves a unit targetable from the ground, so read literally the Air
  Tower's ten rungs have nothing to shoot at and nine of them can hit nothing at
  all. Every one of those five waves is a gyrocopter, a phoenix, a harpy, a
  frost wyrm or a bronze dragon. They fly here.
- **The names are the map's**, misspellings included: "Supper chaos tower",
  "lllidan Evil", "Ereder Sorcerer".

---

## 6. The board

The runtime board is a compact **39 by 39** solo battlefield centred on the
source terrain's tactical loop. It keeps the circular Green TD pressure model
without asking one player to patrol an eight-player-sized landscape.

- The lane is **109.5 tiles** round: long enough for placement and aura coverage
  to matter, short enough that the whole threat can be read without constant
  camera travel.
- The arena holds **56 fortified build pads** sampled from both shoulders of
  the route. Every pad is 1.4–2.8 tiles from the road and keeps at least 2.2
  tiles of centre spacing, so towers stay connected to combat without their
  silhouettes merging.
- The pad set still supports inside, outside, corner and aura-cluster plans; it
  removes overlap and fake decorative placements rather than the placement game.
- The camera frames the lane plus a tower's reach around it, which is roughly
  what a Warcraft III camera sees from edge to edge.
- WASD/arrows and the literal top or bottom of the window pan the camera;
  middle-drag pans directly and the wheel zooms. Vertical edge scrolling reads
  the viewport rather than the board widget, so the HUD cannot swallow it.

There is no mazing. The corridors are painted into the terrain and the creeps
walk them whatever is built.

---

## 7. The shape of a run

**Thirty-six waves.** Thirty-five seconds of quiet, then fifty seconds a wave
through the first act and forty-five from wave 11: about twenty-eight minutes of
clock, plus however long the last stream takes to die.

Each wave's whole count arrives evenly over forty-five seconds of that gap, so
the lane is never empty and no tower is ever idle. The counts are the map's, and
they are streams rather than bursts - sixty to a hundred and sixty creeps in
most waves, and the few small ones are the ones with the health.

**Health climbs two thousandfold**, from a 250 health troll on wave 1 to half a
million on wave 36, and armour climbs alongside it. Every fifth wave is
**Immune**, and so is the last one.

**There are no lives, because there is no exit.** What the towers cannot kill
comes round again, and again, and the lane fills up. The run is lost when more
than seven hundred creeps are circling. That gauge is the map's, and it is a
better one than a life counter for three reasons:

- it moves continuously, so trouble is visible a dozen waves before it is fatal;
- a wave only three-quarters killed is a debt carried into the next one, which
  is what makes the back half of a run feel like a run;
- it cannot be gamed by shoving creeps backwards, because on a loop backwards is
  the same direction.

On Veteran and Nightmare, completed laps also accelerate a survivor by 6% or
10% per lap. The warning ring turns red after three laps, but a survivor never
mints extra bounty by living longer. Veteran commanders must fall before lap
four; Nightmare commanders before lap three. Default First targeting includes
total lap distance, so it never abandons the most dangerous survivor when
distance wraps back to zero.

A wave tagged Boss retains the source map's full authored stream but promotes
only its first enemy to commander. That commander is 28% larger, has eight
times the wave health and bounty, resists 75% of control, and repairs escorts
within 4.5 tiles for 0.8% maximum health per second. Its gold range ring exposes
that rule on the battlefield. Strongest targeting isolates it and Corruption
suppresses escort repair for two seconds, giving the encounter a readable
answer beyond raw damage.

Winning means the last wave has finished arriving **and** the ring is empty.
Outlasting the final stream is not the same as clearing it.

---

## 8. What the player actually does

Every decision in the game is one of three.

**Which of eleven doors to open.** The ten gold seed is one of them, and it is
the only way into six families. Chaos and Destruction are the only towers on the
shop card that answer an Immune wave. And the Air Tower is the only one built
*for* the sky - nine of its ten rungs can hit nothing else - while Siege, Chaos
and Destruction cannot reach it at any rung, which is most of the splash damage
in the game.

**Where to put it.** Seventy-two protected plots, two directions of traffic,
and auras that only reach what is near them. A Damage Tower gives every tower
within fifteen tiles up to eighty percent more damage and a Speed Tower does the same for
attack rate, so tight clusters are worth more than the same gold spread thin -
and neither of them fires a shot.

**When to stop widening and start climbing.** A rung roughly doubles a tower's
damage per second and costs about two thirds again what the rung below it cost,
so climbing a good plot beats buying another copy nearly every time. What stops
that from being the only answer is how long the ladders are - twenty rungs of
Siege, fifteen of Poison, ten each of Bouncing, Multi, Air and Critical - and
that a maxed ladder covers one lane position and one attack type. The campaign
pays about fifty maxed ladders' worth in total, across thirty-six waves that get
exponentially harder, so it is never enough to do both everywhere.

The two ways to lose are both failures of coverage rather than of throughput.
`a_ground_only_board_drowns_in_the_air` builds sixty maxed Siege towers and
loses every single flyer of wave 7 regardless; a board with no Chaos on it does
a twentieth of its damage every fifth wave, and what it fails to kill stays on
the lane for the next one. `a_sensible_build_clears_the_campaign` plays the
whole thirty-six with a bot that covers the next counter-check, spreads useful
lane coverage and then deepens efficient towers. The complementary
`shallow_mixed_spam_cannot_clear_veteran` gives a randomly placed level-one
carpet every shop family and a damage doctrine; it still loses, pinning the
requirement to upgrade and specialise.

---

## 9. Rendering

The map is a cooler, high-contrast take on Lordaeron Summer: a dense green field
cut by worn grey rock corridors. The palette begins with the tileset's texture names - `Agrs` and
`Agrd` for the turf, `Adrt` for the corridors, `Arck` for the stone - as albedo
values, so the lighting does its own work.

Nothing here redistributes Warcraft III `.mdl` files. Runtime art is baked from
free assets: 44 staged Quaternius combat turrets, 56 Poly Pizza enemy/world
models, and Kenney Nature Kit scenery. Procedural constructions remain as
corruption fallbacks, while every shipped roster entry is tested to select its
baked model and fit the tactical silhouette budget.

Four ambientCG surface sets are baked into one texture array. Terrain uses
world-planar albedo and normal detail; every prop, tower and creature receives
material-selected triplanar stone, timber, foliage, hide or metal grain, so the
browser mesh remains compact without falling back to flat vertex colour.

The frame is two render passes at Performance, three at Balanced and six at
Ultra. Pass count is the number that decides whether this runs in a browser: the
simulation costs hundredths of a millisecond even with the ring full and the
board packed, so the frame is spent on framebuffer binds, and every one removed
is worth more than any amount of culling. The preset is chosen by measuring the
opening seconds and stepping down until the frame budget is met.

---

## 10. What is deliberately not here

- **The other seven arenas.** Multiplayer compares separate compact boards;
  rendering unused battlefield acreage would only weaken solo readability.
- **Mazing.** The corridors are painted into the terrain. There is nothing to
  block.
- **Hidden difficulty.** Classic exposes the source curve; Veteran and Nightmare
  state their capacity, Vanguard cadence, lap rage and reward pressure before
  the player starts.
- **Tower-number retuning.** Tower costs, rungs, attacks and abilities remain
  extracted. Standalone challenge is layered around pressure, cadence, elites
  and player-visible command upgrades instead of secretly changing the roster.
- **Server-side simulation.** Every client runs its own board from a shared
  seed. The server relays scoreboards and nothing else, which is what lets it
  hold a thousand players in a gigabyte.
