//! Static game data: the Green Circle TD roster and wave table.
//!
//! Almost nothing here is invented. The towers, their ladders, their gold
//! costs, damage, cooldowns, ranges, attack types and abilities are lifted from
//! `GREEN TD 9.3c PEIN.w3x` and live in the generated [`super::greentd`]
//! module; this file is the layer that lets the rest of the game read them.
//! See `tools/README.md` for how they were got out.
//!
//! The game that data describes:
//!
//!   - **Eleven towers can be bought.** The cheapest is a ten gold Single shot
//!     Tower, and it is a *seed*: it specialises into one of six families that
//!     cannot be bought any other way.
//!   - **Every family is a ladder.** You do not pick a tier, you buy the next
//!     rung. Poison has fifteen, Siege twenty, Aura three.
//!   - **Armour is a number**, not a category, and it climbs to 700 - which on
//!     Warcraft III's curve is 97% reduction. That is why the ladders end in
//!     six figures of damage.
//!   - **Every fifth wave is Divine**, and Divine takes 5% from Normal, Siege
//!     and Magic. Only Chaos and Spells hurt it. Troll, Chaos, Destruction and
//!     Demon exist for those waves and for nothing else.

pub use super::greentd::{LEVELS, WAVES};
pub use super::greentd_types::*;

/// Every rung of every ladder, in one flat table. A tower is an index into it.
pub static TOWERS: &[TowerLevel] = LEVELS;

/// Index of the first rung of a family.
pub fn family_start(f: Family) -> Option<usize> {
    TOWERS.iter().position(|t| t.family == f && t.step == 0)
}

/// The rung above this one, if the ladder goes any higher.
///
/// A Single shot Tower is the exception: its ladder has one rung, and what
/// comes next is a *choice* of six families rather than one next level. See
/// [`specialisations`].
pub fn next_level(i: usize) -> Option<usize> {
    let cur = TOWERS.get(i)?;
    if cur.family == Family::Single {
        return None;
    }
    TOWERS
        .iter()
        .position(|t| t.family == cur.family && t.step == cur.step + 1)
}

/// What a Single shot Tower may become, as indices into [`TOWERS`].
pub fn specialisations() -> Vec<usize> {
    SPECIALISATIONS
        .iter()
        .filter_map(|&f| family_start(f))
        .collect()
}

/// The shop: the eleven towers that can be bought outright, cheapest first.
pub fn shop_order() -> Vec<usize> {
    let mut v: Vec<usize> = TOWERS
        .iter()
        .enumerate()
        .filter(|(_, t)| t.step == 0 && t.family.buildable())
        .map(|(i, _)| i)
        .collect();
    v.sort_by_key(|&i| TOWERS[i].gold);
    v
}

pub fn tower_index(name: &str) -> usize {
    TOWERS.iter().position(|t| t.name == name).unwrap_or(0)
}

/// How many rungs a family has.
pub fn ladder_len(f: Family) -> u32 {
    TOWERS.iter().filter(|t| t.family == f).count() as u32
}

pub fn tower_color(t: &TowerLevel) -> [f32; 3] {
    t.color()
}

// ---------------------------------------------------------------- monsters

/// Which model a wave's monsters wear.
///
/// The map names all thirty-six waves separately - Troll, Salamander, Centaur,
/// Mathog, Bronze Dragon - but they are Warcraft III units and this game has
/// its own dozen models. Waves are mapped onto those by what they *are*: what
/// flies gets a flier, what is Divine gets the boss silhouette, and the rest
/// cycle so consecutive waves never look the same.
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
    Wraith,
    Boss,
    Wisp,
    Drake,
    Seraph,
    Skylord,
}

