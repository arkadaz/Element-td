# Elemental TD - design

This document is the specification. Where the code and this file disagree, the
file is wrong and should be fixed.

---

## 1. What was wrong with the old design

The previous version was a competent tower defense that stopped being a game
around wave 20. Eight towers, all available from wave 1, all affordable by wave
15. After that the only decision left was *which tower do I pour gold into*,
answered once and then repeated sixty times. Length was added by raising health
numbers, which is not the same thing as depth.

Three specific failures:

- **No scarcity.** Everything unlocked immediately, so there was never a build
  to discover, only a build to execute.
- **No variance.** Every run drew the same towers against the same waves on the
  same road. Nothing to learn on run two.
- **One axis of growth.** Gold in, damage out. A single scalar cannot carry an
  hour.

The fix is not more waves. It is a second resource that the player spends on
*what they are allowed to build* rather than on how much of it.

---

## 2. The core loop

> **Draft an element. Elements combine into towers. Towers hold the road.**

The player never buys a tower from an open shop. They earn **essences**, one
element at a time, and their collection of essences decides which of the
twenty-one towers they may build and how far each may be upgraded.

Every run therefore has three interleaved decisions:

1. **Draft** - which element to take, from three offered.
2. **Build** - which of the towers that unlocked to actually put on the road.
3. **Invest** - upgrade what is there, build wider, or bank for interest.

The first is the new one, and it is the one that makes runs differ.

---

## 3. Essences

There are six elements:

| Element | Colour | Temperament |
|---|---|---|
| **Nature** | green | poison, decay, things that get worse over time |
| **Fire** | orange | burst, burn, area |
| **Water** | blue | slow, chain, control |
| **Earth** | brown | weight, armour-breaking, ground only |
| **Light** | gold | precision, range, buffs |
| **Dark** | violet | debuffs, execution, gold |

**Twenty essences** are awarded over a campaign, at waves

```
1, 2, 3, 5, 7, 9, 12, 15, 18, 21, 25, 29, 33, 37, 42, 47, 52, 58, 64, 71
```

front-loaded so the opening has choices, thinning out so the late game is about
using what you built rather than still being handed new toys.

At each award the player is offered **three of the six**, and takes one. The
offer is drawn from the run seed, and is constrained so that - whenever both are
possible - it contains **at least one element already held** (so you can always
deepen) and **at least one not held** (so you can always broaden). Without that
rule a draft can be a non-choice, which is worse than no draft at all.

Combat does not start until the pending draft is taken. It is a decision, not a
notification.

### What essences buy

Let `e[X]` be how many essences of element `X` are held.

- **Pure tower X** is buildable when `e[X] >= 1`.
  Its ceiling is `min(8, 2 + e[X])`.
- **Dual tower XY** is buildable when `e[X] >= 1` **and** `e[Y] >= 1`.
  Its ceiling is `min(8, 2 + min(e[X], e[Y]))`.

So six essences in one element max out that pure tower; six in each of two
elements max out the dual between them.

This is the whole strategic spine, and it is a genuine dilemma:

| Spread over 20 essences | What you get |
|---|---|
| 20 in one | one maxed pure tower and nothing else - a losing build |
| 10 / 10 | two pures and one dual, all at ceiling 8 |
| 7 / 7 / 6 | three pures, three duals, all at 8 |
| 4 / 4 / 4 / 4 / 4 | five pures, ten duals, all capped at 6 |
| 3 each of six, plus 2 | everything unlocked, nothing above 5 |

Breadth buys answers. Depth buys numbers. The waves are built so that neither
extreme survives: a narrow board meets an armour class it cannot hurt, and a
wide board of tier-5 towers cannot out-damage a wave-70 health bar.

Essences are never refunded and never respecced. Towers sell back at 75%.

---

## 4. The twenty-one towers

Six pure, fifteen dual - every unordered pair of elements. Each owns exactly one
role; no two share both a delivery and a special.

### Pure - cheap, immediate, always relevant

| Tower | Element | Damage | Targets | Role |
|---|---|---|---|---|
| **Bramble** | Nature | Toxic | air + ground | stacking poison, the reliable opener |
| **Ember** | Fire | Fire | air + ground | small fast splash |
| **Tide** | Water | Magic | air + ground | slow; buys every other tower more shots |
| **Boulder** | Earth | Physical | **ground only** | one heavy shot, knocks back |
| **Prism** | Light | Magic | air + ground | long range, crits |
| **Shade** | Dark | Toxic | air + ground | weakest damage, pays gold per kill |

### Dual - expensive, specialised, the reason to broaden

