//! Static game data. See `docs/DESIGN.md` for why any of these numbers are
//! what they are.
//!
//! Eight towers, each owning a role nothing else covers, each forking into two
//! specialisations at tier 3. Nine monster types, each punishing one lazy habit.

// ---------------------------------------------------------------- damage / armour

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Damage {
    /// Arrows and cannonballs. Bounces off plate, shreds casters.
    Physical,
    /// Frost and lightning. Melts plate, fizzles on wards.
    Magic,
    /// Venom and fire. Never resisted, never bonus, ignores shields.
    Poison,
    /// Support towers that do not attack.
    None,
}

impl Damage {
    pub fn name(self) -> &'static str {
        match self {
            Damage::Physical => "Physical",
            Damage::Magic => "Magic",
            Damage::Poison => "Poison",
            Damage::None => "Support",
        }
    }
    pub fn color(self) -> [f32; 3] {
        match self {
            Damage::Physical => [0.85, 0.80, 0.62],
            Damage::Magic => [0.42, 0.68, 1.00],
            Damage::Poison => [0.52, 0.90, 0.36],
            Damage::None => [0.80, 0.72, 1.00],
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Armor {
    Unarmoured,
    Heavy,
    Warded,
    Boss,
}

impl Armor {
    pub fn name(self) -> &'static str {
        match self {
            Armor::Unarmoured => "Unarmoured",
            Armor::Heavy => "Heavy",
            Armor::Warded => "Warded",
            Armor::Boss => "Boss",
        }
    }
    pub fn color(self) -> [f32; 3] {
        match self {
            Armor::Unarmoured => [0.58, 0.72, 0.52],
            Armor::Heavy => [0.62, 0.66, 0.76],
            Armor::Warded => [0.72, 0.52, 0.95],
            Armor::Boss => [0.95, 0.42, 0.30],
        }
    }
}

/// The counter triangle. This table is the spine of the whole game.
pub fn armor_mult(d: Damage, a: Armor) -> f32 {
    match (d, a) {
        (Damage::Physical, Armor::Heavy) => 0.55,
        (Damage::Physical, Armor::Warded) => 1.25,
        (Damage::Magic, Armor::Heavy) => 1.25,
        (Damage::Magic, Armor::Warded) => 0.55,
        // Bosses tax everything, so they are beaten with volume and debuffs.
        (Damage::Poison, Armor::Boss) => 1.0,
        (_, Armor::Boss) => 0.85,
        _ => 1.0,
    }
}

/// Which layer a monster travels on. The single biggest source of build
/// tension in the game: your two hardest-hitting towers cannot touch the air,
/// so a board that only answers the road dies the first time something flies.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Layer {
    Ground,
    Air,
}

/// What a tower is able to shoot at.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Targets {
    /// Mortars and fire pools. They own the ground and pay for it.
    GroundOnly,
    Both,
    /// Support and economy - never attacks anything.
    Nothing,
}

impl Targets {
    pub fn can_hit(self, layer: Layer) -> bool {
        match self {
            Targets::Both => true,
            Targets::GroundOnly => layer == Layer::Ground,
            Targets::Nothing => false,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Targets::GroundOnly => "Ground only",
            Targets::Both => "Ground + Air",
            Targets::Nothing => "Support",
        }
    }
}

