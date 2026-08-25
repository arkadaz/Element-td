# Elemental TD — Game Design

## 1. The promise

*Read the wave, pick the counter, commit your gold.* Every wave should make the
player ask one interesting question, and every tower they own should be the answer
to some question and the wrong answer to another.

Three rules the whole design serves:

1. **No tower is universally correct.** Armour types make each tower strong
   against some waves and weak against others. A board of one tower loses.
2. **Upgrades change behaviour, not just numbers.** Tier 3 is a *fork*: two
   specialisations that play differently. That is the decision players replay for.
3. **Money is a resource you can also just... keep.** Interest means "build now"
   and "build later" are both real plays.

---

## 2. Core loop

```
Build phase (16s, first 25s)  ->  Wave walks the road  ->  Payout + interest
        ^                                                        |
        +--------------------------------------------------------+
```

- **50 waves.** New monster type every ~5 waves, boss every 10.
- **20 lives.** A leak costs 1; a boss leak costs 10.
- **Send early** pays 2 gold per second skipped — the skill expression in economy.

---

## 3. Damage and armour

The counter triangle is the spine of the game. Three damage types, four armour types.

|              | Unarmoured | Heavy | Warded | Boss |
|--------------|-----------|-------|--------|------|
| **Physical** | 1.00      | 0.55  | 1.25   | 0.85 |
| **Magic**    | 1.00      | 1.25  | 0.55   | 0.85 |
| **Poison**   | 1.00      | 1.00  | 1.00   | 1.00 |

- **Physical** — arrows, cannonballs. Bounces off Heavy plate, shreds Warded casters.
- **Magic** — frost, lightning. Melts plate, fizzles on Warded.
- **Poison** — venom, fire. Never resisted, never bonus. The dependable floor,
  and the only damage that ignores shields.

Boss armour taxes everything by 15%, so bosses are beaten with *volume and
debuffs*, not a single counter.

---

## 4. Tower roster

Eight towers. Each owns a role nothing else covers. Tier 1 → 2 is a straight
power step; **tier 3 forks into two specialisations**.

| # | Tower | Type | Role | T3 fork |
|---|-------|------|------|---------|
| 1 | **Ballista** | Physical | Cheap reliable single-target | *Marksman* — crits, huge range / *Repeater* — triple rate |
| 2 | **Cannon** | Physical | Splash, slow rate | *Mortar* — long range, wide blast / *Grapeshot* — short cone, many pellets |
| 3 | **Frost** | Magic | Slow, low damage | *Glacier* — freezes / *Rime* — slow stacks into +damage-taken |
| 4 | **Pyre** | Poison | Burn over time | *Inferno* — burn spreads on death / *Furnace* — ramps while firing |
| 5 | **Tesla** | Magic | Chains between targets | *Storm* — more bounces / *Overload* — chains stun |
| 6 | **Venom** | Poison | Stacking DoT, ignores armour | *Plague* — spreads to neighbours / *Blight* — armour shred |
| 7 | **Beacon** | — | Support: buffs nearby towers | *Warhorn* — +damage aura / *Lodestone* — +range and crit |
| 8 | **Mint** | Physical | Economy: pays per wave | *Treasury* — bigger interest / *Toll* — gold per nearby kill |

### Why these eight

- **Ballista** is the tutorial tower and stays relevant via Marksman's range.
- **Cannon** is the swarm answer; without it, wave 11 buries you.
- **Frost** is the tempo tower — it buys every other tower more shots.
- **Pyre** and **Venom** are the Heavy-armour answer, and the only damage that
  keeps working when a wave resists everything else.
- **Tesla** is the "many targets, one tower" answer, and the reason to leave
  monsters bunched instead of killing the leader.
- **Beacon** is the multiplier: it makes a *tight cluster* of towers worth more
  than the same towers spread out. This is the main positional decision.
- **Mint** is the greed line — weak now, decisive by wave 30 if you survive.

### Stat baseline (tier 1)

| Tower | Damage | Rate/s | Range | Cost | DPS |
|-------|-------|--------|-------|------|-----|
| Ballista | 20 | 1.10 | 3.6 | 55 | 22 |
| Cannon | 46 | 0.50 | 3.1 | 70 | 23 |
| Frost | 11 | 1.00 | 3.4 | 60 | 11 (+45% slow) |
| Pyre | 8 | 1.30 | 2.9 | 65 | 10 (+22 burn) |
| Tesla | 15 | 0.85 | 3.3 | 80 | 13 (×3 targets) |
| Venom | 7 | 1.20 | 3.2 | 75 | 8 (+stacking) |
| Beacon | 0 | — | 3.0 | 90 | aura |
| Mint | 9 | 0.70 | 2.6 | 100 | 6 (+18 gold/wave) |

Tiers scale damage **×3.2** and cost **×3.6** per step, so upgrading is
gold-efficient but slot-hungry — the tension that makes pads scarce.

---

## 5. Monster roster

Each type exists to punish exactly one lazy habit.

| Monster | Armour | Trait | Punishes |
|---------|--------|-------|----------|
| **Grunt** | Unarmoured | — | nothing; the baseline |
| **Runner** | Unarmoured | 1.9× speed | slow-firing towers, thin coverage |
| **Brute** | Heavy | 2.2× hp | all-physical boards |
| **Swarm** | Unarmoured | 28 tiny bodies | single-target-only boards |
| **Warden** | Warded | — | all-magic boards |
| **Mender** | Unarmoured | heals nearby 3%/s | slow damage, spread targeting |
| **Bulwark** | Heavy | absorbs first 400 damage | many-weak-hits (Repeater, Swarm clear) |
| **Phaser** | Warded | slow-immune every other second | pure Frost control |
| **Boss** | Boss | 10× hp, stun-immune | everything at once |

Wave composition telegraphs one wave ahead, so the counter-buy is a *decision*,
not a memory test.

### Difficulty curve

`hp = 60 × 1.132^(wave-1) × kind multiplier` — a 50× climb by wave 50, against a
gold curve that affords roughly 8–10 fully upgraded towers. The gap is closed by
*positioning* (Beacon clusters, Frost overlap) rather than raw purchases.

---

## 6. Economy

| Source | Amount |
|--------|--------|
| Kill bounty | ~`(45 + 12×wave) / pack size` each |
| Wave clear | `25 + 5×wave` |
| **Interest** | **5% of gold in hand, every wave** |
| Send early | 2/second skipped |
| Sell | 75% refund |

Interest is the design's quiet engine: holding 1000 gold pays 50 a wave, which is
most of a tower. Spending everything immediately is a real cost, and rushing waves
to bank interest earlier is a real strategy.

---

## 7. Mastery arc

- **Waves 1–10** — learn the road, learn splash vs single target.
- **Waves 11–25** — armour types force a mixed board; Beacon placement starts mattering.
- **Waves 26–40** — tier 3 forks; the player commits to an identity.
- **Waves 41–50** — Menders and Bulwarks demand focus-fire discipline; bosses
  demand every debuff at once.

The player should finish a run able to explain *why* their board was shaped the
way it was. That is the mark we are aiming at.