| Tower | Pair | Damage | Targets | Role |
|---|---|---|---|---|
| **Wildfire** | N+F | Fire | air + ground | fire arcs from target to target |
| **Mire** | N+W | Toxic | **ground only** | swamp: heavy slow, poison, **suppresses healing** |
| **Thornwall** | N+E | Physical | **ground only** | roots: hard slow with splash |
| **Grove** | N+L | - | none | aura: damage, rate and range to neighbours |
| **Blight** | N+D | Toxic | air + ground | ramps on one target, spreads on kill |
| **Steam** | F+W | Fire | air + ground | untargeted nova, hits both layers |
| **Magma** | F+E | Fire | **ground only** | sets the road alight, shreds armour |
| **Solar** | F+L | Fire | air + ground | piercing lance down a whole lane |
| **Hellfire** | F+D | Fire | air + ground | **executes** anything under a third health |
| **Silt** | W+E | Physical | **ground only** | widest splash, slows, shreds |
| **Mirror** | W+L | Magic | air + ground | longest chain in the game |
| **Abyss** | W+D | Magic | air + ground | **drags monsters back down the road** |
| **Bastion** | E+L | Physical | air + ground | highest single-target damage |
| **Tombstone** | E+D | - | none | economy: pays every wave, raises interest |
| **Eclipse** | L+D | Magic | air + ground | stuns and shreds - the boss answer |

Five towers cannot shoot upwards (Boulder, Mire, Thornwall, Magma, Silt). Two do
not shoot at all (Grove, Tombstone). Both facts are load-bearing: they are what
stops "build the highest-DPS thing everywhere" from being correct.

### Levels

Eight levels. No forks - the branching decision now lives in the draft, and
forty-two fork variants on top of twenty-one towers would be noise rather than
choice. Instead there are two visible milestones:

- **Level 4 - Attuned.** The tower's special effect strengthens sharply.
- **Level 7 - Ascendant.** Damage takes a step up and the model changes shape.

Damage per level multiplies by **1.76**, cost by **1.62**. Upgrading is about 9%
more gold-efficient than building another copy, which is small enough that the
choice stays live at every level instead of being settled at level 2.

---

## 5. Damage and armour

Four damage types against five armour classes. This table is the reason a wide
board beats a tall one at least some of the time.

|              | Unarmoured | Plated | Warded | Ethereal | Boss |
|---|---|---|---|---|---|
| **Physical** | 1.00 | 0.55 | 1.25 | 0.70 | 0.85 |
| **Magic**    | 1.00 | 1.25 | 0.55 | 1.30 | 0.85 |
| **Fire**     | 1.15 | 0.85 | 0.85 | 0.60 | 0.90 |
| **Toxic**    | 1.00 | 1.00 | 1.00 | 1.00 | 1.00 |

Toxic is never resisted and never bonused. It is the honest answer to anything,
and it is why the Nature and Dark pures stay worth a pad into the late game
while being the worst raw damage in the roster.

Fire is the swarm answer and nothing else's. It was briefly neutral against
wards, which quietly made magic-plus-fire a board with no hole in it at all -
magic beat plate and ghosts, fire beat crowds, and wards were the one thing left
for the pair to fear. Now a board needs a third answer.

---

## 6. Monsters

Fourteen types. Each one punishes exactly one lazy habit.

| Monster | Layer | Armour | Punishes |
|---|---|---|---|
| Grunt | ground | Unarmoured | - |
| Runner | ground | Unarmoured | slow projectiles, no control |
| Swarm | ground | Unarmoured | single-target boards |
| Brute | ground | Plated | all-physical boards |
| Warden | ground | Warded | all-magic boards |
| **Wraith** | ground | **Ethereal** | all-physical boards, harder |
| Mender | ground | Unarmoured | not focusing, no burst |
| Bulwark | ground | Plated | chip damage - a flat shield absorbs it |
| Phaser | ground | Warded | relying on slow |
| Wisp | **air** | Unarmoured | no anti-air |
| Drake | **air** | Plated | anti-air that is all physical |
| **Seraph** | **air** | **Ethereal** | anti-air that is all physical, harder |
| **Boss** | ground | Boss | thin damage, stun reliance |
| **Skylord** | air | Boss | ground-only boards |

Ethereal is new and deliberately nasty: it takes 70% from physical and 60% from
fire, but 130% from magic. A board of Bastions and Silts meets a Wraith wave and
finds out.

### Escalation

New types arrive on a schedule and **always debut at 55% count and 65% health**,
so the first Wisp wave costs a life, not the run. The game teaches a mechanic
before it tests it.

Wave modifiers layer on from the midgame: regenerating (every 8th wave from 20),
splitting (every 9th from 22), and escorts - a second monster type arriving
alongside the first, from wave 25, always crossing layers from wave 45.

Bosses at every tenth wave, alternating ground and air.

---

## 7. Length and pacing

**Eighty waves.** Sixteen seconds of build time between waves (twenty-five
before the first), and a wave takes thirty to fifty seconds to walk the road.
That is a measured **69 minutes** for a full campaign, and the draft pauses are
load-bearing pacing: they are the moments the player looks up from the road.

Calling a wave early pays **2 gold per second saved**. A player who reads the
preview and knows they are safe can compress the run and get paid for it. This
is the only speed control that is also a decision.

