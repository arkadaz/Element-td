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
pub const MAX_TIER: u32 = 6;
/// The level at which the player must choose a specialisation.
pub const FORK_TIER: u32 = 4;

// Damage doubles-and-a-bit per level while cost exactly doubles, so upgrading is
// meaningfully better gold-per-dps than building wide - which is what makes a
// plot worth investing in. A maxed tower lands near 1,100 dps for ~2,700 gold,
// and a competent run fields about 15 of them by wave 50. See docs/DESIGN.md.
const DMG_STEP: f32 = 2.20;
const COST_STEP: f32 = 2.00;

impl TowerDef {
    pub fn scale(tier: u32) -> f32 {
        DMG_STEP.powi(tier as i32 - 1)
    }
    pub fn cost_at(&self, tier: u32) -> u32 {
        (self.cost as f32 * COST_STEP.powi(tier as i32 - 1)).round() as u32
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
        desc: "Cheap, reliable, always worth having. Bolts punch through wards.",
        dtype: Damage::Physical,
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
        desc: "Slow, heavy shells. The answer to anything that arrives in a crowd.",
        dtype: Damage::Physical,
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
        desc: "Barely hurts. Buys every other tower on the board more shots.",
        dtype: Damage::Magic,
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
        id: "pyre", name: "Pyre", role: "Burn",
        desc: "Weak on impact, brutal afterwards. Armour does not stop fire.",
        dtype: Damage::Poison,
        dmg: 8.0, rate: 1.30, range: 2.9, splash: 0.35, cost: 65,
        delivery: Delivery::Shot { speed: 16.0 },
        specials: &[Special::Burn { dps: 22.0, dur: 3.0 }],
        color: [1.00, 0.52, 0.20],
        forks: [
            Fork {
                name: "Inferno",
                desc: "The fire leaps to whatever is standing nearby when they drop.",
                dmg_mul: 1.1, rate_mul: 1.0, range_add: 0.4, splash_add: 0.3,
                specials: &[
                    Special::Burn { dps: 60.0, dur: 4.0 },
                    Special::Contagion { radius: 1.6 },
                ],
                keep_base: false, delivery: None,
            },
            Fork {
                name: "Furnace",
                desc: "Winds up while it holds a target. Terrifying against bosses.",
                dmg_mul: 1.4, rate_mul: 1.4, range_add: 0.2, splash_add: 0.0,
                specials: &[Special::Ramp { per_hit: 0.05, max: 2.5 }],
                keep_base: true, delivery: None,
            },
        ],
    },
    TowerDef {
        id: "tesla", name: "Tesla", role: "Chain",
        desc: "Arcs from target to target. Its reach per leap grows every level.",
        dtype: Damage::Magic,
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
        id: "venom", name: "Venom", role: "Stacking DoT",
        desc: "Stacks with itself and ignores armour entirely. Start it early.",
        dtype: Damage::Poison,
        dmg: 7.0, rate: 1.20, range: 3.2, splash: 0.0, cost: 75,
        delivery: Delivery::Shot { speed: 17.0 },
        specials: &[Special::Poison { dps: 14.0, dur: 5.0 }],
        color: [0.56, 0.92, 0.38],
        forks: [
            Fork {
                name: "Plague",
                desc: "The venom carries to everything around the corpse.",
                dmg_mul: 1.1, rate_mul: 1.1, range_add: 0.4, splash_add: 0.0,
                specials: &[
                    Special::Poison { dps: 22.0, dur: 6.0 },
                    Special::Contagion { radius: 2.0 },
                ],
                keep_base: false, delivery: None,
            },
            Fork {
                name: "Blight",
                desc: "Eats armour, so every other tower you own hits harder.",
                dmg_mul: 1.0, rate_mul: 1.3, range_add: 0.4, splash_add: 0.0,
                specials: &[Special::Shred { amt: 0.35, dur: 4.0 }],
                keep_base: true, delivery: None,
            },
        ],
    },
    TowerDef {
        id: "beacon", name: "Beacon", role: "Support",
        desc: "Fires nothing. Makes every tower around it substantially better.",
        dtype: Damage::None,
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
        desc: "A poor weapon and a superb investment. Build it early or not at all.",
        dtype: Damage::Physical,
        dmg: 14.0, rate: 0.85, range: 2.9, splash: 0.0, cost: 85,
        delivery: Delivery::Shot { speed: 15.0 },
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
                desc: "Takes a cut of every monster that dies near it.",
                dmg_mul: 1.6, rate_mul: 1.2, range_add: 0.6, splash_add: 0.0,
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
        }
    }
    pub fn armor(self) -> Armor {
        match self {
            Kind::Brute | Kind::Bulwark => Armor::Heavy,
            Kind::Warden | Kind::Phaser => Armor::Warded,
            Kind::Boss => Armor::Boss,
            _ => Armor::Unarmoured,
        }
    }
    pub fn radius(self) -> f32 {
        match self {
            Kind::Swarm => 0.20,
            Kind::Runner => 0.24,
            Kind::Grunt | Kind::Warden | Kind::Phaser => 0.30,
            Kind::Mender => 0.32,
            Kind::Brute | Kind::Bulwark => 0.40,
            Kind::Boss => 0.62,
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
pub const CAMPAIGN_WAVES: u32 = 50;
/// Kept as the old name so UI and tests read naturally.
pub const N_WAVES: u32 = CAMPAIGN_WAVES;

/// Past the campaign the waves keep coming and keep growing. Health climbs a
/// little faster than the purse, so endless always eventually wins - the
/// question is only how far you get.
pub const ENDLESS_HP_STEP: f32 = 1.055;
pub const ENDLESS_GOLD_STEP: f32 = 1.048;

/// The road is roughly 60 tiles long; this sets how briskly monsters cover it.
pub const WALK_SPEED: f32 = 1.7;

/// How punishing the run is. Chosen before the first wave.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Difficulty {
    #[default]
    Normal,
    Hard,
    Nightmare,
}

impl Difficulty {
    pub const ALL: [Difficulty; 3] = [Difficulty::Normal, Difficulty::Hard, Difficulty::Nightmare];

    pub fn label(self) -> &'static str {
        match self {
            Difficulty::Normal => "Normal",
            Difficulty::Hard => "Hard",
            Difficulty::Nightmare => "Nightmare",
        }
    }
    pub fn blurb(self) -> &'static str {
        match self {
            Difficulty::Normal => "The intended run. Twenty lives, room to make mistakes.",
            Difficulty::Hard => "Half again as much health, fewer lives. Counters matter.",
            Difficulty::Nightmare => "Twice the health, six lives. Every plot has to earn itself.",
        }
    }
    /// Monster health multiplier.
    pub fn hp_mul(self) -> f32 {
        match self {
            Difficulty::Normal => 1.0,
            Difficulty::Hard => 1.45,
            Difficulty::Nightmare => 2.10,
        }
    }
    /// Harder runs pay a little better, or they are just a wall.
    pub fn bounty_mul(self) -> f32 {
        match self {
            Difficulty::Normal => 1.0,
            Difficulty::Hard => 1.10,
            Difficulty::Nightmare => 1.22,
        }
    }
    pub fn lives(self) -> i32 {
        match self {
            Difficulty::Normal => 20,
            Difficulty::Hard => 14,
            Difficulty::Nightmare => 8,
        }
    }
    pub fn next(self) -> Difficulty {
        match self {
            Difficulty::Normal => Difficulty::Hard,
            Difficulty::Hard => Difficulty::Nightmare,
            Difficulty::Nightmare => Difficulty::Normal,
        }
    }
}

