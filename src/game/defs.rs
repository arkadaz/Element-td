//! Static game data: the Green Circle TD roster, upgrade graph and wave table.
//!
//! Almost nothing here is invented. The towers, their upgrade paths, their gold
//! costs, refunds, damage, cooldowns, ranges, attack types, abilities and
//! models are lifted from `GREEN TD 9.3c PEIN.w3x` and live in the generated
//! [`super::greentd`] module; this file is the layer the rest of the game reads
//! them through. See `tools/README.md` for how they were got out.
//!
//! The game that data describes:
//!
//!   - **Eleven towers can be bought.** The cheapest is a ten gold Single shot
//!     Tower, and it is a *seed*: it becomes one of six families that cannot be
//!     bought at any price.
//!   - **Upgrading is a graph, not a ladder.** Most towers have one next step,
//!     but the Aura Tower branches into Damage or Speed at every rung and the
//!     King Tower opens four hundred-thousand gold Super towers.
//!   - **Armour is a number** that climbs to 700, which on Warcraft III's curve
//!     is 98% reduction. That is why the roster ends in six figures of damage.
//!   - **Every fifth wave is Immune** and takes five percent from everything
//!     except Chaos and Hero damage. Chaos, Destruction, Troll and the
//!     One-Strike Kill Tower exist for those waves.

pub use super::greentd::{LEVELS, WAVES};
pub use super::greentd_types::*;

/// Every tower in the map, in one flat table. A tower is an index into it.
pub static TOWERS: &[TowerLevel] = LEVELS;

/// Index of the first rung of a family.
#[allow(dead_code)]
pub fn family_start(f: Family) -> Option<usize> {
    TOWERS.iter().position(|t| t.family == f && t.step == 0)
}

/// What this tower can become, and what each costs.
pub fn upgrades_of(i: usize) -> Vec<(usize, u32)> {
    match TOWERS.get(i) {
        Some(t) => t
            .upgrades
            .iter()
            .map(|&u| (u as usize, TOWERS[u as usize].gold))
            .collect(),
        None => Vec::new(),
    }
}

/// The one next step, when there is exactly one. `None` at the top of a path
/// and at every fork.
pub fn next_level(i: usize) -> Option<usize> {
    let t = TOWERS.get(i)?;
    match t.upgrades {
        [one] => Some(*one as usize),
        _ => None,
    }
}

/// The shop: the towers that can be bought outright, cheapest first.
pub fn shop_order() -> Vec<usize> {
    let mut v: Vec<usize> = TOWERS
        .iter()
        .enumerate()
        .filter(|(_, t)| t.shop)
        .map(|(i, _)| i)
        .collect();
    v.sort_by_key(|&i| TOWERS[i].gold);
    v
}

/// How many rungs a family has.
pub fn ladder_len(f: Family) -> u32 {
    TOWERS.iter().filter(|t| t.family == f).count() as u32
}

/// Map any statistical rung onto the same four visual milestones used by the
/// battlefield renderer and the icon atlas.
pub fn tower_visual_stage(def: &TowerLevel) -> usize {
    use Family::*;
    let n = ladder_len(def.family).max(2) - 1;
    let progress = (def.step as f32 / n as f32).clamp(0.0, 1.0);
    let climbed = if progress < 0.20 {
        0
    } else if progress < 0.50 {
        1
    } else if progress < 0.80 {
        2
    } else {
        3
    };
    let floor = match def.family {
        Damage | Speed | Slow | Poison | Critical | Troll => 1,
        Frost | Fire => 2,
        SuperChaos | SuperDestruct | SuperMulti | SuperBounce | OneStrike => 3,
        _ => 0,
    };
    climbed.max(floor).min(3)
}

pub fn tower_color(t: &TowerLevel) -> [f32; 3] {
    t.color()
}

// ---------------------------------------------------------------- waves