// ---------------------------------------------------------------- towers

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Delivery {
    /// Homing shot that tracks its target.
    Shot { speed: f32 },
    /// Instant hitscan beam; `pierce` extra targets behind the first.
    Beam { pierce: u32 },
    /// Straight-line shot that damages everything it passes through.
    Lance { speed: f32 },
    /// Leaps from the target to nearby monsters. `hop` is how far each leap can
    /// reach, which matters more than the bounce count on a spread-out road.
    Chain { bounces: u32, falloff: f32, hop: f32 },
    /// Untargeted shockwave rolling out from the tower.
    Nova,
    /// Sets a patch of road alight and leaves it burning. The tower does not
    /// track a monster at all - it holds ground. Nothing else in the roster
    /// does this, which is the point: it is the only tower whose *position on
    /// the road* is worth more than its stats.
    Zone { radius: f32, dur: f32 },
    /// Does not attack at all.
    Aura,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Special {
    Burn { dps: f32, dur: f32 },
    Slow { amt: f32, dur: f32 },
    Poison { dps: f32, dur: f32 },
    Crit { chance: f32, mult: f32 },
    Stun { chance: f32, dur: f32 },
    /// Target takes `amt` extra damage from everything for `dur`.
    Shred { amt: f32, dur: f32 },
    /// Damage grows while the tower holds the same target.
    Ramp { per_hit: f32, max: f32 },
    Knockback { dist: f32 },
    /// Buffs every tower in range. Beacon only.
    Buff { dmg: f32, rate: f32, range: f32 },
    /// Pays out at the end of every wave.
    Income { per_wave: u32 },
    /// Extra gold on kill.
    Bounty { flat: u32, chance: f32, bonus: u32 },
    /// Adds to the interest rate.
    Interest { extra: f32 },
    /// Damage-over-time jumps to nearby monsters when the victim dies.
    Contagion { radius: f32 },
}

impl Special {
    /// One-line description with the numbers already scaled to a tier.
    pub fn describe(&self, k: f32) -> String {
        match *self {
            Special::Burn { dps, dur } => format!("Burns {:.0}/s for {:.0}s", dps * k, dur),
            Special::Slow { amt, dur } => format!("Slows {:.0}% for {:.1}s", amt * 100.0, dur),
            Special::Poison { dps, dur } => {
                format!("Venom {:.0}/s for {:.0}s, stacks", dps * k, dur)
            }
            Special::Crit { chance, mult } => {
                format!("{:.0}% crit for {:.1}x", chance * 100.0, mult)
            }
            Special::Stun { chance, dur } => {
                format!("{:.0}% chance to freeze {:.1}s", chance * 100.0, dur)
            }
            Special::Shred { amt, dur } => {
                format!("Target takes +{:.0}% damage for {:.0}s", amt * 100.0, dur)
            }
            Special::Ramp { per_hit, max } => format!(
                "+{:.0}% per hit on one target, up to +{:.0}%",
                per_hit * 100.0,
                max * 100.0
            ),
            Special::Knockback { dist } => format!("Knocks back {:.1} tiles", dist),
            Special::Buff { dmg, rate, range } => format!(
                "Nearby towers: +{:.0}% damage, +{:.0}% rate, +{:.1} range",
                dmg * 100.0,
                rate * 100.0,
                range
            ),
            Special::Income { per_wave } => format!("+{per_wave} gold every wave"),
            Special::Bounty { flat, chance, bonus } => {
                format!("+{} gold per kill, {:.0}% for +{}", flat, chance * 100.0, bonus)
            }
            Special::Interest { extra } => format!("+{:.0}% interest each wave", extra * 100.0),
            Special::Contagion { radius } => {
                format!("Damage-over-time spreads {radius:.1} tiles on death")
            }
        }
    }
}

/// A tier-3 specialisation. Applied on top of the tier-3 base stats.
pub struct Fork {
    pub name: &'static str,
    pub desc: &'static str,
    pub dmg_mul: f32,
    pub rate_mul: f32,
    pub range_add: f32,
    pub splash_add: f32,
    /// Replaces the base specials when non-empty, otherwise they carry over.
    pub specials: &'static [Special],
    pub keep_base: bool,
    pub delivery: Option<Delivery>,
}

pub struct TowerDef {
    pub id: &'static str,
    pub name: &'static str,
    pub role: &'static str,
    pub desc: &'static str,
    pub dtype: Damage,
    /// Which layers this tower can shoot at.
    pub targets: Targets,
    pub dmg: f32,
    /// Attacks per second.
    pub rate: f32,
    /// Radius in tiles.
    pub range: f32,
    /// Splash radius in tiles; 0 = single target.
    pub splash: f32,
    pub cost: u32,
    pub delivery: Delivery,
    pub specials: &'static [Special],
    pub color: [f32; 3],
    pub forks: [Fork; 2],
}

/// Six upgrade levels. Levels 1-3 grow the base tower, level 4 forks into one of
/// two specialisations, levels 5-6 grow inside that fork.
pub const MAX_TIER: u32 = 10;
/// The level at which the player must choose a specialisation.
pub const FORK_TIER: u32 = 4;
/// Past this level a tower is "awakened": the fork's identity is dialled up
/// hard, and the model shows it. It gives the back half of the upgrade ladder
/// a moment instead of ten identical steps.
pub const AWAKEN_TIER: u32 = 8;

// Damage grows faster than cost every level, so upgrading always beats building
// wide - that is what makes a single plot worth pouring gold into. The gap per
// level is deliberately small (1.76 vs 1.62, about 9% better per gold) so the
// choice stays close across all ten levels rather than being decided at level 2.
// Ten levels over eighty waves is roughly one upgrade every eight waves per
// tower, which is the pace an hour-long run wants. See docs/DESIGN.md.
const DMG_STEP: f32 = 1.76;
const COST_STEP: f32 = 1.62;
/// Extra damage multiplier applied once a tower is awakened.
const AWAKEN_BONUS: f32 = 1.35;

impl TowerDef {
    pub fn scale(tier: u32) -> f32 {
        DMG_STEP.powi(tier as i32 - 1)
    }
    pub fn cost_at(&self, tier: u32) -> u32 {
        let raw = self.cost as f32 * COST_STEP.powi(tier as i32 - 1);
        // Round to something a player can hold in their head. Nobody prices a
        // decision off "4,187 gold".
        let step = if raw < 200.0 {
            5.0
        } else if raw < 2_000.0 {
            10.0
        } else if raw < 20_000.0 {
            50.0
        } else {
            250.0
        };
        ((raw / step).round() * step).max(step) as u32
    }
    /// Stats for a level, with the fork applied once the tower has reached
    /// [`FORK_TIER`].
    pub fn stats(&self, tier: u32, fork: Option<usize>) -> Stats {
        let k = Self::scale(tier);
        let step = (tier - 1) as f32;
        let mut s = Stats {
            dmg: self.dmg * k,
            rate: self.rate,
            range: self.range + 0.16 * step,
            splash: if self.splash > 0.0 { self.splash + 0.09 * step } else { 0.0 },
            delivery: self.delivery,
            scale: k,
        };
        if tier >= AWAKEN_TIER {
            s.dmg *= AWAKEN_BONUS;
        }
        if tier >= FORK_TIER {
            if let Some(f) = fork.and_then(|i| self.forks.get(i)) {
                s.dmg *= f.dmg_mul;
                s.rate *= f.rate_mul;
                s.range += f.range_add;
                if s.splash > 0.0 || f.splash_add > 0.0 {
                    s.splash = (s.splash + f.splash_add).max(0.0);
                }
                if let Some(d) = f.delivery {
                    s.delivery = d;
                }
            }
        }
        // A chain tower's reach per leap grows with its level, so upgrading it
        // widens the web instead of only hardening each hit.
        if let Delivery::Chain { bounces, falloff, hop } = s.delivery {
            s.delivery = Delivery::Chain {
                bounces: bounces + (tier / 3),
                falloff,
                hop: hop + 0.30 * step,
            };
        }
        s
    }

    /// How much stronger a support or economy effect is at this level. These do
    /// not scale with damage, so they get their own curve - otherwise a Beacon or
    /// a Mint stops being worth a plot by the midgame.
    pub fn utility_scale(tier: u32) -> f32 {
        1.0 + 0.55 * (tier - 1) as f32
    }
    /// Every special active at this level and fork. Returned by value so the
    /// combat loop never allocates.
    pub fn specials_for(&self, fork: Option<usize>) -> SpecialSet {
        let mut set = SpecialSet::default();
        match fork.and_then(|i| self.forks.get(i)) {
            Some(f) => {
                if f.keep_base {
                    set.extend(self.specials);
                }
                set.extend(f.specials);
            }
            None => set.extend(self.specials),
        }
        set
    }
    pub fn dps_at(&self, tier: u32, fork: Option<usize>) -> f32 {
        let s = self.stats(tier, fork);
        s.dmg * s.rate
    }

    /// Roughly how much damage a shot lands across all targets, so chain and
    /// splash towers are not judged on their single-target number alone.
    pub fn effective_dps_at(&self, tier: u32, fork: Option<usize>) -> f32 {
        let s = self.stats(tier, fork);
        let spread = match s.delivery {
            Delivery::Chain { bounces, falloff, .. } => {
                let mut total = 1.0;
                let mut f = 1.0;
                for _ in 0..bounces {
                    f *= falloff;
                    total += f;
                }
                total
            }
            Delivery::Beam { pierce } => 1.0 + pierce as f32 * 0.6,
            _ if s.splash > 0.0 => 1.0 + s.splash * 1.2,
            _ => 1.0,
        };
        s.dmg * s.rate * spread
    }
}

/// A tower never has more than a handful of specials, so they live inline.
#[derive(Clone, Copy)]
pub struct SpecialSet {
    arr: [Special; 6],
    n: usize,
}

impl Default for SpecialSet {
    fn default() -> Self {
        Self { arr: [Special::Knockback { dist: 0.0 }; 6], n: 0 }
    }
}

impl SpecialSet {
    fn extend(&mut self, src: &[Special]) {
        for s in src {
            if self.n < self.arr.len() {
                self.arr[self.n] = *s;
                self.n += 1;
            }
        }
    }
    pub fn iter(&self) -> impl Iterator<Item = &Special> {
        self.arr[..self.n].iter()
    }
    /// How much extra damage this tower's shred makes a target take.
    pub fn shred_amt(&self) -> f32 {
        self.iter()
            .find_map(|s| match *s {
                Special::Shred { amt, .. } => Some(amt),
                _ => None,
            })
            .unwrap_or(0.0)
    }
    pub fn find_buff(&self) -> Option<(f32, f32, f32)> {
        self.iter().find_map(|s| match *s {
            Special::Buff { dmg, rate, range } => Some((dmg, rate, range)),
            _ => None,
        })
    }
}

#[derive(Clone, Copy)]
pub struct Stats {
    pub dmg: f32,
    pub rate: f32,
    pub range: f32,
    pub splash: f32,
    pub delivery: Delivery,
    pub scale: f32,
}

pub fn tower_color(d: &TowerDef) -> [f32; 3] {
    d.color
}

pub static TOWERS: &[TowerDef] = &[
    TowerDef {
        id: "ballista", name: "Ballista", role: "Single target",
        desc: "Cheap, reliable, always worth having. Bolts punch through wards, and they go up.",
        dtype: Damage::Physical,
        targets: Targets::Both,
        dmg: 20.0, rate: 1.10, range: 3.6, splash: 0.0, cost: 55,
        delivery: Delivery::Shot { speed: 22.0 },
        specials: &[],
        color: [0.86, 0.74, 0.46],
        forks: [
            Fork {
                name: "Marksman",
                desc: "One enormous bolt at extreme range, and it crits.",
                dmg_mul: 2.2, rate_mul: 0.55, range_add: 1.6, splash_add: 0.0,
                specials: &[Special::Crit { chance: 0.35, mult: 2.5 }],
                keep_base: true, delivery: None,
            },
            Fork {
                name: "Repeater",
                desc: "Triple rate of fire. Shreds anything without a shield.",
                dmg_mul: 0.45, rate_mul: 3.0, range_add: 0.2, splash_add: 0.0,
                specials: &[], keep_base: true, delivery: None,
            },
        ],
    },
    TowerDef {
        id: "cannon", name: "Cannon", role: "Splash",
        desc: "The hardest hitter on the road, and it cannot elevate. Pair it with something that can.",
        dtype: Damage::Physical,
        targets: Targets::GroundOnly,
        dmg: 46.0, rate: 0.50, range: 3.1, splash: 1.10, cost: 70,
        delivery: Delivery::Shot { speed: 11.0 },
        specials: &[],
        color: [0.78, 0.56, 0.36],
        forks: [
            Fork {
                name: "Mortar",
                desc: "Lobs across the map with a much wider blast.",
                dmg_mul: 1.5, rate_mul: 0.70, range_add: 1.8, splash_add: 0.9,
                specials: &[], keep_base: true, delivery: None,
            },
            Fork {
                name: "Grapeshot",
                desc: "Short-range shrapnel, fired fast. Devastating up close.",
                dmg_mul: 0.55, rate_mul: 2.2, range_add: -0.5, splash_add: 0.5,
                specials: &[Special::Knockback { dist: 0.35 }],
                keep_base: true, delivery: None,
            },
        ],
    },
    TowerDef {
        id: "frost", name: "Frost", role: "Control",
        desc: "Barely hurts. Buys every other tower on the board more shots, on either layer.",
        dtype: Damage::Magic,
        targets: Targets::Both,
        dmg: 15.0, rate: 1.05, range: 3.4, splash: 0.0, cost: 60,
        delivery: Delivery::Shot { speed: 18.0 },
        specials: &[Special::Slow { amt: 0.45, dur: 2.0 }],
        color: [0.46, 0.80, 1.00],
        forks: [
            Fork {
                name: "Glacier",
                desc: "Freezes outright, and hits far harder while it does.",
                dmg_mul: 1.3, rate_mul: 1.0, range_add: 0.3, splash_add: 0.0,
                specials: &[Special::Stun { chance: 0.22, dur: 1.1 }],
                keep_base: true, delivery: None,
            },
            Fork {
                name: "Rime",
                desc: "The chill makes them brittle - everything else hits harder.",
                dmg_mul: 1.0, rate_mul: 1.2, range_add: 0.3, splash_add: 0.0,
                specials: &[
                    Special::Slow { amt: 0.55, dur: 2.5 },
                    Special::Shred { amt: 0.30, dur: 3.0 },
                ],
                keep_base: false, delivery: None,
            },
        ],
    },
    TowerDef {
        id: "pyre", name: "Pyre", role: "Area denial",
        desc: "Sets the road itself on fire. Everything standing in it takes more damage from everything else you own.",
        dtype: Damage::Poison,
        targets: Targets::GroundOnly,
        dmg: 26.0, rate: 0.42, range: 3.2, splash: 0.0, cost: 75,
        delivery: Delivery::Zone { radius: 1.15, dur: 4.5 },
        // The damage is the smaller half of this tower. The shred is why you
        // build it: a Pyre on a corner makes every other tower on the board
        // hit harder for as long as the fire burns.
        specials: &[Special::Shred { amt: 0.28, dur: 1.0 }],
        color: [1.00, 0.52, 0.20],
        forks: [
            Fork {
                name: "Wildfire",
                desc: "A far wider blaze that burns much longer. Shuts down a whole bend.",
                dmg_mul: 0.80, rate_mul: 1.0, range_add: 0.8, splash_add: 0.0,
                specials: &[Special::Shred { amt: 0.24, dur: 1.0 }],
                keep_base: false,
                delivery: Some(Delivery::Zone { radius: 1.95, dur: 7.0 }),
            },
            Fork {
                name: "Crucible",
                desc: "A tighter, hotter blaze that strips defences almost completely.",
                dmg_mul: 1.55, rate_mul: 1.15, range_add: 0.2, splash_add: 0.0,
                specials: &[Special::Shred { amt: 0.55, dur: 1.0 }],
                keep_base: false,
                delivery: Some(Delivery::Zone { radius: 0.90, dur: 4.0 }),
            },
        ],
    },
    TowerDef {
        id: "tesla", name: "Tesla", role: "Chain",
        desc: "Arcs from target to target, ground or air. Its reach per leap grows every level.",
        dtype: Damage::Magic,
        targets: Targets::Both,
        dmg: 19.0, rate: 0.90, range: 3.6, splash: 0.0, cost: 80,
        delivery: Delivery::Chain { bounces: 3, falloff: 0.80, hop: 3.2 },
        specials: &[],
        color: [0.62, 0.86, 1.00],
        forks: [
            Fork {
                name: "Storm",
                desc: "Far more leaps, each reaching much further. Clears a road on its own.",
                dmg_mul: 1.15, rate_mul: 1.0, range_add: 0.8, splash_add: 0.0,
                specials: &[], keep_base: true,
                delivery: Some(Delivery::Chain { bounces: 7, falloff: 0.88, hop: 4.6 }),
            },
            Fork {
                name: "Overload",
                desc: "Fewer leaps, but every one can stop a monster dead.",
                dmg_mul: 1.55, rate_mul: 1.0, range_add: 0.4, splash_add: 0.0,
                specials: &[Special::Stun { chance: 0.25, dur: 0.7 }],
                keep_base: true,
                delivery: Some(Delivery::Chain { bounces: 4, falloff: 0.90, hop: 3.8 }),
            },
        ],
    },
    TowerDef {
        id: "venom", name: "Venom", role: "Single target ramp",
        desc: "Stacks on one target and grows the longer it holds it. Never resisted - the answer to a boss of any armour.",
        dtype: Damage::Poison,
        targets: Targets::Both,
        dmg: 9.0, rate: 1.10, range: 3.2, splash: 0.0, cost: 80,
        delivery: Delivery::Shot { speed: 17.0 },
        // Poison plus ramp: weak on a swarm it never stays on, brutal on one
        // enormous health bar it can sit on for twenty seconds.
        specials: &[
            Special::Poison { dps: 16.0, dur: 5.0 },
            Special::Ramp { per_hit: 0.055, max: 2.4 },
        ],
        color: [0.56, 0.92, 0.38],
        forks: [
            Fork {
                name: "Blight",
                desc: "Ramps far higher and far faster. Melts anything that lives long enough.",
                dmg_mul: 1.15, rate_mul: 1.15, range_add: 0.3, splash_add: 0.0,
                specials: &[
                    Special::Poison { dps: 26.0, dur: 6.0 },
                    Special::Ramp { per_hit: 0.075, max: 4.0 },
                ],
                keep_base: false, delivery: None,
            },
            Fork {
                name: "Plague",
                desc: "Whatever it kills infects the pack around it. Gives up the ramp for reach.",
                dmg_mul: 1.05, rate_mul: 1.25, range_add: 0.5, splash_add: 0.0,
                specials: &[
                    Special::Poison { dps: 24.0, dur: 6.0 },
                    Special::Contagion { radius: 2.2 },
                ],
                keep_base: false, delivery: None,
            },
        ],
    },
    TowerDef {
        id: "beacon", name: "Beacon", role: "Support",
        desc: "Fires nothing. Makes every tower around it substantially better.",
        dtype: Damage::None,
        targets: Targets::Nothing,
        dmg: 0.0, rate: 0.0, range: 3.0, splash: 0.0, cost: 90,
        delivery: Delivery::Aura,
        specials: &[Special::Buff { dmg: 0.30, rate: 0.22, range: 0.6 }],
        color: [0.86, 0.76, 1.00],
        forks: [
            Fork {
                name: "Warhorn",
                desc: "Pure aggression: a large damage boost to everything nearby.",
                dmg_mul: 1.0, rate_mul: 1.0, range_add: 0.4, splash_add: 0.0,
                specials: &[Special::Buff { dmg: 0.70, rate: 0.22, range: 0.5 }],
                keep_base: false, delivery: None,
            },
            Fork {
                name: "Lodestone",
                desc: "Stretches the reach of the whole cluster, and sharpens it.",
                dmg_mul: 1.0, rate_mul: 1.0, range_add: 0.6, splash_add: 0.0,
                specials: &[
                    Special::Buff { dmg: 0.25, rate: 0.45, range: 1.5 },
                    Special::Crit { chance: 0.20, mult: 2.0 },
                ],
                keep_base: false, delivery: None,
            },
        ],
    },
    TowerDef {
        id: "mint", name: "Mint", role: "Economy",
        // It fires nothing at all. A Mint that also shot things would just be a
        // worse Ballista with a bonus, and the decision to build one would be
        // free - the whole point is that it costs you a plot during the waves
        // you are most fragile.
        desc: "Fires nothing. Pays every wave, forever. Build it early or not at all.",
        dtype: Damage::None,
        targets: Targets::Nothing,
        dmg: 0.0, rate: 0.0, range: 2.9, splash: 0.0, cost: 85,
        delivery: Delivery::Aura,
        specials: &[Special::Income { per_wave: 26 }],
        color: [1.00, 0.82, 0.34],
        forks: [
            Fork {
                name: "Treasury",
                desc: "Raises the interest on every gold piece you are holding.",
                dmg_mul: 1.0, rate_mul: 1.0, range_add: 0.0, splash_add: 0.0,
                specials: &[
                    Special::Income { per_wave: 60 },
                    Special::Interest { extra: 0.04 },
                ],
                keep_base: false, delivery: None,
            },
            Fork {
                name: "Toll",
                desc: "Takes a cut of every monster that dies anywhere near it.",
                dmg_mul: 1.0, rate_mul: 1.0, range_add: 1.2, splash_add: 0.0,
                specials: &[
                    Special::Income { per_wave: 32 },
                    Special::Bounty { flat: 6, chance: 0.25, bonus: 25 },
                ],
                keep_base: false, delivery: None,
            },
        ],
    },
];

pub fn tower_index(id: &str) -> usize {
    TOWERS.iter().position(|t| t.id == id).unwrap_or(0)
}

/// Shop order: cheapest first. Nothing is gated - gold is the only limit.
pub fn shop_order() -> Vec<usize> {
    let mut idx: Vec<usize> = (0..TOWERS.len()).collect();
    idx.sort_by_key(|&i| TOWERS[i].cost);
    idx
}

// ---------------------------------------------------------------- monsters

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Grunt,
    Runner,
    Brute,
    Swarm,
    Warden,
    Mender,
    Bulwark,
    Phaser,
    Boss,
    /// Air. Fast, fragile, arrives in a cloud. Punishes having no anti-air.
    Wisp,
    /// Air, heavily plated. Punishes anti-air that is all physical damage.
    Drake,
    /// Air boss. If your whole answer to bosses was a wall of cannons, this is
    /// where the run ends.
    Skylord,
}