/// Which monster shows up on which wave. New types arrive on a schedule so the
/// player always has one wave of warning to buy the counter.
fn kind_for(wave: u32) -> Kind {
    if wave % 10 == 0 {
        return Kind::Boss;
    }
    // Types unlock as the run goes on, then cycle through what is available.
    let mut pool: Vec<Kind> = vec![Kind::Grunt];
    if wave >= 3 {
        pool.push(Kind::Runner);
    }
    if wave >= 5 {
        pool.push(Kind::Swarm);
    }
    if wave >= 8 {
        pool.push(Kind::Brute);
    }
    if wave >= 13 {
        pool.push(Kind::Warden);
    }
    if wave >= 18 {
        pool.push(Kind::Mender);
    }
    if wave >= 24 {
        pool.push(Kind::Bulwark);
    }
    if wave >= 30 {
        pool.push(Kind::Phaser);
    }
    pool[(wave as usize * 7 + wave as usize / 3) % pool.len()]
}

/// Builds any wave, at any number, at any difficulty. Waves past the campaign
/// keep escalating, so a run is only over when the player runs out of lives.
pub fn wave_at(i: u32, diff: Difficulty) -> WaveDef {
    let i = i.max(1);
    let kind = kind_for(i);
    let (count, hp_mul, speed) = match kind {
        Kind::Boss => (1, 10.0, 0.95),
        Kind::Swarm => (28, 0.42, 1.30),
        Kind::Runner => (12, 0.70, 1.90),
        Kind::Brute => (9, 2.20, 0.85),
        Kind::Bulwark => (8, 1.60, 0.85),
        Kind::Mender => (10, 1.10, 1.00),
        Kind::Warden => (12, 1.15, 1.05),
        Kind::Phaser => (12, 1.25, 1.15),
        Kind::Grunt => (14, 1.0, 1.15),
    };

    // Campaign curve, then a gentler but unbounded endless curve on top.
    let campaign = i.min(CAMPAIGN_WAVES);
    let over = i.saturating_sub(CAMPAIGN_WAVES);
    let base = 60.0 * 1.132f32.powi(campaign as i32 - 1);
    let hp = base * ENDLESS_HP_STEP.powi(over as i32) * hp_mul * diff.hp_mul();

    let purse = (45.0 + campaign as f32 * 12.0)
        * ENDLESS_GOLD_STEP.powi(over as i32)
        * diff.bounty_mul();
    let bounty = (purse / count as f32).max(1.0).round() as u32;

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
    (1..=N_WAVES).map(|i| wave_at(i, Difficulty::Normal)).collect()
}

/// Gold handed out for clearing a wave.
pub fn wave_clear_bonus(wave: u32) -> u32 {
    let campaign = wave.min(CAMPAIGN_WAVES);
    let over = wave.saturating_sub(CAMPAIGN_WAVES);
    ((25 + campaign * 5) as f32 * ENDLESS_GOLD_STEP.powi(over as i32)) as u32
}

/// Gold per second remaining when a wave is called early.
pub const EARLY_BONUS_PER_SEC: f32 = 2.0;