/// A wave, as the game runs it.
#[derive(Clone, Copy)]
pub struct WaveDef {
    /// The map's own name for the creep: Troll, Salamander, Bronze Dragon.
    pub name: &'static str,
    /// The map's own banner: "Air", "Immune", "Hero", "Boss", or nothing.
    pub tag: &'static str,
    pub model: Model,
    pub scale: f32,
    pub count: u32,
    pub hp: f32,
    pub armour: i32,
    pub armour_type: ArmourType,
    /// Tiles per second.
    pub speed: f32,
    pub flying: bool,
    /// Seconds between one creep and the next.
    pub spawn_gap: f32,
    /// Seconds the map waits before this wave starts.
    pub lead_in: f32,
}

impl WaveDef {
    /// Health scaled by armour, ignoring the Immune multiplier - the toughness
    /// the player is being paid for rather than the toughness of having brought
    /// the wrong attack type.
    fn payable_hp(&self) -> f32 {
        self.hp * (1.0 + 0.06 * self.armour.max(0) as f32)
    }
}

/// Warcraft III movement speed to tiles per second: 128 units to a tile.
fn walk(speed: f32) -> f32 {
    speed / 128.0
}

/// The whole wave spawns over this many seconds, whatever its count - the map
/// divides 45 by the number of creeps to get the gap between them.
pub const WAVE_SPAWN_WINDOW: f32 = 45.0;

pub const CAMPAIGN_WAVES: u32 = 36;
/// Kept under the old name so the HUD and tests read naturally.
pub const N_WAVES: u32 = CAMPAIGN_WAVES;

/// Past the last authored wave the run continues, growing geometrically.
pub const ENDLESS_HP_STEP: f32 = 1.35;

/// Any wave, at any number. Waves past the campaign keep escalating.
pub fn wave_at(i: u32) -> WaveDef {
    let i = i.max(1);
    let idx = ((i - 1) as usize).min(WAVES.len() - 1);
    let row = &WAVES[idx];
    let over = i.saturating_sub(CAMPAIGN_WAVES);
    let count = row.count.max(1);
    WaveDef {
        name: row.name,
        tag: row.tag,
        model: row.model,
        scale: row.scale,
        count,
        hp: row.hp * ENDLESS_HP_STEP.powi(over as i32),
        armour: row.armour + over as i32 * 20,
        armour_type: row.armour_type,
        speed: walk(row.speed),
        flying: row.flying,
        spawn_gap: WAVE_SPAWN_WINDOW / count as f32,
        lead_in: row.gap,
    }
}

// ---------------------------------------------------------------- economy

/// What the player starts with, exactly as the map hands it out.
pub const START_GOLD: i64 = 1_000;

/// How many creeps may be alive on the ring before the run is over. The map's
/// own leaderboard says it: "When enemies > 700, game over."
pub const FLOOD_LIMIT: usize = 700;

/// Gold a single kill pays.
///
/// This is the one number the map does not contain. Warcraft III's bounties
/// live in the game's gameplay constants rather than in the map file, and the
/// map leaves them at their defaults - which are footman-sized, and meaningless
/// against a tower that costs a hundred thousand. So a kill pays a fixed share
/// of what it took to kill: the creep's health after its armour, divided down.
/// That keeps the purse on the same exponential as the roster without any
/// hand-authored table to drift out of step with it.
///
/// The divisor was 900 while the lane was a hand-built eighty-five tile U. The
/// map's real circuit is two hundred and thirty-six, which needs about thirty-
/// four towers to cover rather than a dozen, and 900 could not pay for them:
/// a played board died on wave fifteen with fifteen towers up. Swept against
/// `a_sensible_build_clears_the_campaign`, 500 still loses on wave sixteen and
/// 350 clears - so 350, with the margin on the side of the player.
pub fn bounty_of(w: &WaveDef) -> u32 {
    (w.payable_hp() / 350.0).round().max(1.0) as u32
}

pub fn bounty_for(wave: u32) -> u32 {
    bounty_of(&wave_at(wave))
}

/// Paid when a wave is called, whatever happened in the last one.
///
/// A board that has fallen behind still needs the money to climb back out, or
/// one bad wave quietly decides the whole run.
pub fn wave_clear_bonus(wave: u32) -> u32 {
    let w = wave_at(wave);
    (bounty_of(&w) as u64 * 12).min(u32::MAX as u64) as u32
}

/// Gold per second remaining when a wave is called early.
pub const EARLY_BONUS_PER_SEC: f32 = 4.0;
