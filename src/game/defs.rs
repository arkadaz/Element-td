//! Static game data. See `docs/DESIGN.md` for why any of these numbers are
//! what they are.
//!
//! Six elements. Every element gives a **pure** tower, and every unordered pair
//! of elements gives a **dual** tower - six plus fifteen, twenty-one in all.
//! Which of them a player may build, and how far they may be upgraded, is
//! decided entirely by the essences they have drafted. Gold buys towers; only
//! essences decide *which* towers exist for you at all.

use crate::rng::Rng;

// ---------------------------------------------------------------- elements

/// The six elements. Declaration order is the canonical order everywhere: in
/// the draft offer, in the build panel, and in the essence counters.
#[derive(Clone, Copy, PartialEq, Eq, Debug, PartialOrd, Ord)]
pub enum Element {
    Nature,
    Fire,
    Water,
    Earth,
    Light,
    Dark,
}

pub const ELEMENTS: [Element; 6] = [
    Element::Nature,
    Element::Fire,
    Element::Water,
    Element::Earth,
    Element::Light,
    Element::Dark,
];

impl Element {
    pub fn idx(self) -> usize {
        self as usize
    }
    pub fn name(self) -> &'static str {
        match self {
            Element::Nature => "Nature",
            Element::Fire => "Fire",
            Element::Water => "Water",
            Element::Earth => "Earth",
            Element::Light => "Light",
            Element::Dark => "Dark",
        }
    }
    /// One-word temperament, shown under the name in the draft.
    pub fn flavour(self) -> &'static str {
        match self {
            Element::Nature => "Poison that gets worse",
            Element::Fire => "Burst, burn and area",
            Element::Water => "Slow, chain, control",
            Element::Earth => "Weight and broken armour",
            Element::Light => "Range, precision, buffs",
            Element::Dark => "Debuffs, execution, gold",
        }
    }
    pub fn color(self) -> [f32; 3] {
        match self {
            Element::Nature => [0.44, 0.86, 0.38],
            Element::Fire => [1.00, 0.50, 0.16],
            Element::Water => [0.34, 0.72, 1.00],
            Element::Earth => [0.80, 0.58, 0.32],
            Element::Light => [1.00, 0.88, 0.48],
            Element::Dark => [0.70, 0.46, 0.98],
        }
    }
    /// Single letter for the cramped corners of the HUD.
    pub fn glyph(self) -> &'static str {
        match self {
            Element::Nature => "N",
            Element::Fire => "F",
            Element::Water => "W",
            Element::Earth => "E",
            Element::Light => "L",
            Element::Dark => "D",
        }
    }
}

/// How many essences the campaign hands out, and on which waves.
///
/// Front-loaded, so the opening has real choices, and thinning out, so the late
/// game is about using the board you committed to rather than still being
/// handed new toys.
pub const ESSENCE_WAVES: [u32; 20] = [
    1, 2, 3, 5, 7, 9, 12, 15, 18, 21, 25, 29, 33, 37, 42, 47, 52, 58, 64, 71,
];

/// How many elements the player chooses between at each award.
pub const DRAFT_SIZE: usize = 3;

/// Essences of one element needed before a tower using it reaches full level.
/// Two levels are free, so six essences reach [`MAX_TIER`].
pub const TIER_PER_ESSENCE: u32 = 1;
pub const FREE_TIERS: u32 = 2;

/// The ceiling a tower can be upgraded to, given the essences held.
///
/// A pure tower reads its own element; a dual tower reads whichever of its two
/// elements the player has fewer of, which is what makes a two-element
/// commitment cost twice as much as a one-element one.
pub fn tier_cap(essence: &[u8; 6], def: &TowerDef) -> u32 {
    let held = |e: Element| essence[e.idx()] as u32;
    let n = match def.elem {
        (a, None) => held(a),
        (a, Some(b)) => held(a).min(held(b)),
    };
    if n == 0 {
        return 0;
    }
    (FREE_TIERS + n * TIER_PER_ESSENCE).min(MAX_TIER)
}

/// Builds the three-element offer for a draft.
///
/// Whenever both are possible the offer holds **at least one element already
/// held** and **at least one not held**, so the choice is always between going
/// deeper and going wider. An offer of three elements the player already has
/// six of is not a decision, it is a notification with buttons.
pub fn draft_offer(rng: &mut Rng, essence: &[u8; 6]) -> [Element; DRAFT_SIZE] {
    let mut pool = ELEMENTS;
    // Fisher-Yates from the run's own stream, so a seed reproduces every offer.
    for i in (1..pool.len()).rev() {
        let j = (rng.next_u64() % (i as u64 + 1)) as usize;
        pool.swap(i, j);
    }
    let held = |e: Element| essence[e.idx()] > 0;
    let mut offer = [pool[0], pool[1], pool[2]];

    // Only enforce the mix when the board can actually satisfy it.
    let any_held = ELEMENTS.iter().any(|&e| held(e));
    let any_new = ELEMENTS.iter().any(|&e| !held(e));
    if any_held && !offer.iter().any(|&e| held(e)) {
        if let Some(&swap) = pool[DRAFT_SIZE..].iter().find(|&&e| held(e)) {
            offer[DRAFT_SIZE - 1] = swap;
        }
    }
    if any_new && !offer.iter().any(|&e| !held(e)) {
        if let Some(&swap) = pool[DRAFT_SIZE..].iter().find(|&&e| !held(e)) {
            offer[0] = swap;
        }
    }
    offer
}

// ---------------------------------------------------------------- damage / armour

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Damage {
    /// Weight and edges. Bounces off plate, punches through wards, and passes
    /// straight through anything that is not properly there.
    Physical,
    /// Frost, light and the void. Melts plate, fizzles on wards, and is the one
    /// thing an Ethereal cannot ignore.
    Magic,
    /// Heat. Loves a crowd of unarmoured things and hates a ghost.
    Fire,
    /// Venom and rot. Never resisted, never bonus. The honest answer.
    Toxic,
    /// Support towers that do not attack.
    None,
}

