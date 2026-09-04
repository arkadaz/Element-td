//! Curated commercial Campaign rules.
//!
//! The playable game still runs the extracted Legacy tables in [`super::defs`].
//! This module is deliberately separate: it is the typed contract for the
//! smaller, readable Campaign described in `docs/MASTERCLASS_PLAN.md`. The
//! first implementation slice contains the complete tower-job vocabulary and
//! Act I encounter timeline so simulation can be proven before UI and content
//! production are built on top of it.

// This is a staged rules/data layer. Most items become live when Campaign is
// wired into the simulation in the next delivery gate.
#![allow(dead_code)]

pub const CAMPAIGN_ENCOUNTERS: u8 = 28;
pub const ENCOUNTERS_PER_ACT: u8 = 7;
pub const STARTING_GOLD: u16 = 600;
pub const PRESSURE_WARNING: f32 = 80.0;
pub const BREACH_SECONDS: f32 = 3.0;
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