impl Kind {
    pub fn name(self) -> &'static str {
        match self {
            Kind::Grunt => "Grunt",
            Kind::Runner => "Runner",
            Kind::Brute => "Brute",
            Kind::Swarm => "Swarm",
            Kind::Warden => "Warden",
            Kind::Mender => "Mender",
            Kind::Bulwark => "Bulwark",
            Kind::Phaser => "Phaser",
            Kind::Boss => "BOSS",
            Kind::Wisp => "Wisp",
            Kind::Drake => "Drake",
            Kind::Skylord => "SKYLORD",
        }
    }

    /// Which layer this monster travels on.
    pub fn layer(self) -> Layer {
        match self {
            Kind::Wisp | Kind::Drake | Kind::Skylord => Layer::Air,
            _ => Layer::Ground,
        }
    }
    pub fn flying(self) -> bool {
        self.layer() == Layer::Air
    }
    /// How high above the road it travels, in tiles.
    pub fn altitude(self) -> f32 {
        match self {
            Kind::Wisp => 1.55,
            Kind::Drake => 1.75,
            Kind::Skylord => 2.10,
            _ => 0.0,
        }
    }
    pub fn is_boss(self) -> bool {
        matches!(self, Kind::Boss | Kind::Skylord)
    }
    pub fn armor(self) -> Armor {
        match self {
            Kind::Brute | Kind::Bulwark | Kind::Drake => Armor::Heavy,
            Kind::Warden | Kind::Phaser => Armor::Warded,
            Kind::Boss | Kind::Skylord => Armor::Boss,
            _ => Armor::Unarmoured,
        }
    }
    pub fn radius(self) -> f32 {
        match self {
            Kind::Swarm => 0.20,
            Kind::Wisp => 0.22,
            Kind::Runner => 0.24,
            Kind::Grunt | Kind::Warden | Kind::Phaser => 0.30,
            Kind::Mender => 0.32,
            Kind::Drake => 0.38,
            Kind::Brute | Kind::Bulwark => 0.40,
            Kind::Boss => 0.62,
            Kind::Skylord => 0.58,
        }
    }
    /// What this monster punishes, shown in the wave preview.
    pub fn tell(self) -> &'static str {
        match self {
            Kind::Grunt => "",
            Kind::Runner => "very fast",
            Kind::Brute => "heavy armour",
            Kind::Swarm => "huge pack",
            Kind::Warden => "warded",
            Kind::Mender => "heals nearby",
            Kind::Bulwark => "damage shield",
            Kind::Phaser => "shrugs off slows",
            Kind::Boss => "immune to stun",
            Kind::Wisp => "FLYING, fast swarm",
            Kind::Drake => "FLYING, heavy armour",
            Kind::Skylord => "FLYING boss",
        }
    }
}