impl Damage {
    pub fn name(self) -> &'static str {
        match self {
            Damage::Physical => "Physical",
            Damage::Magic => "Magic",
            Damage::Fire => "Fire",
            Damage::Toxic => "Toxic",
            Damage::None => "Support",
        }
    }
    pub fn color(self) -> [f32; 3] {
        match self {
            Damage::Physical => [0.85, 0.80, 0.62],
            Damage::Magic => [0.42, 0.68, 1.00],
            Damage::Fire => [1.00, 0.56, 0.22],
            Damage::Toxic => [0.52, 0.90, 0.36],
            Damage::None => [0.80, 0.72, 1.00],
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Armor {
    Unarmoured,
    Plated,
    Warded,
    /// Half-there. Physical weapons pass through and fire barely warms it, but
    /// magic bites harder than on anything else in the game.
    Ethereal,
    Boss,
}

impl Armor {
    pub fn name(self) -> &'static str {
        match self {
            Armor::Unarmoured => "Unarmoured",
            Armor::Plated => "Plated",
            Armor::Warded => "Warded",
            Armor::Ethereal => "Ethereal",
            Armor::Boss => "Boss",
        }
    }
    pub fn color(self) -> [f32; 3] {
        match self {
            Armor::Unarmoured => [0.58, 0.72, 0.52],
            Armor::Plated => [0.62, 0.66, 0.76],
            Armor::Warded => [0.72, 0.52, 0.95],
            Armor::Ethereal => [0.58, 0.92, 0.90],
            Armor::Boss => [0.95, 0.42, 0.30],
        }
    }
}

/// The counter table. This is the spine of the whole game: it is the reason a
/// board of the single highest-damage tower loses, and the reason breadth in the
/// draft is worth giving up levels for.
pub fn armor_mult(d: Damage, a: Armor) -> f32 {
    use Armor::*;
    use Damage::*;
    match (d, a) {
        // Toxic is flat against everything, including bosses. It is the worst
        // raw damage in the roster and the only damage that is never wrong.
        (Toxic, _) => 1.0,
        (Physical, Plated) => 0.55,
        (Physical, Warded) => 1.25,
        (Physical, Ethereal) => 0.70,
        (Magic, Plated) => 1.25,
        (Magic, Warded) => 0.55,
        (Magic, Ethereal) => 1.30,
        (Fire, Unarmoured) => 1.15,
        (Fire, Plated) => 0.85,
        // Fire is not the universal second answer. Without this a board of
        // magic and fire has no hole at all - magic beats plate and ghosts,
        // fire beats crowds, and wards were the one thing left for it to fear.
        (Fire, Warded) => 0.85,
        (Fire, Ethereal) => 0.60,
        (Fire, Boss) => 0.90,
        (_, Boss) => 0.85,
        _ => 1.0,
    }
}

/// Which layer a monster travels on. The single biggest source of build
/// tension in the game: five towers cannot touch the air, so a board that only
/// answers the road dies the first time something flies.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Layer {
    Ground,
    Air,
}

/// What a tower is able to shoot at.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Targets {
    /// Mortars, swamps and fire pools. They own the ground and pay for it.
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
    Chain {
        bounces: u32,
        falloff: f32,
        hop: f32,
    },
    /// Untargeted shockwave rolling out from the tower.
    Nova,
    /// Claims a patch of road and holds it. The tower does not track a monster
    /// at all - which is what makes *where it stands* worth more than its stats.
    Zone { radius: f32, dur: f32 },
    /// Does not attack at all.
    Aura,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Special {
    Burn {
        dps: f32,
        dur: f32,
    },
    Slow {
        amt: f32,
        dur: f32,
    },
    Poison {
        dps: f32,
        dur: f32,
    },
    Crit {
        chance: f32,
        mult: f32,
    },
    Stun {
        chance: f32,
        dur: f32,
    },
    /// Target takes `amt` extra damage from everything for `dur`.
    Shred {
        amt: f32,
        dur: f32,
    },
    /// Damage grows while the tower holds the same target.
    Ramp {
        per_hit: f32,
        max: f32,
    },
    Knockback {
        dist: f32,
    },
    /// Drags the target back down the road. Unlike knockback this is not a
    /// shove away from the tower - it is distance *un-walked*, which is the
    /// only effect in the game that buys back time rather than health.
    Pull {
        dist: f32,
    },
    /// Multiplies damage against a target already below `below` health.
    Execute {
        below: f32,
        mult: f32,
    },
    /// Fires at this many extra targets every shot, each taking a full hit.
    ///
    /// On a circuit where a hundred and fifty monsters are on the road at once,
    /// targets-per-second matters more than damage-per-target, and this is the
    /// only effect that buys it directly.
    Multishot {
        extra: u32,
    },
    /// Chance to destroy a non-boss outright, whatever its health.
    ///
    /// A lottery rather than a damage source: worth exactly as much as the
    /// number of things walking past, which makes it the one effect that gets
    /// *better* as the ring fills up.
    Instakill {
        chance: f32,
    },
    /// Stops regeneration and Mender healing on anything it touches.
    Suppress,
    /// Buffs every tower in range. Grove only.
    Buff {
        dmg: f32,
        rate: f32,
        range: f32,
    },
    /// Pays out at the end of every wave.
    Income {
        per_wave: u32,
    },
    /// Extra gold on kill.
    Bounty {
        flat: u32,
        chance: f32,
        bonus: u32,
    },
    /// Adds to the interest rate.
    Interest {
        extra: f32,
    },
    /// Damage-over-time jumps to nearby monsters when the victim dies.
    Contagion {
        radius: f32,
    },
}

impl Special {
    /// One-line description with the numbers already scaled to a level.
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
            Special::Knockback { dist } => format!("Knocks back {dist:.1} tiles"),
            Special::Pull { dist } => format!("Drags {dist:.1} tiles back down the road"),
            Special::Execute { below, mult } => {
                format!("{:.1}x damage below {:.0}% health", mult, below * 100.0)
            }
            Special::Multishot { extra } => {
                format!(
                    "Hits {} extra target{} per shot",
                    extra,
                    if extra == 1 { "" } else { "s" }
                )
            }
            Special::Instakill { chance } => {
                format!("{:.1}% chance to kill outright", chance * 100.0)
            }
            Special::Suppress => "Stops regeneration and healing".to_string(),
            Special::Buff { dmg, rate, range } => format!(
                "Nearby towers: +{:.0}% damage, +{:.0}% rate, +{:.1} range",
                dmg * 100.0,
                rate * 100.0,
                range
            ),
            Special::Income { per_wave } => format!("+{per_wave} gold every wave"),
            Special::Bounty {
                flat,
                chance,
                bonus,
            } => {
                format!(
                    "+{} gold per kill, {:.0}% for +{}",
                    flat,
                    chance * 100.0,
                    bonus
                )
            }
            Special::Interest { extra } => format!("+{:.0}% interest each wave", extra * 100.0),
            Special::Contagion { radius } => {
                format!("Damage-over-time spreads {radius:.1} tiles on death")
            }
        }
    }
}

