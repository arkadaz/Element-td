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

## 2. The circuit

> **There is no exit, and there are no lives. You are defending a rate.**

The road is a **closed ring**. Monsters enter it and walk, and nothing ever
reaches an end, because there is no end. What the towers cannot kill comes round
again, and again, and the ring fills up. The run is lost when more than
**320 monsters** are circling at once.

This replaced a twenty-life counter, and it is a better gauge for four reasons:

- **It moves continuously.** A life counter jumps in whole lives, so a player
  learns they are in trouble at the moment it becomes unrecoverable. The ring
  fills over a dozen waves, visibly.
- **It makes leftovers a debt.** A wave you only three-quarters killed is
  carried into the next one, and the one after. Pressure is cumulative instead
  of per-wave, which is what makes the back half of a run feel like a run.
- **It cannot be gamed by displacement.** A tower that shoves monsters backwards
  used to buy free distance from the exit. On a ring, backwards is the same
  direction.
- **It gives every monster more than one pass.** A survivor is not a loss, it is
  a second chance at it - so a board slightly behind the curve degrades
  gracefully instead of falling off a cliff.

Waves arrive on a **fixed 42-second clock** whether the last one is dead or not,
and each wave's whole count streams in evenly across its own period. There is no
build phase: gold is spent while the road is busy. The only quiet stretch in a
run is the twenty-two seconds before wave one.

Taken from Green Circle TD, which does all of this - a closed circle, no lives,
a loss condition of 700 living monsters, and 36 waves on a 45-second clock. What
is not taken from it is the wave *content*: this keeps its own elements, armour
table and draft.

---

## 2b. The core loop

> **Draft an element. Elements combine into towers. Towers hold the ring.**

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
| **Shade** | Dark | Toxic | air + ground | **one-strike kill** chance, pays gold per kill |

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
| **Bastion** | E+L | Physical | air + ground | **multishot** - three targets at once |
| **Tombstone** | E+D | - | none | economy: pays every wave, raises interest |
| **Eclipse** | L+D | Magic | air + ground | stuns and shreds - the boss answer |

Five towers cannot shoot upwards (Boulder, Mire, Thornwall, Magma, Silt). Two do
not shoot at all (Grove, Tombstone). Both facts are load-bearing: they are what
stops "build the highest-DPS thing everywhere" from being correct.

### Two effects the circuit made necessary

A hundred and fifty monsters on the road at once is a different problem from a
dozen, and two of Green Circle TD's towers answer it directly:

- **Multishot** (Bastion) fires a full hit at three targets at a time. On a
  circuit, targets-per-second matters more than damage-per-target, and this is
  the only effect that buys it outright. Bastion used to be the
  highest-single-target tower, which is an identity worth almost nothing here.
- **One-strike kill** (Shade) has a small chance to delete a non-boss outright,
  whatever its health. It is a lottery rather than a damage source, so it is
  worth exactly as much as the number of things walking past - the one effect
  in the game that gets *better* as the ring fills up. Never on a boss: a boss
  deleted by a coin flip is not a boss.

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

**Eighty waves on a 42-second clock**, plus twenty-two seconds before the
first. That is **58 minutes**, exactly, and the arithmetic is now trivial -
which is itself worth having. The old version had to estimate how long each
wave would take to walk a road, and got it wrong by a factor of 1.7.

The draft pauses are the load-bearing pacing. They are the only moments the
clock stops, and therefore the only moments the player looks up.

Calling a wave early pays **2 gold per second saved**, and on a circuit that is
a genuine gamble rather than a free speed-up: whatever you have not killed yet
does not go anywhere, so the new stream stacks on top of it. The gold is the
reward for judging that your board can take it.

Winning means surviving all eighty **and clearing the ring**. Outlasting the
final stream is not the same as killing it. The run may continue into
**endless**, where health climbs 7.5% and the purse 6.2% per wave - health
outruns gold, so endless always ends eventually. The question is only how far.

### What a wave is

A wave is **not a burst**. Its whole count arrives one monster at a time, evenly
spread across its own 42 seconds, so the road is never empty and no tower is
ever idle.

Counts are **eight times** what they were, and each monster is **an eighth** as
tough - the same total health per wave, so the tuned difficulty curve carried
over intact, but a completely different shape of problem. A dozen fat monsters
is a game about single-target damage; a hundred thin ones is a game about area,
throughput and coverage. With no exit to leak from, the second is the game worth
having.

Bosses are the exception: five of them per boss wave rather than a hundred, at
a fifth of the wave's health each. A single unkillable boss on a circuit is not
a threat, it is a nuisance occupying one slot of a three-hundred-slot gauge; a
pack of five is a real damage sink, because every tower pointed at it is a tower
not killing the stream arriving behind it.

### The four acts