#[derive(Clone, Copy)]
pub struct WaveDef {
    pub kind: Kind,
    pub count: u32,
    pub hp: f32,
    pub speed: f32,
    pub bounty: u32,
    pub gap: f32,
    /// Absorbs this much damage before its health is touched.
    pub shield: f32,
    /// Heals nearby monsters this fraction of their max health per second.
    pub heal: f32,
    /// Ignores slows for half of every second.
    pub phasing: bool,
    pub regen: bool,
    pub split: bool,
}

impl WaveDef {
    pub fn armor(&self) -> Armor {
        self.kind.armor()
    }
    pub fn modifiers(&self) -> Vec<&'static str> {
        let mut v = Vec::new();
        let t = self.kind.tell();
        if !t.is_empty() {
            v.push(t);
        }
        if self.regen {
            v.push("regenerating");
        }
        if self.split {
            v.push("splits on death");
        }
        v
    }
}

/// Length of the authored campaign. Clearing it is a win - but the run does not
/// have to stop there, see [`ENDLESS_HP_STEP`].
pub const CAMPAIGN_WAVES: u32 = 80;
/// Kept as the old name so UI and tests read naturally.
pub const N_WAVES: u32 = CAMPAIGN_WAVES;

/// Past the campaign the waves keep coming and keep growing. Health climbs a
/// little faster than the purse, so endless always eventually wins - the
/// question is only how far you get.
pub const ENDLESS_HP_STEP: f32 = 1.075;
pub const ENDLESS_GOLD_STEP: f32 = 1.062;

