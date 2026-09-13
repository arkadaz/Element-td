//! Game simulation: the road, the monsters walking it, the towers on the pads.

pub mod board;
pub mod campaign;
pub mod combat;
pub mod defs;
pub mod fx;
pub mod greentd;
pub mod greentd_map;
#[cfg(test)]
mod greentd_tests;
pub mod greentd_types;
#[cfg(test)]
pub mod tests;

use board::{BH, BW, Board, SurfaceBlock, TOWER_CLEARANCE, TOWER_FOOTPRINT_RADIUS};
use defs::*;
use fx::{Fx, ParticleStyle};

pub const FREE_TOWER_SLOT: usize = usize::MAX;
pub const TOWER_RENDER_FOOTPRINT_SCALE: f32 = 1.14;
pub const TOWER_RENDER_HEIGHT_SCALE: f32 = 1.58;

use crate::rng::Rng;

pub const MAX_CREEPS: usize = 4000;
/// Live missiles are gameplay objects as well as visuals. A legal but absurdly
/// dense board must not be able to grow this vector until a browser tab runs
/// out of memory. Real campaign boards stay hundreds below this ceiling.
pub const MAX_PROJECTILES: usize = 4096;
/// Short-lived combat overlays have bounded visual value. Beyond these limits
/// the newest effect is omitted; combat resolution itself is never skipped.
pub const MAX_BEAMS: usize = 512;
pub const MAX_FLOAT_TEXTS: usize = 512;

/// Standalone boss-wave rules. The source map's `Boss` text labels an entire
/// stream; treating all 145 Bronze Dragons as individual bosses made the word
/// meaningless and multiplied every boss-only effect by 145. One commander now
/// leads the authored stream, with enough health and reward to deserve the bar.
pub const BOSS_HP_MULT: f32 = 8.0;
pub const BOSS_REWARD_MULT: u32 = 8;

/// Hard-mode Campaign commanders are authored objectives, rather than a
/// second application of the ordinary-body curve. This is deliberately
/// independent of the player's roster and is applied after the existing
/// one-time boss multiplier at spawn.
pub fn campaign_commander_hp_floor(difficulty: Difficulty, encounter: u16) -> f32 {
    let base = 3500.0 + 75.0 * (encounter.max(1) as f32).powf(1.7);
    match difficulty {
        Difficulty::Veteran => base,
        Difficulty::Nightmare => base * 1.25,
        Difficulty::Classic => 0.0,
    }
}
pub const BOSS_SCALE_MULT: f32 = 1.28;
pub const BOSS_MENDER_RANGE: f32 = 4.5;
pub const BOSS_MENDER_PER_SEC: f32 = 0.008;

/// How many monsters may be circling before the run is lost.
///
/// This is the whole loss condition. The circuit has no exit, so nothing ever
/// leaks and there are no lives to lose - a monster the board cannot kill comes
/// round again, and again, and the ring fills up. What the player is defending
/// is not a gate, it is a *rate*: kill faster than the waves arrive, or drown.
///
/// It replaces a twenty-life counter, and it is a better gauge for three
/// reasons. It moves continuously instead of in whole-life steps, so the player
/// can see trouble twenty seconds before it arrives. It cannot be gamed by a
/// tower that shoves monsters backwards, because backwards is the same
/// direction. And it makes every wave's leftovers a debt carried into the next
/// one, which is what makes the pressure cumulative rather than per-wave.
pub use defs::FLOOD_LIMIT;

/// The longest a wave ever takes to arrive, in seconds.
///
/// The real gap is per-wave and comes out of the map's own triggers - `call
/// PolledWait(50.)` at the top of every wave's script - so this is only the
/// bound the HUD's countdown bar is drawn against.
pub const WAVE_PERIOD: f32 = 50.0;
pub const CAMPAIGN_STANDARD_SPEED: f32 = 2.0;
pub const CAMPAIGN_DEFAULT_SPEED: f32 = 10.0;
pub const MAX_CAMPAIGN_SPEED: f32 = 100.0;
pub const MAX_ENDLESS_SPEED: f32 = 100.0;
pub const CAMPAIGN_AUTOSTART_SECONDS: f32 = 0.75;
pub const CAMPAIGN_SPEED_STEPS: [f32; 4] = [10.0, 25.0, 50.0, 100.0];
pub const LEGACY_SPEED_STEPS: [f32; 4] = CAMPAIGN_SPEED_STEPS;

/// Quiet time before the first wave. The map waits twenty seconds, prints a
/// fifteen second warning, and then waits fifteen more.
pub const PREP_TIME: f32 = 35.0;
pub use defs::START_GOLD;
/// How much stun resistance one stun adds, and the ceiling it climbs to.
/// At the ceiling a stun still lands, but briefly - hard control should be
/// strong, never absolute.
pub const STUN_DR_STEP: f32 = 0.34;
pub const STUN_DR_MAX: f32 = 0.85;
/// How fast stun resistance bleeds off, per second.
pub const STUN_DR_DECAY: f32 = 0.30;
/// How long a monster cannot be stunned again after a stun ends.
///
/// Diminishing returns alone do **not** prevent a permanent freeze, and this is
/// worth being precise about because the bug came back twice. Resistance only
/// shortens each stun; it does not slow how often they land. Once enough stun
/// towers cover one stretch of road, the next hit arrives before the shortened
/// stun has expired, the monster never takes a step, never dies and never
/// leaks, and the wave runs forever - a run stalled on wave 43 exactly this
/// way, with seventeen monsters frozen a third of the way down the road after
/// two hundred seconds.
///
/// A hard window with no stun in it is what makes progress provable: whatever
/// the board does, a monster moves for at least this long out of every
/// `stun + STUN_IMMUNE` seconds.
pub const STUN_IMMUNE: f32 = 1.2;
/// The shortest gap between two knockbacks on the same monster.
pub const KNOCKBACK_CD: f32 = 0.75;
/// Total distance, in tiles, that any one monster can ever be pushed or dragged
/// backwards over its whole life.
///
/// A cooldown is not enough on its own, and this is the third time the same
/// class of bug has had to be fixed. A monster crawling under a slow moves
/// about one tile a second; one shove of 0.85 tiles every 0.75 seconds moves it
/// backwards faster than that, so a single row of Abyss towers pinned a wave
/// short of the towers forever - nothing died, nothing leaked, and the wave
/// never ended. A per-monster budget makes forward progress provable: once it
/// is spent, the road is a one-way street whatever the board does.
pub const PUSHBACK_BUDGET: f32 = 5.0;

// ---------------------------------------------------------------- small types

#[derive(Clone, Copy, Default)]
pub struct Timed {
    pub amt: f32,
    pub t: f32,
}

impl Timed {
    #[inline]
    fn tick(&mut self, dt: f32) {
        if self.t > 0.0 {
            self.t -= dt;
            if self.t <= 0.0 {
                self.t = 0.0;
                self.amt = 0.0;
            }
        }
    }
    /// Keep whichever application is stronger, refresh the timer.
    #[inline]
    fn apply(&mut self, amt: f32, dur: f32) {
        if amt >= self.amt || self.t <= 0.0 {
            self.amt = amt;
        }
        self.t = self.t.max(dur);
    }
    #[inline]
    fn active(&self) -> bool {
        self.t > 0.0
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Phase {
    /// Between waves; the timer is running down.
    Build,
    Combat,
    Defeat,
    Victory,
}

/// The shipped game has two intentionally separate rule sets.  `Legacy` is
/// the original extracted 36-wave loop and is never silently reinterpreted;
/// `Campaign` owns the authored 600-encounter expedition in `campaign.rs`.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum RunMode {
    #[default]
    Legacy,
    Campaign,
}

impl RunMode {
    pub const fn as_u8(self) -> u8 {
        match self {
            Self::Legacy => 0,
            Self::Campaign => 1,
        }
    }

    pub const fn from_u8(value: u8) -> Self {
        match value {
            1 => Self::Campaign,
            _ => Self::Legacy,
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Legacy => "Legacy",
            Self::Campaign => "Campaign",
        }
    }
}

/// Visible rulesets layered over the faithfully extracted map data.
///
/// Classic is the Warcraft III source. The harder modes deliberately change
/// pressure rather than tower numbers, so one roster and one counter system
/// remain learnable across every mode.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Difficulty {
    Classic,
    #[default]
    Veteran,
    Nightmare,
}

impl Difficulty {
    pub const ALL: [Difficulty; 3] = [
        Difficulty::Classic,
        Difficulty::Veteran,
        Difficulty::Nightmare,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Difficulty::Classic => "Classic",
            Difficulty::Veteran => "Veteran",
            Difficulty::Nightmare => "Nightmare",
        }
    }

    pub fn blurb(self) -> &'static str {
        match self {
            Difficulty::Classic => {
                "Learning campaign: 175 pressure, full authored income, no commander lap limit."
            }
            Difficulty::Veteran => {
                "Recommended: 140 pressure, scaled encounter budgets, commanders fall within 4 laps."
            }
            Difficulty::Nightmare => {
                "Expert: 120 pressure, leaner encounter budgets, commanders fall within 3 laps."
            }
        }
    }

    /// Short difficulty identity for the three title-screen cards.
    pub fn tagline(self) -> &'static str {
        match self {
            Difficulty::Classic => "LEARN THE RING",
            Difficulty::Veteran => "INTENDED CAMPAIGN",
            Difficulty::Nightmare => "SOLVED BUILDS ONLY",
        }
    }

    pub fn as_u8(self) -> u8 {
        match self {
            Difficulty::Classic => 0,
            Difficulty::Veteran => 1,
            Difficulty::Nightmare => 2,
        }
    }

    pub fn from_u8(value: u8) -> Self {
        match value {
            0 => Difficulty::Classic,
            2 => Difficulty::Nightmare,
            _ => Difficulty::Veteran,
        }
    }

    pub fn flood_limit(self, wave: u32) -> usize {
        // A shrinking limit makes old survivors dangerous without making the
        // tutorial waves end from raw spawn count. Each hard mode therefore
        // gets a short, visible opening grace period before the ring contracts
        // steadily to its end-game capacity.
        let tightening = |grace_wave: u32| {
            let span = CAMPAIGN_WAVES.saturating_sub(grace_wave).max(1);
            (wave.saturating_sub(grace_wave) as f32 / span as f32).clamp(0.0, 1.0)
        };
        match self {
            Difficulty::Classic => FLOOD_LIMIT,
            Difficulty::Veteran => (450.0 - 170.0 * tightening(9)).round() as usize,
            Difficulty::Nightmare => (400.0 - 200.0 * tightening(7)).round() as usize,
        }
    }

    /// Hard modes introduce a readable pressure piece inside ordinary waves.
    /// Classic deliberately returns none: it remains the source-map ruleset.
    pub fn elite_stride(self, wave: u32) -> Option<u32> {
        match self {
            Difficulty::Classic => None,
            Difficulty::Veteran if wave >= 4 => Some(8),
            Difficulty::Nightmare if wave >= 2 => Some(6),
            _ => None,
        }
    }

    fn elite_hp(self) -> f32 {
        match self {
            Difficulty::Classic => 1.0,
            Difficulty::Veteran => 2.35,
            Difficulty::Nightmare => 3.20,
        }
    }

    fn elite_speed(self) -> f32 {
        match self {
            Difficulty::Classic => 1.0,
            Difficulty::Veteran => 1.09,
            Difficulty::Nightmare => 1.14,
        }
    }

    /// Survivors become more urgent each time they complete the circuit.
    /// Classic remains byte-for-byte faithful to the source-map movement.
    pub fn lap_haste(self) -> f32 {
        match self {
            Difficulty::Classic => 0.0,
            Difficulty::Veteran => 0.06,
            Difficulty::Nightmare => 0.10,
        }
    }

    /// How many full circuits a marked commander may survive. Veteran gives
    /// one recovery lap after the warning; Nightmare demands the tighter kill.
    pub fn commander_lap_limit(self) -> Option<u32> {
        match self {
            Difficulty::Classic => None,
            Difficulty::Veteran => Some(4),
            Difficulty::Nightmare => Some(3),
        }
    }

    /// Starting gold for the 600-encounter campaign. Harder modes demand more
    /// careful opening purchases rather than buying multiple seeds at once.
    pub fn campaign_starting_gold(self) -> i64 {
        match self {
            Difficulty::Classic => 600,
            Difficulty::Veteran => 500,
            Difficulty::Nightmare => 420,
        }
    }

    /// Scaling factor for Campaign encounter rewards and kill bounties.
    /// Classic pays 100% of the authored budget. Veteran and Nightmare scale
    /// down income as chapters advance to avoid runaway late-game surplus.
    pub fn campaign_reward_scale(self, _chapter: u16, local: u16) -> f32 {
        let chapter_progress = ((local.saturating_sub(1)) as f32 / 59.0).clamp(0.0, 1.0);
        match self {
            Difficulty::Classic => 1.0,
            Difficulty::Veteran => 0.82 - 0.28 * chapter_progress,
            Difficulty::Nightmare => 0.68 - 0.32 * chapter_progress,
        }
    }

    /// Allocates one integer scaled encounter reward budget for Campaign mode.
    /// Deployment and kill budgets are integer splits of this value.
    pub fn campaign_encounter_budget(self, raw_reward: u32, chapter: u16, local: u16) -> u32 {
        let scale = self.campaign_reward_scale(chapter, local);
        (raw_reward as f32 * scale).round().max(1.0) as u32
    }

    fn reward_scale(self, wave: u32) -> f32 {
        // The opening needs enough income to teach counters and establish a
        // lane. After that grace window, rewards contract hard so late kills
        // fund deliberate upgrades instead of entire replacement boards.
        let after_opening = |grace_wave: u32| {
            let span = CAMPAIGN_WAVES.saturating_sub(grace_wave).max(1);
            (wave.saturating_sub(grace_wave) as f32 / span as f32).clamp(0.0, 1.0)
        };
        match self {
            Difficulty::Classic => 1.0,
            Difficulty::Veteran => 0.88 - 0.62 * after_opening(8),
            Difficulty::Nightmare => 0.72 - 0.54 * after_opening(6),
        }
    }

    fn apply(self, mut wave: WaveDef, number: u32) -> WaveDef {
        let campaign = ((number.saturating_sub(1)) as f32 / 35.0).clamp(0.0, 1.0);
        let (hp, speed, stream, cadence) = match self {
            Difficulty::Classic => (1.0, 1.0, 1.0, 1.0),
            Difficulty::Veteran => (
                1.02 + 0.73 * campaign,
                1.05 + 0.03 * campaign,
                0.90 - 0.04 * campaign,
                0.88 - 0.06 * campaign,
            ),
            Difficulty::Nightmare => (
                1.08 + 1.17 * campaign,
                1.08 + 0.05 * campaign,
                0.86 - 0.08 * campaign,
                0.86 - 0.10 * campaign,
            ),
        };
        wave.hp *= hp;
        wave.speed *= speed;
        wave.spawn_gap = (wave.spawn_gap * stream).max(0.04);
        // Never let a harder cadence silently truncate the tail of a wave.
        // Every authored enemy must enter before the next boundary replaces
        // `spawn_left`; pressure comes from overlap on the road, not missing
        // units in the schedule.
        let spawn_window = wave.spawn_gap * wave.count.saturating_sub(1) as f32;
        wave.lead_in = (wave.lead_in * cadence).max(spawn_window + 0.10).max(28.0);
        wave
    }
}