| Act | Waves | What it is about |
|---|---|---|
| **I - The Ring** | 1-20 | Learning layers and armour. First air at 7, first Plated at 9. The gauge stays near empty. |
| **II - Pressure** | 21-40 | Healers, swarms, wards. Focus fire and area damage. Leftovers start to be visible. |
| **III - Attrition** | 41-60 | Ethereal, shields, splitting, phasing. The gauge begins to climb and does not come back down. |
| **IV - The Deep** | 61-80 | Cross-layer escorts every wave. The ring sits at 85-95% full and every wave is the one that might overflow. |

A measured run bears this out: a naive board keeps the ring under 70 of 320 for
fifty waves, is at 194 by wave 59, and then spends waves 60 to 80 between 260
and 310 - ten straight waves of not knowing whether it will hold. The old
twenty-life version, by contrast, lost nothing at all for sixty-four waves and
then died twice in the last fifteen.

---

## 8. Economy

- Start with **260 gold**. There are no lives; see section 2.
- A wave's purse is fixed. **55%** rides on kills, **45%** is a stipend paid
  when the wave *starts* - because on a circuit no wave ever ends. The stipend
  exists so that a board which has fallen behind still has the money to climb
  back out. Paying the whole purse on kills made the balance bimodal: cruise to
  victory, or collapse at wave 60, with almost nothing between.
- Escorts split the same purse across more monsters. They are a threat, not a
  payday.
- **Interest** pays 5% of gold in hand at each wave boundary, up to **20%** with
  Tombstones, and only on gold up to a ceiling of twelve wave-purses. Uncapped
  compound interest is not an economy, it is a runaway - the previous version
  reached 813 billion gold by wave 89 and ran to wave 136 without difficulty.
- Selling refunds **75%** of everything invested.

### The ring, and its hundred and sixteen pads

The circuit is a rounded rectangle roughly **59 tiles** round - so a lap takes
about half a minute - with **116 build plots** on alternating tiles, inside the
ring and outside it.

Both numbers matter more than they look. Pads were once on every tile in the
band, which gave a hundred and ninety-eight of them, and a campaign purse that
can cover two hundred plots is a campaign with no placement decision in it:
filling the board with cheap towers beat levelling good ones, which is the
opposite of what the cost curve is built to reward. A simulated run confirmed
it - a bot that papered the board with level-one towers cleared eighty waves
without losing a life. Alternating tiles makes each plot worth thinking about
and leaves levels as the real sink for gold.

The lap time matters because it sets how many chances a board gets at each
monster. Too short and the ring is a blur with no positional meaning; too long
and a tower on the far side is irrelevant to what is happening on this one.

### Nothing may pin a wave in place

Three separate stalls shipped in this game before the circuit existed, all the
same bug wearing different clothes: a wave that can be held still forever
neither dies nor leaks, so it never ends and the run hangs. Stun-lock did it,
then knockback, then Abyss - whose pull briefly scaled with the *damage* curve
and dragged monsters six tiles per hit at level eight.

The circuit removes the *hang* - the wave clock does not care whether anything
moved - but the underlying problem is worse here, not better: a monster pinned
in place is a monster occupying a slot in the flood gauge forever, so hard
control that never expires is a slow way to lose.

Cooldowns and diminishing returns are not enough on their own, because they only
shorten each effect; they do nothing about how often it lands. Two hard bounds
fix it, and both are stated as guarantees rather than tunings:

- After a stun ends, a monster **cannot be stunned again for 1.2 seconds**. It
  therefore moves for at least that long out of every stun-plus-window.
- Every monster has a **pushback budget of 5 tiles per lap**, shared by
  knockback and pull. Once spent, the ring turns one-way until it comes round.

The harness asserts the wave clock actually advances, which is what catches the
next one.

---

## 9. Rendering

The game is top-down, the camera is fixed, and the models are stylised low-poly
solids. It was being drawn with a deferred-flavoured pipeline costing seven
render passes a frame - shadow map, multisampled scene, separate effects buffer,
three bloom blits, composite - and in a browser falling back to WebGL2 that is
where the frame went. Measured on a packed board (198 towers, 5,300 instances)
the CPU side costs **0.09 ms** to build the draw list and **0.08 ms** to step the
simulation on the worst frame the game can produce - a full ring of 320 monsters
against a full board of 116 towers, ten thousand instances. Nothing about the
slowness was the simulation, and `a_packed_board_costs_almost_nothing_on_the_cpu`
keeps that true.

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

- **No mazing.** The circuit is fixed. A creep's position is one scalar - how
  far round it has walked - which is what keeps three hundred of them cheap: the
  worst frame the game can produce, a full ring against a full board, costs
  0.09 ms to build and 0.08 ms to simulate.
- **No difficulty menu.** One curve, tuned properly, beats three curves of which
  one was ever tested.
- **No ray tracing.** WebGPU does not expose it, so it cannot ship to a browser,
  and the GPU was never the bottleneck the request assumed it was.
- **No server-side simulation.** Every client runs its own board from a shared
  seed. The server relays scoreboards and nothing else, which is what lets it
  hold a thousand players in a gigabyte.