/// Per-wave growth of monster health and of the gold a wave pays out.
///
/// These two numbers are the difficulty curve, and they are tuned by playing
/// the game, not by argument - `a_sensible_build_clears_the_campaign` runs a
/// full eighty-wave campaign with a deliberately unsophisticated strategy and
/// checks it wins with lives in single figures. At 1.1350 it finishes with 8
/// of 20; at 1.1375 it dies on wave 76. That margin is the game.
///
/// Health has to outrun gold by a lot, because the player's board also grows by
/// *count* and not only by level - and because a board stops growing entirely
/// once it is maxed, while the waves never do.
/// Health of a wave-1 monster. Sets how tight the opening is, and therefore how
/// much surplus the player carries into the rest of the run - a generous
/// opening compounds into a trivial endgame.
const HP_BASE: f32 = 70.0;
/// Gold a wave-1 wave pays out in total.
const GOLD_BASE: f32 = 58.0;
const HP_STEP: f32 = 1.1350;
const GOLD_STEP: f32 = 1.0655;

/// The road is roughly 60 tiles long; this sets how briskly monsters cover it.
pub const WALK_SPEED: f32 = 1.7;

/// Lives you start with. One number, because there is one difficulty.
///
/// Three difficulty settings meant three curves, and only one of them was ever
/// tuned properly. A single curve that is actually good is worth more than a
/// menu of curves that are roughly right - so the run is balanced to be beaten
/// by a player who reads the wave preview and answers it, and to punish one who
/// does not.
pub const START_LIVES: i32 = 20;