Clearing wave 80 is a win. The run may continue into **endless**, where health
climbs 7.5% and the purse 6.2% per wave - health outruns gold, so endless always
ends eventually. The question is only how far.

### The four acts

| Act | Waves | What it is about |
|---|---|---|
| **I - The Road** | 1-20 | Learning layers and armour. First air at 7, first Plated at 9. |
| **II - Pressure** | 21-40 | Healers, swarms, wards. Focus fire and area damage. |
| **III - Attrition** | 41-60 | Ethereal, shields, splitting, phasing. Burst versus sustain. |
| **IV - The Deep** | 61-80 | Cross-layer escorts every wave. Full-board answers only. |

---

## 8. Economy

- Start with **260 gold** and **20 lives**.
- A wave's purse is fixed. **55%** rides on kills, **45%** is paid for surviving
  it. A half-cleared wave costs lives, which is the real currency, but does not
  quietly destroy the economy needed to recover. Paying the whole purse on kills
  made the balance bimodal: cruise to victory, or collapse at wave 60, with
  almost nothing between.
- Escorts split the same purse across more monsters. They are a threat, not a
  payday.
- **Interest** pays 5% of gold in hand at each wave end, up to **20%** with
  Tombstones, and only on gold up to a ceiling of twelve wave-purses. Uncapped
  compound interest is not an economy, it is a runaway - the previous version
  reached 813 billion gold by wave 89 and ran to wave 136 without difficulty.
- Selling refunds **75%** of everything invested.

### Ninety-nine pads

The road has ninety-nine build plots beside it, on alternating tiles.

The number matters more than it looks. There used to be one on every tile in the
band - a hundred and ninety-eight - and a campaign purse that can cover two
hundred plots is a campaign with no placement decision in it. Filling the board
with cheap towers beat levelling good ones, which is the opposite of what the
cost curve is built to reward, and a simulated run confirmed it: a bot that
papered the board with level-one towers cleared eighty waves without losing a
life. Half as many plots makes each one worth thinking about and leaves levels
as the real sink for gold.

### Nothing may pin a wave in place

Three separate stalls have shipped in this game, all the same bug wearing
different clothes: a wave that can be held still forever neither dies nor leaks,
so it never ends and the run hangs. Stun-lock did it, then knockback, then
Abyss - whose pull briefly scaled with the *damage* curve and dragged monsters
six tiles per hit at level eight.

Cooldowns and diminishing returns are not enough, because they only shorten each
effect; they do nothing about how often it lands. Two hard bounds fix it for
good, and both are stated as guarantees rather than tunings:

- After a stun ends, a monster **cannot be stunned again for 1.2 seconds**. It
  therefore moves for at least that long out of every stun-plus-window.
- Every monster has a **total pushback budget of 5 tiles** for its whole life,
  shared by knockback and pull. Once spent, the road is a one-way street.

The balance harness asserts that no wave takes longer than 200 seconds, which is
what catches the next one.

---

## 9. Rendering

The game is top-down, the camera is fixed, and the models are stylised low-poly
solids. It was being drawn with a deferred-flavoured pipeline costing seven
render passes a frame - shadow map, multisampled scene, separate effects buffer,
three bloom blits, composite - and in a browser falling back to WebGL2 that is
where the frame went. Measured on a packed board (198 towers, 5,300 instances)
the CPU side costs **0.05 ms** to build the draw list and **0.03 ms** to step the
simulation on a full board of ninety-nine towers and a hundred and twenty
monsters. Nothing about the slowness was the simulation, and
`a_packed_board_costs_almost_nothing_on_the_cpu` keeps that true.

The pipeline is therefore collapsed by quality tier:

| Quality | Passes | What runs |
|---|---|---|
| **Low** | 1 | scene straight to the screen, no shadows, no MSAA, no post |
| **Medium** | 2 | 1024px shadow map, 1 tap, then scene to screen |
| **High** | 3 | 2048px shadow, 4 taps, scene to HDR, tonemap composite |
| **Ultra** | 7 | as before - 4x MSAA, bloom chain, the lot |

Low and Medium write **directly to the swapchain** - no intermediate HDR
texture, no resolve, no composite blit. Glows are drawn in the same pass as the
solids with additive blending and depth-test-no-write, which removes the
separate effects target entirely.

The default is chosen by measuring the opening seconds of frames and stepping
down until the frame budget is met, so a phone gets Low and a desktop with a
discrete GPU gets High without anyone being asked.

---

## 10. What is deliberately not here

- **No mazing.** The road is fixed. A creep's position is one scalar - how far
  along it has walked - which is what keeps four thousand of them cheap.
- **No difficulty menu.** One curve, tuned properly, beats three curves of which
  one was ever tested.
- **No ray tracing.** WebGPU does not expose it, so it cannot ship to a browser,
  and the GPU was never the bottleneck the request assumed it was.
- **No server-side simulation.** Every client runs its own board from a shared
  seed. The server relays scoreboards and nothing else, which is what lets it
  hold a thousand players in a gigabyte.
