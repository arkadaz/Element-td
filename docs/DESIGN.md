# Elemental TD — game design

One difficulty, eighty waves, about an hour. Everything below exists to make
each of those waves a decision rather than a wait.

## 1. The thesis

A tower defense is only as good as the question it asks each wave. Ours asks
three at once, and no single build answers all three:

1. **Layer** — is it walking or flying? Your two hardest hitters cannot shoot up.
2. **Armour** — physical bounces off plate, magic fizzles on wards, poison is
   never resisted but never bonused.
3. **Tempo** — gold spent on damage now, or on economy that pays for more damage
   in ten waves' time.

Any two of these can be solved by one build. All three cannot. That is the game.

## 2. The roster: eight towers, eight verbs

The previous roster had a real flaw, and it is worth naming: **Pyre and Venom
were the same tower.** Both were poison-type damage-over-time that ignored
armour. Two towers competing for one job means one of them is always strictly
worse, and the player is right to feel the roster is padded.

Every tower now owns exactly one verb that nothing else in the game does:

| Tower | Verb — owned outright | Damage | Layers | Why you build it |
|---|---|---|---|---|
| **Ballista** | Single-target burst | Physical | Ground + Air | The reliable answer to one big thing |
| **Cannon** | Instant splash on impact | Physical | **Ground only** | The answer to a crowd on the road |
| **Frost** | Slow | Magic | Ground + Air | Buys every other tower more shots |
| **Tesla** | Chain between targets | Magic | Ground + Air | The answer to a spread-out line, and to air |
| **Venom** | Stacking DoT that ramps on one target | Poison | Ground + Air | The boss killer |
| **Pyre** | **Persistent ground zone + shred** | Poison | **Ground only** | Makes the road itself dangerous |
| **Beacon** | Buffs neighbouring towers | — | — | Makes a cluster worth more than its parts |
| **Mint** | Income and interest | — | — | Trades tempo now for a bigger late game |

### What changed and why

**Pyre is no longer a burn tower.** It does not target a monster at all. It
lights a **patch of road on fire** and leaves it burning. Everything standing in
the patch takes modest damage and — the real payload — is **shredded**: it takes
more damage from *everything else on the board* while it stands there.

That gives Pyre a verb nobody else has (a persistent zone), a distinct reason to
exist (it is the force multiplier for the *road*, the way Beacon is the force
multiplier for *towers*), and a natural, honest reason it cannot hit air: it
burns the ground. Placement matters for the first time — a Pyre on a corner
where monsters bunch is worth several elsewhere.

**Venom becomes the single-target specialist.** Its poison stacks with itself
and ramps the longer it stays on one target, so it is weak against a swarm and
devastating against one enormous health bar. It is the answer to a boss, and
being poison it is the answer to a boss of *any* armour type.

So the four damage towers now pair off cleanly rather than overlapping:

- **Cannon and Pyre** both hit the ground crowd — but one is instant burst and
  the other is sustained area denial. You want both.
- **Ballista and Venom** both kill one big thing — but one is burst that lands
  now and the other is a ramp that pays off over ten seconds.
- **Frost and Tesla** are the layer-agnostic pair: hold everything still, arc
  through everything at once.

## 3. Ground and air

The single biggest addition, and the reason the roster has tension.

| Monster | Layer | Punishes |
|---|---|---|
| Wisp | Air | Having no anti-air at all. Fast, fragile, arrives in a cloud. |
| Drake | Air | Anti-air that is all one damage type — it is heavily plated. |
| Skylord | Air | A boss answer built entirely out of cannons. |

**Cannon and Pyre — the two biggest area dealers — cannot touch the air.** This
is deliberate and it is the core build tension: the strongest ground board in
the game is helpless against a wave it cannot reach. Every other attacker hits
both layers, so the fix is never "build the anti-air tower", it is "do not spend
your whole board on the road".

Air arrives at **wave 7**, early enough that learning it costs a life or two
rather than the run.

## 4. The wave schedule

Eighty waves. Every tenth is a boss, and bosses **alternate layers** — 10, 30,
50, 70 walk; 20, 40, 60, 80 fly.

New types arrive on a schedule, always with a wave of warning in the preview:

| Wave | Arrival | Lesson |
|---|---|---|
| 1 | Grunt | — |
| 3 | Runner | Speed; range coverage matters |
| 5 | Swarm | Volume; you need splash |
| **7** | **Wisp** | **You need something that shoots up** |
| 9 | Brute | Heavy armour; physical is the wrong tool |
| 13 | Warden | Warded; magic is the wrong tool |
| **17** | **Drake** | Armoured air — anti-air needs a damage type |
| 21 | Mender | Focus fire, or nothing dies |
| 26 | Bulwark | Shields; poison bypasses them |
| 32 | Phaser | Slows stop working half the time |

After 32 the pool cycles, mixing types so no single counter carries.

## 5. Pacing to an hour

| | |
|---|---|
| First build phase | 30 s |
| Build phase after that | 15 s |
| Combat, typical wave | ~30 s |
| **Per wave** | **~45 s** |
| **80 waves** | **~60 min** |

Calling a wave early pays a gold bonus proportional to the time left, so a
player who knows the run compresses it — the classic tower-defense skill
expression, and the reason the hour is a ceiling rather than a floor.

## 6. Ten tower levels

| Levels | What happens |
|---|---|
| 1–3 | The base tower grows |
| **4** | **Fork** — pick one of two specialisations, permanently |
| 5–7 | The fork grows |
| **8** | **Awaken** — the fork's identity is amplified hard, and the model changes |
| 9–10 | Final growth |

Damage per level ×1.76, cost per level ×1.62. The gap is deliberately narrow —
about 9% better damage-per-gold each level — so "upgrade this one or build
another" stays a close call at every level instead of being settled at level 2.
Ten levels across eighty waves is roughly one upgrade per tower every eight
waves, which is the rhythm an hour-long run wants.

Costs round to human numbers (5s, 10s, 50s, 250s). Nobody prices a decision off
"4,187 gold".

## 7. The economy

Monster health grows **1.155×** per wave. Wave gold grows **1.0655×** per wave.

That gap looks enormous, and it has to be. The player's board does not grow at
the rate of their income — it grows by tower *count* and by *level* on top of
it, and damage-per-gold improves at every level. Those compound. A gap that
"looks fair" on paper produces a game you win with 18 of 20 lives.

These two numbers were not reasoned into place, they were played into place.
`a_sensible_build_clears_the_campaign` runs a full eighty-wave campaign and
checks the result.

The bot models **competent** play, and getting that right mattered more than the
curve did. It first placed towers to spread evenly along the road — which is
wrong, and losing: a monster then meets one tower at a time and survives each of
them in turn. Concentrated fire kills, and overlapping ranges stack Beacon
auras, so the bot now builds a **killbox**. The moment it did, it cleared the
old curve **without losing a single life** — proof the game was too easy for
anyone playing well, whatever the naive version had suggested.

Against a killbox, 1.155 finishes with **5 of 20 lives and 15 leaks**. That is
the number to hold.

### Gold does not all ride on kills

**55% of a wave's purse is paid per kill; the rest is paid for surviving it.**

Paying everything on kills sounds right and plays badly. A wave you half-clear
pays half, so your next board is weaker, so you clear even less — one bad wave
used to spiral into a dead run. It made the balance bimodal: the same curve
either cruised to victory with 18 lives or collapsed around wave 60, with almost
nothing in between.

Splitting the purse fixes the shape. Falling behind still costs lives, which is
the currency that actually matters, but it no longer quietly destroys the
economy you need to recover with.

### Escorts

From wave 25 some waves bring a **second monster type**, and from wave 45 that
escort is always on the *other layer* — a ground wave brings flyers, a flying
wave brings walkers. Bosses gain a guard from wave 50.

One type per wave means one counter always answers it and the roster stops
mattering by the midgame. Escorts are how a wave asks two questions at once.

A wave's purse is fixed, so an escort **splits** the same gold across more
monsters rather than paying extra for them. Getting that backwards made escorted
waves *easier* — more targets, more income, a bigger board.

### Debut waves are softened

The first time any monster type appears it comes at **55% count and 65%
health**. The game should teach a mechanic before it tests it. The first flying
wave arriving at full strength against a board with no anti-air does not teach
anything — it just ends the run before the player knows what hit them. Without
this, wave 7 was an instant loss; with it, wave 7 costs a few lives and wave 10
is where ignoring the lesson actually kills you.

Mint and interest let a player bet on the far side of the curve. Building a Mint
on wave 5 is a real gamble: it is a plot not shooting anything during the waves
where you are most fragile — and it fires nothing at all, so there is no
consolation prize.

## 8. What a good run looks like