/// Which monster shows up on which wave. New types arrive on a schedule so the
/// player always has one wave of warning to buy the counter.
fn kind_for(wave: u32) -> Kind {
    // Bosses alternate layers, so a board built entirely out of cannons meets
    // something it cannot touch every twenty waves.
    if wave % 10 == 0 {
        return if (wave / 10) % 2 == 0 { Kind::Skylord } else { Kind::Boss };
    }
    // Types unlock on a schedule, always with a wave of warning in the preview.
    let mut pool: Vec<Kind> = vec![Kind::Grunt];
    if wave >= 3 {
        pool.push(Kind::Runner);
    }
    if wave >= 5 {
        pool.push(Kind::Swarm);
    }
    if wave >= 7 {
        // The first flying wave. Early enough that having no anti-air costs a
        // life or two rather than the run.
        pool.push(Kind::Wisp);
    }
    if wave >= 9 {
        pool.push(Kind::Brute);
    }
    if wave >= 13 {
        pool.push(Kind::Warden);
    }
    if wave >= 17 {
        pool.push(Kind::Drake);
    }
    if wave >= 21 {
        pool.push(Kind::Mender);
    }
    if wave >= 26 {
        pool.push(Kind::Bulwark);
    }
    if wave >= 32 {
        pool.push(Kind::Phaser);
    }
    pool[(wave as usize * 7 + wave as usize / 3) % pool.len()]
}

