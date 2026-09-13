//! Standalone Campaign rules and encounter authoring.
//!
//! The extracted Legacy tables remain in [`super::defs`]. This module contains
//! the Campaign vocabulary, encounter timelines and progression rules. See
//! `docs/realistic-campaign/IMPLEMENTATION_STATUS.md` for the current release.

#![allow(dead_code)]

pub const CAMPAIGN_ENCOUNTERS: u8 = 28;
pub const ENCOUNTERS_PER_ACT: u8 = 7;
pub const STARTING_GOLD: u16 = 600;
pub const PRESSURE_WARNING: f32 = 80.0;
/// Continuous formations intentionally overlap. Give the player a visible
/// recovery window when pressure crosses the line rather than letting a single
/// 10x render beat turn an exciting horde into an instant defeat.
pub const BREACH_SECONDS: f32 = 8.0;
pub const COMMAND_INCOME_PERCENT: u16 = 45;
pub const KILL_INCOME_PERCENT: u16 = 55;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CampaignDifficulty {
    Apprentice,
    Veteran,
    Nightmare,
}

impl CampaignDifficulty {
    pub const ALL: [Self; 3] = [Self::Apprentice, Self::Veteran, Self::Nightmare];

    pub const fn pressure_capacity(self) -> f32 {
        match self {
            Self::Apprentice => 120.0,
            Self::Veteran => 100.0,
            Self::Nightmare => 85.0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TowerJob {
    Hunter,
    Bombard,
    Repeater,
    Warden,
    Hexer,
    Skyguard,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Specialization {
    Executioner,
    Destroyer,
    Siege,
    Pyre,
    Volley,
    Ricochet,
    Frost,
    Thornwall,
    Venom,
    Corruption,
    Flak,
    Lancer,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TargetLayer {
    Ground,
    Air,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TowerJobDef {
    pub job: TowerJob,
    pub base_cost: u16,
    pub armed_cost: u16,
    pub specializations: [Specialization; 2],
    pub specialization_costs: [u16; 2],
    pub apex_costs: [u16; 2],
    /// Percent effectiveness against ground and air respectively.
    pub target_efficiency: [u8; 2],
}

impl TowerJobDef {
    pub const fn efficiency(self, layer: TargetLayer) -> u8 {
        match layer {
            TargetLayer::Ground => self.target_efficiency[0],
            TargetLayer::Air => self.target_efficiency[1],
        }
    }

    pub const fn full_path_cost(self, branch: usize) -> u16 {
        self.base_cost
            + self.armed_cost
            + self.specialization_costs[branch]
            + self.apex_costs[branch]
    }
}

pub const TOWER_JOBS: [TowerJobDef; 6] = [
    TowerJobDef {
        job: TowerJob::Hunter,
        base_cost: 180,
        armed_cost: 280,
        specializations: [Specialization::Executioner, Specialization::Destroyer],
        specialization_costs: [520, 560],
        apex_costs: [930, 960],
        target_efficiency: [100, 100],
    },
    TowerJobDef {
        job: TowerJob::Bombard,
        base_cost: 220,
        armed_cost: 320,
        specializations: [Specialization::Siege, Specialization::Pyre],
        specialization_costs: [560, 600],
        apex_costs: [1_020, 1_060],
        target_efficiency: [100, 0],
    },
    TowerJobDef {
        job: TowerJob::Repeater,
        base_cost: 190,
        armed_cost: 280,
        specializations: [Specialization::Volley, Specialization::Ricochet],
        specialization_costs: [500, 540],
        apex_costs: [900, 940],
        target_efficiency: [100, 65],
    },
    TowerJobDef {
        job: TowerJob::Warden,
        base_cost: 170,
        armed_cost: 260,
        specializations: [Specialization::Frost, Specialization::Thornwall],
        specialization_costs: [480, 520],
        apex_costs: [850, 900],
        target_efficiency: [100, 100],
    },
    TowerJobDef {
        job: TowerJob::Hexer,
        base_cost: 210,
        armed_cost: 300,
        specializations: [Specialization::Venom, Specialization::Corruption],
        specialization_costs: [520, 560],
        apex_costs: [940, 980],
        target_efficiency: [100, 100],
    },
    TowerJobDef {
        job: TowerJob::Skyguard,
        base_cost: 190,
        armed_cost: 280,
        specializations: [Specialization::Flak, Specialization::Lancer],
        specialization_costs: [500, 560],
        apex_costs: [920, 980],
        target_efficiency: [35, 100],
    },
];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ThreatTrait {
    Swarm,
    Armoured,
    Swift,
    Regenerator,
    Flying,
    Veiled,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EnemyRank {
    Swarm,
    Standard,
    Vanguard,
    Commander,
}

impl EnemyRank {
    pub const fn pressure_weight(self) -> f32 {
        match self {
            Self::Swarm => 0.35,
            Self::Standard => 1.0,
            Self::Vanguard => 2.5,
            Self::Commander => 15.0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DirectionPattern {
    Clockwise,
    CounterClockwise,
    Alternating,
    Simultaneous,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SpawnGroupDef {
    /// Share of the encounter's pressure budget, not raw unit count.
    pub pressure_percent: u8,
    pub rank: EnemyRank,
    pub traits: [Option<ThreatTrait>; 2],
    pub direction: DirectionPattern,
}

impl SpawnGroupDef {
    pub const fn trait_count(self) -> u8 {
        self.traits[0].is_some() as u8 + self.traits[1].is_some() as u8
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BossMechanic {
    ArmourAura,
    BroodThresholds,
    VeilWindows,
    LinkedLieutenants,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EncounterDef {
    pub number: u8,
    pub name: &'static str,
    pub groups: &'static [SpawnGroupDef],
    pub deployment_seconds: f32,
    pub boundary_seconds: f32,
    pub boss: Option<BossMechanic>,
}

const E1: &[SpawnGroupDef] = &[
    SpawnGroupDef {
        pressure_percent: 70,
        rank: EnemyRank::Standard,
        traits: [None, None],
        direction: DirectionPattern::Alternating,
    },
    SpawnGroupDef {
        pressure_percent: 30,
        rank: EnemyRank::Swarm,
        traits: [Some(ThreatTrait::Swarm), None],
        direction: DirectionPattern::Alternating,
    },
];

const E2: &[SpawnGroupDef] = &[SpawnGroupDef {
    pressure_percent: 100,
    rank: EnemyRank::Swarm,
    traits: [Some(ThreatTrait::Swarm), None],
    direction: DirectionPattern::Simultaneous,
}];

const E3: &[SpawnGroupDef] = &[
    SpawnGroupDef {
        pressure_percent: 65,
        rank: EnemyRank::Standard,
        traits: [None, None],
        direction: DirectionPattern::Alternating,
    },
    SpawnGroupDef {
        pressure_percent: 35,
        rank: EnemyRank::Vanguard,
        traits: [Some(ThreatTrait::Armoured), None],
        direction: DirectionPattern::Clockwise,
    },
];

const E4: &[SpawnGroupDef] = &[SpawnGroupDef {
    pressure_percent: 100,
    rank: EnemyRank::Standard,
    traits: [Some(ThreatTrait::Swift), None],
    direction: DirectionPattern::Alternating,
}];

const E5: &[SpawnGroupDef] = &[
    SpawnGroupDef {
        pressure_percent: 60,
        rank: EnemyRank::Swarm,
        traits: [Some(ThreatTrait::Swarm), Some(ThreatTrait::Armoured)],
        direction: DirectionPattern::Simultaneous,
    },
    SpawnGroupDef {
        pressure_percent: 40,
        rank: EnemyRank::Standard,
        traits: [Some(ThreatTrait::Armoured), None],
        direction: DirectionPattern::Alternating,
    },
];

const E6: &[SpawnGroupDef] = &[
    SpawnGroupDef {
        pressure_percent: 70,
        rank: EnemyRank::Standard,
        traits: [None, None],
        direction: DirectionPattern::Alternating,
    },
    SpawnGroupDef {
        pressure_percent: 30,
        rank: EnemyRank::Standard,
        traits: [Some(ThreatTrait::Flying), None],
        direction: DirectionPattern::Simultaneous,
    },
];

const E7: &[SpawnGroupDef] = &[
    SpawnGroupDef {
        pressure_percent: 55,
        rank: EnemyRank::Vanguard,
        traits: [Some(ThreatTrait::Armoured), None],
        direction: DirectionPattern::Simultaneous,
    },
    SpawnGroupDef {
        pressure_percent: 45,
        rank: EnemyRank::Commander,
        traits: [Some(ThreatTrait::Armoured), None],
        direction: DirectionPattern::Clockwise,
    },
];

pub const ACT_ONE: [EncounterDef; ENCOUNTERS_PER_ACT as usize] = [
    EncounterDef {
        number: 1,
        name: "Broken Formation",
        groups: E1,
        deployment_seconds: 30.0,
        boundary_seconds: 50.0,
        boss: None,
    },
    EncounterDef {
        number: 2,
        name: "Two-Way Swarm",
        groups: E2,
        deployment_seconds: 28.0,
        boundary_seconds: 48.0,
        boss: None,
    },
    EncounterDef {
        number: 3,
        name: "Plated Convoy",
        groups: E3,
        deployment_seconds: 31.0,
        boundary_seconds: 50.0,
        boss: None,
    },
    EncounterDef {
        number: 4,
        name: "Gale Runners",
        groups: E4,
        deployment_seconds: 28.0,
        boundary_seconds: 48.0,
        boss: None,
    },
    EncounterDef {
        number: 5,
        name: "Iron Tide",
        groups: E5,
        deployment_seconds: 34.0,
        boundary_seconds: 50.0,
        boss: None,
    },
    EncounterDef {
        number: 6,
        name: "First Flight",
        groups: E6,
        deployment_seconds: 32.0,
        boundary_seconds: 50.0,
        boss: None,
    },
    EncounterDef {
        number: 7,
        name: "Iron Warden",
        groups: E7,
        deployment_seconds: 34.0,
        boundary_seconds: 74.0,
        boss: Some(BossMechanic::ArmourAura),
    },
];

pub const fn act_of(encounter: u8) -> u8 {
    encounter.saturating_sub(1) / ENCOUNTERS_PER_ACT
}

const fn clamp_encounter(encounter: u8) -> u8 {
    if encounter < 1 {
        1
    } else if encounter > CAMPAIGN_ENCOUNTERS {
        CAMPAIGN_ENCOUNTERS
    } else {
        encounter
    }
}

pub const fn encounter_gold(encounter: u8) -> u16 {
    let e = clamp_encounter(encounter);
    let raw = 120 + 9 * e as u16 + 30 * act_of(e) as u16;
    ((raw + 2) / 5) * 5
}

pub const fn gold_split(encounter: u8) -> [u16; 2] {
    let total = encounter_gold(encounter);
    let command = (total * COMMAND_INCOME_PERCENT + 50) / 100;
    [command, total - command]
}

pub fn base_health_multiplier(encounter: u8) -> f32 {
    1.135_f32.powi(encounter.max(1) as i32 - 1)
}

pub fn pressure_budget(encounter: u8) -> f32 {
    let e = encounter.max(1).min(CAMPAIGN_ENCOUNTERS);
    24.0 + 1.5 * e as f32 + 6.0 * act_of(e) as f32
}

pub fn unit_pressure(rank: EnemyRank, trait_count: u8, completed_laps: u8) -> f32 {
    let extra_trait = if trait_count >= 2 { 1.25 } else { 1.0 };
    let lap_multiplier = 1.0 + 0.20 * completed_laps.min(3) as f32;
    rank.pressure_weight() * extra_trait * lap_multiplier
}

// ---------------------------------------------------------------------------
// Realistic Campaign v2
//
// The first campaign table above is retained only for compatibility with a
// short-lived pre-release UI.  New play must use this authored schedule.  It
// deliberately keeps its state independent of `Game`: Legacy can continue to
// use extracted Warcraft waves and its old saves without being reinterpreted.

pub const REALISTIC_CHAPTERS: u16 = 10;
pub const ENCOUNTERS_PER_CHAPTER: u16 = 60;
pub const REALISTIC_ENCOUNTERS: u16 = REALISTIC_CHAPTERS * ENCOUNTERS_PER_CHAPTER;
pub const NORMAL_DEPLOYMENT_SECONDS: u16 = 54;
/// A normal formation hands directly into the next one as soon as its last
/// authored packet has physically entered the board. There is no recovery
/// tail that makes a player wait for a clear: surviving monsters remain live
/// and the next formation overlaps them.
pub const NORMAL_BOUNDARY_SECONDS: u16 = NORMAL_DEPLOYMENT_SECONDS;
pub const COMMANDER_SECONDS: u16 = 120;
pub const FASTEST_ACTIVE_SECONDS: u32 = 36_360;
pub const SCHEDULED_ACTIVE_SECONDS: u32 = FASTEST_ACTIVE_SECONDS;

/// The complete trait vocabulary used by the authored manifest.  This is
/// intentionally separate from old `ThreatTrait`, whose values are serialized
/// by the staged prototype.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum ResolvedTrait {
    Swarm, Armoured, Swift, Flying, Shielded, Regenerator, Resistant,
}

impl ResolvedTrait {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Swarm => "Swarm",
            Self::Armoured => "Armoured",
            Self::Swift => "Swift",
            Self::Flying => "Flying",
            Self::Shielded => "Shielded",
            Self::Regenerator => "Regenerator",
            Self::Resistant => "Resistant",
        }
    }
}

pub fn trait_threat_advice(t: ResolvedTrait) -> (&'static str, &'static str) {
    match t {
        ResolvedTrait::Swarm => (
            "Low-HP dense horde",
            "Multi and Siege splash shred unarmoured swarms",
        ),
        ResolvedTrait::Armoured => (
            "Heavy plates deflect light pellets (0.60x)",
            "Deploy Single, Siege, Chaos, or Corruption (armour pen)",
        ),
        ResolvedTrait::Shielded => (
            "Active energy barrier absorbs spread fire (0.65x)",
            "Disrupt with Chaos/Corruption (+20%) or burst; after shield breaks, medium core armour still resists Multi (0.60x)",
        ),
        ResolvedTrait::Flying => (
            "Airborne bodies bypass ground-only towers",
            "Deploy dedicated Air towers (+25%) or Both-targeting weapons",
        ),
        ResolvedTrait::Regenerator => (
            "Continuous health repair (Medium armour resists Multi 0.60x)",
            "Corruption suppresses healing for 2s (per-hit pen); poison/burst overwhelms",
        ),
        ResolvedTrait::Resistant => (
            "Control resistance & dense heavy armour",
            "Pierce with unresisted Chaos, stacking Poison, and focused damage",
        ),
        ResolvedTrait::Swift => (
            "Fast runners sprint past short coverage",
            "Deploy Slow auras, Frost freeze, and deep crossfire coverage",
        ),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[repr(u8)]
pub enum Formation {
    SplitScouts, BroodTide, IronConvoy, RunningFlank, BrokenColumn, Reversal,
    WingEscort, PatientShields, LastSurge, HealingCaravan, NeedleFlight,
    TwoFronts, ThinScreen, RecoveringPack, RelentlessMarch, Crosswind,
    ProtectedRear, CombinedRehearsal,
}

impl Formation {
    pub const ALL: [Self; 18] = [
        Self::SplitScouts, Self::BroodTide, Self::IronConvoy, Self::RunningFlank,
        Self::BrokenColumn, Self::Reversal, Self::WingEscort, Self::PatientShields,
        Self::LastSurge, Self::HealingCaravan, Self::NeedleFlight, Self::TwoFronts,
        Self::ThinScreen, Self::RecoveringPack, Self::RelentlessMarch, Self::Crosswind,
        Self::ProtectedRear, Self::CombinedRehearsal,
    ];
    pub const fn name(self) -> &'static str { match self {
        Self::SplitScouts => "Split scouts", Self::BroodTide => "Brood tide",
        Self::IronConvoy => "Iron convoy", Self::RunningFlank => "Running flank",
        Self::BrokenColumn => "Broken column", Self::Reversal => "Reversal",
        Self::WingEscort => "Wing escort", Self::PatientShields => "Patient shields",
        Self::LastSurge => "Last surge", Self::HealingCaravan => "Healing caravan",
        Self::NeedleFlight => "Needle flight", Self::TwoFronts => "Two fronts",
        Self::ThinScreen => "Thin screen", Self::RecoveringPack => "Recovering pack",
        Self::RelentlessMarch => "Relentless march", Self::Crosswind => "Crosswind",
        Self::ProtectedRear => "Protected rear", Self::CombinedRehearsal => "Combined rehearsal",
    }}
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum CommanderClass { Bulwark, HuntCaptain, BroodKeeper, WardKeeper, SiphonMarshal, Signature }

impl CommanderClass {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Bulwark => "Bulwark",
            Self::HuntCaptain => "Hunt Captain",
            Self::BroodKeeper => "Brood Keeper",
            Self::WardKeeper => "Ward Keeper",
            Self::SiphonMarshal => "Siphon Marshal",
            Self::Signature => "Signature Commander",
        }
    }
}

pub fn commander_threat_advice(c: CommanderClass) -> (&'static str, &'static str) {
    match c {
        CommanderClass::Bulwark => (
            "Heavily shielded commander with plate segments",
            "Break the active shield to shatter armour (0 armour) for a 6s focus window!",
        ),
        CommanderClass::HuntCaptain => (
            "Periodically pulses 10% speed haste to nearby escorts",
            "Anchor lane flanks with Slow/Frost to prevent runners escaping",
        ),
        CommanderClass::BroodKeeper => (
            "Splits into zero-bounty swarms at 66% and 33% health",
            "Keep Multi/Siege towers positioned to rapidly clear splitting broods",
        ),
        CommanderClass::WardKeeper => (
            "Raises a finite shield during the first 7s of each 16s cycle (9s open window)",
            "Disrupt shield with Chaos/burst early, or punish the unshielded 9s window",
        ),
        CommanderClass::SiphonMarshal => (
            "Drains living nearby escorts to self-heal",
            "Hit with Corruption to suppress healing, or clear escorts first",
        ),
        CommanderClass::Signature => (
            "Chapter commander combining periodic wards and splitting broods",
            "Time burst during ward drops and use Multi/Siege for split brood waves",
        ),
    }
}

pub fn encounter_threat_traits(encounter: &ResolvedEncounter) -> Vec<ResolvedTrait> {
    let mut traits = Vec::new();
    for p in &encounter.packets {
        for t in p.traits.into_iter().flatten() {
            if !traits.contains(&t) {
                traits.push(t);
            }
        }
    }
    traits
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EncounterId { pub chapter: u16, pub local: u16, pub global: u16 }

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SpawnPacket {
    pub at_seconds: u16,
    pub bodies: u16,
    pub clockwise: bool,
    pub formation: Formation,
    pub traits: [Option<ResolvedTrait>; 2],
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedEncounter {
    pub id: EncounterId,
    pub title: String,
    pub formation: Formation,
    pub commander: Option<CommanderClass>,
    pub mechanic_id: &'static str,
    pub duration_seconds: u16,
    pub rush_earliest_seconds: Option<u16>,
    pub reward: u32,
    pub packets: Vec<SpawnPacket>,
    pub telegraph_seconds: u16,
}

const NORMAL_PERMUTATION: [usize; 54] = [
    0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,
    11,3,9,1,6,14,2,10,5,13,7,15,4,12,0,16,8,17,
    5,11,2,13,10,1,14,6,3,12,9,7,15,4,16,0,8,17,
];

pub const fn encounter_id(global: u16) -> EncounterId {
    let clamped = if global < 1 { 1 } else if global > REALISTIC_ENCOUNTERS { REALISTIC_ENCOUNTERS } else { global };
    EncounterId { chapter: (clamped - 1) / ENCOUNTERS_PER_CHAPTER + 1, local: (clamped - 1) % ENCOUNTERS_PER_CHAPTER + 1, global: clamped }
}

pub const fn is_commander(local: u16) -> bool { local % 10 == 0 }

fn unlocked_traits(id: EncounterId, formation: Formation) -> [Option<ResolvedTrait>; 2] {
    // The gating mirrors ENCOUNTERS.md.  A requested future mechanic resolves
    // to its listed basic counterpart, so previews can never lie to players.
    let local = id.local;
    // C1E21 is the explicitly forecast first-air encounter.  Its template
    // remains a two-front formation, but its resolved manifest truthfully
    // replaces one ground packet with the introductory flight.
    if id.chapter == 1 && local == 21 { return [Some(ResolvedTrait::Flying), None]; }
    let primary = match formation {
        Formation::BroodTide | Formation::Reversal | Formation::LastSurge | Formation::ThinScreen => Some(ResolvedTrait::Swarm),
        Formation::IronConvoy | Formation::BrokenColumn | Formation::TwoFronts => Some(ResolvedTrait::Armoured),
        Formation::RunningFlank | Formation::Crosswind => (local >= 4).then_some(ResolvedTrait::Swift),
        Formation::NeedleFlight | Formation::WingEscort => (id.chapter > 1 || local >= 21).then_some(ResolvedTrait::Flying),
        Formation::PatientShields | Formation::ProtectedRear => (id.chapter >= 2 && local >= 11).then_some(ResolvedTrait::Shielded),
        Formation::HealingCaravan | Formation::RecoveringPack => (id.chapter >= 3).then_some(ResolvedTrait::Regenerator),
        Formation::RelentlessMarch => if id.chapter >= 4 && local >= 21 { Some(ResolvedTrait::Resistant) } else { Some(ResolvedTrait::Armoured) },
        _ => None,
    };
    let secondary = if id.chapter >= 5 && matches!(formation, Formation::CombinedRehearsal | Formation::ProtectedRear | Formation::Crosswind) {
        Some(ResolvedTrait::Armoured)
    } else { None };
    [primary, secondary]
}

fn normal_bodies(id: EncounterId, formation: Formation) -> u16 {
    // A Campaign packet should feel like a horde, not like six isolated
    // Legacy trickles. The authored timing is unchanged; this raises visible
    // density and makes splash, control and target priority matter early.
    let base = 145 + ((id.local * 7 + id.chapter * 11) % 81) as u16;
    match formation { Formation::BroodTide | Formation::LastSurge => base + 70, _ => base }
}

pub const fn commander_class(local: u16) -> CommanderClass {
    match local { 10 => CommanderClass::Bulwark, 20 => CommanderClass::HuntCaptain, 30 => CommanderClass::BroodKeeper, 40 => CommanderClass::WardKeeper, 50 => CommanderClass::SiphonMarshal, _ => CommanderClass::Signature }
}

/// Commander identity is authored by the encounter number, not sampled while
/// a creep is alive. Saves can therefore reconstruct a live commander's role
/// solely from its persisted `campaign_encounter`.
pub const fn commander_for_encounter(global: u16) -> Option<CommanderClass> {
    let id = encounter_id(global);
    if is_commander(id.local) { Some(commander_class(id.local)) } else { None }
}

pub fn resolved_encounter(global: u16) -> ResolvedEncounter {
    let id = encounter_id(global);
    let commander = commander_for_encounter(global);
    let normal_index = id.local - 1 - (id.local - 1) / 10;
    let formation = if commander.is_some() {
        Formation::CombinedRehearsal
    } else { Formation::ALL[NORMAL_PERMUTATION[normal_index as usize]] };
    let duration_seconds = if commander.is_some() { COMMANDER_SECONDS } else { NORMAL_BOUNDARY_SECONDS };
    let timings: &[u16] = if commander.is_some() { &[0, 18, 38, 60, 82, 104, 120] } else { &[0, 10, 20, 32, 44, 54] };
    // Put a visible fighting column on the road immediately.  The old first
    // normal packet was only fifteen percent of the round, which meant an
    // opening advertised as a 160-body formation looked like two dozen units
    // at the intended 2x tempo.  The total, timings and final deployment stay
    // authored; this only front-loads the first tactical decision.
    let shares: &[u16] = if commander.is_some() { &[13, 14, 15, 15, 15, 14, 14] } else { &[24, 18, 18, 16, 14, 10] };
    let total = if commander.is_some() { 185 + id.chapter * 12 } else { normal_bodies(id, formation) };
    let mut issued = 0u16;
    let packets = timings.iter().enumerate().map(|(i, &at_seconds)| {
        let bodies = if i + 1 == timings.len() { total - issued } else { total * shares[i] / 100 };
        issued += bodies;
        SpawnPacket { at_seconds, bodies, clockwise: (i + id.local as usize + id.chapter as usize) % 2 == 0, formation, traits: unlocked_traits(id, formation) }
    }).collect();
    let reward = (70 + 3 * id.local as u32 + 20 * id.chapter as u32)
        + if commander.is_some() { 100 + 30 * id.chapter as u32 } else { 0 };
    let mechanic_id = match commander {
        Some(CommanderClass::Bulwark) => "plate-segments", Some(CommanderClass::HuntCaptain) => "marked-flank",
        Some(CommanderClass::BroodKeeper) => "brood-thresholds", Some(CommanderClass::WardKeeper) => "shield-window",
        Some(CommanderClass::SiphonMarshal) => "escort-siphon", Some(CommanderClass::Signature) => "chapter-signature",
        None => "formation",
    };
    ResolvedEncounter { id, title: if let Some(c) = commander { format!("Chapter {} {:?}", id.chapter, c) } else { formation.name().to_owned() }, formation, commander, mechanic_id, duration_seconds, rush_earliest_seconds: commander.is_none().then_some(NORMAL_DEPLOYMENT_SECONDS), reward, packets, telegraph_seconds: if commander.is_some() { 116 } else { 50 } }
}

/// State that is safe to serialize for a campaign checkpoint or an exact
/// mid-encounter save.  It contains timing and reward guards; callers own the
/// live board entities and RNG alongside this value.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct CampaignState {
    pub version: u16,
    pub encounter: u16,
    pub elapsed_seconds: f32,
    pub active_seconds: f64,
    pub deployed_packets: u8,
    pub rush_used: bool,
    pub reward_paid: u32,
    pub speed: f32,
    /// A packet is only marked deployed after every one of its bodies has
    /// crossed into the live simulation.  This is persisted so a reload
    /// cannot turn a partly entering convoy into a missing convoy.
    #[serde(default)]
    pub queued_packet: Option<u8>,
    #[serde(default)]
    pub queued_bodies_left: u16,
    #[serde(default)]
    pub packet_spawn_timer: f32,
    /// Bodies that have been allocated their deterministic kill share.  It is
    /// deliberately separate from `deployed_packets`: a group streams in over
    /// a short visible window rather than appearing as one invisible batch.
    #[serde(default)]
    pub spawned_bodies: u32,
    /// The encounter reward is split into a 40% deployment purse and exact
    /// per-body kill shares.  This records the portion already represented in
    /// the live economy, guarding save/reload against duplicate grants.
    #[serde(default)]
    pub reward_issued: u32,
    #[serde(default)]
    pub deployment_paid: bool,
    /// Authoritative encounter budget locked when the encounter begins.
    /// Distinguishes legacy saves (where this field is None) and ensures
    /// mid-encounter saves converge deterministically without stalls.
    #[serde(default)]
    pub in_flight_budget: Option<u32>,
    /// One-shot commander mechanics already fired in this encounter. Bit 0
    /// is the Bulwark break/Hunt pulse, bits 1-2 are brood thresholds. The
    /// state is saved so an exact mid-fight reload cannot mint reinforcements.
    #[serde(default)]
    pub commander_triggers: u8,
    /// Timestamp of the current temporary commander window (Bulwark exposed
    /// plate or the most recent Hunt pulse). It is encounter-relative and
    /// deterministic across save/resume.
    #[serde(default = "commander_window_default")]
    pub commander_window_at: f32,
    /// Exact encounter-relative time that the single commander physically
    /// entered the ring. It is set only after the final packet reaches its
    /// first body, so delayed queues never make a Hunt pulse happen offscreen.
    #[serde(default)]
    pub commander_spawned_at: Option<f32>,
    #[serde(default)]
    pub complete: bool,
}

impl Default for CampaignState {
    fn default() -> Self {
        Self {
            version: 3,
            encounter: 1,
            elapsed_seconds: 0.0,
            active_seconds: 0.0,
            deployed_packets: 0,
            rush_used: false,
            reward_paid: 0,
            speed: 1.0,
            queued_packet: None,
            queued_bodies_left: 0,
            packet_spawn_timer: 0.0,
            spawned_bodies: 0,
            reward_issued: 0,
            deployment_paid: false,
            in_flight_budget: None,
            commander_triggers: 0,
            commander_window_at: -1.0,
            commander_spawned_at: None,
            complete: false,
        }
    }
}

const fn commander_window_default() -> f32 { -1.0 }

impl CampaignState {
    pub fn current(&self) -> ResolvedEncounter { resolved_encounter(self.encounter) }
    pub fn can_rush(&self) -> bool {
        let e = self.current();
        !self.complete
            && e.rush_earliest_seconds.is_some_and(|t| self.elapsed_seconds >= t as f32)
            && self.deployed_packets as usize == e.packets.len()
            && self.queued_packet.is_none()
            && !self.rush_used
    }
    /// Advances simulation time only.  Callers spawn every packet returned by
    /// `due_packets`; this function never silently drops a scheduled horde.
    pub fn advance(&mut self, seconds: f32) {
        if self.complete {
            return;
        }
        let dt = seconds.max(0.0);
        self.elapsed_seconds += dt;
        self.active_seconds += dt as f64;
    }

    /// Arms the next scheduled packet, but does not yet claim it as deployed.
    /// The game loop calls this before streaming its individual bodies; that
    /// ordering is what makes a save during deployment lossless.
    pub fn arm_due_packet(&mut self) -> Option<SpawnPacket> {
        if self.complete || self.queued_packet.is_some() {
            return self.queued_packet();
        }
        let e = self.current();
        let index = self.deployed_packets as usize;
        let packet = *e.packets.get(index)?;
        if self.elapsed_seconds < packet.at_seconds as f32 {
            return None;
        }
        self.queued_packet = Some(index as u8);
        self.queued_bodies_left = packet.bodies;
        self.packet_spawn_timer = 0.0;
        Some(packet)
    }

    pub fn queued_packet(&self) -> Option<SpawnPacket> {
        self.queued_packet
            .and_then(|index| self.current().packets.get(index as usize).copied())
    }

    /// Records one body entering the world.  Returns false only for malformed
    /// state; callers treat that as a rejected save rather than spawning an
    /// invented replacement body.
    pub fn consume_queued_body(&mut self, gap: f32) -> bool {
        if self.queued_packet.is_none() || self.queued_bodies_left == 0 {
            return false;
        }
        self.queued_bodies_left -= 1;
        self.spawned_bodies = self.spawned_bodies.saturating_add(1);
        self.packet_spawn_timer += gap.max(0.001);
        if self.queued_bodies_left == 0 {
            self.deployed_packets = self.deployed_packets.saturating_add(1);
            self.queued_packet = None;
            self.packet_spawn_timer = 0.0;
        }
        true
    }

    /// Compatibility helper for manifest-only callers.  Runtime code uses
    /// `arm_due_packet`/`consume_queued_body` so the schedule remains live.
    pub fn due_packets(&mut self) -> Vec<SpawnPacket> {
        let e = self.current();
        let mut out = Vec::new();
        while self.queued_packet.is_none() {
            let Some(packet) = self.arm_due_packet() else { break };
            out.push(packet);
            self.deployed_packets = self.deployed_packets.saturating_add(1);
            self.queued_packet = None;
            self.queued_bodies_left = 0;
        }
        debug_assert!(self.deployed_packets as usize <= e.packets.len());
        out
    }

    pub fn rush(&mut self) -> bool {
        if !self.can_rush() {
            return false;
        }
        self.rush_used = true;
        let target = self.current().duration_seconds as f32;
        // Rush removes only the authored recovery tail. That tail is not
        // active combat time, so it is intentionally excluded from the
        // fastest-route duration metric (54s normal deployment + commanders).
        self.elapsed_seconds = target;
        true
    }

    pub fn can_finish(&self, living_objectives: usize) -> bool {
        !self.complete
            && self.elapsed_seconds >= self.current().duration_seconds as f32
            && living_objectives == 0
            && self.queued_packet.is_none()
            && self.deployed_packets as usize == self.current().packets.len()
    }

    pub fn finish(&mut self, living_objectives: usize) -> Option<u16> {
        if !self.can_finish(living_objectives) {
            return None;
        }
        let finished = self.encounter;
        if finished < REALISTIC_ENCOUNTERS {
            self.encounter += 1;
            self.elapsed_seconds = 0.0;
            self.deployed_packets = 0;
            self.rush_used = false;
            self.reward_paid = 0;
            self.queued_packet = None;
            self.queued_bodies_left = 0;
            self.packet_spawn_timer = 0.0;
            self.spawned_bodies = 0;
            self.reward_issued = 0;
            self.deployment_paid = false;
            self.in_flight_budget = None;
            // Commander state is encounter-local. Carrying a Bulwark break or
            // Hunt pulse timestamp into the next commander makes its mechanic
            // fire before that commander has earned its own telegraphed age.
            self.commander_triggers = 0;
            self.commander_window_at = commander_window_default();
            self.commander_spawned_at = None;
        } else {
            self.complete = true;
            self.in_flight_budget = None;
        }
        Some(finished)
    }
}

pub fn fastest_legal_active_seconds() -> u32 { (1..=REALISTIC_ENCOUNTERS).map(|id| if resolved_encounter(id).commander.is_some() { COMMANDER_SECONDS as u32 } else { NORMAL_DEPLOYMENT_SECONDS as u32 }).sum() }

/// Human-readable output generated from the same resolver used at runtime.
pub fn manifest_report() -> String {
    let mut out = String::from("# Realistic Campaign Encounter Manifest\\n\\n");
    for global in 1..=REALISTIC_ENCOUNTERS { let e = resolved_encounter(global); let groups = e.packets.iter().map(|p| format!("{}:{}{}", p.at_seconds, p.bodies, if p.clockwise { " CW" } else { " CCW" })).collect::<Vec<_>>().join(", "); out.push_str(&format!("{:03} C{}E{:02} | {} | {}s | {}g | {}\\n", global, e.id.chapter, e.id.local, e.title, e.duration_seconds, e.reward, groups)); }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn campaign_scope_is_small_and_every_branch_is_unique() {
        assert_eq!(TOWER_JOBS.len(), 6);
        let mut branches = HashSet::new();
        for tower in TOWER_JOBS {
            assert!((170..=220).contains(&tower.base_cost));
            assert!((260..=320).contains(&tower.armed_cost));
            for branch in 0..2 {
                assert!(branches.insert(tower.specializations[branch]));
                assert!((480..=600).contains(&tower.specialization_costs[branch]));
                assert!((850..=1_100).contains(&tower.apex_costs[branch]));
                assert!((1_760..=2_220).contains(&tower.full_path_cost(branch)));
            }
        }
        assert_eq!(branches.len(), 12);
    }

    #[test]
    fn target_rules_are_soft_except_for_the_bombard_promise() {
        for tower in TOWER_JOBS {
            match tower.job {
                TowerJob::Bombard => assert_eq!(tower.efficiency(TargetLayer::Air), 0),
                TowerJob::Repeater => assert_eq!(tower.efficiency(TargetLayer::Air), 65),
                TowerJob::Skyguard => assert_eq!(tower.efficiency(TargetLayer::Ground), 35),
                _ => {
                    assert_eq!(tower.efficiency(TargetLayer::Ground), 100);
                    assert_eq!(tower.efficiency(TargetLayer::Air), 100);
                }
            }
        }
    }

    #[test]
    fn act_one_is_a_six_minute_teaching_arc_with_one_commander() {
        assert_eq!(ACT_ONE.len(), 7);
        let seconds: f32 = ACT_ONE.iter().map(|e| e.boundary_seconds).sum();
        assert!((360.0..=480.0).contains(&seconds));
        assert_eq!(ACT_ONE.iter().filter(|e| e.boss.is_some()).count(), 1);
        assert_eq!(ACT_ONE.last().unwrap().boss, Some(BossMechanic::ArmourAura));

        for (index, encounter) in ACT_ONE.iter().enumerate() {
            assert_eq!(encounter.number as usize, index + 1);
            assert!((28.0..=34.0).contains(&encounter.deployment_seconds));
            assert_eq!(
                encounter
                    .groups
                    .iter()
                    .map(|group| group.pressure_percent as u16)
                    .sum::<u16>(),
                100
            );
            assert!(
                encounter
                    .groups
                    .iter()
                    .all(|group| group.trait_count() <= 2)
            );
        }
    }

    #[test]
    fn campaign_economy_is_bounded_and_laps_cannot_change_it() {
        let total: u32 = (1..=CAMPAIGN_ENCOUNTERS)
            .map(|encounter| encounter_gold(encounter) as u32)
            .sum();
        assert_eq!(total, 8_275);
        for encounter in 1..=CAMPAIGN_ENCOUNTERS {
            let split = gold_split(encounter);
            assert_eq!(split[0] + split[1], encounter_gold(encounter));
        }
    }

    #[test]
    fn ring_pressure_escalates_for_three_laps_then_caps() {
        let base = unit_pressure(EnemyRank::Vanguard, 2, 0);
        assert!((base - 3.125).abs() < 0.001);
        assert!((unit_pressure(EnemyRank::Vanguard, 2, 3) - 5.0).abs() < 0.001);
        assert_eq!(
            unit_pressure(EnemyRank::Vanguard, 2, 3),
            unit_pressure(EnemyRank::Vanguard, 2, 8)
        );
    }

    #[test]
    fn campaign_curves_match_the_blueprint() {
        assert_eq!(CampaignDifficulty::Apprentice.pressure_capacity(), 120.0);
        assert_eq!(CampaignDifficulty::Veteran.pressure_capacity(), 100.0);
        assert_eq!(CampaignDifficulty::Nightmare.pressure_capacity(), 85.0);
        assert!((base_health_multiplier(CAMPAIGN_ENCOUNTERS) - 30.75).abs() < 0.5);
        assert!((pressure_budget(1) - 25.5).abs() < f32::EPSILON);
        assert!((pressure_budget(28) - 84.0).abs() < f32::EPSILON);
    }
}

#[cfg(test)]
mod realistic_campaign_tests {
    use super::*;

    #[test]
    fn full_manifest_has_six_honest_packets_and_no_missing_ids() {
        assert_eq!(REALISTIC_ENCOUNTERS, 600);
        for global in 1..=REALISTIC_ENCOUNTERS {
            let e = resolved_encounter(global);
            assert_eq!(e.id.global, global);
            assert_eq!(e.packets.first().unwrap().at_seconds, 0);
            assert_eq!(e.packets.last().unwrap().at_seconds, if e.commander.is_some() { 120 } else { 54 });
            assert_eq!(e.packets.iter().map(|p| p.bodies as u32).sum::<u32>(), if e.commander.is_some() { 185 + e.id.chapter as u32 * 12 } else { normal_bodies(e.id, e.formation) as u32 });
            assert!(e.packets.iter().all(|p| p.bodies > 0));
            if e.commander.is_none() {
                let total: u32 = e.packets.iter().map(|p| p.bodies as u32).sum();
                assert!(
                    e.packets[0].bodies as u32 * 100 >= total * 23,
                    "C{}E{} opens too thinly: {}/{} bodies",
                    e.id.chapter,
                    e.id.local,
                    e.packets[0].bodies,
                    total,
                );
            }
        }
    }

    #[test]
    fn legal_rush_route_proves_five_hour_minimum() {
        assert_eq!(fastest_legal_active_seconds(), FASTEST_ACTIVE_SECONDS);
        // Five hours at the stated 2x pace is 36,000 seconds of simulated
        // campaign activity. The authored fastest legal route is 36,360s;
        // 18,000 was one real-time hour too lenient.
        assert!(fastest_legal_active_seconds() >= 36_000);
        assert_eq!(SCHEDULED_ACTIVE_SECONDS, FASTEST_ACTIVE_SECONDS);
    }

    #[test]
    fn rush_never_skips_a_pending_packet_or_a_commander() {
        let mut normal = CampaignState::default();
        normal.advance(53.99);
        assert!(!normal.rush());
        normal.advance(0.01);
        let all = normal.due_packets();
        assert_eq!(all.len(), 6);
        assert!(normal.rush());
        assert!(normal.can_finish(0));

        let mut commander = CampaignState { encounter: 10, ..CampaignState::default() };
        commander.advance(120.0);
        assert!(!commander.rush());
        assert_eq!(commander.due_packets().len(), 7);
        assert!(commander.can_finish(0));
    }

    #[test]
    fn trait_introductions_and_save_state_are_versioned() {
        assert_eq!(resolved_encounter(3).packets[0].traits[0], Some(ResolvedTrait::Armoured));
        assert_eq!(resolved_encounter(7).packets[0].traits[0], None, "early wing schedule resolves to a ground substitute");
        assert_eq!(resolved_encounter(21).packets[0].traits[0], Some(ResolvedTrait::Flying));
        let before = CampaignState {
            encounter: 241,
            elapsed_seconds: 54.0,
            active_seconds: 1234.0,
            deployed_packets: 6,
            rush_used: false,
            reward_paid: 0,
            speed: 2.0,
            ..CampaignState::default()
        };
        let restored: CampaignState = serde_json::from_str(&serde_json::to_string(&before).unwrap()).unwrap();
        assert_eq!(before, restored);
    }
}