impl Kind {
    pub fn flying(self) -> bool {
        matches!(self, Kind::Wisp | Kind::Drake | Kind::Seraph | Kind::Skylord)
    }
    pub fn is_boss(self) -> bool {
        matches!(self, Kind::Boss | Kind::Skylord)
    }
    pub fn altitude(self) -> f32 {
        match self {
            Kind::Wisp => 1.55,
            Kind::Drake => 1.75,
            Kind::Seraph => 1.90,
            Kind::Skylord => 2.10,
            _ => 0.0,
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
}

/// Picks a model for a wave from what the wave actually is.
fn kind_for(row: &WaveRow) -> Kind {
    if row.flying {
        return match row.armour_type {
            ArmourType::Divine => Kind::Skylord,
            ArmourType::Hero => Kind::Seraph,
            _ if row.count <= 40 => Kind::Drake,
            _ => Kind::Wisp,
        };
    }
    match row.armour_type {
        // Divine waves are the ones the whole counter table exists for, so they
        // get a silhouette that stops the player mid-sentence.
        ArmourType::Divine => Kind::Boss,
        ArmourType::Hero => Kind::Wraith,
        ArmourType::Medium => Kind::Bulwark,
        _ => match row.wave % 6 {
            0 => Kind::Grunt,
            1 => Kind::Runner,
            2 => Kind::Swarm,
            3 => Kind::Brute,
            4 => Kind::Warden,
            _ => Kind::Phaser,
        },
    }
}

// ---------------------------------------------------------------- waves

/// A wave, as the game runs it.
#[derive(Clone, Copy)]
pub struct WaveDef {
    /// The map's own name for the creep: Troll, Salamander, Bronze Dragon.
    pub name: &'static str,
    pub kind: Kind,
    pub count: u32,
    pub hp: f32,
    pub armour: i32,
    pub armour_type: ArmourType,
    /// Tiles per second.
    pub speed: f32,
    pub flying: bool,
}

impl WaveDef {
    pub fn has_air(&self) -> bool {
        self.flying
    }
    pub fn has_ground(&self) -> bool {
        !self.flying
    }
    /// The one line a player needs before it arrives.
    pub fn tell(&self) -> &'static str {
        self.armour_type.counter()
    }
    /// Everything about it worth putting in the preview.
    pub fn modifiers(&self) -> Vec<&'static str> {
        let mut v = vec![self.armour_type.name()];
        if self.flying {
            v.push("FLYING");
        }
        v
    }
}

/// Warcraft III movement speed to tiles per second: 128 units to a tile.
fn walk(speed: f32) -> f32 {
    speed / 128.0
}

pub const CAMPAIGN_WAVES: u32 = 36;
/// Kept under the old name so the HUD and tests read naturally.
pub const N_WAVES: u32 = CAMPAIGN_WAVES;

/// Past the last authored wave the run continues, growing geometrically.
pub const ENDLESS_HP_STEP: f32 = 1.25;
pub const ENDLESS_GOLD_STEP: f32 = 1.12;

/// Any wave, at any number. Waves past the campaign keep escalating.
pub fn wave_at(i: u32) -> WaveDef {
    let i = i.max(1);
    let idx = ((i - 1) as usize).min(WAVES.len() - 1);
    let row = &WAVES[idx];
    let over = i.saturating_sub(CAMPAIGN_WAVES);
    WaveDef {
        name: row.name,
        kind: kind_for(row),
        // Wave 7 in the map spawns nothing at all, which is a quirk of its
        // trigger rather than a design. One monster keeps the schedule honest.
        count: row.count.max(1),
        hp: row.hp * ENDLESS_HP_STEP.powi(over as i32),
        armour: row.armour + over as i32 * 15,
        armour_type: row.armour_type,
        speed: walk(row.speed),
        flying: row.flying,
    }
}

pub fn build_waves() -> Vec<WaveDef> {
    (1..=N_WAVES).map(wave_at).collect()
}

// ---------------------------------------------------------------- economy

/// Gold a single kill pays.
///
/// The tower ladder spans ten gold to thirty-six thousand, so the purse has to
/// span the same range. Kills are most of it, which is what makes falling
/// behind on the ring hurt twice.
pub fn bounty_for(wave: u32) -> u32 {
    let w = wave.max(1) as f32;
    (1.0 + w * w * 0.42).round() as u32
}

/// Paid at the start of every wave, whatever happened in the last one.
///
/// A board that has fallen behind still needs the money to climb back out, or
/// one bad wave quietly decides the whole run.
pub fn wave_clear_bonus(wave: u32) -> u32 {
    let w = wave.min(CAMPAIGN_WAVES).max(1) as f32;
    let over = wave.saturating_sub(CAMPAIGN_WAVES);
    ((40.0 + w * w * 4.0) * ENDLESS_GOLD_STEP.powi(over as i32)).min(MAX_PAYOUT) as u32
}

const MAX_PAYOUT: f32 = 100_000_000.0;

/// Gold per second remaining when a wave is called early.
pub const EARLY_BONUS_PER_SEC: f32 = 4.0;