/// Whether this is the first wave a given monster type ever appears on.
///
/// A new type's debut is deliberately softened. The game should *teach* a
/// mechanic before it tests it: the first flying wave arriving at full strength
/// against a board with no anti-air does not teach anything, it just ends the
/// run before the player knows what hit them.
fn is_debut(wave: u32, kind: Kind) -> bool {
    wave > 1 && (1..wave).all(|w| kind_for(w) != kind)
}

/// Builds any wave, at any number, at any difficulty. Waves past the campaign
/// keep escalating, so a run is only over when the player runs out of lives.
pub fn wave_at(i: u32) -> WaveDef {
    let i = i.max(1);
    let kind = kind_for(i);
    let debut = is_debut(i, kind);
    let (count, hp_mul, speed) = match kind {
        Kind::Boss => (1, 12.0, 0.95),
        Kind::Skylord => (1, 9.0, 1.10),
        Kind::Swarm => (28, 0.42, 1.30),
        Kind::Wisp => (22, 0.38, 1.55),
        Kind::Runner => (12, 0.70, 1.90),
        Kind::Brute => (9, 2.20, 0.85),
        Kind::Bulwark => (8, 1.60, 0.85),
        Kind::Drake => (9, 1.45, 1.05),
        Kind::Mender => (10, 1.10, 1.00),
        Kind::Warden => (12, 1.15, 1.05),
        Kind::Phaser => (12, 1.25, 1.15),
        Kind::Grunt => (14, 1.0, 1.15),
    };

    // Campaign curve, then an unbounded endless curve on top.
    //
    // Health and gold both grow geometrically, and health grows faster. That
    // gap is the difficulty: early on your gold outruns the monsters and the
    // board fills up, late on it does not and you have to choose what to feed.
    // A flat gold curve against a geometric health curve (which is what this
    // used to be) makes the back half unwinnable rather than hard.
    let campaign = i.min(CAMPAIGN_WAVES);
    let over = i.saturating_sub(CAMPAIGN_WAVES);
    let base = HP_BASE * HP_STEP.powi(campaign as i32 - 1);
    // A debut wave is half the size and two thirds the health: enough to hurt,
    // not enough to end a run that had no answer ready.
    let (count, debut_hp) = if debut { ((count as f32 * 0.55).ceil() as u32, 0.65) } else { (count, 1.0) };
    let hp = base * ENDLESS_HP_STEP.powi(over as i32) * hp_mul * debut_hp;

    let purse =
        GOLD_BASE * GOLD_STEP.powi(campaign as i32 - 1) * ENDLESS_GOLD_STEP.powi(over as i32);
    // Only part of the purse rides on kills. See KILL_SHARE.
    let bounty = (purse * KILL_SHARE / count as f32).max(1.0).round() as u32;

    WaveDef {
        kind,
        count,
        hp,
        speed: speed * WALK_SPEED,
        bounty,
        gap: match kind {
            Kind::Boss => 0.0,
            Kind::Swarm => 0.26,
            _ => 0.60,
        },
        // Bulwarks absorb a flat pool that scales with the wave.
        shield: if kind == Kind::Bulwark {
            120.0 * 1.14f32.powi(campaign as i32 - 1) * ENDLESS_HP_STEP.powi(over as i32)
        } else {
            0.0
        },
        heal: if kind == Kind::Mender { 0.030 } else { 0.0 },
        phasing: kind == Kind::Phaser,
        regen: i >= 20 && i % 8 == 7,
        split: i >= 22 && i % 9 == 2 && kind != Kind::Boss,
    }
}