/// A permanent campaign upgrade chosen before waves 10, 20 and 30.
///
/// These are deliberately global and legible. The player is choosing a plan,
/// not solving an inventory puzzle, and every existing tower benefits at once.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Doctrine {
    Arsenal,
    Overdrive,
    Reach,
}

impl Doctrine {
    pub const ALL: [Doctrine; 3] = [Doctrine::Arsenal, Doctrine::Overdrive, Doctrine::Reach];

    pub fn index(self) -> usize {
        match self {
            Doctrine::Arsenal => 0,
            Doctrine::Overdrive => 1,
            Doctrine::Reach => 2,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Doctrine::Arsenal => "Sharpen Arsenal",
            Doctrine::Overdrive => "Overdrive",
            Doctrine::Reach => "High Ground",
        }
    }

    pub fn effect(self) -> &'static str {
        match self {
            Doctrine::Arsenal => "+12% damage to every tower",
            Doctrine::Overdrive => "+10% attack speed to every tower",
            Doctrine::Reach => "+0.6 range to every tower and aura",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TargetMode {
    First,
    Last,
    Strongest,
    Closest,
}

impl TargetMode {
    fn default_for(family: Family) -> Self {
        match family {
            // These are the roster's expensive single-target commander tools.
            // Making their useful behaviour the default removes a hidden UI
            // tax while still letting the player cycle to another priority.
            Family::King | Family::OneStrike => TargetMode::Strongest,
            _ => TargetMode::First,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            TargetMode::First => "First",
            TargetMode::Last => "Last",
            TargetMode::Strongest => "Strongest",
            TargetMode::Closest => "Closest",
        }
    }
    pub fn next(self) -> Self {
        match self {
            TargetMode::First => TargetMode::Last,
            TargetMode::Last => TargetMode::Strongest,
            TargetMode::Strongest => TargetMode::Closest,
            TargetMode::Closest => TargetMode::First,
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            TargetMode::First => "Focus the enemy furthest through all completed laps.",
            TargetMode::Last => "Focus the newest enemy furthest back in the stream.",
            TargetMode::Strongest => "Focus the enemy with the most health remaining.",
            TargetMode::Closest => "Focus the enemy nearest to this tower.",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PlacementIssue {
    OutsideWorld,
    Road,
    SolidScenery,
    TowerOverlap,
    NotEnoughGold,
}

impl PlacementIssue {
    pub const fn label(self) -> &'static str {
        match self {
            Self::OutsideWorld => "Outside the meadow boundary",
            Self::Road => "Keep towers clear of the road",
            Self::SolidScenery => "Solid scenery blocks that spot",
            Self::TowerOverlap => "Too close to another tower",
            Self::NotEnoughGold => "Not enough gold",
        }
    }
}

impl From<SurfaceBlock> for PlacementIssue {
    fn from(value: SurfaceBlock) -> Self {
        match value {
            SurfaceBlock::OutsideWorld => Self::OutsideWorld,
            SurfaceBlock::Road => Self::Road,
            SurfaceBlock::SolidScenery => Self::SolidScenery,
        }
    }
}

// ---------------------------------------------------------------- entities

#[derive(Clone)]
pub struct Creep {
    pub uid: u32,
    /// How far along the road, in tiles. This is the creep's real position.
    pub dist: f32,
    /// +1 clockwise, -1 anticlockwise. The Warcraft trigger at Red's first
    /// junction chooses between these two routes for every entering unit.
    pub route_dir: f32,
    /// Sideways offset from the road centre so a pack does not walk in a line.
    pub lane: f32,
    /// Cached world position, refreshed every step.
    pub pos: [f32; 2],
    pub facing: f32,
    pub hp: f32,
    pub max_hp: f32,
    pub base_speed: f32,
    /// Warcraft III armour: a *number*, reduced with diminishing returns, plus
    /// a type deciding which attacks are resisted at all. Together those two
    /// are the whole counter system - see `greentd_types::damage_taken`.
    pub armour: i32,
    pub armour_type: ArmourType,
    /// Which Warcraft III unit this wave is wearing.
    pub model: Model,
    pub flying: bool,
    pub radius: f32,
    pub bounty: u32,
    /// The source map explicitly tagged this wave as a boss wave.
    pub boss: bool,
    /// A hard-mode Vanguard: tougher, faster, and resistant to control. The
    /// gold ground ring makes the rule visible before the player feels it.
    pub elite: bool,
    pub slow: Timed,
    pub burn: Timed,
    pub poison: Timed,
    pub shred: Timed,
    pub stun: f32,
    /// Resistance to further stuns, 0..[`STUN_DR_MAX`]. Grows with each stun
    /// and decays when the target is left alone.
    pub stun_dr: f32,
    /// Counts down to when this monster can be shoved back again.
    pub kb_cd: f32,
    /// Seconds of Mire suppression left. While this is above zero the monster
    /// neither regenerates nor accepts a Mender's healing.
    pub suppress: f32,
    /// Counts down the window after a stun during which no stun can land.
    /// See [`STUN_IMMUNE`].
    pub stun_immune: f32,
    /// How much further this monster can ever be pushed backwards.
    /// See [`PUSHBACK_BUDGET`].
    pub push_left: f32,
    /// Complete laps of the circuit. First targeting includes this distance,
    /// and hard-mode survivors gain speed—but never more bounty—as it rises.
    pub laps: u32,
    pub flash: f32,
    pub bob: f32,
    /// Campaign-only physical behavior. Legacy creeps keep the zero/false
    /// defaults, which preserves their old simulation and saves.
    pub shield: f32,
    pub max_shield: f32,
    pub regen_per_second: f32,
    pub resistant: bool,
    pub campaign_encounter: u16,
    pub pressure: f32,
    pub death_killer: Option<usize>,
}

impl Creep {
    #[inline]
    pub fn speed(&self) -> f32 {
        if self.stun > 0.0 {
            return 0.0;
        }
        let slow = if self.slow.active() {
            self.slow.amt
        } else {
            0.0
        };
        self.base_speed * (1.0 - slow).max(0.15)
    }
    #[inline]
    pub fn hp_frac(&self) -> f32 {
        (self.hp / self.max_hp).clamp(0.0, 1.0)
    }
    /// Height of the body's centre above the ground, for the 3D view.
    #[inline]
    pub fn height(&self) -> f32 {
        // Flyers ride above the road, drifting gently so a formation of them
        // does not look like a decal sheet.
        let alt = if self.flying { 1.7 } else { 0.0 };
        let drift = if alt > 0.0 {
            (self.bob * 0.9).sin() * 0.16
        } else {
            0.0
        };
        alt + drift + self.radius * 1.4 + (self.bob.sin() * 0.5 + 0.5) * 0.10
    }
    /// Whether this is one of the map's explicitly authored boss waves.
    pub fn is_boss(&self) -> bool {
        self.boss
    }

    /// Elites still accept control, but only at half strength. A dedicated
    /// property keeps that rule in one place for slows, roots and knockback.
    pub fn control_scale(&self) -> f32 {
        if self.is_boss() {
            0.25
        } else if self.elite || self.resistant {
            0.5
        } else {
            1.0
        }
    }
}

// ---------------------------------------------------------------- campaign bodies

/// The campaign resolver owns *when* a body arrives; this compact adapter owns
/// how its resolved trait becomes a real simulation entity.  Keeping this
/// mapping beside `Creep` avoids a second, visual-only enemy roster.
#[derive(Clone, Copy)]
struct CampaignBodySpec {
    model: Model,
    hp: f32,
    speed: f32,
    armour: i32,
    armour_type: ArmourType,
    flying: bool,
    scale: f32,
    pressure: f32,
    shield: f32,
    regen_per_second: f32,
    resistant: bool,
}

fn campaign_trait_share(formation: campaign::Formation) -> u32 {
    use campaign::Formation::*;
    match formation {
        BroodTide => 85,
        IronConvoy => 35,
        RunningFlank | Crosswind => 35,
        WingEscort => 30,
        PatientShields | ProtectedRear => 25,
        LastSurge | Reversal => 50,
        HealingCaravan => 25,
        NeedleFlight => 40,
        TwoFronts => 45,
        ThinScreen => 30,
        RecoveringPack => 35,
        RelentlessMarch => 30,
        CombinedRehearsal => 34,
        SplitScouts | BrokenColumn => 30,
    }
}

/// Resolves a deterministic individual role inside a packet.  The schedule
/// describes budget fractions, not a line of cloned pawns, so this preserves
/// the formation's actual mixed composition without adding runtime RNG.
fn campaign_body_trait(packet: campaign::SpawnPacket, ordinal: u16) -> Option<campaign::ResolvedTrait> {
    let primary = packet.traits[0];
    let secondary = packet.traits[1];
    let roll = ((ordinal as u32 * 37
        + packet.at_seconds as u32 * 11
        + packet.formation as u32 * 17)
        % 100) as u32;
    let primary_share = campaign_trait_share(packet.formation);
    match (primary, secondary) {
        (Some(a), Some(_b)) if roll < primary_share => Some(a),
        (Some(_), Some(b)) if roll < (primary_share + 22).min(100) => Some(b),
        (Some(a), None) if roll < primary_share => Some(a),
        (None, Some(b)) if roll < 28 => Some(b),
        _ => None,
    }
}

fn campaign_body_spec(
    chapter: u16,
    local: u16,
    trait_: Option<campaign::ResolvedTrait>,
    difficulty: Difficulty,
) -> CampaignBodySpec {
    use campaign::ResolvedTrait::*;
    // Persistent tower tiers and a growing economy need a real late campaign
    // counterweight. Use global encounter progress rather than a chapter
    // step: C1E60 and C2E1 stay continuous instead of receiving a hidden
    // boundary spike. The curved health term is calibrated against the legal
    // E121 frozen roster (~0.28M direct DPS at E600) and the reinvesting
    // roster (~2.35M): later bodies require sustained new investment, while
    // the early counter introductions remain readable.
    let global = (chapter.saturating_sub(1) as f32 * campaign::ENCOUNTERS_PER_CHAPTER as f32
        + local.max(1) as f32)
        .clamp(1.0, campaign::REALISTIC_ENCOUNTERS as f32);
    let progress = (global - 1.0) / (campaign::REALISTIC_ENCOUNTERS - 1) as f32;
    let baseline_scale = 1.0 + 1.2 * progress;
    let sustained_health_scale = 1.0 + 250.0 * progress * progress;
    let scale = baseline_scale * sustained_health_scale;
    let (diff_mult, speed_mult) = match difficulty {
        Difficulty::Classic => (1.0, 1.0),
        Difficulty::Veteran => (1.06 + 0.06 * ((local.saturating_sub(1)) as f32 / 59.0), 1.03),
        Difficulty::Nightmare => (1.12 + 0.12 * ((local.saturating_sub(1)) as f32 / 59.0), 1.06),
    };
    let mut spec = match trait_ {
        Some(Swarm) => CampaignBodySpec {
            model: Model::Gnoll, hp: 24.0, speed: 1.22, armour: 0,
            armour_type: ArmourType::Unarmoured, flying: false, scale: 0.80,
            pressure: 0.20, shield: 0.0, regen_per_second: 0.0, resistant: false,
        },
        Some(Armoured) => CampaignBodySpec {
            model: Model::Brute, hp: 118.0, speed: 0.76, armour: 9,
            armour_type: ArmourType::Medium, flying: false, scale: 1.05,
            pressure: 2.0, shield: 0.0, regen_per_second: 0.0, resistant: false,
        },
        Some(Swift) => CampaignBodySpec {
            model: Model::Warrior, hp: 48.0, speed: 1.48, armour: 1,
            armour_type: ArmourType::Light, flying: false, scale: 0.90,
            pressure: 0.75, shield: 0.0, regen_per_second: 0.0, resistant: false,
        },
        Some(Flying) => CampaignBodySpec {
            model: Model::Harpy, hp: 54.0, speed: 1.18, armour: 1,
            armour_type: ArmourType::Light, flying: true, scale: 0.92,
            pressure: 0.75, shield: 0.0, regen_per_second: 0.0, resistant: false,
        },
        Some(Shielded) => CampaignBodySpec {
            model: Model::Warrior, hp: 86.0, speed: 0.82, armour: 3,
            armour_type: ArmourType::Medium, flying: false, scale: 1.0,
            pressure: 1.2, shield: 64.0, regen_per_second: 0.0, resistant: false,
        },
        Some(Regenerator) => CampaignBodySpec {
            model: Model::Troll, hp: 98.0, speed: 0.88, armour: 3,
            armour_type: ArmourType::Medium, flying: false, scale: 1.02,
            pressure: 1.25, shield: 0.0, regen_per_second: 4.0, resistant: false,
        },
        Some(Resistant) => CampaignBodySpec {
            model: Model::Brute, hp: 124.0, speed: 0.91, armour: 7,
            armour_type: ArmourType::Heavy, flying: false, scale: 1.06,
            pressure: 2.0, shield: 0.0, regen_per_second: 0.0, resistant: true,
        },
        None => CampaignBodySpec {
            model: Model::Warrior, hp: 62.0, speed: 0.97, armour: 1,
            armour_type: ArmourType::Unarmoured, flying: false, scale: 0.94,
            pressure: 0.75, shield: 0.0, regen_per_second: 0.0, resistant: false,
        },
    };
    spec.hp *= scale * diff_mult;
    spec.shield *= scale * diff_mult;
    spec.armour += (36.0 * progress).round() as i32;
    // Late formations are not just slower bags of health: each survivor uses
    // substantially more of the ring's recovery budget. A settled early board
    // therefore cannot ignore later chapters by hoarding income.
    spec.pressure *= 1.0 + 6.12 * progress;
    spec.speed *= speed_mult;
    spec.regen_per_second *= (scale * diff_mult).sqrt();
    spec
}

#[derive(Clone)]
pub struct Tower {
    /// Index into [`TOWERS`] - a family *and* a rung, together. Upgrading moves
    /// this to the next rung rather than bumping a separate tier counter.
    pub def: usize,
    pub slot: usize,
    pub pos: [f32; 2],
    /// Fixed distance from this plot to the creep lane. Towers beyond their
    /// current reach can skip targeting entirely; upgrades may make them live.
    pub road_dist: f32,
    pub cooldown: f32,
    pub angle: f32,
    pub target_uid: u32,
    /// Seconds of self-frenzy left. Only the Troll Tower uses it.
    pub ramp: f32,
    /// Counts down to the next frenzy.
    pub frenzy_cd: f32,
    /// Counts down to the next tick of a standing aura - a Slow Tower's chill
    /// or a Fire Tower's immolation.
    pub aura_timer: f32,
    pub kills: u32,
    pub damage: f64,
    pub invested: u32,
    pub mode: TargetMode,
    pub flash: f32,
    pub built_at: f32,
    /// Aura bonuses from nearby Groves, recomputed when the board changes.
    pub buff_dmg: f32,
    pub buff_rate: f32,
    pub buff_range: f32,
    /// Gold this tower has personally generated - income and kill bounties.
    pub gold_earned: u64,
}

impl Tower {
    pub fn def(&self) -> &'static TowerLevel {
        &TOWERS[self.def]
    }
    pub fn family(&self) -> Family {
        self.def().family
    }
    /// Which rung of its ladder this is, counting from one.
    pub fn level(&self) -> u32 {
        self.def().step + 1
    }
    pub fn ladder_len(&self) -> u32 {
        ladder_len(self.family())
    }
    /// How far up its family it stands, from 0 at the first rung to 1 at the
    /// last. A Siege Tower has twenty rungs and a Chaos tower five, so the
    /// step number alone says nothing about how far along you are.
    pub fn progress(&self) -> f32 {
        let n = self.ladder_len().max(2) - 1;
        (self.def().step as f32 / n as f32).clamp(0.0, 1.0)
    }
    pub fn full_name(&self) -> &'static str {
        self.def().name
    }
    pub fn attack(&self) -> Attack {
        self.def().attack
    }
    pub fn targets(&self) -> Targets {
        self.def().targets
    }
    pub fn abil(&self) -> &'static Abil {
        &self.def().abil
    }
    /// Damage of one hit, with any aura bonus.
    pub fn dmg(&self) -> f32 {
        self.def().damage * (1.0 + self.buff_dmg)
    }
    /// Attacks per second, with any aura bonus and any frenzy running.
    pub fn rate(&self) -> f32 {
        let cd = self.def().cooldown.max(0.05);
        let frenzy = if self.ramp > 0.0 {
            self.abil().frenzy
        } else {
            0.0
        };
        (1.0 / cd) * (1.0 + self.buff_rate + frenzy)
    }
    pub fn range(&self) -> f32 {
        self.def().range + self.buff_range
    }
    pub fn is_support(&self) -> bool {
        !self.def().attacks()
    }
    /// What selling pays. Some source-map branches contain point values a few
    /// hundred gold above their reachable purchase path. Paying that raw value
    /// lets build -> upgrade -> sell mint unlimited gold, so the runtime never
    /// refunds more than this specific tower actually cost.
    pub fn sell_value(&self) -> u32 {
        self.def().refund.min(self.invested)
    }
    /// Everything this tower can become, and what each costs.
    pub fn upgrades(&self) -> Vec<(usize, u32)> {
        upgrades_of(self.def)
    }
    /// The next step, when there is exactly one. `None` at the top of a path
    /// and at every fork.
    pub fn upgrade_target(&self) -> Option<(usize, u32)> {
        next_level(self.def).map(|i| (i, TOWERS[i].gold))
    }
    /// Whether upgrading is a choice rather than a single next level. True for
    /// the ten gold seed, for every rung of the Aura Tower, and for the King.
    pub fn has_choice(&self) -> bool {
        self.def().upgrades.len() > 1
    }
    /// Legacy height estimate retained for save/UI compatibility.
    #[allow(dead_code)]
    pub fn height(&self) -> f32 {
        0.55 + 0.14 * (self.def().step.min(10) as f32)
    }
    /// Scale shared by the battlefield mesh and its projectile launch point.
    /// Statistical rungs map onto four visual milestones, so a twentieth
    /// Siege upgrade cannot make shots float a full tower above its barrel.
    pub fn visual_model_scale(&self) -> f32 {
        let stage = tower_visual_stage(self.def());
        let scale = 0.94 + stage as f32 * 0.055 + self.progress() * 0.06;
        // Pivot-correct Seed barrels are long. A small cap on their upper two
        // milestones keeps the radial envelope inside half the 2.2-tile pad
        // spacing even when neighbouring guns aim directly at each other.
        if self.family() == Family::Single {
            match stage {
                2 => scale.min(1.065),
                3 => scale.min(1.105),
                _ => scale,
            }
        } else {
            scale
        }
    }
    /// Authored barrel tip in normalized turret space: forward reach, height.
    /// Values are measured from the exact staged Quaternius assets. Vertical
    /// launchers and teleporters intentionally emit from their top centre.
    fn muzzle_profile(&self) -> [f32; 2] {
        const SEED: [[f32; 2]; 4] = [[0.50, 0.92], [0.85, 0.82], [1.03, 0.65], [0.99, 0.81]];
        const SIEGE: [[f32; 2]; 4] = [[0.38, 0.73], [0.45, 0.67], [0.38, 0.71], [0.64, 0.70]];
        const BOUNCE: [[f32; 2]; 4] = [[0.53, 0.72], [0.45, 0.67], [0.54, 0.78], [0.54, 0.78]];
        const MULTI: [[f32; 2]; 4] = [[0.92, 0.78], [0.94, 0.78], [0.80, 0.77], [0.80, 0.77]];
        const CORRUPT: [[f32; 2]; 4] = [[0.0, 0.81], [0.0, 1.0], [0.0, 1.0], [0.0, 1.0]];
        const AIR: [[f32; 2]; 4] = [[0.0, 1.0]; 4];
        const CHAOS: [[f32; 2]; 4] = [[0.74, 0.75], [0.89, 0.74], [0.91, 0.77], [0.91, 0.77]];
        const DESTROY: [[f32; 2]; 4] = [[0.70, 0.74], [0.52, 0.80], [0.63, 0.77], [0.63, 0.77]];
        const AURA: [[f32; 2]; 4] = [[0.0, 1.0], [0.0, 1.0], [0.0, 0.46], [0.0, 0.46]];
        const DEMON: [[f32; 2]; 4] = [[0.50, 0.71], [0.85, 0.79], [0.44, 0.75], [0.44, 0.75]];
        const KING: [[f32; 2]; 4] = [[0.84, 0.66], [0.83, 0.66], [0.83, 0.63], [0.64, 0.58]];

        use Family::*;
        let profiles = match self.family() {
            Single => &SEED,
            Siege => &SIEGE,
            Bouncing | SuperBounce => &BOUNCE,
            Multi | Critical | SuperMulti => &MULTI,
            Corruption | Poison => &CORRUPT,
            Air | Frost | Slow => &AIR,
            Chaos | SuperChaos => &CHAOS,
            Destruction | SuperDestruct | Fire => &DESTROY,
            Aura | Damage | Speed => &AURA,
            Demon | Troll => &DEMON,
            King | OneStrike => &KING,
        };
        profiles[tower_visual_stage(self.def()).min(3)]
    }
    pub fn muzzle_height(&self) -> f32 {
        // Plinth deck is at z=0.30.
        0.30 + self.visual_model_scale() * self.muzzle_profile()[1]
    }
    pub fn muzzle_reach(&self) -> f32 {
        self.muzzle_profile()[0] * self.visual_model_scale()
    }
    #[allow(dead_code)]
    pub fn scale(&self) -> f32 {
        (0.85 + 0.06 * self.def().step as f32) * self.def().scale.clamp(0.6, 2.0)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ProjKind {
    Dart,
    Shell,
    Glaive,
    Bolt,
    Acid,
    Missile,
    Chaos,
    Flame,
    Orb,
    Royal,
}

#[derive(Clone)]
pub struct Proj {
    pub pos: [f32; 2],
    pub z: f32,
    pub vel: [f32; 2],
    pub kind: ProjKind,
    pub tower: usize,
    pub def: usize,
    pub dmg: f32,
    pub splash: f32,
    /// How many further targets this shot may leap to.
    pub bounces: u32,
    pub crit: bool,
    pub target_idx: usize,
    pub target_uid: u32,
    pub life: f32,
    pub trail: f32,
}

#[derive(Clone, Copy)]
pub struct Beam {
    pub from: [f32; 3],
    pub to: [f32; 3],
    pub color: [f32; 3],
    pub t: f32,
    /// 0 marks a ground shockwave rather than a line.
    pub width: f32,
}

#[derive(Clone, Copy, PartialEq)]
pub enum TextKind {
    Damage,
    Crit,
    Gold,
}

#[derive(Clone, Copy)]
pub struct FloatText {
    pub pos: [f32; 3],
    pub value: f32,
    pub kind: TextKind,
    pub t: f32,
}

// ---------------------------------------------------------------- spatial hash

/// Uniform grid over the board so towers only test nearby creeps.
pub struct SpatialHash {
    cell: f32,
    cols: usize,
    rows: usize,
    starts: Vec<u32>,
    items: Vec<u32>,
    counts: Vec<u32>,
    cursor: Vec<u32>,
}

impl SpatialHash {
    fn new() -> Self {
        let cell = 2.0;
        let cols = (BW / cell).ceil() as usize + 4;
        let rows = (BH / cell).ceil() as usize + 4;
        Self {
            cell,
            cols,
            rows,
            starts: vec![0; cols * rows + 1],
            items: Vec::new(),
            counts: vec![0; cols * rows],
            cursor: vec![0; cols * rows],
        }
    }

    #[inline]
    fn cell_of(&self, p: [f32; 2]) -> (usize, usize) {
        let cx = ((p[0] / self.cell) + 2.0).clamp(0.0, self.cols as f32 - 1.0) as usize;
        let cy = ((p[1] / self.cell) + 2.0).clamp(0.0, self.rows as f32 - 1.0) as usize;
        (cx, cy)
    }

    fn rebuild(&mut self, creeps: &[Creep]) {
        self.counts.iter_mut().for_each(|c| *c = 0);
        for c in creeps {
            let (cx, cy) = self.cell_of(c.pos);
            self.counts[cy * self.cols + cx] += 1;
        }
        let mut acc = 0u32;
        for i in 0..self.counts.len() {
            self.starts[i] = acc;
            acc += self.counts[i];
        }
        self.starts[self.counts.len()] = acc;
        self.items.resize(creeps.len(), 0);
        self.cursor
            .copy_from_slice(&self.starts[..self.counts.len()]);
        for (i, c) in creeps.iter().enumerate() {
            let (cx, cy) = self.cell_of(c.pos);
            let k = cy * self.cols + cx;
            self.items[self.cursor[k] as usize] = i as u32;
            self.cursor[k] += 1;
        }
    }

    /// Visit every creep index whose cell overlaps the circle.
    fn query(&self, pos: [f32; 2], r: f32, mut f: impl FnMut(usize)) {
        let (x0, y0) = self.cell_of([pos[0] - r, pos[1] - r]);
        let (x1, y1) = self.cell_of([pos[0] + r, pos[1] + r]);
        for cy in y0..=y1 {
            for cx in x0..=x1 {
                let k = cy * self.cols + cx;
                let (s, e) = (self.starts[k] as usize, self.starts[k + 1] as usize);
                for &i in &self.items[s..e] {
                    f(i as usize);
                }
            }
        }
    }
}

// ---------------------------------------------------------------- stats

#[derive(Default, Clone, Copy)]
pub struct RunStats {
    pub kills: u64,
    pub leaked: u32,
    pub gold_earned: u64,
    pub gold_spent: u64,
    pub damage: f64,
    pub towers_built: u32,
    /// The most monsters ever circling at once.
    ///
    /// The run's high-water mark, and the only honest measure of how close it
    /// came to ending - a run that finished with an empty ring might have been
    /// one monster from drowning at wave 60, and nothing else records that.
    pub peak_circling: u32,
    /// Automatic wave boundaries reached with no old enemy still alive.
    pub clean_sweeps: u32,
    /// Gold earned by deliberately stacking waves before their timer expires.
    pub rush_gold: u64,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ToastTone {
    Info,
    Good,
    Bad,
}

#[derive(Clone, Copy, PartialEq)]
pub enum Cue {
    Select,
    Build,
    Sell,
    Error,
    /// One representative impact from this frame. The audio mixer throttles
    /// these hard, so a dense late-game board remains punchy rather than loud.
    Impact(ProjKind),
    /// The launch transient is separate from impact so a slow shell and a
    /// homing spell feel physical across their whole flight.
    Shot(ProjKind),
    /// One representative creature death per render frame. Mechanical enemies
    /// use a fracture sound; living and spectral enemies use a short growl.
    MonsterDeath {
        boss: bool,
        mechanical: bool,
    },
    WaveStart,
    Boss,
    Victory,
    Defeat,
}

// ---------------------------------------------------------------- game

pub struct Game {
    pub board: Board,
    pub creeps: Vec<Creep>,
    pub towers: Vec<Tower>,
    pub projs: Vec<Proj>,
    pub beams: Vec<Beam>,
    pub texts: Vec<FloatText>,
    pub fx: Fx,
    pub rng: Rng,
    /// The seed this run was started from. Saved, so a resumed run faces the
    /// same waves - they are generated, never stored.
    pub seed: u64,
    /// Player-chosen, visible pressure rules layered over the map data.
    pub difficulty: Difficulty,
    /// Explicit run identity. It is persisted independently of difficulty so
    /// an existing Legacy save never wakes up as a 600-encounter run.
    pub mode: RunMode,
    /// Live encounter timing/deployment state for `RunMode::Campaign`.
    pub campaign: Option<campaign::CampaignState>,
    pub diagnostic_fixture: Option<u16>,
    /// The campaign ring measures weighted pressure. This is its remaining
    /// three-second breach grace, not a hidden capacity multiplier.
    pub campaign_pressure_grace: f32,
    pub spatial: SpatialHash,

    /// True once the campaign has been cleared and the run has continued.
    pub endless: bool,
    pub wave: u32,
    pub phase: Phase,
    pub gold: i64,

    /// Counts down to the next wave. Always running.
    pub wave_timer: f32,
    /// True until the first wave has been called; the only quiet moment in a run.
    pub prep: bool,
    pub spawn_left: u32,
    /// How much of the wave's escort is still to arrive.
    pub escort_left: u32,
    escort_timer: f32,
    pub spawn_timer: f32,
    pub time: f32,
    pub next_uid: u32,

    pub selected: Option<usize>,
    pub build_choice: Option<(usize, u32)>,
    /// Pad the cursor is over, if any.
    pub hover_slot: Option<usize>,
    pub hover_pos: Option<[f32; 2]>,
    pub speed: f32,
    pub paused: bool,
    pub shake: f32,
    pub stats: RunStats,
    /// Interest paid at the end of the last wave, for the scoreboard.
    pub last_interest: i64,
    pub toast: Option<(String, f32, ToastTone)>,
    /// Permanent command upgrades, in Arsenal / Overdrive / Reach order.
    pub doctrines: [u8; 3],
    /// A hard-mode milestone choice currently waiting for the player.
    pub pending_doctrine: bool,
    pub sound_cues: Vec<Cue>,
    /// Raised at a wave boundary; the app writes the save and clears it.
    pub wants_save: bool,
    /// Reused scratch buffer for spatial queries; avoids per-tower allocation.
    pub scratch: Vec<usize>,
}

impl Default for Game {
    fn default() -> Self {
        Self::new()
    }
}

impl Game {
    pub fn new() -> Self {
        Self {
            board: Board::new(),
            creeps: Vec::with_capacity(512),
            towers: Vec::with_capacity(128),
            projs: Vec::with_capacity(512),
            beams: Vec::with_capacity(64),
            texts: Vec::with_capacity(64),
            fx: Fx::default(),
            rng: Rng::new(0x5eed_1234_abcd_9876),
            seed: 0x5eed_1234_abcd_9876,
            difficulty: Difficulty::Classic,
            mode: RunMode::Legacy,
            campaign: None,
            diagnostic_fixture: None,
            campaign_pressure_grace: campaign::BREACH_SECONDS,
            spatial: SpatialHash::new(),
            endless: false,
            wave: 0,
            phase: Phase::Build,
            gold: START_GOLD,
            wave_timer: PREP_TIME,
            prep: true,
            spawn_left: 0,
            escort_left: 0,
            escort_timer: 0.0,
            spawn_timer: 0.0,
            time: 0.0,
            next_uid: 1,
            selected: None,
            build_choice: None,
            hover_slot: None,
            hover_pos: None,
            speed: 1.0,
            paused: false,
            shake: 0.0,
            stats: RunStats::default(),
            last_interest: 0,
            toast: None,
            doctrines: [0; 3],
            pending_doctrine: false,
            sound_cues: Vec::new(),
            wants_save: false,
            scratch: Vec::with_capacity(256),
        }
    }

    pub fn reset(&mut self) {
        self.restart();
    }

    // ------------------------------------------------ queries

    /// Waves are generated on demand, so the run can continue past the campaign.
    pub fn wave_def(&self, wave: u32) -> WaveDef {
        if self.is_campaign() {
            return self.campaign_wave_def(wave.clamp(1, campaign::REALISTIC_ENCOUNTERS as u32) as u16);
        }
        self.difficulty.apply(wave_at(wave), wave)
    }

    /// A representative definition for existing HUD/preview consumers.  Live
    /// Campaign spawning does not use this as a hidden wave table: each body
    /// instead comes from the active `SpawnPacket` and its resolved traits.
    fn campaign_wave_def(&self, encounter: u16) -> WaveDef {
        let e = campaign::resolved_encounter(encounter);
        let packet = e.packets.first().copied().expect("every authored encounter has a packet");
        let spec = campaign_body_spec(e.id.chapter, e.id.local, packet.traits[0], self.difficulty);
        let tag = if e.commander.is_some() {
            "Commander"
        } else if packet.traits[0].is_some() {
            "Trait"
        } else {
            "Formation"
        };
        WaveDef {
            name: e.formation.name(),
            tag,
            model: spec.model,
            scale: spec.scale,
            count: e.packets.iter().map(|p| p.bodies as u32).sum(),
            hp: spec.hp,
            armour: spec.armour,
            armour_type: spec.armour_type,
            speed: spec.speed,
            flying: spec.flying,
            spawn_gap: 0.04,
            lead_in: e.duration_seconds as f32,
        }
    }

    /// Restarts the run on a fresh road.
    pub fn restart(&mut self) {
        let difficulty = self.difficulty;
        let seed = self.rng.next_u64() ^ 0x51ED_2A17_9C3B_44D1;
        if self.mode == RunMode::Campaign {
            self.start_campaign(seed, difficulty);
        } else {
            self.start_run_with_difficulty(seed, difficulty);
        }
    }

    /// Restarts from an exact seed.
    ///
    /// This is what makes multiplayer work without the server simulating
    /// anything: every client in a room is handed the same seed, so everyone
    /// faces byte-identical waves on their own board.
    pub fn start_run(&mut self, seed: u64) {
        self.start_run_with_difficulty(seed, Difficulty::Classic);
    }

    /// Starts from an exact seed under an explicit, player-visible ruleset.
    pub fn start_run_with_difficulty(&mut self, seed: u64, difficulty: Difficulty) {
        *self = Game::new();
        self.rng = Rng::new(seed);
        self.seed = seed;
        self.difficulty = difficulty;
    }

    /// Starts the new, separately labelled 600-encounter expedition.  This
    /// deliberately does not call `start_run_with_difficulty`: that method is
    /// Legacy-only and has a different economy, progression, and ending.
    pub fn start_campaign(&mut self, seed: u64, difficulty: Difficulty) {
        *self = Game::new();
        self.rng = Rng::new(seed);
        self.seed = seed;
        self.difficulty = difficulty;
        self.mode = RunMode::Campaign;
        self.campaign = Some(campaign::CampaignState::default());
        self.speed = CAMPAIGN_DEFAULT_SPEED;
        self.gold = difficulty.campaign_starting_gold();
        self.campaign_pressure_grace = campaign::BREACH_SECONDS;
        self.prep = false;
        self.wave_timer = CAMPAIGN_AUTOSTART_SECONDS;
        self.wants_save = true;
        self.notice("Campaign ready: Chapter 1 · Encounter 1 auto-deploys now · rapid 10x");
    }

    #[inline]
    pub const fn is_campaign(&self) -> bool {
        matches!(self.mode, RunMode::Campaign)
    }

    /// The encounter currently being previewed or fought.  During the small
    /// build beat after a resolution the `CampaignState` has already advanced
    /// to the next authored encounter, while `wave` remains the one just
    /// cleared; this avoids a misleading off-by-one HUD.
    pub fn campaign_encounter(&self) -> Option<u16> {
        self.campaign.as_ref().map(|state| state.encounter)
    }

    pub fn upcoming_wave_number(&self) -> u32 {
        self.campaign
            .as_ref()
            .map(|state| state.encounter as u32)
            .unwrap_or_else(|| self.wave.saturating_add(1))
    }

    pub fn shop_entries(&self) -> Vec<usize> {
        let entries = shop_order();
        if !self.is_campaign() {
            return entries;
        }
        entries
            .into_iter()
            .filter(|&index| {
                matches!(
                    TOWERS[index].family,
                    Family::Single
                        | Family::Siege
                        | Family::Multi
                        | Family::Slow
                        | Family::Corruption
                        | Family::Air
                )
            })
            .collect()
    }

    pub fn campaign_chapter(&self) -> Option<u16> {
        self.campaign
            .as_ref()
            .map(|state| campaign::encounter_id(state.encounter).chapter)
    }

    pub fn campaign_pressure_capacity(&self) -> f32 {
        match self.difficulty {
            Difficulty::Classic => 175.0,
            Difficulty::Veteran => 140.0,
            Difficulty::Nightmare => 120.0,
        }
    }

    /// Weighted ring pressure.  It is intentionally not `creeps.len()`: a
    /// swarm body and a commander do not consume the same recovery capacity.
    pub fn campaign_pressure(&self) -> f32 {
        self.creeps
            .iter()
            .map(|c| c.pressure * (1.0 + 0.15 * c.laps.min(4) as f32))
            .sum()
    }

    pub fn flood_limit(&self) -> usize {
        if self.is_campaign() {
            self.campaign_pressure_capacity().round() as usize
        } else {
            self.difficulty.flood_limit(self.wave)
        }
    }

    pub fn bounty_for_wave(&self, wave: u32) -> u32 {
        if self.is_campaign() {
            return 1;
        }
        let base = bounty_of(&wave_at(wave));
        (base as f32 * self.difficulty.reward_scale(wave))
            .round()
            .max(1.0) as u32
    }

    pub fn stipend_for_wave(&self, wave: u32) -> u32 {
        if self.is_campaign() {
            return 0;
        }
        let base = wave_clear_bonus(wave);
        (base as f32 * self.difficulty.reward_scale(wave))
            .round()
            .max(1.0) as u32
    }

    /// The scoreboard line shared with the rest of the room.
    pub fn snapshot(&self) -> td_proto::Snapshot {
        td_proto::Snapshot {
            wave: self.wave.min(u16::MAX as u32) as u16,
            // The scoreboard still speaks in "lives", so the flood gauge is
            // reported as headroom: how many more monsters the ring will take.
            lives: (self.flood_limit().saturating_sub(self.creeps.len())).min(i16::MAX as usize)
                as i16,
            gold: self.gold.clamp(i32::MIN as i64, i32::MAX as i64) as i32,
            net_worth: self.net_worth().clamp(0, i32::MAX as i64) as i32,
            kills: self.stats.kills.min(u32::MAX as u64) as u32,
            leaked: self.stats.leaked.min(u16::MAX as u32) as u16,
            towers: self.towers.len().min(u16::MAX as usize) as u16,
            alive: self.phase != Phase::Defeat,
            endless: self.endless,
        }
    }

    /// Clearing the campaign is a win; the player may keep going for score.
    pub fn continue_endless(&mut self) {
        if self.phase == Phase::Victory {
            self.endless = true;
            self.phase = Phase::Combat;
            self.wave_timer = WAVE_PERIOD;
        }
    }

    pub fn cycle_speed(&mut self) {
        if self.is_campaign() {
            let current = self.speed.clamp(CAMPAIGN_DEFAULT_SPEED, MAX_CAMPAIGN_SPEED);
            let at_or_after = CAMPAIGN_SPEED_STEPS
                .iter()
                .position(|&speed| current <= speed + 0.01)
                .unwrap_or(CAMPAIGN_SPEED_STEPS.len() - 1);
            self.speed = CAMPAIGN_SPEED_STEPS[(at_or_after + 1) % CAMPAIGN_SPEED_STEPS.len()];
            self.wants_save = true;
            return;
        }
        let current = self.speed.clamp(CAMPAIGN_DEFAULT_SPEED, MAX_ENDLESS_SPEED);
        let at_or_after = LEGACY_SPEED_STEPS
            .iter()
            .position(|&speed| current <= speed + 0.01)
            .unwrap_or(LEGACY_SPEED_STEPS.len() - 1);
        self.speed = LEGACY_SPEED_STEPS[(at_or_after + 1) % LEGACY_SPEED_STEPS.len()];
        self.wants_save = true;
    }

    pub fn next_wave_def(&self) -> WaveDef {
        if let Some(state) = self.campaign.as_ref() {
            return self.campaign_wave_def(state.encounter);
        }
        self.wave_def(self.wave + 1)
    }

    /// Everything this tower can become, with prices. One entry for most
    /// towers, six for the Single shot seed, three for an Aura Tower and five
    /// for the King.
    pub fn upgrade_choices(&self, ti: usize) -> Vec<(usize, u32)> {
        self.towers
            .get(ti)
            .map(|t| t.upgrades())
            .unwrap_or_default()
    }

    /// Gold in hand plus everything sunk into towers.
    pub fn net_worth(&self) -> i64 {
        self.gold + self.towers.iter().map(|t| t.invested as i64).sum::<i64>()
    }

    pub fn can_afford(&self, cost: u32) -> bool {
        self.gold >= cost as i64
    }

    /// Exact refund shown by the UI and paid by [`Game::sell`]. Classic keeps
    /// the source refund. Hard modes make placement and counter commitments
    /// matter, with a better rate after the ring has been fully cleared.
    pub fn tower_sell_value(&self, ti: usize) -> u32 {
        let Some(tower) = self.towers.get(ti) else {
            return 0;
        };
        let active = self.spawn_left > 0 || self.escort_left > 0 || !self.creeps.is_empty();
        let percent = match (self.difficulty, active) {
            (Difficulty::Classic, _) => 100,
            (Difficulty::Veteran, false) => 80,
            (Difficulty::Veteran, true) => 65,
            (Difficulty::Nightmare, false) => 70,
            (Difficulty::Nightmare, true) => 50,
        };
        tower.sell_value().saturating_mul(percent) / 100
    }

    /// The tower standing on a pad, if any.
    pub fn tower_in_slot(&self, slot: usize) -> Option<usize> {
        self.board.slots.get(slot).and_then(|s| s.tower)
    }

    pub fn toast(&mut self, msg: impl Into<String>) {
        self.toast = Some((msg.into(), 2.2, ToastTone::Info));
    }

    pub fn notice(&mut self, msg: impl Into<String>) {
        self.toast = Some((msg.into(), 2.6, ToastTone::Good));
    }

    pub fn error(&mut self, msg: impl Into<String>) {
        self.toast = Some((msg.into(), 2.2, ToastTone::Bad));
    }

    pub fn doctrine_rank(&self, doctrine: Doctrine) -> u8 {
        self.doctrines[doctrine.index()]
    }

    pub fn doctrine_picks(&self) -> u8 {
        self.doctrines.iter().copied().sum()
    }

    /// Take the milestone choice and immediately refresh every existing tower.
    pub fn choose_doctrine(&mut self, doctrine: Doctrine) {
        if !self.pending_doctrine {
            return;
        }
        self.doctrines[doctrine.index()] = self.doctrines[doctrine.index()].saturating_add(1);
        self.pending_doctrine = false;
        self.paused = false;
        self.rebuild_auras();
        self.notice(format!(
            "{} rank {}: {}",
            doctrine.label(),
            self.doctrine_rank(doctrine),
            doctrine.effect()
        ));
    }

    /// A score intended for run comparison, not combat balance.
    pub fn command_score(&self) -> u64 {
        let base = self.wave as u64 * 10_000
            + self.stats.kills.saturating_mul(10)
            + self.net_worth().max(0) as u64 / 10
            + self.stats.clean_sweeps as u64 * 2_500
            + self.stats.rush_gold;
        let scale = match self.difficulty {
            Difficulty::Classic => 1.0,
            Difficulty::Veteran => 1.35,
            Difficulty::Nightmare => 1.80,
        };
        (base as f64 * scale) as u64
    }

    pub fn command_rating(&self) -> &'static str {
        let progress = self.wave.min(CAMPAIGN_WAVES) as f32 / CAMPAIGN_WAVES as f32;
        let pressure = self.stats.peak_circling as f32 / self.flood_limit() as f32;
        if self.phase == Phase::Victory && pressure <= 0.35 {
            "S"
        } else if self.phase == Phase::Victory && pressure <= 0.58 {
            "A"
        } else if self.phase == Phase::Victory {
            "B"
        } else if progress >= 0.75 {
            "C"
        } else if progress >= 0.45 {
            "D"
        } else {
            "F"
        }
    }

    // ------------------------------------------------ player actions

    pub fn tower_at(&self, p: [f32; 2]) -> Option<usize> {
        let select_r = TOWER_FOOTPRINT_RADIUS + 0.20;
        let max_d2 = select_r * select_r;
        self.towers
            .iter()
            .enumerate()
            .filter_map(|(i, tower)| {
                let dx = tower.pos[0] - p[0];
                let dy = tower.pos[1] - p[1];
                let d2 = dx * dx + dy * dy;
                (d2 <= max_d2).then_some((i, d2))
            })
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(i, _)| i)
    }

    pub fn buildability_at(
        &self,
        p: [f32; 2],
        include_gold: bool,
    ) -> Result<[f32; 2], PlacementIssue> {
        let p = self.board.quantize_build_pos(p);
        if let Some(block) = self.board.surface_block(p, TOWER_FOOTPRINT_RADIUS) {
            return Err(block.into());
        }
        let separation = TOWER_FOOTPRINT_RADIUS * 2.0 + TOWER_CLEARANCE;
        if self.towers.iter().any(|tower| {
            let dx = tower.pos[0] - p[0];
            let dy = tower.pos[1] - p[1];
            dx * dx + dy * dy < separation * separation
        }) {
            return Err(PlacementIssue::TowerOverlap);
        }
        if include_gold
            && self
                .build_choice
                .and_then(|(def, _)| TOWERS.get(def))
                .is_some_and(|level| !self.can_afford(level.gold))
        {
            return Err(PlacementIssue::NotEnoughGold);
        }
        Ok(p)
    }

    pub fn first_clear_grass(&self) -> Option<[f32; 2]> {
        let b = self.board.build_world();
        let mut y = b[1] + TOWER_FOOTPRINT_RADIUS;
        while y <= b[3] - TOWER_FOOTPRINT_RADIUS {
            let mut x = b[0] + TOWER_FOOTPRINT_RADIUS;
            while x <= b[2] - TOWER_FOOTPRINT_RADIUS {
                if let Ok(pos) = self.buildability_at([x, y], false) {
                    return Some(pos);
                }
                x += 0.50;
            }
            y += 0.50;
        }
        None
    }

    pub fn try_build(&mut self, slot: usize) -> bool {
        let Some(s) = self.board.slots.get(slot) else {
            return false;
        };
        self.try_build_internal(s.pos, Some(slot), true)
    }

    pub fn try_build_at(&mut self, pos: [f32; 2]) -> bool {
        self.try_build_internal(pos, None, true)
    }

    pub fn restore_build_at(&mut self, pos: [f32; 2], legacy_slot: Option<usize>) -> bool {
        self.try_build_internal(pos, legacy_slot, false)
    }

    fn try_build_internal(
        &mut self,
        requested_pos: [f32; 2],
        legacy_slot: Option<usize>,
        charge: bool,
    ) -> bool {
        let Some((def, _)) = self.build_choice else {
            return false;
        };
        let Some(level) = TOWERS.get(def) else {
            return false;
        };
        let pos = if legacy_slot.is_some() {
            let p = self.board.quantize_build_pos(requested_pos);
            let separation = TOWER_FOOTPRINT_RADIUS * 2.0 + TOWER_CLEARANCE;
            let overlaps = self.towers.iter().any(|tower| {
                let dx = tower.pos[0] - p[0];
                let dy = tower.pos[1] - p[1];
                dx * dx + dy * dy < separation * separation
            });
            if !p[0].is_finite() || !p[1].is_finite() || overlaps {
                self.error(if overlaps {
                    PlacementIssue::TowerOverlap.label()
                } else {
                    PlacementIssue::OutsideWorld.label()
                });
                self.sound_cues.push(Cue::Error);
                return false;
            }
            p
        } else {
            match self.buildability_at(requested_pos, charge) {
                Ok(pos) => pos,
                Err(issue) => {
                    self.error(issue.label());
                    self.sound_cues.push(Cue::Error);
                    return false;
                }
            }
        };
        let cost = level.gold;
        if charge && !self.can_afford(cost) {
            self.error("Not enough gold");
            self.sound_cues.push(Cue::Error);
            return false;
        }

        if charge {
            self.gold -= cost as i64;
            self.stats.gold_spent += cost as u64;
            self.stats.towers_built += 1;
        }
        let ti = self.towers.len();
        self.towers.push(Tower {
            def,
            slot: legacy_slot.unwrap_or(FREE_TOWER_SLOT),
            pos,
            road_dist: self.board.dist_to_road(pos),
            cooldown: 0.28,
            angle: 0.0,
            target_uid: 0,
            ramp: 0.0,
            frenzy_cd: 0.0,
            aura_timer: 0.0,
            kills: 0,
            damage: 0.0,
            invested: cost,
            mode: TargetMode::default_for(level.family),
            flash: 0.0,
            built_at: self.time,
            buff_dmg: 0.0,
            buff_rate: 0.0,
            buff_range: 0.0,
            gold_earned: 0,
        });
        if let Some(slot) = legacy_slot {
            if let Some(pad) = self.board.slots.get_mut(slot) {
                pad.tower = Some(ti);
            }
        }
        self.rebuild_auras();
        self.selected = Some(ti);
        self.wants_save = charge;
        self.sound_cues.push(Cue::Build);
        let c = level.color();
        self.fx.burst(
            &mut self.rng,
            pos,
            22,
            3.0,
            [c[0], c[1], c[2], 1.0],
            0.5,
            0.30,
        );
        true
    }

    pub fn sell(&mut self, ti: usize) {
        if ti >= self.towers.len() {
            return;
        }
        let refund = self.tower_sell_value(ti);
        let t = self.towers[ti].clone();
        self.gold += refund as i64;
        if t.slot != FREE_TOWER_SLOT {
            if let Some(slot) = self.board.slots.get_mut(t.slot) {
                slot.tower = None;
            }
        }
        self.towers.swap_remove(ti);
        if ti < self.towers.len() {
            let moved = self.towers[ti].slot;
            if moved != FREE_TOWER_SLOT {
                if let Some(slot) = self.board.slots.get_mut(moved) {
                    slot.tower = Some(ti);
                }
            }
        }
        self.repoint_projectiles(ti);
        self.rebuild_auras();
        self.selected = None;
        self.sound_cues.push(Cue::Sell);
        self.fx.burst(
            &mut self.rng,
            t.pos,
            16,
            2.5,
            [1.0, 0.85, 0.35, 1.0],
            0.5,
            0.25,
        );
    }

    /// Projectiles credit their tower by index; fix them up after a swap_remove.
    fn repoint_projectiles(&mut self, removed: usize) {
        let moved_from = self.towers.len();
        for p in &mut self.projs {
            if p.tower == removed {
                p.tower = usize::MAX;
            } else if p.tower == moved_from {
                p.tower = removed;
            }
        }
    }

    /// Buys this tower's next step, when it has exactly one.
    ///
    /// Three towers do not: the Single shot seed offers six families, an Aura
    /// Tower offers Damage, Speed or the next Aura, and the King offers four
    /// Super towers as well as its own next level. Those go through
    /// [`Game::upgrade_into`].
    pub fn upgrade(&mut self, ti: usize) {
        if ti >= self.towers.len() {
            return;
        }
        if self.towers[ti].has_choice() {
            self.error("Choose what it becomes");
            self.sound_cues.push(Cue::Error);
            return;
        }
        let Some((next, cost)) = self.towers[ti].upgrade_target() else {
            self.error("Nothing above this");
            self.sound_cues.push(Cue::Error);
            return;
        };
        if !self.can_afford(cost) {
            self.error("Not enough gold");
            self.sound_cues.push(Cue::Error);
            return;
        }
        self.pay_and_replace(ti, next, cost);
    }

    /// Takes one of the branches a forking tower offers.
    pub fn upgrade_into(&mut self, ti: usize, into: usize) {
        if ti >= self.towers.len() {
            return;
        }
        if !self.towers[ti].def().upgrades.contains(&(into as u16)) {
            return;
        }
        let Some(level) = TOWERS.get(into) else {
            return;
        };
        let cost = level.gold;
        if !self.can_afford(cost) {
            self.error("Not enough gold");
            self.sound_cues.push(Cue::Error);
            return;
        }
        self.pay_and_replace(ti, into, cost);
    }

    fn pay_and_replace(&mut self, ti: usize, into: usize, cost: u32) {
        self.gold -= cost as i64;
        self.stats.gold_spent += cost as u64;
        let t = &mut self.towers[ti];
        let old_family = t.family();
        let kept_default_mode = t.mode == TargetMode::default_for(old_family);
        t.def = into;
        t.invested += cost;
        if old_family != t.family() && kept_default_mode {
            t.mode = TargetMode::default_for(t.family());
        }
        t.flash = 1.0;
        // A rung change can move the tower onto a different attack type, so its
        // target is no longer necessarily something it can hurt.
        t.target_uid = 0;
        t.ramp = 0.0;
        t.frenzy_cd = 0.0;
        t.aura_timer = 0.0;
        let pos = t.pos;
        let c = TOWERS[into].color();
        self.rebuild_auras();
        self.sound_cues.push(Cue::Build);
        self.fx.burst(
            &mut self.rng,
            pos,
            30,
            3.6,
            [c[0], c[1], c[2], 1.0],
            0.6,
            0.34,
        );
    }

    /// Recomputes every tower's aura bonus. Only runs when the board changes.
    ///
    /// Three towers hand these out: the Damage Tower (+30/50/80% damage), the
    /// Speed Tower (+30/50/80% attack rate) and the Fire Tower, which carries a
    /// smaller damage aura on top of its own attack. The Slow Tower gives
    /// attack speed as well - the map's own tooltip says so.
    pub fn rebuild_auras(&mut self) {
        let global_damage = self.doctrine_rank(Doctrine::Arsenal) as f32 * 0.12;
        let global_rate = self.doctrine_rank(Doctrine::Overdrive) as f32 * 0.10;
        let global_range = self.doctrine_rank(Doctrine::Reach) as f32 * 0.60;
        for t in &mut self.towers {
            t.buff_dmg = global_damage;
            t.buff_rate = global_rate;
            t.buff_range = global_range;
        }
        // Collect the auras first so the loop below can stay a simple scan.
        let beacons: Vec<([f32; 2], f32, f32, f32)> = self
            .towers
            .iter()
            .filter(|t| t.abil().is_aura())
            .map(|t| {
                let a = t.abil();
                (t.pos, a.aura_range.max(t.range()), a.dmg_aura, a.speed_aura)
            })
            .collect();
        if beacons.is_empty() {
            return;
        }
        for i in 0..self.towers.len() {
            if self.towers[i].is_support() {
                continue;
            }
            let p = self.towers[i].pos;
            let mut strongest_damage = 0.0f32;
            let mut strongest_rate = 0.0f32;
            for (bp, br, dmg, rate) in &beacons {
                let d2 = (bp[0] - p[0]).powi(2) + (bp[1] - p[1]).powi(2);
                if d2 <= br * br {
                    strongest_damage = strongest_damage.max(*dmg);
                    strongest_rate = strongest_rate.max(*rate);
                }
            }
            // Warcraft-style auras of the same stat do not stack. Apart from
            // closing a 30x-DPS exploit, this makes placement about coverage
            // rather than carpeting one kill zone with identical supports.
            self.towers[i].buff_dmg += strongest_damage;
            self.towers[i].buff_rate += strongest_rate;
        }
    }

    /// Calls the next wave now, and pays for the time skipped.
    ///
    /// Rush unlocks after the current stream has entered the ring. Survivors do
    /// not go anywhere, so the next stream still stacks on top of them, but the
    /// button cannot be mashed to compress all 36 authored waves into minutes.
    pub fn send_wave(&mut self) {
        if self.is_campaign() {
            self.send_campaign_encounter();
            return;
        }
        if matches!(self.phase, Phase::Defeat | Phase::Victory) || self.pending_doctrine {
            return;
        }
        if self.wave >= self.last_wave() {
            self.toast("No more waves to call");
            return;
        }
        if self.wave > 0 && self.spawn_left > 0 {
            self.toast(format!(
                "Wave {} is deploying: {} enemies left",
                self.wave, self.spawn_left
            ));
            return;
        }
        let bonus = if self.wave == 0 || self.prep {
            0
        } else {
            (self.wave_timer * EARLY_BONUS_PER_SEC).round().max(0.0) as i64
        };
        if bonus > 0 {
            self.gold += bonus;
            self.stats.gold_earned += bonus as u64;
            self.stats.rush_gold += bonus as u64;
            let s = self.board.start();
            if self.texts.len() < MAX_FLOAT_TEXTS {
                self.texts.push(FloatText {
                    pos: [s[0] + 1.5, s[1], 1.2],
                    value: bonus as f32,
                    kind: TextKind::Gold,
                    t: 1.6,
                });
            }
            self.notice(format!("RUSH BONUS  +{bonus}g"));
        }
        self.begin_wave(true);
    }

    /// The last wave the campaign will send. Endless has no last wave.
    pub fn last_wave(&self) -> u32 {
        if self.endless {
            u32::MAX
        } else if self.is_campaign() {
            campaign::REALISTIC_ENCOUNTERS as u32
        } else {
            CAMPAIGN_WAVES
        }
    }

    /// Campaign `Send` has two legal meanings: start the prepared authored
    /// encounter, or use the one allowed normal-encounter Rush after every
    /// packet is already in the world.  It never starts a second encounter on
    /// top of one that is still resolving.
    fn send_campaign_encounter(&mut self) {
        if matches!(self.phase, Phase::Defeat | Phase::Victory) || self.pending_doctrine {
            return;
        }
        let active = self.phase == Phase::Combat;
        if active {
            let rushed = self
                .campaign
                .as_mut()
                .is_some_and(campaign::CampaignState::rush);
            if rushed {
                self.wave_timer = 0.0;
                self.notice("RUSH: recovery tail waived; all packets remain on the ring");
                self.wants_save = true;
            } else if self.spawn_left > 0 {
                self.toast(format!(
                    "Encounter {} is deploying: {} bodies in the current packet",
                    self.wave, self.spawn_left
                ));
            } else if self
                .campaign
                .as_ref()
                .is_some_and(|state| state.current().commander.is_some())
            {
                self.toast("Commander encounters cannot Rush");
            } else {
                self.toast("Rush unlocks after the final packet enters");
            }
            return;
        }
        self.begin_campaign_encounter();
    }

    fn begin_campaign_encounter(&mut self) {
        let Some(mut state) = self.campaign.take() else {
            self.error("Campaign state is unavailable");
            return;
        };
        if state.complete || state.encounter > campaign::REALISTIC_ENCOUNTERS {
            self.campaign = Some(state);
            self.error("The campaign has already concluded");
            return;
        }
        let encounter = state.current();
        self.wave = encounter.id.global as u32;
        self.prep = false;
        self.phase = Phase::Combat;
        self.wave_timer = encounter.duration_seconds as f32;
        self.spawn_left = state.queued_bodies_left as u32;
        self.escort_left = 0;
        self.spawn_timer = 0.0;
        self.escort_timer = 0.0;
        state.speed = self.speed;

        let budget = self.difficulty.campaign_encounter_budget(encounter.reward, encounter.id.chapter, encounter.id.local);
        if !state.deployment_paid {
            let deployment = budget * 40 / 100;
            state.deployment_paid = true;
            state.reward_issued = deployment;
            state.reward_paid = deployment;
            state.in_flight_budget = Some(budget);
            let gold_payout = deployment as i64;
            self.gold += gold_payout;
            self.stats.gold_earned += gold_payout as u64;
            if self.texts.len() < MAX_FLOAT_TEXTS {
                let s = self.board.start();
                self.texts.push(FloatText {
                    pos: [s[0] + 1.4, s[1], 1.1],
                    value: gold_payout as f32,
                    kind: TextKind::Gold,
                    t: 1.5,
                });
            }
        }
        let name = encounter.title.clone();
        self.notice(format!(
            "C{} · Encounter {}: {}",
            encounter.id.chapter, encounter.id.local, name
        ));
        self.sound_cues.push(if encounter.commander.is_some() {
            Cue::Boss
        } else {
            Cue::WaveStart
        });
        // Exact mid-combat saves are supported, and the start is also a clear
        // safe checkpoint before the first packet enters on the next tick.
        self.wants_save = true;
        self.campaign = Some(state);
    }

    /// Starts the next wave streaming, and pays the stipend for it.
    ///
    /// Automatic boundaries and Rush both happen only after the full stream
    /// has arrived, so neither path can delete enemies that were still queued.
    fn begin_wave(&mut self, called_early: bool) {
        // Waiting and rushing are both valid tempo choices. An early call pays
        // immediately; waiting until the clock expires pays only if the old
        // stream has been completely cleared. The two rewards make the button
        // a decision instead of a compulsory source of free money.
        let clean = !called_early
            && self.wave > 0
            && self.spawn_left == 0
            && self.escort_left == 0
            && self.creeps.is_empty();
        let clean_bonus = if clean {
            (self.bounty_for_wave(self.wave) * 4).max(20) as i64
        } else {
            0
        };
        self.wave += 1;
        self.prep = false;
        self.phase = Phase::Combat;
        // The map sets its own gap at the top of every wave's trigger: fifty
        // seconds early on, forty-five once the waves get big.
        self.wave_timer = self.wave_def(self.wave + 1).lead_in;
        let w = self.wave_def(self.wave);
        self.spawn_left = w.count;
        self.escort_left = 0;
        self.spawn_timer = 0.0;
        self.escort_timer = 0.0;
        self.pay_wave_stipend();
        if clean_bonus > 0 {
            self.gold += clean_bonus;
            self.stats.gold_earned += clean_bonus as u64;
            self.stats.clean_sweeps += 1;
            let s = self.board.start();
            if self.texts.len() < MAX_FLOAT_TEXTS {
                self.texts.push(FloatText {
                    pos: [s[0] + 1.5, s[1], 1.2],
                    value: clean_bonus as f32,
                    kind: TextKind::Gold,
                    t: 1.8,
                });
            }
            self.notice(format!("CLEAN SWEEP  +{clean_bonus}g"));
        }
        // Three pauses in a half-hour campaign create build-defining choices
        // without turning every wave into an upgrade-menu interruption.
        if self.difficulty != Difficulty::Classic
            && matches!(self.wave, 10 | 20 | 30)
            && self.doctrine_picks() < 3
        {
            self.pending_doctrine = true;
            self.paused = true;
        }
        // A wave boundary is the only moment worth checkpointing: no monster is
        // mid-spawn, so a resumed run never starts half a wave in.
        self.wants_save = true;
        self.sound_cues.push(if w.tag == "Boss" {
            Cue::Boss
        } else {
            Cue::WaveStart
        });
    }

    /// Runs one wave boundary's payout. Tests only.
    #[cfg(test)]
    pub fn end_wave_for_test(&mut self) {
        self.pay_wave_stipend();
    }

    /// The per-wave stipend, paid when a wave is called.
    ///
    /// It is paid when a wave *starts*, because on a circuit no wave ever ends.
    /// There is no interest and no income tower in this map - gold comes from
    /// kills, and this exists for one reason: a board that has fallen behind
    /// still needs the money to climb back out, or one bad wave quietly decides
    /// the whole run.
    fn pay_wave_stipend(&mut self) {
        let stipend = self.stipend_for_wave(self.wave) as i64;
        self.last_interest = 0;
        self.gold += stipend;
        self.stats.gold_earned += stipend.max(0) as u64;
    }

    /// Checks the two ways a run can end.
    pub fn check_end(&mut self) {
        if matches!(self.phase, Phase::Defeat | Phase::Victory) {
            return;
        }
        self.stats.peak_circling = self.stats.peak_circling.max(self.creeps.len() as u32);
        if self.is_campaign() {
            if self.campaign.as_ref().is_some_and(|s| s.complete) && self.creeps.is_empty() {
                self.phase = Phase::Victory;
                self.sound_cues.push(Cue::Victory);
                self.notice("The Last Circle is clear · Campaign complete");
                return;
            }
            let pressure = self.campaign_pressure();
            if pressure > self.campaign_pressure_capacity()
                && self.campaign_pressure_grace <= 0.0
            {
                self.phase = Phase::Defeat;
                self.sound_cues.push(Cue::Defeat);
                return;
            }
            // Hard-mode commanders are moving objectives with an enforced circuit limit.
            if let Some(limit) = self.difficulty.commander_lap_limit() {
                if self
                    .creeps
                    .iter()
                    .any(|creep| creep.is_boss() && creep.laps >= limit)
                {
                    self.phase = Phase::Defeat;
                    self.sound_cues.push(Cue::Defeat);
                    return;
                }
            }
            self.finish_campaign_if_ready();
            return;
        }
        if self.creeps.len() > self.flood_limit() {
            self.phase = Phase::Defeat;
            self.sound_cues.push(Cue::Defeat);
            return;
        }
        // Hard-mode commanders are objectives, not harmless residents. Their
        // visible lap limit creates a bounded single-target check even when
        // raw crowd pressure is low.
        if let Some(limit) = self.difficulty.commander_lap_limit() {
            if self
                .creeps
                .iter()
                .any(|creep| creep.is_boss() && creep.laps >= limit)
            {
                self.phase = Phase::Defeat;
                self.sound_cues.push(Cue::Defeat);
                return;
            }
        }
        // Winning means the last wave has finished arriving *and* the ring is
        // empty. Surviving the final stream is not the same as clearing it.
        if !self.endless
            && self.wave >= CAMPAIGN_WAVES
            && self.spawn_left == 0
            && self.escort_left == 0
            && self.creeps.is_empty()
        {
            self.phase = Phase::Victory;
            self.sound_cues.push(Cue::Victory);
        }
    }

    /// How full the ring is, 0 to 1. The gauge the HUD draws.
    pub fn flood(&self) -> f32 {
        self.creeps.len() as f32 / self.flood_limit() as f32
    }

    // ------------------------------------------------ update

    pub fn update(&mut self, real_dt: f32) {
        self.tick_ui(real_dt);
        if self.paused
            || self.pending_doctrine
            || matches!(self.phase, Phase::Defeat | Phase::Victory)
        {
            return;
        }
        self.campaign_auto_start(real_dt);
        // Fixed steps keep behaviour identical at any game speed or frame rate.
        const STEP: f32 = 1.0 / 120.0;
        let scaled = (real_dt.max(0.0) * self.speed).min(3.50);
        let mut left = scaled;
        let mut guard = 0;
        while left > 0.0 && guard < 448 {
            let dt = left.min(STEP);
            self.step(dt);
            left -= dt;
            guard += 1;
        }
    }

    fn campaign_auto_start(&mut self, real_dt: f32) {
        if !self.is_campaign() || self.phase != Phase::Build || self.pending_doctrine {
            return;
        }
        self.wave_timer = (self.wave_timer - real_dt.max(0.0)).max(0.0);
        if self.wave_timer <= 0.0 {
            self.begin_campaign_encounter();
        }
    }

    fn tick_ui(&mut self, dt: f32) {
        if let Some((_, t, _)) = &mut self.toast {
            *t -= dt;
            if *t <= 0.0 {
                self.toast = None;
            }
        }
        self.shake = (self.shake - dt * 3.0).max(0.0);
        self.beams.retain_mut(|b| {
            b.t -= dt * 6.0;
            b.t > 0.0
        });
        self.texts.retain_mut(|t| {
            t.t -= dt;
            t.pos[2] += dt * 0.9;
            t.t > 0.0
        });
    }

    fn step(&mut self, dt: f32) {
        self.time += dt;

        if self.is_campaign() {
            if self.phase == Phase::Combat && !self.prep {
                self.campaign_spawn_step(dt);
            }
        } else {
            if self.wave < self.last_wave() {
                self.wave_timer -= dt;
                if self.wave_timer <= 0.0 && self.spawn_left == 0 {
                    self.begin_wave(false);
                }
            }
            if !self.prep {
                self.spawn_step(dt);
            }
        }

        self.spatial.rebuild(&self.creeps);
        self.step_creeps(dt);
        combat::step_towers(self, dt);
        combat::step_projectiles(self, dt);
        if self.is_campaign() {
            if self.campaign_pressure() > self.campaign_pressure_capacity() {
                self.campaign_pressure_grace = (self.campaign_pressure_grace - dt).max(0.0);
            } else {
                let grace_cap = self.campaign_pressure_grace.max(campaign::BREACH_SECONDS);
                self.campaign_pressure_grace =
                    (self.campaign_pressure_grace + dt * 1.5).min(grace_cap);
            }
        }
        self.check_end();
    }

    fn campaign_spawn_step(&mut self, dt: f32) {
        let Some(mut state) = self.campaign.take() else {
            self.error("Campaign state was lost; play has paused safely");
            self.paused = true;
            return;
        };
        state.advance(dt);
        state.speed = self.speed;
        state.packet_spawn_timer -= dt;
        let encounter = state.current();

        let packet_window = |packet: campaign::SpawnPacket| {
            if packet.at_seconds == encounter.duration_seconds {
                if encounter.commander.is_some() { 1.4 } else { 0.9 }
            } else {
                2.6
            }
        };

        let mut safety = 0usize;
        loop {
            safety += 1;
            if safety > 2_048 {
                self.error("Campaign packet safety stop");
                break;
            }
            if state.queued_packet.is_none() && state.arm_due_packet().is_none() {
                break;
            }
            let Some(packet) = state.queued_packet() else { break };
            if state.packet_spawn_timer > 0.0 || state.queued_bodies_left == 0 {
                break;
            }
            if self.creeps.len() >= MAX_CREEPS {
                state.packet_spawn_timer = 0.25;
                self.error("Spawn queue held: simulation body safety ceiling");
                break;
            }

            let ordinal = packet.bodies.saturating_sub(state.queued_bodies_left);
            let global_body = state.spawned_bodies;
            let total_bodies: u32 = encounter.packets.iter().map(|p| p.bodies as u32).sum();
            let budget = state.in_flight_budget.unwrap_or_else(|| {
                self.difficulty.campaign_encounter_budget(encounter.reward, encounter.id.chapter, encounter.id.local)
            });
            if state.in_flight_budget.is_none() {
                state.in_flight_budget = Some(budget);
            }
            let deployment = budget * 40 / 100;
            let kill_budget = budget.saturating_sub(deployment);
            let per_body = kill_budget / total_bodies.max(1);
            let remainder = kill_budget % total_bodies.max(1);
            let bounty = per_body + u32::from(global_body < remainder);
            let boss = encounter.commander.is_some()
                && packet.at_seconds == encounter.duration_seconds
                && ordinal == 0;
            self.spawn_campaign_body(&encounter, packet, ordinal, bounty, boss);
            if boss {
                state.commander_spawned_at = Some(state.elapsed_seconds);
            }
            state.reward_issued = state.reward_issued.saturating_add(bounty);
            state.reward_paid = state.reward_paid.saturating_add(bounty);
            let gap = packet_window(packet) / packet.bodies.max(1) as f32;
            if !state.consume_queued_body(gap) {
                self.error("Campaign packet cursor was invalid");
                break;
            }
        }
        self.spawn_left = state.queued_bodies_left as u32;
        self.escort_left = 0;
        self.wave_timer = (encounter.duration_seconds as f32 - state.elapsed_seconds).max(0.0);
        self.campaign = Some(state);
    }

    fn spawn_campaign_body(
        &mut self,
        encounter: &campaign::ResolvedEncounter,
        packet: campaign::SpawnPacket,
        ordinal: u16,
        bounty: u32,
        boss: bool,
    ) {
        let trait_ = campaign_body_trait(packet, ordinal);
        let spec = campaign_body_spec(encounter.id.chapter, encounter.id.local, trait_, self.difficulty);
        let wave = WaveDef {
            name: packet.formation.name(),
            tag: if boss { "Commander" } else { "Formation" },
            model: spec.model,
            scale: spec.scale,
            count: 1,
            hp: spec.hp,
            armour: spec.armour,
            armour_type: spec.armour_type,
            speed: spec.speed,
            flying: spec.flying,
            spawn_gap: 0.0,
            lead_in: 0.0,
        };
        self.spawn_creep_ranked(&wave, wave.hp, 1.0, 0.0, false, boss);
        let Some(c) = self.creeps.last_mut() else { return };
        c.route_dir = if packet.clockwise { 1.0 } else { -1.0 };
        c.bounty = bounty;
        c.shield = spec.shield;
        c.max_shield = spec.shield;
        c.regen_per_second = spec.regen_per_second;
        c.resistant = spec.resistant;
        c.campaign_encounter = encounter.id.global;
        c.pressure = if boss { 12.0 } else { spec.pressure };
        if boss {
            let floor = campaign_commander_hp_floor(self.difficulty, encounter.id.global);
            if floor > 0.0 {
                // `spawn_creep_ranked` already applied BOSS_HP_MULT. The
                // floor is compared to that final value, never multiplied.
                c.max_hp = c.max_hp.max(floor);
                c.hp = c.max_hp;
            }
            let segment_shield = 110.0 * encounter.id.chapter as f32;
            c.shield += segment_shield;
            c.max_shield += segment_shield;
            c.armour += 3 + encounter.id.chapter as i32;
            c.resistant = true;
        }
        place(&self.board, c);
    }

    fn finish_campaign_if_ready(&mut self) {
        let Some(mut state) = self.campaign.take() else { return };
        let encounter = state.current();
        let required_living = if encounter.commander.is_some() {
            self.creeps
                .iter()
                .filter(|creep| creep.campaign_encounter == encounter.id.global)
                .count()
        } else {
            0
        };
        if !state.can_finish(required_living) {
            self.campaign = Some(state);
            return;
        }
        let expected_budget = state.in_flight_budget.unwrap_or_else(|| {
            self.difficulty.campaign_encounter_budget(encounter.reward, encounter.id.chapter, encounter.id.local)
        });
        if state.reward_issued != expected_budget && !(state.in_flight_budget == Some(encounter.reward) && state.reward_issued >= encounter.reward) {
            self.error("Campaign reward ledger is incomplete; encounter remains open");
            self.campaign = Some(state);
            return;
        }
        let Some(finished) = state.finish(required_living) else {
            self.campaign = Some(state);
            return;
        };
        self.wave = finished as u32;
        self.spawn_left = 0;
        self.escort_left = 0;
        self.wave_timer = 0.0;
        self.wants_save = true;
        if state.complete {
            if self.creeps.is_empty() {
                self.phase = Phase::Victory;
                self.sound_cues.push(Cue::Victory);
                self.notice("The Last Circle is clear · Campaign complete");
            }
            self.campaign = Some(state);
            return;
        }

        let next = state.current();
        self.phase = Phase::Build;
        self.prep = false;
        if encounter.commander.is_some() {
            self.pending_doctrine = true;
            self.paused = true;
            self.notice(format!(
                "Commander resolved · choose a campaign perk before C{}E{}",
                next.id.chapter, next.id.local
            ));
        } else {
            self.wave_timer = CAMPAIGN_AUTOSTART_SECONDS;
            self.notice(format!(
                "Encounter {} resolved · prepare C{}E{}",
                finished, next.id.chapter, next.id.local
            ));
        }
        if next.id.local == 1 {
            let scale = self.difficulty.campaign_reward_scale(next.id.chapter, 1);
            let chapter_purse = ((180 + next.id.chapter as i64 * 20) as f32 * scale).round() as i64;
            self.gold += chapter_purse;
            self.stats.gold_earned += chapter_purse as u64;
            self.notice(format!(
                "Chapter {} supplies +{}g · choose your next deployment",
                next.id.chapter, chapter_purse
            ));
        }
        self.campaign = Some(state);
    }

    pub const fn diagnostic_fixture_target(start: u16) -> Option<u16> {
        match start {
            35 => Some(37),
            60 => Some(61),
            599 => Some(600),
            _ => None,
        }
    }

    pub fn start_campaign_diagnostic_fixture(
        &mut self,
        seed: u64,
        difficulty: Difficulty,
        start: u16,
    ) -> bool {
        let Some(target) = Self::diagnostic_fixture_target(start) else {
            return false;
        };
        self.start_campaign(seed, difficulty);
        let state = self
            .campaign
            .as_mut()
            .expect("start_campaign must create CampaignState");
        state.encounter = start;
        self.wave = start.saturating_sub(1) as u32;
        self.prep = false;
        self.phase = Phase::Build;
        self.wave_timer = 0.0;
        self.speed = 100.0;
        self.campaign_pressure_grace = 1_000_000.0;
        self.diagnostic_fixture = Some(start);
        self.wants_save = false;
        self.notice(format!(
            "DIAGNOSTIC fixture E{start} -> E{target}: production transition trace at 100x"
        ));
        true
    }

    pub fn diagnostic_pilot_tick(&mut self) {
        if self.diagnostic_fixture.is_none() {
            return;
        }
        if self.pending_doctrine {
            let pick = Doctrine::ALL
                .into_iter()
                .find(|&doctrine| self.doctrine_rank(doctrine) < 3)
                .expect("campaign diagnostic exhausted all legal doctrines");
            self.choose_doctrine(pick);
            return;
        }
        if self.phase == Phase::Build {
            self.send_wave();
        }
        let alive = self.creeps.len();
        for ci in 0..alive {
            if self.creeps[ci].hp > 0.0 {
                combat::damage_creep(self, ci, 1_000_000.0, usize::MAX, false);
            }
        }
        if self
            .campaign
            .as_ref()
            .is_some_and(campaign::CampaignState::can_rush)
        {
            self.send_wave();
        }
    }

    fn spawn_step(&mut self, dt: f32) {
        if self.spawn_left == 0 {
            return;
        }
        let w = self.wave_def(self.wave);
        self.spawn_timer -= dt;
        // The map spreads every wave evenly over forty-five seconds however
        // many creeps it holds, so a wave of a hundred and sixty arrives in a
        // stream and a wave of fifteen arrives in ones.
        let gap = w.spawn_gap.max(0.04);
        while self.spawn_left > 0 && self.spawn_timer <= 0.0 {
            let order = w.count.saturating_sub(self.spawn_left) + 1;
            let elite = self
                .difficulty
                .elite_stride(self.wave)
                .is_some_and(|stride| order % stride == 0);
            let boss = w.tag == "Boss" && order == 1;
            self.spawn_creep_ranked(&w, w.hp, 1.0, 0.0, elite, boss);
            self.spawn_left -= 1;
            self.spawn_timer += gap;
        }
    }

    pub(crate) fn spawn_creep(&mut self, w: &WaveDef, hp: f32, scale: f32, at_dist: f32) {
        self.spawn_creep_ranked(w, hp, scale, at_dist, false, w.tag == "Boss");
    }

    fn spawn_creep_ranked(
        &mut self,
        w: &WaveDef,
        hp: f32,
        scale: f32,
        at_dist: f32,
        elite: bool,
        boss: bool,
    ) {
        if self.creeps.len() >= MAX_CREEPS {
            return;
        }
        let uid = self.next_uid;
        self.next_uid = self.next_uid.wrapping_add(1).max(1);
        // The reference trigger uses a 50/50 random branch at Red's first
        // junction. Keep the two streams on the left of their own direction of
        // travel, with only a little individual jitter, so they pass cleanly
        // instead of occupying one tangled centre line.
        let route_dir = if self.rng.range(0.0, 1.0) < 0.5 {
            -1.0
        } else {
            1.0
        };
        let lane = self.rng.range(-0.15, 0.15);
        let dist = at_dist - self.rng.range(0.0, 0.35);
        let rank_hp =
            hp * if elite {
                self.difficulty.elite_hp()
            } else {
                1.0
            } * if boss { BOSS_HP_MULT } else { 1.0 };
        let mut c = Creep {
            uid,
            dist,
            route_dir,
            lane,
            pos: [0.0, 0.0],
            facing: 0.0,
            hp: rank_hp,
            max_hp: rank_hp,
            base_speed: w.speed
                * if elite {
                    self.difficulty.elite_speed()
                } else {
                    1.0
                },
            armour: w.armour,
            armour_type: w.armour_type,
            model: w.model,
            flying: w.flying,
            radius: w.model.radius()
                * w.scale.clamp(0.7, 1.35)
                * scale
                * if elite { 1.16 } else { 1.0 }
                * if boss { BOSS_SCALE_MULT } else { 1.0 },
            bounty: self
                .bounty_for_wave(self.wave)
                .saturating_mul(if elite { 2 } else { 1 })
                .saturating_mul(if boss { BOSS_REWARD_MULT } else { 1 }),
            boss,
            elite,
            slow: Timed::default(),
            burn: Timed::default(),
            poison: Timed::default(),
            shred: Timed::default(),
            stun: 0.0,
            stun_dr: 0.0,
            kb_cd: 0.0,
            suppress: 0.0,
            stun_immune: 0.0,
            push_left: PUSHBACK_BUDGET,
            laps: 0,
            flash: 0.0,
            bob: self.rng.range(0.0, std::f32::consts::TAU),
            shield: 0.0,
            max_shield: 0.0,
            regen_per_second: 0.0,
            resistant: false,
            campaign_encounter: 0,
            pressure: 1.0,
            death_killer: None,
        };
        place(&self.board, &mut c);
        self.creeps.push(c);
    }

    fn step_creeps(&mut self, dt: f32) {
        let mut died: Vec<usize> = Vec::new();
        let mut boss_notices: Vec<String> = Vec::new();
        let lap_haste = self.difficulty.lap_haste();

        for i in 0..self.creeps.len() {
            {
                let c = &mut self.creeps[i];
                c.flash = (c.flash - dt * 6.0).max(0.0);
                c.bob += dt * 6.0;
                c.stun_immune = (c.stun_immune - dt).max(0.0);
                if c.stun > 0.0 {
                    c.stun -= dt;
                    // The window opens the moment the stun ends, not when the
                    // next one is attempted.
                    if c.stun <= 0.0 {
                        c.stun_immune = STUN_IMMUNE;
                    }
                } else {
                    // Resistance only bleeds off while the target is actually
                    // free to move, so chain-stunning never resets it.
                    c.stun_dr = (c.stun_dr - STUN_DR_DECAY * dt).max(0.0);
                }
                c.kb_cd = (c.kb_cd - dt).max(0.0);
                c.slow.tick(dt);
                c.shred.tick(dt);
                if c.burn.active() {
                    c.hp -= c.burn.amt * dt;
                    c.burn.tick(dt);
                }
                if c.poison.active() {
                    c.hp -= c.poison.amt * dt;
                    c.poison.tick(dt);
                }
                c.suppress = (c.suppress - dt).max(0.0);
                if c.hp <= 0.0 {
                    died.push(i);
                    continue;
                }
                c.dist += c.speed() * dt;
            }
            // Round the circuit rather than off the end of it. Nothing leaks,
            // so the only bookkeeping a lap needs is a counter.
            let total = self.board.total;
            let c = &mut self.creeps[i];
            if total > 0.0 && c.dist >= total {
                // Usually this is one lap, but a restored/catch-up frame can
                // cross more than one. Fold every completed circuit at once
                // so distance never escapes the board sampler.
                let completed = (c.dist / total).floor() as u32;
                c.dist %= total;
                let old_laps = c.laps;
                c.laps = c.laps.saturating_add(completed);
                // The first four laps make a survivor visibly and mechanically
                // urgent. Reward is fixed at spawn: failing to kill something
                // on its first circuit must never mint gold.
                let haste_laps = c.laps.min(4).saturating_sub(old_laps.min(4));
                for _ in 0..haste_laps {
                    if lap_haste > 0.0 {
                        c.base_speed *= 1.0 + lap_haste;
                    }
                }
                if c.is_boss() && completed > 0 {
                    c.pressure += 6.0 * completed as f32;
                    if let Some(limit) = self.difficulty.commander_lap_limit() {
                        let left = limit.saturating_sub(c.laps);
                        if left == 1 {
                            boss_notices.push(format!("Commander lap {}: breach imminent in 1 lap!", c.laps));
                        } else if left > 1 {
                            boss_notices.push(format!("Commander completed lap {}: {} lap(s) before breach!", c.laps, left));
                        }
                    }
                }
                // A pushback budget that never refreshed meant a monster on its
                // fifth lap could not be slowed by a Thornwall at all. One lap
                // survived is worth a fresh shove.
                c.push_left = PUSHBACK_BUDGET;
            }
            place(&self.board, &mut self.creeps[i]);
        }
        for msg in boss_notices {
            self.notice(msg);
        }

        self.step_campaign_commander_mechanics(dt);

        // A boss is a moving objective, not merely a large health pool. Its
        // nearby escort slowly repairs while the commander lives, which asks
        // the player to focus the boss or bring Corruption to suppress the
        // repair. Only commanders are collected, so this remains linear in the
        // number of creeps even on a full ring.
        let menders: Vec<[f32; 2]> = self
            .creeps
            .iter()
            .filter(|c| {
                c.is_boss()
                    && c.hp > 0.0
                    && campaign::commander_for_encounter(c.campaign_encounter)
                        != Some(campaign::CommanderClass::SiphonMarshal)
            })
            .map(|c| c.pos)
            .collect();
        if !menders.is_empty() {
            let range2 = BOSS_MENDER_RANGE * BOSS_MENDER_RANGE;
            for c in &mut self.creeps {
                if c.is_boss() || c.hp <= 0.0 || c.suppress > 0.0 || c.hp >= c.max_hp {
                    continue;
                }
                if menders.iter().any(|p| {
                    let dx = c.pos[0] - p[0];
                    let dy = c.pos[1] - p[1];
                    dx * dx + dy * dy <= range2
                }) {
                    c.hp = (c.hp + c.max_hp * BOSS_MENDER_PER_SEC * dt).min(c.max_hp);
                }
            }
        }

        // The Siphon Marshal does the inverse: it consumes nearby *living*
        // escorts to heal itself. Killing escorts (or suppressing the Marshal)
        // cleanly denies the heal; the drain never awards gold or creates a
        // replacement body.
        let siphons: Vec<usize> = self.creeps.iter().enumerate().filter_map(|(i, c)| {
            (c.is_boss()
                && c.hp > 0.0
                && c.suppress <= 0.0
                && campaign::commander_for_encounter(c.campaign_encounter)
                    == Some(campaign::CommanderClass::SiphonMarshal))
                .then_some(i)
        }).collect();
        for boss_index in siphons {
            let boss = self.creeps[boss_index].clone();
            let range2 = BOSS_MENDER_RANGE * BOSS_MENDER_RANGE;
            let escorts: Vec<usize> = self.creeps.iter().enumerate().filter_map(|(i, c)| {
                if i == boss_index || c.is_boss() || c.hp <= 0.0
                    || c.campaign_encounter != boss.campaign_encounter { return None; }
                let dx = c.pos[0] - boss.pos[0];
                let dy = c.pos[1] - boss.pos[1];
                (dx * dx + dy * dy <= range2).then_some(i)
            }).take(8).collect();
            let mut drained = 0.0;
            for i in escorts {
                let amount = (self.creeps[i].hp * 0.025 * dt).min(4.0 * dt);
                self.creeps[i].hp -= amount;
                drained += amount;
            }
            if drained > 0.0 {
                let c = &mut self.creeps[boss_index];
                c.hp = (c.hp + drained * 1.35).min(c.max_hp);
            }
        }

        // Remove back-to-front so swap_remove never invalidates a pending index.
        died.sort_unstable();
        died.dedup();
        for &i in died.iter().rev() {
            let c = self.creeps[i].clone();
            self.on_creep_died(&c, None);
            self.creeps.swap_remove(i);
        }
    }

    /// Resolve the authored commander behaviors from persistent encounter
    /// state. The only random-looking result is ordinary creep lane jitter,
    /// which already uses the saved run RNG; trigger decisions themselves are
    /// deterministic and every spawned brood body has zero bounty.
    fn step_campaign_commander_mechanics(&mut self, dt: f32) {
        let Some(state) = self.campaign.as_ref() else { return };
        let encounter = state.current();
        let Some(class) = encounter.commander else { return };
        let elapsed = state.elapsed_seconds;
        // Old saves did not record the birth moment. Treat their commander as
        // newborn rather than retroactively firing pulses before the player
        // sees it; new saves retain the exact delayed-queue timestamp.
        let age = (elapsed - state.commander_spawned_at.unwrap_or(elapsed)).max(0.0);
        let mut triggers = state.commander_triggers;
        let mut window_at = state.commander_window_at;
        let Some(boss_index) = self.creeps.iter().position(|c| {
            c.is_boss() && c.campaign_encounter == encounter.id.global && c.hp > 0.0
        }) else { return };
        let boss = self.creeps[boss_index].clone();
        let mut notices = Vec::new();
        let mut brood_batches = 0u8;

        match class {
            campaign::CommanderClass::Bulwark => {
                if boss.shield <= 0.0 && triggers & 1 == 0 {
                    triggers |= 1;
                    window_at = elapsed;
                    self.creeps[boss_index].armour = 0;
                    notices.push("Bulwark plates broken: focus the exposed core!".to_owned());
                } else if triggers & 1 != 0 && elapsed - window_at >= 6.0 {
                    triggers &= !1;
                    let c = &mut self.creeps[boss_index];
                    c.shield = c.max_shield * 0.55;
                    c.armour = 3 + encounter.id.chapter as i32;
                    notices.push("Bulwark plates reforming.".to_owned());
                }
            }
            campaign::CommanderClass::HuntCaptain => {
                let due = ((age / 22.0).floor() as u8).min(3);
                let fired = triggers & 0x0f;
                if due > fired {
                    triggers = (triggers & !0x0f) | due;
                    window_at = elapsed;
                    let r2 = 7.0 * 7.0;
                    for escort in &mut self.creeps {
                        if escort.is_boss() || escort.campaign_encounter != encounter.id.global { continue; }
                        let dx = escort.pos[0] - boss.pos[0];
                        let dy = escort.pos[1] - boss.pos[1];
                        if dx * dx + dy * dy <= r2 { escort.base_speed *= 1.10; }
                    }
                    notices.push("Hunt Captain pulse: escorts accelerate—control the flank.".to_owned());
                }
            }
            campaign::CommanderClass::BroodKeeper | campaign::CommanderClass::Signature => {
                // Two bounded, one-shot health thresholds. The mask makes an
                // exact reload before/after either threshold converge.
                for (bit, threshold) in [(1u8, 0.66f32), (2u8, 0.33f32)] {
                    if boss.hp_frac() <= threshold && triggers & (1 << bit) == 0 {
                        triggers |= 1 << bit;
                        brood_batches += 1;
                    }
                }
                if brood_batches > 0 {
                    notices.push("Brood Keeper splits: clear the zero-bounty brood.".to_owned());
                }
                // A chapter Signature combines a proven brood threshold with
                // the Ward's readable shield window below.
                if class == campaign::CommanderClass::Signature {
                    let cycle = (age / 16.0).floor();
                    if cycle > (window_at / 16.0).floor() {
                        window_at = age;
                        let c = &mut self.creeps[boss_index];
                        c.shield = c.max_shield;
                        c.armour = 3 + encounter.id.chapter as i32;
                    }
                    if age.rem_euclid(16.0) >= 7.0 { self.creeps[boss_index].shield = 0.0; }
                }
            }
            campaign::CommanderClass::WardKeeper => {
                let cycle = (age / 16.0).floor();
                if cycle > (window_at / 16.0).floor() {
                    window_at = age;
                    let c = &mut self.creeps[boss_index];
                    c.shield = c.max_shield;
                    c.armour = 3 + encounter.id.chapter as i32;
                    notices.push("Ward raised: prepare burst for its open window.".to_owned());
                }
                if age.rem_euclid(16.0) >= 7.0 {
                    self.creeps[boss_index].shield = 0.0;
                }
            }
            campaign::CommanderClass::SiphonMarshal => {}
        }
        if let Some(state) = self.campaign.as_mut() {
            state.commander_triggers = triggers;
            state.commander_window_at = window_at;
        }
        for _ in 0..brood_batches {
            self.spawn_commander_brood(&encounter, &boss, 6);
        }
        for notice in notices { self.notice(notice); }
        let _ = dt; // keeps the phase contract explicit for future timed cues.
    }

    fn spawn_commander_brood(
        &mut self,
        encounter: &campaign::ResolvedEncounter,
        boss: &Creep,
        count: u8,
    ) {
        let spec = campaign_body_spec(
            encounter.id.chapter,
            encounter.id.local,
            Some(campaign::ResolvedTrait::Swarm),
            self.difficulty,
        );
        let wave = WaveDef {
            name: "Brood", tag: "Brood", model: spec.model, scale: spec.scale,
            count: 1, hp: spec.hp, armour: spec.armour, armour_type: spec.armour_type,
            speed: spec.speed, flying: false, spawn_gap: 0.0, lead_in: 0.0,
        };
        let available = MAX_CREEPS.saturating_sub(self.creeps.len());
        for ordinal in 0..usize::from(count).min(available) {
            let before = self.creeps.len();
            self.spawn_creep_ranked(
                &wave, wave.hp, 0.85, (boss.dist - ordinal as f32 * 0.18).max(0.0), false, false,
            );
            if self.creeps.len() == before { break; }
            let Some(c) = self.creeps.last_mut() else { break };
            c.route_dir = boss.route_dir;
            c.campaign_encounter = encounter.id.global;
            c.pressure = spec.pressure;
            c.bounty = 0;
            place(&self.board, c);
        }
    }

    pub(crate) fn on_creep_died(&mut self, c: &Creep, killer: Option<usize>) {
        self.stats.kills += 1;
        let bounty = c.bounty as i64;
        if let Some(ti) = killer {
            if ti < self.towers.len() {
                self.towers[ti].kills += 1;
                self.towers[ti].gold_earned += bounty.max(0) as u64;
            }
        }
        self.gold += bounty;
        self.stats.gold_earned += bounty.max(0) as u64;

        let col = c.armour_type.color();
        let big = c.is_boss();
        let mechanical = matches!(
            c.model,
            Model::Gyrocopter
                | Model::Turret
                | Model::Turbolazer
                | Model::RebelTurret
                | Model::Vulcan
                | Model::SamSite
                | Model::Cannon
                | Model::MeatWagon
                | Model::Ship
                | Model::Golem
        );
        let spectral = matches!(
            c.model,
            Model::Wraith
                | Model::Wisp
                | Model::Phoenix
                | Model::FrostWyrm
                | Model::Infernal
                | Model::FlameLord
        );
        let n = if big {
            42
        } else if c.elite {
            16
        } else {
            8
        };
        let spread = if big { 4.5 } else { 2.4 };
        self.fx.burst_styled_at(
            &mut self.rng,
            [c.pos[0], c.pos[1], c.height()],
            n,
            spread,
            [col[0], col[1], col[2], 1.0],
            if big { 0.55 } else { 0.34 },
            [c.radius * if big { 0.72 } else { 0.44 }, c.radius * 0.05],
            if mechanical {
                ParticleStyle::Shard
            } else if spectral {
                ParticleStyle::Magic
            } else {
                ParticleStyle::Spark
            },
        );
        // The secondary layer carries material: machinery fractures and
        // smokes, spirits unravel into light, living creatures kick up a small
        // dust/mist body instead of simply blinking out.
        if mechanical {
            self.fx.burst_styled_at(
                &mut self.rng,
                [c.pos[0], c.pos[1], c.height() * 0.72],
                if big { 12 } else { 4 },
                if big { 2.0 } else { 0.8 },
                [0.30, 0.28, 0.26, 0.34],
                if big { 1.15 } else { 0.65 },
                [c.radius * 0.30, c.radius * 1.35],
                ParticleStyle::Smoke,
            );
        } else if spectral {
            self.fx.ring(
                [c.pos[0], c.pos[1], c.height()],
                if big { 0.68 } else { 0.38 },
                [c.radius * 0.20, c.radius * if big { 3.8 } else { 2.2 }],
                [col[0], col[1], col[2], 0.72],
            );
        } else {
            self.fx.burst_styled_at(
                &mut self.rng,
                [c.pos[0], c.pos[1], c.height() * 0.55],
                if big { 8 } else { 2 },
                if big { 1.2 } else { 0.45 },
                [0.26, 0.23, 0.20, 0.20],
                if big { 0.95 } else { 0.52 },
                [c.radius * 0.25, c.radius * 1.05],
                ParticleStyle::Smoke,
            );
        }
        if big || c.elite {
            self.fx.ring(
                [c.pos[0], c.pos[1], c.height() * 0.72],
                if big { 0.76 } else { 0.38 },
                [c.radius * 0.24, c.radius * if big { 4.8 } else { 2.5 }],
                [col[0], col[1], col[2], if big { 0.92 } else { 0.58 }],
            );
        }
        // Collapse a whole fixed-step kill storm into one voice. If the frame
        // also contains a boss, promote the queued cue so its death cannot be
        // hidden by a smaller creature that happened to resolve first.
        if let Some(cue) = self
            .sound_cues
            .iter_mut()
            .find(|cue| matches!(cue, Cue::MonsterDeath { .. }))
        {
            if big {
                *cue = Cue::MonsterDeath {
                    boss: true,
                    mechanical,
                };
            }
        } else {
            self.sound_cues.push(Cue::MonsterDeath {
                boss: big,
                mechanical,
            });
        }
        if big {
            self.shake = 1.0;
            if self.beams.len() < MAX_BEAMS {
                self.beams.push(Beam {
                    from: [c.pos[0], c.pos[1], 0.18],
                    to: [c.pos[0] + c.radius * 5.2, c.pos[1], 0.18],
                    color: col,
                    t: 1.0,
                    width: 0.0,
                });
            }
        }
    }
}

/// Refreshes a creep's world position and facing from its distance along the road.
#[inline]
fn place(board: &Board, c: &mut Creep) {
    let p = board.sample_travel(c.dist, c.route_dir);
    let h = board.heading_travel(c.dist, c.route_dir);
    c.facing = h[1].atan2(h[0]);
    // Left-hand traffic naturally places opposite directions on opposite sides
    // of the corridor. Flyers follow the same authored route; they only widen
    // the formation and rise above ground weapons.
    let lane = 0.22 + if c.flying { c.lane * 1.35 } else { c.lane };
    c.pos = [p[0] - h[1] * lane, p[1] + h[0] * lane];
}