- **Waves 1–10.** Cheap Ballistas and a Cannon. One Mint if you are confident.
  Wave 7 forces the first anti-air.
- **Waves 11–30.** Forks are chosen. Frost goes on the first corner. A Pyre
  lands on the tightest bend on the road. Wave 20 is the first flying boss.
- **Waves 31–55.** Beacons start paying. Venom goes down for the bosses. The
  board stops growing outward and starts growing upward.
- **Waves 56–80.** Nothing new is built; everything is being awakened. The
  squeeze is real and the last five waves should genuinely hurt.

## 9. Hard control is strong, never absolute

Two effects could stop the game outright, and both did:

- **Stuns** now diminish on repeat (each lands ~34% shorter, to a floor) and the
  resistance only bleeds off while the target is free to move.
- **Knockback** has a per-target cooldown of 0.75 s.

Without those, a board of Frost and Grapeshot pinned a wave in place forever:
every monster permanently frozen or shoved backwards, so nothing died, nothing
leaked, and the wave never ended. A full campaign hung on wave 76. A control
board should slow a wave down, not stop time.

## 10. The economy cannot run away

Interest is capped at **20%**, and it is paid only on gold up to a **ceiling**
that rises with the wave.

This is a bug that shipped, and it is worth stating plainly: each Treasury used
to add `0.04 × utility_scale(tier)` to the interest rate, which at level 10 is
**+23.8% each**, with no ceiling. Four of them put compound interest over 100%
a wave. A real game reached **813 billion gold on wave 89**, kept everything
maxed permanently, and coasted to wave 136. Infinite money is the same thing as
no game.

Anything that multiplies a compounding rate multiplies an exponential, so the
Treasury bonus is no longer scaled by tier at all — a Treasury pays for itself
through flat income and a modest rate bump.

Banking a wave or two of income is a real strategy and should pay. Hoarding
forever should not, which is what the ceiling is for.

## 11. Saving

The run is simulated entirely on the player's machine, so the save lives there:
`localStorage` in the browser, a file in the OS config directory natively. The
server stores nothing — it is sized so a gigabyte of RAM holds a thousand
players, and per-player run state would undo that at a stroke.

It is deliberately **not** keyed by IP address. An IP is not an identity: a
phone changes it several times an hour, and everyone behind one router or one
carrier-grade NAT shares it, so players would resume into each other's games.
It is also personal data this needs none of.

Only a seed, a wave number, a purse and one line per tower are stored — about a
kilobyte. Waves are generated from their number, so replaying the seed
reproduces the run exactly without storing any of it. A save is a file anyone
can edit, so it is validated on load and refused whole rather than half-applied.

## 12. Endless

Clearing wave 80 is a win. The run may continue: health then climbs 7.5% a wave
against 6.2% gold growth, a much steeper squeeze, so endless always eventually
wins. The only question is how far.

## 13. How this is kept honest

Balance arguments on paper are worth very little — the curve before this one
looked reasonable written down and was arithmetically impossible past wave 40.
The tests play the game instead:

| Test | What it refuses to let happen |
|---|---|
| `a_sensible_build_clears_the_campaign` | The campaign becomes unwinnable, or winnable while asleep |
| `ignoring_the_air_loses_the_run` | The ground/air split becomes decoration |
| `every_tower_owns_a_verb_nothing_else_has` | Two towers doing the same job (this is how Pyre/Venom was caught) |
| `the_air_layer_splits_the_roster` | The ground-only pair losing its compensation |
| `the_run_tightens_from_start_to_finish` | Any wave becoming a wall rather than a step |
| `a_full_run_is_about_an_hour` | The session length drifting |
| `ground_towers_cannot_touch_the_air` | A mortar quietly learning to shoot upward |
| `pyre_burns_the_road_and_only_the_road` | Area denial losing either its zone or its restriction |
| `a_wall_of_control_towers_cannot_freeze_a_wave_forever` | Stun or knockback stopping the game outright |
| `a_board_built_entirely_of_treasuries_cannot_run_away` | The economy compounding into infinity |
| `interest_pays_on_a_bounded_pile` | Hoarding becoming better than spending |
| `deep_endless_payouts_stay_finite` | Gold saturating `u32` and paying nonsense |
| `a_run_survives_a_round_trip` | A resumed board not being the board that was saved |
| `a_corrupt_save_is_refused_rather_than_half_applied` | An edited save producing a board the game would refuse to build |