pub fn build_waves() -> Vec<WaveDef> {
    (1..=N_WAVES).map(wave_at).collect()
}

/// Gold handed out for clearing a wave.
/// How much of a wave's purse is paid per kill rather than for surviving it.
///
/// Paying the whole purse on kills sounds right and plays badly: a wave you
/// only half-clear pays half, so the board you build next wave is weaker, so
/// you clear even less. One bad wave used to spiral into a dead run, which made
/// the balance bimodal - the same curve either cruised to victory with 18 lives
/// or collapsed at wave 60, with almost nothing in between.
///
/// Splitting the purse fixes the shape. Falling behind still costs lives, which
/// is the real currency, but it no longer quietly destroys the economy you need
/// to recover. Killing things is still clearly better than not.
const KILL_SHARE: f32 = 0.55;

/// Paid for reaching the end of a wave, whatever leaked. The other side of
/// [`KILL_SHARE`].
pub fn wave_clear_bonus(wave: u32) -> u32 {
    let campaign = wave.min(CAMPAIGN_WAVES);
    let over = wave.saturating_sub(CAMPAIGN_WAVES);
    let purse = GOLD_BASE * GOLD_STEP.powi(campaign as i32 - 1);
    // The split half of the purse, plus the flat survival payment that has
    // always been here. Total wave income is unchanged by the split - only who
    // has to earn it is.
    let flat = (25 + campaign * 5) as f32;
    ((purse * (1.0 - KILL_SHARE) + flat) * ENDLESS_GOLD_STEP.powi(over as i32)) as u32
}

/// Gold per second remaining when a wave is called early.
pub const EARLY_BONUS_PER_SEC: f32 = 2.0;