pub struct TowerDef {
    pub id: &'static str,
    pub name: &'static str,
    pub role: &'static str,
    pub desc: &'static str,
    /// The element, or pair of elements, that unlocks this tower.
    pub elem: (Element, Option<Element>),
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
}

impl TowerDef {
    pub fn is_dual(&self) -> bool {
        self.elem.1.is_some()
    }
    /// Both elements, in canonical order.
    pub fn elements(&self) -> impl Iterator<Item = Element> {
        [Some(self.elem.0), self.elem.1].into_iter().flatten()
    }
    /// "Nature" or "Nature + Fire".
    pub fn element_label(&self) -> String {
        match self.elem {
            (a, None) => a.name().to_string(),
            (a, Some(b)) => format!("{} + {}", a.name(), b.name()),
        }
    }
}

/// Eight levels. There are no forks - the branching decision lives in the essence
/// draft now, and forty-two fork variants layered on twenty-one towers would be
/// noise rather than choice. Two visible milestones mark the ladder instead.
pub const MAX_TIER: u32 = 8;
/// The level at which a tower's special effect sharpens hard.
pub const ATTUNE_TIER: u32 = 4;
/// The level at which damage takes a step up and the model changes shape.
pub const ASCEND_TIER: u32 = 7;

// Damage grows faster than cost every level, so upgrading always beats building
// wide - that is what makes a single pad worth pouring gold into. The gap per
// level is deliberately small (1.76 vs 1.62, about 9% better per gold) so the
// choice stays close across all eight levels rather than being decided at level
// two.
const DMG_STEP: f32 = 1.76;
const COST_STEP: f32 = 1.62;
/// Extra damage multiplier applied once a tower has ascended.
const ASCEND_BONUS: f32 = 1.32;
/// How much harder a special hits once the tower is attuned.
const ATTUNE_BONUS: f32 = 1.45;

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
    /// Stats for a level.
    pub fn stats(&self, tier: u32) -> Stats {
        let k = Self::scale(tier);
        let step = (tier - 1) as f32;
        let mut s = Stats {
            dmg: self.dmg * k,
            rate: self.rate,
            range: self.range + 0.16 * step,
            splash: if self.splash > 0.0 {
                self.splash + 0.09 * step
            } else {
                0.0
            },
            delivery: self.delivery,
            scale: k,
        };
        if tier >= ASCEND_TIER {
            s.dmg *= ASCEND_BONUS;
        }
        // A chain tower's reach per leap grows with its level, so upgrading it
        // widens the web instead of only hardening each hit.
        if let Delivery::Chain {
            bounces,
            falloff,
            hop,
        } = s.delivery
        {
            s.delivery = Delivery::Chain {
                bounces: bounces + (tier / 3),
                falloff,
                hop: hop + 0.30 * step,
            };
        }
        if let Delivery::Zone { radius, dur } = s.delivery {
            s.delivery = Delivery::Zone {
                radius: radius + 0.06 * step,
                dur,
            };
        }
        s
    }

    /// How much stronger a support, control or economy effect is at this level.
    /// These do not ride the damage curve, so they get their own - otherwise a
    /// Grove or a Tombstone stops being worth a pad by the midgame.
    pub fn utility_scale(tier: u32) -> f32 {
        let base = 1.0 + 0.55 * (tier - 1) as f32;
        if tier >= ATTUNE_TIER {
            base * ATTUNE_BONUS
        } else {
            base
        }
    }
    /// Every special active at this level. Returned by value so the combat loop
    /// never allocates.
    pub fn specials_for(&self) -> SpecialSet {
        let mut set = SpecialSet::default();
        set.extend(self.specials);
        set
    }
    pub fn dps_at(&self, tier: u32) -> f32 {
        let s = self.stats(tier);
        s.dmg * s.rate
    }

    /// Sustained damage per second from burn and poison riders.
    ///
    /// Counted at a discount, because a damage-over-time never all lands: some
    /// of it is overkill on a target that was going to die anyway, and a stack
    /// takes seconds to build. Without this term at all - which is how it was -
    /// every toxic tower reads as the worst damage in the game, in the tooltip
    /// the player is looking at as much as in any valuation, and Bramble's
    /// twelve stacks of venom are simply invisible.
    pub fn dot_dps_at(&self, tier: u32) -> f32 {
        const REALISM: f32 = 0.6;
        let k = Self::scale(tier);
        let rate = self.stats(tier).rate;
        self.specials
            .iter()
            .map(|s| match *s {
                // Burn refreshes rather than stacking, so it is worth its
                // per-second value once.
                Special::Burn { dps, .. } => dps * k,
                // Poison stacks, up to twelve applications deep.
                Special::Poison { dps, dur } => dps * k * (rate * dur).min(12.0),
                _ => 0.0,
            })
            .sum::<f32>()
            * REALISM
    }

    /// Roughly how much damage a shot lands across all targets, so chain and
    /// splash towers are not judged on their single-target number alone.
    pub fn effective_dps_at(&self, tier: u32) -> f32 {
        let s = self.stats(tier);
        let spread = match s.delivery {
            Delivery::Chain {
                bounces, falloff, ..
            } => {
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
        // Crit is a flat multiplier on everything the tower fires, so it goes
        // on the direct term. Leaving it out understated Prism and Bastion in
        // the tooltip as badly as leaving out poison understated Bramble.
        let crit = self
            .specials
            .iter()
            .find_map(|sp| match *sp {
                Special::Crit { chance, mult } => Some(1.0 + chance * (mult - 1.0)),
                _ => None,
            })
            .unwrap_or(1.0);
        s.dmg * s.rate * spread * crit + self.dot_dps_at(tier)
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
        Self {
            arr: [Special::Knockback { dist: 0.0 }; 6],
            n: 0,
        }
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

// ---------------------------------------------------------------- the roster

use Element::{Dark, Earth, Fire, Light, Nature, Water};

/// Twenty-one towers: six pure, then fifteen duals in canonical pair order.
///
/// The ordering matters - [`pure_index`] and [`dual_index`] compute positions
/// from it rather than searching, and the build panel reads it straight through.
pub static TOWERS: &[TowerDef] = &[
    // ---------------------------------------------------------- pure
    TowerDef {
        id: "bramble",
        name: "Bramble",
        role: "Stacking poison",
        desc: "Cheap, and never wrong. Toxic damage is the one thing no armour in the game reduces.",
        elem: (Nature, None),
        dtype: Damage::Toxic,
        targets: Targets::Both,
        dmg: 11.0,
        rate: 1.20,
        range: 3.4,
        splash: 0.0,
        cost: 60,
        delivery: Delivery::Shot { speed: 20.0 },
        specials: &[Special::Poison { dps: 6.0, dur: 4.0 }],
        color: [0.42, 0.80, 0.36],
    },
    TowerDef {
        id: "ember",
        name: "Ember",
        role: "Small splash",
        desc: "Fast, hot and slightly wide. Loves a crowd of unarmoured things.",
        elem: (Fire, None),
        dtype: Damage::Fire,
        targets: Targets::Both,
        dmg: 16.0,
        rate: 1.30,
        range: 3.2,
        splash: 0.55,
        cost: 65,
        delivery: Delivery::Shot { speed: 19.0 },
        specials: &[Special::Burn { dps: 6.0, dur: 2.0 }],
        color: [1.00, 0.54, 0.20],
    },
    TowerDef {
        id: "tide",
        name: "Tide",
        role: "Slow",
        desc: "Barely hurts. Buys every other tower on the board more shots, on either layer.",
        elem: (Water, None),
        dtype: Damage::Magic,
        targets: Targets::Both,
        dmg: 14.0,
        rate: 1.10,
        range: 3.5,
        splash: 0.0,
        cost: 60,
        delivery: Delivery::Shot { speed: 18.0 },
        specials: &[Special::Slow {
            amt: 0.40,
            dur: 2.0,
        }],
        color: [0.36, 0.74, 1.00],
    },
    TowerDef {
        id: "boulder",
        name: "Boulder",
        role: "Heavy ground hit",
        desc: "The hardest single hit you can buy early, and it cannot elevate. Pair it with something that can.",
        elem: (Earth, None),
        dtype: Damage::Physical,
        targets: Targets::GroundOnly,
        dmg: 44.0,
        rate: 0.50,
        range: 3.0,
        splash: 0.75,
        cost: 70,
        delivery: Delivery::Shot { speed: 12.0 },
        specials: &[Special::Knockback { dist: 0.30 }],
        color: [0.78, 0.58, 0.34],
    },
    TowerDef {
        id: "prism",
        name: "Prism",
        role: "Long range",
        desc: "Sees further than anything else this cheap, and lands the occasional enormous hit.",
        elem: (Light, None),
        dtype: Damage::Magic,
        targets: Targets::Both,
        dmg: 26.0,
        rate: 0.85,
        range: 4.4,
        splash: 0.0,
        cost: 70,
        delivery: Delivery::Shot { speed: 26.0 },
        specials: &[Special::Crit {
            chance: 0.25,
            mult: 2.0,
        }],
        color: [1.00, 0.90, 0.54],
    },
    TowerDef {
        id: "shade",
        name: "Shade",
        role: "One-strike kill",
        desc: "The weakest damage in the roster, and every shot is a lottery ticket that ignores health entirely. Worth exactly as much as the number of things walking past.",
        elem: (Dark, None),
        dtype: Damage::Toxic,
        targets: Targets::Both,
        dmg: 17.0,
        rate: 1.00,
        range: 3.3,
        splash: 0.0,
        cost: 65,
        delivery: Delivery::Shot { speed: 20.0 },
        specials: &[
            Special::Instakill { chance: 0.04 },
            Special::Bounty {
                flat: 2,
                chance: 0.15,
                bonus: 12,
            },
        ],
        color: [0.66, 0.44, 0.96],
    },
    // ---------------------------------------------------------- dual
    TowerDef {
        id: "wildfire",
        name: "Wildfire",
        role: "Burning arcs",
        desc: "Fire jumps from target to target and keeps burning after it lands. Answers a packed road.",
        elem: (Nature, Some(Fire)),
        dtype: Damage::Fire,
        targets: Targets::Both,
        dmg: 22.0,
        rate: 0.95,
        range: 3.6,
        splash: 0.0,
        cost: 175,
        delivery: Delivery::Chain {
            bounces: 3,
            falloff: 0.84,
            hop: 3.2,
        },
        specials: &[Special::Burn {
            dps: 22.0,
            dur: 3.5,
        }],
        color: [0.86, 0.66, 0.20],
    },
    TowerDef {
        id: "mire",
        name: "Mire",
        role: "Swamp",
        desc: "Floods a stretch of road. Everything in it crawls, rots, and stops healing.",
        elem: (Nature, Some(Water)),
        dtype: Damage::Toxic,
        targets: Targets::GroundOnly,
        dmg: 30.0,
        rate: 0.45,
        range: 3.4,
        splash: 0.0,
        cost: 180,
        delivery: Delivery::Zone {
            radius: 1.40,
            dur: 5.5,
        },
        specials: &[
            Special::Slow {
                amt: 0.50,
                dur: 1.2,
            },
            Special::Suppress,
        ],
        color: [0.38, 0.66, 0.56],
    },
    TowerDef {
        id: "thornwall",
        name: "Thornwall",
        role: "Roots",
        desc: "Throws a wall of thorns across the road. Wide, heavy, and it stops things dead.",
        elem: (Nature, Some(Earth)),
        dtype: Damage::Physical,
        targets: Targets::GroundOnly,
        dmg: 44.0,
        rate: 0.72,
        range: 3.2,
        splash: 1.00,
        cost: 175,
        delivery: Delivery::Shot { speed: 14.0 },
        specials: &[
            Special::Slow {
                amt: 0.45,
                dur: 2.2,
            },
            Special::Knockback { dist: 0.25 },
        ],
        color: [0.54, 0.64, 0.32],
    },
    TowerDef {
        id: "grove",
        name: "Grove",
        role: "Support aura",
        desc: "Fires nothing. Makes every tower around it substantially better, and costs you a pad to do it.",
        elem: (Nature, Some(Light)),
        dtype: Damage::None,
        targets: Targets::Nothing,
        dmg: 0.0,
        rate: 0.0,
        range: 3.0,
        splash: 0.0,
        cost: 190,
        delivery: Delivery::Aura,
        specials: &[Special::Buff {
            dmg: 0.30,
            rate: 0.22,
            range: 0.6,
        }],
        color: [0.72, 0.90, 0.46],
    },
    TowerDef {
        id: "blight",
        name: "Blight",
        role: "Ramp and spread",
        desc: "Grows the longer it holds one target, and whatever it kills infects the pack around it.",
        elem: (Nature, Some(Dark)),
        dtype: Damage::Toxic,
        targets: Targets::Both,
        dmg: 13.0,
        rate: 1.15,
        range: 3.4,
        splash: 0.0,
        cost: 185,
        delivery: Delivery::Shot { speed: 18.0 },
        specials: &[
            Special::Poison {
                dps: 24.0,
                dur: 5.0,
            },
            Special::Ramp {
                per_hit: 0.065,
                max: 2.8,
            },
            Special::Contagion { radius: 2.2 },
        ],
        color: [0.50, 0.72, 0.34],
    },
    TowerDef {
        id: "steam",
        name: "Steam",
        role: "Nova",
        desc: "Aims at nothing. Scalding pressure rolls out in every direction and catches both layers at once.",
        elem: (Fire, Some(Water)),
        dtype: Damage::Fire,
        targets: Targets::Both,
        dmg: 30.0,
        rate: 0.80,
        range: 3.1,
        splash: 0.0,
        cost: 175,
        delivery: Delivery::Nova,
        specials: &[Special::Slow {
            amt: 0.25,
            dur: 1.4,
        }],
        color: [0.78, 0.86, 0.92],
    },
    TowerDef {
        id: "magma",
        name: "Magma",
        role: "Burning ground",
        desc: "Sets the road itself alight. The damage is the smaller half - the shred is why you build it.",
        elem: (Fire, Some(Earth)),
        dtype: Damage::Fire,
        targets: Targets::GroundOnly,
        dmg: 34.0,
        rate: 0.48,
        range: 3.2,
        splash: 0.0,
        cost: 185,
        delivery: Delivery::Zone {
            radius: 1.20,
            dur: 4.5,
        },
        specials: &[Special::Shred {
            amt: 0.30,
            dur: 1.0,
        }],
        color: [0.94, 0.42, 0.18],
    },
    TowerDef {
        id: "solar",
        name: "Solar",
        role: "Piercing lance",
        desc: "A beam of daylight down the whole lane. Everything standing in the line takes it.",
        elem: (Fire, Some(Light)),
        dtype: Damage::Fire,
        targets: Targets::Both,
        dmg: 52.0,
        rate: 0.62,
        range: 4.6,
        splash: 0.0,
        cost: 205,
        delivery: Delivery::Lance { speed: 34.0 },
        specials: &[Special::Burn {
            dps: 26.0,
            dur: 3.0,
        }],
        color: [1.00, 0.80, 0.30],
    },
    TowerDef {
        id: "hellfire",
        name: "Hellfire",
        role: "Execute",
        desc: "Mediocre against a full health bar. Annihilates anything already hurt - the finisher for a board that chips.",
        elem: (Fire, Some(Dark)),
        dtype: Damage::Fire,
        targets: Targets::Both,
        dmg: 34.0,
        rate: 0.78,
        range: 3.5,
        splash: 0.55,
        cost: 195,
        delivery: Delivery::Shot { speed: 21.0 },
        specials: &[Special::Execute {
            below: 0.35,
            mult: 2.6,
        }],
        color: [0.90, 0.28, 0.32],
    },
    TowerDef {
        id: "silt",
        name: "Silt",
        role: "Wide splash",
        desc: "The widest blast on the board, and it leaves everything it hits slower and softer.",
        elem: (Water, Some(Earth)),
        dtype: Damage::Physical,
        targets: Targets::GroundOnly,
        dmg: 54.0,
        rate: 0.58,
        range: 3.6,
        splash: 1.55,
        cost: 190,
        delivery: Delivery::Shot { speed: 13.0 },
        specials: &[
            Special::Slow {
                amt: 0.30,
                dur: 1.8,
            },
            Special::Shred {
                amt: 0.20,
                dur: 2.5,
            },
        ],
        color: [0.60, 0.68, 0.62],
    },
    TowerDef {
        id: "mirror",
        name: "Mirror",
        role: "Long chain",
        desc: "Refracts from target to target and barely loses anything on the way. Clears a road on its own.",
        elem: (Water, Some(Light)),
        dtype: Damage::Magic,
        targets: Targets::Both,
        dmg: 24.0,
        rate: 0.95,
        range: 3.8,
        splash: 0.0,
        cost: 200,
        delivery: Delivery::Chain {
            bounces: 4,
            falloff: 0.82,
            hop: 4.2,
        },
        specials: &[],
        color: [0.66, 0.90, 1.00],
    },
    TowerDef {
        id: "abyss",
        name: "Abyss",
        role: "Pull",
        desc: "Drags them back down the road they just walked. The only tower that buys time instead of health.",
        elem: (Water, Some(Dark)),
        dtype: Damage::Magic,
        targets: Targets::Both,
        dmg: 27.0,
        rate: 0.80,
        range: 3.5,
        splash: 0.45,
        cost: 195,
        delivery: Delivery::Shot { speed: 16.0 },
        specials: &[
            Special::Pull { dist: 0.85 },
            Special::Slow {
                amt: 0.20,
                dur: 1.5,
            },
        ],
        color: [0.44, 0.42, 0.86],
    },
    TowerDef {
        id: "bastion",
        name: "Bastion",
        role: "Multishot",
        desc: "Fires on three things at once, hard, and it can elevate. Expensive for exactly that reason.",
        elem: (Earth, Some(Light)),
        dtype: Damage::Physical,
        targets: Targets::Both,
        dmg: 78.0,
        rate: 0.55,
        range: 4.0,
        splash: 0.0,
        cost: 215,
        delivery: Delivery::Beam { pierce: 1 },
        specials: &[
            Special::Multishot { extra: 2 },
            Special::Crit {
                chance: 0.20,
                mult: 2.2,
            },
        ],
        color: [0.88, 0.82, 0.58],
    },
    TowerDef {
        id: "tombstone",
        name: "Tombstone",
        role: "Economy",
        // It fires nothing at all. A Tombstone that also shot things would just
        // be a worse Bramble with a bonus, and the decision to build one would
        // be free - the whole point is that it costs a pad during the waves you
        // are most fragile.
        desc: "Fires nothing. Pays every wave, forever, and lifts the interest on what you are holding. Build it early or not at all.",
        elem: (Earth, Some(Dark)),
        dtype: Damage::None,
        targets: Targets::Nothing,
        dmg: 0.0,
        rate: 0.0,
        range: 2.9,
        splash: 0.0,
        cost: 190,
        delivery: Delivery::Aura,
        specials: &[
            Special::Income { per_wave: 34 },
            Special::Interest { extra: 0.015 },
        ],
        color: [0.72, 0.66, 0.78],
    },
    TowerDef {
        id: "eclipse",
        name: "Eclipse",
        role: "Boss answer",
        desc: "Stops things dead and leaves them defenceless. Thin damage of its own - it is what the rest of your board hits through.",
        elem: (Light, Some(Dark)),
        dtype: Damage::Magic,
        targets: Targets::Both,
        dmg: 21.0,
        rate: 0.90,
        range: 3.7,
        splash: 0.0,
        cost: 210,
        delivery: Delivery::Shot { speed: 22.0 },
        specials: &[
            Special::Stun {
                chance: 0.22,
                dur: 0.9,
            },
            Special::Shred {
                amt: 0.45,
                dur: 3.5,
            },
        ],
        color: [0.86, 0.62, 1.00],
    },
];

/// Index of the pure tower for an element.
pub fn pure_index(e: Element) -> usize {
    e.idx()
}

/// Index of the dual tower for a pair, in either order.
pub fn dual_index(a: Element, b: Element) -> Option<usize> {
    if a == b {
        return None;
    }
    let (lo, hi) = if a < b { (a, b) } else { (b, a) };
    TOWERS.iter().position(|t| t.elem == (lo, Some(hi)))
}

pub fn tower_index(id: &str) -> usize {
    TOWERS.iter().position(|t| t.id == id).unwrap_or(0)
}

/// Build-panel order: the six pures first, then the duals, cheapest first
/// inside each group. Availability is decided by essences, not by this.
pub fn shop_order() -> Vec<usize> {
    let mut idx: Vec<usize> = (0..TOWERS.len()).collect();
    idx.sort_by_key(|&i| (TOWERS[i].is_dual(), TOWERS[i].cost));
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
    /// Ground, half-there. Physical weapons pass straight through it.
    Wraith,
    Boss,
    /// Air. Fast, fragile, arrives in a cloud. Punishes having no anti-air.
    Wisp,
    /// Air, heavily plated. Punishes anti-air that is all physical damage.
    Drake,
    /// Air and Ethereal. Punishes it much harder.
    Seraph,
    /// Air boss. If your whole answer to bosses was a wall of Boulders, this is
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
            Kind::Wraith => "Wraith",
            Kind::Boss => "BOSS",
            Kind::Wisp => "Wisp",
            Kind::Drake => "Drake",
            Kind::Seraph => "Seraph",
            Kind::Skylord => "SKYLORD",
        }
    }

    /// Which layer this monster travels on.
    pub fn layer(self) -> Layer {
        match self {
            Kind::Wisp | Kind::Drake | Kind::Seraph | Kind::Skylord => Layer::Air,
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
            Kind::Seraph => 1.90,
            Kind::Skylord => 2.10,
            _ => 0.0,
        }
    }
    pub fn is_boss(self) -> bool {
        matches!(self, Kind::Boss | Kind::Skylord)
    }
    pub fn armor(self) -> Armor {
        match self {
            Kind::Brute | Kind::Bulwark | Kind::Drake => Armor::Plated,
            Kind::Warden | Kind::Phaser => Armor::Warded,
            Kind::Wraith | Kind::Seraph => Armor::Ethereal,
            Kind::Boss | Kind::Skylord => Armor::Boss,
            _ => Armor::Unarmoured,
        }
    }
    pub fn radius(self) -> f32 {
        match self {
            Kind::Swarm => 0.20,
            Kind::Wisp => 0.22,
            Kind::Runner => 0.24,
            Kind::Wraith | Kind::Seraph => 0.28,
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
            Kind::Brute => "heavy plate",
            Kind::Swarm => "huge pack",
            Kind::Warden => "warded",
            Kind::Mender => "heals nearby",
            Kind::Bulwark => "damage shield",
            Kind::Phaser => "shrugs off slows",
            Kind::Wraith => "ETHEREAL, needs magic",
            Kind::Boss => "immune to stun",
            Kind::Wisp => "FLYING, fast swarm",
            Kind::Drake => "FLYING, heavy plate",
            Kind::Seraph => "FLYING, ETHEREAL",
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
    /// Absorbs this much damage before its health is touched.
    pub shield: f32,
    /// Heals nearby monsters this fraction of their max health per second.
    pub heal: f32,
    /// Ignores slows for half of every second.
    pub phasing: bool,
    pub regen: bool,
    pub split: bool,
    /// A second monster type arriving alongside the first.
    ///
    /// One type per wave means one counter always answers it, and the roster
    /// stops mattering by the midgame. Escorts are how a wave asks two
    /// questions at once - and from wave 45 the escort is always on the *other*
    /// layer, so a board that only answers the road cannot coast on a lucky
    /// run of ground waves.
    pub escort: Option<Escort>,
}

#[derive(Clone, Copy, Debug)]
pub struct Escort {
    pub kind: Kind,
    pub count: u32,
    pub hp: f32,
}

impl WaveDef {
    /// Everything that will arrive, main body first.
    pub fn parts(&self) -> impl Iterator<Item = (Kind, u32, f32)> + '_ {
        std::iter::once((self.kind, self.count, self.hp))
            .chain(self.escort.map(|e| (e.kind, e.count, e.hp)))
    }
    /// Whether anything in this wave flies.
    pub fn has_air(&self) -> bool {
        self.parts().any(|(k, _, _)| k.flying())
    }
    /// Whether anything in this wave walks.
    pub fn has_ground(&self) -> bool {
        self.parts().any(|(k, _, _)| !k.flying())
    }
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

/// Health of a wave-1 monster. Sets how tight the opening is.
const HP_BASE: f32 = 70.0;
/// Gold a wave-1 wave pays out in total.
const GOLD_BASE: f32 = 58.0;
/// Per-wave growth of the gold a wave pays out. This one really is a geometric
/// curve, because it is the only thing in the game that compounds cleanly.
const GOLD_STEP: f32 = 1.0700;

/// Every gold piece the player could have had by the end of wave `w`.
///
/// The opening purse is part of this and matters enormously: by wave ten the
/// campaign has paid out about eight hundred gold, and starting with two
/// hundred and sixty of it changes the board's growth over those ten waves from
/// fourteenfold to threefold. Leaving it out made the first fifteen waves
/// unwinnable.
fn purse_through(w: u32) -> f32 {
    let n = w.max(1) as i32;
    crate::game::START_GOLD as f32 + GOLD_BASE * (GOLD_STEP.powi(n) - 1.0) / (GOLD_STEP - 1.0)
}

/// How fast a board can grow, as a power of the gold poured into it.
///
/// A level costs `COST_STEP` times the last and deals `DMG_STEP` times the
/// damage, so a board's output is a super-linear function of its total spend:
/// `ln(DMG_STEP) / ln(COST_STEP)` = 1.17. Anything the player can do is bounded
/// by this, which makes it the right yardstick to measure a wave against.
const BOARD_EXP: f32 = 1.1718;

/// How much of that growth the waves actually claim.
///
/// **This is the difficulty knob, and its shape matters more than its value.**
///
/// Health used to be a plain geometric curve, and a plain geometric curve is
/// the wrong shape. Gold arrives as a *sum* of wave purses, so the player's
/// board grows very fast in the first twenty waves - from nothing to a full
/// road - and then far more slowly, because after that the only thing left to
/// buy is levels. A geometric health curve is flatter than that early and
/// steeper late, so it produced a campaign that was measurably free: a bot
/// reached wave 64 of 80 without losing a single life, and then died twice in
/// the last fifteen. Sixty-four waves of no pressure is not a difficulty curve,
/// it is a loading screen.
///
/// Pinning health to the *cumulative purse* instead makes the wave grow at the
/// same shape as the board that has to answer it, so the pressure is roughly
/// even from wave 5 to wave 80. The exponent below one is what leaves the
/// player a margin to actually play well inside.
const HP_EXP: f32 = 1.36;

/// Health multiplier for a campaign wave.
fn campaign_hp(w: u32) -> f32 {
    HP_BASE * (purse_through(w.min(CAMPAIGN_WAVES)) / purse_through(1)).powf(HP_EXP * BOARD_EXP)
}

/// The road is roughly 60 tiles long; this sets how briskly monsters cover it.
pub const WALK_SPEED: f32 = 1.7;

/// How many more monsters a wave holds than it used to, and how much less each
/// one is worth.
///
/// Both, by exactly the same factor - so a wave's *total* health is unchanged
/// and the tuned difficulty curve carries over intact, while the shape of a
/// wave changes completely: eight times as many monsters, each an eighth as
/// tough, arriving as a steady stream instead of a burst.
///
/// That is the whole point. A dozen fat monsters is a game about single-target
/// damage; a hundred thin ones is a game about area, throughput and coverage -
/// and with no exit to leak from, the second is the game worth having. Green
/// Circle TD runs waves of sixty to a hundred and sixty for the same reason.
pub const COUNT_SCALE: f32 = 8.0;

/// How many monsters a boss wave brings.
///
/// Not one. On a circuit a single unkillable boss is not a threat, it is a
/// nuisance that circles forever occupying one slot of a three-hundred-slot
/// gauge. A handful is a genuine damage sink: every tower pointed at the boss
/// pack is a tower not killing the stream arriving behind it.
pub const BOSS_COUNT: u32 = 5;

/// Lives you start with. One number, because there is one difficulty.
///
/// Three difficulty settings meant three curves, and only one of them was ever
/// tuned properly. A single curve that is actually good is worth more than a
/// menu of curves that are roughly right.
pub const START_LIVES: i32 = 20;

/// Which monster shows up on which wave. New types arrive on a schedule so the
/// player always has one wave of warning to buy the counter.
fn kind_for(wave: u32) -> Kind {
    // Bosses alternate layers, so a board built entirely out of Boulders meets
    // something it cannot touch every twenty waves.
    if wave % 10 == 0 {
        return if (wave / 10) % 2 == 0 {
            Kind::Skylord
        } else {
            Kind::Boss
        };
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
    // Act III opens with the armour class that answers nothing the player has
    // been leaning on: a board of Boulders and Silts has no reply at all.
    if wave >= 41 {
        pool.push(Kind::Wraith);
    }
    if wave >= 48 {
        pool.push(Kind::Seraph);
    }
    pool[(wave as usize * 7 + wave as usize / 3) % pool.len()]
}

/// How many of a type arrive, how tough each is, and how fast they move.
fn shape_of(kind: Kind) -> (u32, f32, f32) {
    match kind {
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
        Kind::Wraith => (11, 1.30, 1.20),
        Kind::Seraph => (9, 1.35, 1.25),
        Kind::Grunt => (14, 1.0, 1.15),
    }
}

/// Walking speed for a type, in the same units as [`WaveDef::speed`].
pub fn kind_speed(kind: Kind) -> f32 {
    shape_of(kind).2 * WALK_SPEED
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

/// Builds any wave, at any number. Waves past the campaign keep escalating, so
/// a run is only over when the player runs out of lives.
pub fn wave_at(i: u32) -> WaveDef {
    let i = i.max(1);
    let kind = kind_for(i);
    let debut = is_debut(i, kind);
    let (count, hp_mul, speed) = shape_of(kind);

    // Campaign curve, then an unbounded endless curve on top.
    //
    // Health and gold both grow geometrically, and health grows faster. That
    // gap is the difficulty: early on your gold outruns the monsters and the
    // board fills up, late on it does not and you have to choose what to feed.
    let campaign = i.min(CAMPAIGN_WAVES);
    let over = i.saturating_sub(CAMPAIGN_WAVES);
    // Same total health per wave, spread over `COUNT_SCALE` times as many
    // bodies. Bosses are exempt: a boss split into fifty pieces is a swarm.
    let boss = kind.is_boss();
    let spread = if boss { BOSS_COUNT as f32 } else { COUNT_SCALE };
    let base = campaign_hp(campaign) / spread;
    let count = if boss {
        BOSS_COUNT
    } else {
        ((count as f32 * COUNT_SCALE).round() as u32).max(1)
    };
    // A debut wave is half the size and two thirds the health: enough to hurt,
    // not enough to end a run that had no answer ready.
    let (count, debut_hp) = if debut {
        ((count as f32 * 0.55).ceil() as u32, 0.65)
    } else {
        (count, 1.0)
    };
    let hp = base * ENDLESS_HP_STEP.powi(over as i32) * hp_mul * debut_hp;

    let purse =
        GOLD_BASE * GOLD_STEP.powi(campaign as i32 - 1) * ENDLESS_GOLD_STEP.powi(over as i32);
    // The escort is decided before the bounty, because a wave's purse is fixed:
    // adding an escort splits the same money across more monsters rather than
    // paying extra for them. Getting this wrong made escorted waves *easier* -
    // more targets, more gold, a bigger board.
    let escort = escort_for(i, kind, base);
    let paying = count + escort.map_or(0, |e| e.count);
    // Clamped: an f32 above u32::MAX saturates rather than wrapping, and the
    // result is a wave that pays exactly 4,294,967,295 gold. Deep endless is
    // meant to end because the monsters win, not because the arithmetic gives
    // up.
    let bounty = (purse * KILL_SHARE / paying as f32).clamp(1.0, MAX_PAYOUT) as u32;

    WaveDef {
        kind,
        count,
        hp,
        speed: speed * WALK_SPEED,
        bounty,
        // Bulwarks absorb a flat pool that scales with the wave.
        // A Bulwark's shield rides the same curve its health does, at a fixed
        // fraction of it - so a flat shield stays a meaningful wall late
        // instead of becoming a rounding error on the health bar.
        shield: if kind == Kind::Bulwark {
            hp * 0.45
        } else {
            0.0
        },
        heal: if kind == Kind::Mender { 0.030 } else { 0.0 },
        phasing: kind == Kind::Phaser,
        regen: i >= 20 && i % 8 == 7,
        split: i >= 22 && i % 9 == 2 && kind != Kind::Boss,
        escort,
    }
}

/// What arrives alongside the main body of a wave.
///
/// Escorts start at wave 25 as an occasional second type, and from wave 45
/// every escorted wave crosses the layers: a ground wave brings flyers, a
/// flying wave brings walkers. That is what stops a board from being a pile of
/// Boulders that happens to survive.
fn escort_for(wave: u32, main: Kind, base: f32) -> Option<Escort> {
    if wave < 25 {
        return None;
    }
    // Bosses arrive with a guard from wave 50 - a boss alone is a single
    // target, which the whole board is already pointed at.
    let boss = main.is_boss();
    if boss && wave < 50 {
        return None;
    }
    // Not every wave, or an escort stops being an event. Act IV is the
    // exception: from wave 61 every wave brings one, which is what makes the
    // last twenty a different game rather than the same one with bigger numbers.
    if !boss && wave < 61 && wave % 3 != 1 {
        return None;
    }

    let cross = wave >= 45 || boss;
    let candidates: &[Kind] = if cross && !main.flying() {
        if wave >= 48 {
            &[Kind::Wisp, Kind::Drake, Kind::Seraph]
        } else {
            &[Kind::Wisp, Kind::Drake]
        }
    } else if cross {
        &[Kind::Swarm, Kind::Brute, Kind::Runner, Kind::Wraith]
    } else {
        &[Kind::Runner, Kind::Swarm, Kind::Warden, Kind::Brute]
    };
    let kind = candidates[(wave as usize / 3) % candidates.len()];
    if kind == main {
        return None;
    }

    let (base_count, hp_mul, _) = shape_of(kind);
    // An escort is a real threat, not a garnish - about half a wave of it.
    let count = ((base_count as f32) * if boss { 0.45 } else { 0.55 }).ceil() as u32;
    Some(Escort {
        kind,
        count: count.max(2),
        hp: base * hp_mul * 0.85,
    })
}

pub fn build_waves() -> Vec<WaveDef> {
    (1..=N_WAVES).map(wave_at).collect()
}

/// How much of a wave's purse is paid per kill rather than for surviving it.
///
/// Paying the whole purse on kills sounds right and plays badly: a wave you
/// only half-clear pays half, so the board you build next wave is weaker, so
/// you clear even less. One bad wave used to spiral into a dead run, which made
/// the balance bimodal - the same curve either cruised to victory with 18 lives
/// or collapsed at wave 60, with almost nothing in between.
///
/// Splitting the purse fixes the shape. Falling behind still costs lives, which
/// is the real currency, but it no longer quietly destroys the economy needed to
/// recover. Killing things is still clearly better than not.
const KILL_SHARE: f32 = 0.55;

/// Paid for reaching the end of a wave, whatever leaked. The other side of
/// [`KILL_SHARE`].
pub fn wave_clear_bonus(wave: u32) -> u32 {
    let campaign = wave.min(CAMPAIGN_WAVES);
    let over = wave.saturating_sub(CAMPAIGN_WAVES);
    let purse = GOLD_BASE * GOLD_STEP.powi(campaign as i32 - 1);
    let flat = (25 + campaign * 5) as f32;
    ((purse * (1.0 - KILL_SHARE) + flat) * ENDLESS_GOLD_STEP.powi(over as i32)).min(MAX_PAYOUT)
        as u32
}

/// Ceiling on any single gold payout. See the note in [`wave_at`].
const MAX_PAYOUT: f32 = 100_000_000.0;

/// Gold per second remaining when a wave is called early.
pub const EARLY_BONUS_PER_SEC: f32 = 2.0;
