//! Saving and resuming a run.
//!
//! The whole simulation happens on the player's own machine, so the save lives
//! there too: `localStorage` in the browser, a file in the OS config directory
//! natively. That costs the server exactly nothing, which matters - it is sized
//! so a gigabyte of RAM holds a thousand players, and per-player run state
//! would undo that at a stroke.
//!
//! It is deliberately *not* keyed by IP address. An IP is not an identity: a
//! phone changes it several times an hour, and everyone behind one router or
//! one carrier-grade NAT shares it, so players would resume into each other's
//! games. It is also personal data, and this needs none.
//!
//! What is stored is still compact, but it is a real checkpoint: the wave
//! stream, every living monster and status, tower cooldowns, command doctrine,
//! tempo score and deterministic random state. Resume must never erase the
//! pressure that made the saved position interesting.

use serde::{Deserialize, Serialize};

use crate::game::greentd_types::{ArmourType, Model};
use crate::game::{
    Creep, Difficulty, FREE_TOWER_SLOT, Game, RunMode, MAX_CREEPS, Phase,
    TargetMode, Timed,
};
use crate::rng::Rng;

/// Bumped whenever the shape below changes. An older save is discarded rather
/// than half-read, because a half-restored board is worse than a fresh start.
// Version 12 stores actual world positions for free grass placement. Versions
// 10 and 11 remain readable: their pad IDs are migrated to the old pad centre
// without converting a player's Legacy run into a different mode.
const VERSION: u16 = 12;
const PREVIOUS_VERSION: u16 = 11;
const LEGACY_VERSION: u16 = 10;

// Retain the established key so a v10 Legacy save is discovered and migrated
// in place on its first successful v11 write.
const KEY: &str = "green_td_save_v10";

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SavedTower {
    /// Index into `defs::TOWERS`, which is family *and* level together, so
    /// nothing else about the tower's position on its path has to be stored.
    pub def: u16,
    pub slot: u16,
    /// Actual free-placement position. Absent in v10/v11 saves, which are
    /// restored at their historical pad position. `Some` also records a
    /// migrated legacy tower, so its silhouette stays exactly where it stood.
    #[serde(default)]
    pub pos: Option<[f32; 2]>,
    pub invested: u32,
    pub kills: u32,
    pub damage: f64,
    pub gold_earned: u64,
    pub mode: u8,
    pub cooldown: f32,
    pub angle: f32,
    pub target_uid: u32,
    pub ramp: f32,
    pub frenzy_cd: f32,
    pub aura_timer: f32,
    pub flash: f32,
    pub built_at: f32,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug)]
pub struct SavedTimed {
    pub amt: f32,
    pub t: f32,
}

impl From<Timed> for SavedTimed {
    fn from(value: Timed) -> Self {
        Self {
            amt: value.amt,
            t: value.t,
        }
    }
}

impl From<SavedTimed> for Timed {
    fn from(value: SavedTimed) -> Self {
        Self {
            amt: value.amt,
            t: value.t,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SavedCreep {
    pub uid: u32,
    pub dist: f32,
    /// Added without invalidating v7 saves; old runs resume clockwise.
    #[serde(default = "forward_route")]
    pub route_dir: f32,
    pub lane: f32,
    pub pos: [f32; 2],
    pub facing: f32,
    pub hp: f32,
    pub max_hp: f32,
    pub base_speed: f32,
    pub armour: i32,
    pub armour_type: u8,
    pub model: u8,
    pub flying: bool,
    pub radius: f32,
    pub bounty: u32,
    pub boss: bool,
    pub elite: bool,
    pub slow: SavedTimed,
    pub burn: SavedTimed,
    pub poison: SavedTimed,
    pub shred: SavedTimed,
    pub stun: f32,
    pub stun_dr: f32,
    pub kb_cd: f32,
    pub suppress: f32,
    pub stun_immune: f32,
    pub push_left: f32,
    pub laps: u32,
    pub flash: f32,
    pub bob: f32,
    /// Campaign physical layers were added after the Legacy save format. The
    /// defaults reproduce a pre-Campaign creep exactly when loading v10.
    #[serde(default)]
    pub shield: f32,
    #[serde(default)]
    pub max_shield: f32,
    #[serde(default)]
    pub regen_per_second: f32,
    #[serde(default)]
    pub resistant: bool,
    #[serde(default)]
    pub campaign_encounter: u16,
    #[serde(default = "legacy_pressure")]
    pub pressure: f32,
    #[serde(default)]
    pub death_killer: Option<u16>,
}

const SAVED_FREE_SLOT: u16 = u16::MAX;

fn saved_tower_position(t: &SavedTower, board: &crate::game::board::Board) -> Option<[f32; 2]> {
    t.pos.or_else(|| board.slots.get(t.slot as usize).map(|slot| slot.pos))
}

fn saved_legacy_slot(t: &SavedTower, board: &crate::game::board::Board) -> Option<usize> {
    let slot = t.slot as usize;
    let pad = board.slots.get(slot)?;
    match t.pos {
        // Old saves only know a pad. New saves retain the pad link only when
        // their world position is still that exact historical pad centre.
        None => Some(slot),
        Some(pos) => {
            let p = board.quantize_build_pos(pos);
            let q = board.quantize_build_pos(pad.pos);
            ((p[0] - q[0]).abs() < 0.001 && (p[1] - q[1]).abs() < 0.001).then_some(slot)
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Save {
    pub version: u16,
    /// `RunMode` encoded explicitly; absent v10 saves default to Legacy.
    #[serde(default)]
    pub mode: u8,
    /// Exact active encounter cursor, queued packet and reward ledger for the
    /// new Campaign. It is absent for Legacy by design.
    #[serde(default)]
    pub campaign: Option<crate::game::campaign::CampaignState>,
    #[serde(default = "default_campaign_grace")]
    pub campaign_pressure_grace: f32,
    pub seed: u64,
    pub difficulty: u8,
    pub wave: u32,
    pub gold: i64,
    pub rng_state: u64,
    pub time: f32,
    #[serde(default = "default_speed")]
    pub speed: f32,
    /// Phase is needed for a Campaign save at the explicit resolution/build
    /// beat; old saves use `UNKNOWN_PHASE` and retain their old derivation.
    #[serde(default = "unknown_phase")]
    pub phase: u8,
    pub wave_timer: f32,
    pub prep: bool,
    pub spawn_left: u32,
    pub spawn_timer: f32,
    pub next_uid: u32,
    /// How many monsters were circling. There are no lives to store any more -
    /// what a resumed run has to bring back is the *debt*, because a board that
    /// was two hundred monsters behind is in a completely different position
    /// from one that was clear.
    pub circling: u16,
    pub endless: bool,
    /// Arsenal, Overdrive and High Ground ranks.
    pub doctrines: [u8; 3],
    pub pending_doctrine: bool,
    /// Essences held, by element index. Without these a resumed run could not
    /// rebuild the board it saved, let alone upgrade it.
    pub kills: u64,
    pub leaked: u32,
    pub gold_earned: u64,
    pub gold_spent: u64,
    pub damage: f64,
    pub towers_built: u32,
    pub clean_sweeps: u32,
    pub rush_gold: u64,
    pub creeps: Vec<SavedCreep>,
    pub towers: Vec<SavedTower>,
}

impl SavedTimed {
    fn valid(self) -> bool {
        self.amt.is_finite() && self.t.is_finite() && self.amt >= 0.0 && self.t >= 0.0
    }
}

impl SavedCreep {
    fn capture(c: &Creep) -> Self {
        Self {
            uid: c.uid,
            dist: c.dist,
            route_dir: c.route_dir,
            lane: c.lane,
            pos: c.pos,
            facing: c.facing,
            hp: c.hp,
            max_hp: c.max_hp,
            base_speed: c.base_speed,
            armour: c.armour,
            armour_type: c.armour_type.as_u8(),
            model: c.model.as_u8(),
            flying: c.flying,
            radius: c.radius,
            bounty: c.bounty,
            boss: c.boss,
            elite: c.elite,
            slow: c.slow.into(),
            burn: c.burn.into(),
            poison: c.poison.into(),
            shred: c.shred.into(),
            stun: c.stun,
            stun_dr: c.stun_dr,
            kb_cd: c.kb_cd,
            suppress: c.suppress,
            stun_immune: c.stun_immune,
            push_left: c.push_left,
            laps: c.laps,
            flash: c.flash,
            bob: c.bob,
            shield: c.shield,
            max_shield: c.max_shield,
            regen_per_second: c.regen_per_second,
            resistant: c.resistant,
            campaign_encounter: c.campaign_encounter,
            pressure: c.pressure,
            death_killer: c.death_killer.and_then(|i| u16::try_from(i).ok()),
        }
    }

    fn restore(&self) -> Option<Creep> {
        let finite = [
            self.dist,
            self.route_dir,
            self.lane,
            self.pos[0],
            self.pos[1],
            self.facing,
            self.hp,
            self.max_hp,
            self.base_speed,
            self.radius,
            self.stun,
            self.stun_dr,
            self.kb_cd,
            self.suppress,
            self.stun_immune,
            self.push_left,
            self.flash,
            self.bob,
            self.shield,
            self.max_shield,
            self.regen_per_second,
            self.pressure,
        ]
        .into_iter()
        .all(f32::is_finite);
        if !finite
            || self.uid == 0
            || self.hp <= 0.0
            || self.max_hp <= 0.0
            || self.hp > self.max_hp * 1.01
            || self.base_speed <= 0.0
            || !(0.05..=5.0).contains(&self.radius)
            || self.shield < 0.0
            || self.max_shield < 0.0
            || self.shield > self.max_shield * 1.01
            || self.regen_per_second < 0.0
            || self.pressure <= 0.0
            || !self.slow.valid()
            || !self.burn.valid()
            || !self.poison.valid()
            || !self.shred.valid()
        {
            return None;
        }
        Some(Creep {
            uid: self.uid,
            dist: self.dist,
            route_dir: if self.route_dir < 0.0 { -1.0 } else { 1.0 },
            lane: self.lane,
            pos: self.pos,
            facing: self.facing,
            hp: self.hp,
            max_hp: self.max_hp,
            base_speed: self.base_speed,
            armour: self.armour,
            armour_type: ArmourType::from_u8(self.armour_type)?,
            model: Model::from_u8(self.model)?,
            flying: self.flying,
            radius: self.radius,
            bounty: self.bounty,
            boss: self.boss,
            elite: self.elite,
            slow: self.slow.into(),
            burn: self.burn.into(),
            poison: self.poison.into(),
            shred: self.shred.into(),
            stun: self.stun,
            stun_dr: self.stun_dr,
            kb_cd: self.kb_cd,
            suppress: self.suppress,
            stun_immune: self.stun_immune,
            push_left: self.push_left,
            laps: self.laps,
            flash: self.flash,
            bob: self.bob,
            shield: self.shield,
            max_shield: self.max_shield,
            regen_per_second: self.regen_per_second,
            resistant: self.resistant,
            campaign_encounter: self.campaign_encounter,
            pressure: self.pressure,
            death_killer: self.death_killer.map(usize::from),
        })
    }
}

fn forward_route() -> f32 {
    1.0
}

fn legacy_pressure() -> f32 {
    1.0
}

fn default_campaign_grace() -> f32 {
    crate::game::campaign::BREACH_SECONDS
}

fn default_speed() -> f32 {
    1.0
}

const UNKNOWN_PHASE: u8 = u8::MAX;

fn unknown_phase() -> u8 {
    UNKNOWN_PHASE
}

fn phase_to_u8(phase: Phase) -> u8 {
    match phase {
        Phase::Build => 0,
        Phase::Combat => 1,
        Phase::Defeat => 2,
        Phase::Victory => 3,
    }
}

fn phase_from_u8(value: u8) -> Option<Phase> {
    match value {
        0 => Some(Phase::Build),
        1 => Some(Phase::Combat),
        2 => Some(Phase::Defeat),
        3 => Some(Phase::Victory),
        _ => None,
    }
}

fn mode_to_u8(m: TargetMode) -> u8 {
    match m {
        TargetMode::First => 0,
        TargetMode::Last => 1,
        TargetMode::Strongest => 2,
        TargetMode::Closest => 3,
    }
}

fn mode_from_u8(v: u8) -> TargetMode {
    match v {
        1 => TargetMode::Last,
        2 => TargetMode::Strongest,
        3 => TargetMode::Closest,
        _ => TargetMode::First,
    }
}

impl Save {
    pub fn capture(g: &Game) -> Save {
        Save {
            version: VERSION,
            mode: g.mode.as_u8(),
            campaign: g.campaign.clone(),
            campaign_pressure_grace: g.campaign_pressure_grace,
            seed: g.seed,
            difficulty: g.difficulty.as_u8(),
            wave: g.wave,
            gold: g.gold,
            rng_state: g.rng.state(),
            time: g.time,
            speed: g.speed,
            phase: phase_to_u8(g.phase),
            wave_timer: g.wave_timer,
            prep: g.prep,
            spawn_left: g.spawn_left,
            spawn_timer: g.spawn_timer,
            next_uid: g.next_uid,
            circling: g.creeps.len().min(u16::MAX as usize) as u16,
            endless: g.endless,
            doctrines: g.doctrines,
            pending_doctrine: g.pending_doctrine,
            kills: g.stats.kills,
            leaked: g.stats.leaked,
            gold_earned: g.stats.gold_earned,
            gold_spent: g.stats.gold_spent,
            damage: g.stats.damage,
            towers_built: g.stats.towers_built,
            clean_sweeps: g.stats.clean_sweeps,
            rush_gold: g.stats.rush_gold,
            creeps: g.creeps.iter().map(SavedCreep::capture).collect(),
            towers: g
                .towers
                .iter()
                .map(|t| SavedTower {
                    def: t.def as u16,
                    slot: if t.slot == FREE_TOWER_SLOT {
                        SAVED_FREE_SLOT
                    } else {
                        t.slot.min(u16::MAX as usize - 1) as u16
                    },
                    pos: Some(t.pos),
                    invested: t.invested,
                    kills: t.kills,
                    damage: t.damage,
                    gold_earned: t.gold_earned,
                    mode: mode_to_u8(t.mode),
                    cooldown: t.cooldown,
                    angle: t.angle,
                    target_uid: t.target_uid,
                    ramp: t.ramp,
                    frenzy_cd: t.frenzy_cd,
                    aura_timer: t.aura_timer,
                    flash: t.flash,
                    built_at: t.built_at,
                })
                .collect(),
        }
    }

    /// Rebuilds a run. Returns false and leaves the game untouched if the save
    /// does not describe a board this build can actually construct.
    pub fn restore(&self, g: &mut Game) -> bool {
        if self.version != VERSION
            && self.version != PREVIOUS_VERSION
            && self.version != LEGACY_VERSION
        {
            return false;
        }
        let mode = RunMode::from_u8(self.mode);
        let is_campaign = mode == RunMode::Campaign;
        let towers = crate::game::defs::TOWERS.len();
        // Every tower in the roster can be reached from the shop.  Position
        // validation is stricter than an old pad index: current saves must
        // have a legal world footprint, while v10/v11 pads are migrated at
        // their paid-for historical centres.
        let tower_positions: Option<Vec<[f32; 2]>> = self
            .towers
            .iter()
            .map(|t| saved_tower_position(t, &g.board))
            .collect();
        let valid = self.towers.iter().enumerate().all(|(i, t)| {
            let slot_ok = t.slot == SAVED_FREE_SLOT || (t.slot as usize) < g.board.slots.len();
            let pos = tower_positions.as_ref().and_then(|positions| positions.get(i)).copied();
            let position_ok = match (t.pos, pos) {
                // A v12 free position must obey the live terrain footprint
                // rule. It is not allowed to smuggle a tower under a road.
                (Some(_), Some(pos)) if t.slot == SAVED_FREE_SLOT => g
                    .board
                    .surface_block(
                        g.board.quantize_build_pos(pos),
                        crate::game::board::TOWER_FOOTPRINT_RADIUS,
                    )
                    .is_none(),
                // Old pads remain valid migration anchors even where the new
                // larger free-build road clearance would reject a fresh base.
                (_, Some(pos)) => pos[0].is_finite() && pos[1].is_finite() && slot_ok,
                _ => false,
            };
            (t.def as usize) < towers
                && slot_ok
                && position_ok
                && [
                    t.cooldown,
                    t.angle,
                    t.ramp,
                    t.frenzy_cd,
                    t.aura_timer,
                    t.flash,
                    t.built_at,
                ]
                .into_iter()
                .all(f32::is_finite)
        });
        let positions_do_not_overlap = tower_positions.as_ref().is_some_and(|positions| {
            let separation = crate::game::board::TOWER_FOOTPRINT_RADIUS * 2.0
                + crate::game::board::TOWER_CLEARANCE;
            positions.iter().enumerate().all(|(i, a)| {
                positions.iter().enumerate().skip(i + 1).all(|(j, b)| {
                    if self.towers[i].slot != SAVED_FREE_SLOT && self.towers[i].slot == self.towers[j].slot {
                        return true;
                    }
                    let dx = a[0] - b[0];
                    let dy = a[1] - b[1];
                    dx * dx + dy * dy >= separation * separation
                })
            })
        });
        let difficulty = Difficulty::from_u8(self.difficulty);
        let saved_phase = phase_from_u8(self.phase);
        let doctrine_total = self.doctrines.iter().map(|&rank| rank as u16).sum::<u16>();
        let valid_doctrines = if is_campaign {
            // Campaign awards one reinforcement after each resolved commander
            // through E590. At Build, `wave` is the completed encounter; an
            // active Combat E10 has not yet resolved its commander, so its
            // completed cursor is one encounter behind. E600 is the finale,
            // not a sixty-first reward.
            let completed = match saved_phase {
                Some(Phase::Build) => self.wave,
                Some(Phase::Combat) => self.wave.saturating_sub(1),
                _ => 0,
            };
            let resolved = (completed / 10).min(59) as u16;
            let chosen_limit = resolved.saturating_sub(u16::from(self.pending_doctrine));
            doctrine_total <= chosen_limit
        } else {
            // Preserve the generated Legacy campaign's three-per-stat and
            // three-total doctrine contract exactly.
            self.doctrines.iter().all(|&rank| rank <= 3) && doctrine_total <= 3
        };
        let pending_ok = !self.pending_doctrine
            || is_campaign
            || (difficulty != Difficulty::Classic && matches!(self.wave, 10 | 20 | 30));
        let restored_creeps: Vec<Creep> = self
            .creeps
            .iter()
            .map(SavedCreep::restore)
            .collect::<Option<Vec<_>>>()
            .unwrap_or_default();
        let mut creep_uids: Vec<u32> = restored_creeps.iter().map(|c| c.uid).collect();
        creep_uids.sort_unstable();
        let unique_uids = creep_uids.windows(2).all(|w| w[0] != w[1]);
        let finite_clock = [
            self.time,
            self.speed,
            self.wave_timer,
            self.spawn_timer,
            self.campaign_pressure_grace,
        ]
            .into_iter()
            .all(f32::is_finite);
        let sane_positions = restored_creeps.iter().all(|c| {
            (-1.0..=g.board.total + 1.0).contains(&c.dist)
                && c.lane.abs() <= 3.0
                && c.laps <= 1_000_000
        });
        let legacy_wave_count = if self.wave == 0 {
            0
        } else {
            crate::game::defs::wave_at(self.wave).count
        };
        let valid_campaign = if is_campaign {
            let Some(state) = self.campaign.as_ref() else { return false };
            let encounter = state.current();
            let packet_count = encounter.packets.len();
            let total_bodies: u32 = encounter.packets.iter().map(|p| p.bodies as u32).sum();
            let queued_ok = match state.queued_packet {
                Some(index) => (index as usize) == state.deployed_packets as usize
                    && (index as usize) < packet_count
                    && state.queued_bodies_left > 0
                    && state.queued_bodies_left
                        <= encounter.packets[index as usize].bodies,
                None => state.queued_bodies_left == 0,
            };
            let phase_ok = matches!(saved_phase, Some(Phase::Build | Phase::Combat));
            let wave_ok = match saved_phase {
                Some(Phase::Build) => self.wave.saturating_add(1) == state.encounter as u32
                    || (self.prep && self.wave == 0 && state.encounter == 1),
                Some(Phase::Combat) => self.wave == state.encounter as u32,
                _ => false,
            };
            // Continuous Campaign formation scheduling deliberately leaves
            // older living bodies on the ring when the next encounter begins.
            // They are still real campaign bodies, not a malformed Legacy
            // carry-over, so preserve their encounter attribution rather than
            // rejecting an otherwise exact save at C1 -> C2 or later.
            let creep_history_ok = restored_creeps.iter().all(|c| {
                (1..=state.encounter).contains(&c.campaign_encounter)
            });
            let expected_budget = state.in_flight_budget.unwrap_or_else(|| {
                difficulty.campaign_encounter_budget(encounter.reward, encounter.id.chapter, encounter.id.local)
            });
            let max_reward = state.in_flight_budget.unwrap_or_else(|| encounter.reward.max(expected_budget));
            let min_deployment = state.in_flight_budget.map(|b| b * 40 / 100).unwrap_or_else(|| (expected_budget * 40 / 100).min(encounter.reward * 40 / 100));
            let complete_ok = !state.complete
                || (state.encounter == crate::game::campaign::REALISTIC_ENCOUNTERS
                    && matches!(saved_phase, Some(Phase::Combat))
                    && self.wave == crate::game::campaign::REALISTIC_ENCOUNTERS as u32
                    && state.queued_packet.is_none()
                    && state.deployed_packets as usize == packet_count
                    && (state.reward_issued == expected_budget || (state.in_flight_budget.is_none() && state.reward_issued == encounter.reward)));
            state.version >= 2
                && complete_ok
                && (1..=crate::game::campaign::REALISTIC_ENCOUNTERS).contains(&state.encounter)
                && state.elapsed_seconds.is_finite()
                && state.active_seconds.is_finite()
                && state.packet_spawn_timer.is_finite()
                && state.speed.is_finite()
                && state.elapsed_seconds >= 0.0
                // The packet schedule ends at the commander arrival, but a
                // real boss cleanup continues afterwards. Bound malformed
                // clocks without rejecting a player who saves mid-cleanup.
                && state.elapsed_seconds <= encounter.duration_seconds as f32 + 3600.0
                && state.active_seconds >= 0.0
                && state.commander_window_at.is_finite()
                && state.commander_window_at >= -1.0
                && state.commander_window_at <= state.elapsed_seconds + 0.01
                && state.commander_spawned_at.is_none_or(|at| {
                    at.is_finite() && at >= 0.0 && at <= state.elapsed_seconds + 0.01
                })
                && state.commander_triggers <= 0x0f
                && state.deployed_packets as usize <= packet_count
                && state.spawned_bodies <= total_bodies
                && state.reward_issued <= max_reward
                && state.reward_paid <= max_reward
                && (!state.deployment_paid || state.reward_issued >= min_deployment)
                && queued_ok
                && self.spawn_left == state.queued_bodies_left as u32
                && self.campaign_pressure_grace >= 0.0
                && self.campaign_pressure_grace <= crate::game::campaign::BREACH_SECONDS + 0.01
                && !self.endless
                && phase_ok
                && wave_ok
                && creep_history_ok
        } else {
            self.campaign.is_none()
        };
        if !valid
            || !positions_do_not_overlap
            || !valid_doctrines
            || !pending_ok
            || !valid_campaign
            || !finite_clock
            || self.rng_state == 0
            || self.time < 0.0
            // Campaign has its own explicit rapid ladder through the
            // player-requested 100x maximum. Legacy remains capped at its
            // historical ladder, so a corrupted Legacy envelope cannot use
            // the Campaign limit as a loophole.
            || !(1.0..=if is_campaign {
                crate::game::MAX_CAMPAIGN_SPEED
            } else {
                crate::game::MAX_ENDLESS_SPEED
            })
                .contains(&self.speed)
            || self.wave_timer < 0.0
            || self.spawn_timer < 0.0
            || !sane_positions
            || restored_creeps.len() != self.creeps.len()
            || restored_creeps.len() != self.circling as usize
            || restored_creeps.len() > MAX_CREEPS
            || (!is_campaign && restored_creeps.len() > difficulty.flood_limit(self.wave))
            || !unique_uids
            || (!is_campaign && self.spawn_left > legacy_wave_count)
            || saved_phase.is_some_and(|phase| matches!(phase, Phase::Defeat | Phase::Victory))
        {
            return false;
        }

        if is_campaign {
            g.start_campaign(self.seed, difficulty);
            g.campaign = self.campaign.clone();
            if let Some(state) = g.campaign.as_mut() {
                let enc = state.current();
                let scaled_budget = difficulty.campaign_encounter_budget(enc.reward, enc.id.chapter, enc.id.local);
                if state.in_flight_budget.is_none() && state.deployment_paid {
                    let total_bodies: u32 = enc.packets.iter().map(|p| p.bodies as u32).sum();
                    let scaled_deployment = scaled_budget * 40 / 100;
                    let scaled_kill_budget = scaled_budget.saturating_sub(scaled_deployment);
                    let scaled_per_body = scaled_kill_budget / total_bodies.max(1);
                    let scaled_remainder = scaled_kill_budget % total_bodies.max(1);
                    let spawned = state.spawned_bodies.min(total_bodies);
                    let scaled_spawned_bounties = scaled_per_body * spawned + scaled_remainder.min(spawned);
                    let expected_scaled_issued = scaled_deployment + scaled_spawned_bounties;

                    let is_scaled_mid_packet = state.reward_issued == expected_scaled_issued;
                    let is_scaled_fully_issued = state.spawned_bodies == total_bodies && state.reward_issued == scaled_budget;

                    if scaled_budget != enc.reward && (is_scaled_mid_packet || is_scaled_fully_issued) {
                        // Intermediate scaled-v12 save that already used the scaled budget
                        state.in_flight_budget = Some(scaled_budget);
                    } else {
                        // Genuine old missing-field save: preserve original raw encounter.reward budget
                        state.in_flight_budget = Some(enc.reward);
                    }
                }
            }
            g.campaign_pressure_grace = self.campaign_pressure_grace;
        } else {
            g.start_run_with_difficulty(self.seed, difficulty);
        }
        g.wave = self.wave;
        g.gold = self.gold;
        g.time = self.time;
        g.speed = self.speed;
        g.wave_timer = self.wave_timer.max(0.0);
        g.prep = self.prep;
        g.spawn_left = self.spawn_left;
        g.spawn_timer = self.spawn_timer.max(0.0);
        g.next_uid = self.next_uid.max(
            restored_creeps
                .iter()
                .map(|c| c.uid)
                .max()
                .unwrap_or(0)
                .saturating_add(1)
                .max(1),
        );
        g.endless = self.endless && !is_campaign;
        g.doctrines = self.doctrines;
        g.pending_doctrine = self.pending_doctrine;
        g.paused = self.pending_doctrine;
        g.stats.kills = self.kills;
        g.stats.leaked = self.leaked;
        g.stats.gold_earned = self.gold_earned;
        g.stats.gold_spent = self.gold_spent;
        g.stats.damage = self.damage;
        g.stats.towers_built = self.towers_built;
        g.stats.clean_sweeps = self.clean_sweeps;
        g.stats.rush_gold = self.rush_gold;

        // Rebuilt through the same footprint path a click takes. A legacy pad
        // is preserved at its original centre; a free tower remains at its
        // stored world position rather than being snapped back to a socket.
        for t in &self.towers {
            let Some(pos) = saved_tower_position(t, &g.board) else { continue };
            let legacy_slot = saved_legacy_slot(t, &g.board);
            g.build_choice = Some((t.def as usize, 1));
            let ok = g.restore_build_at(pos, legacy_slot);
            if !ok {
                continue;
            }
            let ti = g.towers.len() - 1;
            g.towers[ti].invested = t.invested;
            g.towers[ti].kills = t.kills;
            g.towers[ti].damage = t.damage;
            g.towers[ti].gold_earned = t.gold_earned;
            g.towers[ti].mode = mode_from_u8(t.mode);
            g.towers[ti].cooldown = t.cooldown;
            g.towers[ti].angle = t.angle;
            g.towers[ti].target_uid = t.target_uid;
            g.towers[ti].ramp = t.ramp;
            g.towers[ti].frenzy_cd = t.frenzy_cd;
            g.towers[ti].aura_timer = t.aura_timer;
            g.towers[ti].flash = t.flash;
            g.towers[ti].built_at = t.built_at;
        }
        g.build_choice = None;
        g.selected = None;
        g.stats.towers_built = self.towers_built;
        g.rebuild_auras();
        g.creeps = restored_creeps;
        // Building the saved tower list creates construction particles and
        // consumes randomness. Neither happened at resume time in the saved
        // run, so discard those effects and restore the exact RNG state last.
        g.fx.particles.clear();
        g.projs.clear();
        g.beams.clear();
        g.texts.clear();
        g.sound_cues.clear();
        g.rng = Rng::from_state(self.rng_state);
        g.phase = saved_phase.unwrap_or_else(|| {
            if self.wave == 0 && self.prep {
                Phase::Build
            } else {
                Phase::Combat
            }
        });
        g.wants_save = false;
        true
    }

    /// A one-line summary for the menu button.
    pub fn label(&self) -> String {
        if RunMode::from_u8(self.mode) == RunMode::Campaign {
            if let Some(state) = &self.campaign {
                let id = crate::game::campaign::encounter_id(state.encounter);
                return format!(
                    "Campaign / Chapter {} / encounter {} of 600 / {} towers",
                    id.chapter,
                    id.local,
                    self.towers.len(),
                );
            }
            return "Campaign save needs repair".to_owned();
        }
        format!(
            "{} · wave {} of {} · {} towers · {} circling",
            Difficulty::from_u8(self.difficulty).label(),
            self.wave,
            crate::game::defs::CAMPAIGN_WAVES,
            self.towers.len(),
            self.circling
        )
    }
}

// ---------------------------------------------------------------- storage

pub fn store(g: &Game) {
    // A finished run is not worth resuming into.
    if matches!(g.phase, Phase::Defeat | Phase::Victory) {
        clear();
        return;
    }
    let Ok(text) = serde_json::to_string(&Save::capture(g)) else {
        return;
    };
    write(&text);
}

pub fn load() -> Option<Save> {
    let text = read()?;
    let save: Save = serde_json::from_str(&text).ok()?;
    let difficulty = Difficulty::from_u8(save.difficulty);
    let compatible = save.version == VERSION
        || save.version == PREVIOUS_VERSION
        || save.version == LEGACY_VERSION;
    let capacity_ok = RunMode::from_u8(save.mode) == RunMode::Campaign
        || save.circling as usize <= difficulty.flood_limit(save.wave);
    (compatible && capacity_ok).then_some(save)
}

pub fn clear() {
    #[cfg(target_arch = "wasm32")]
    {
        if let Some(s) = storage() {
            let _ = s.remove_item(KEY);
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        if let Some(p) = path() {
            let _ = std::fs::remove_file(p);
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn storage() -> Option<web_sys::Storage> {
    web_sys::window()?.local_storage().ok()?
}

#[cfg(target_arch = "wasm32")]
fn write(text: &str) {
    if let Some(s) = storage() {
        let _ = s.set_item(KEY, text);
    }
}

#[cfg(target_arch = "wasm32")]
fn read() -> Option<String> {
    storage()?.get_item(KEY).ok()?
}

#[cfg(not(target_arch = "wasm32"))]
fn path() -> Option<std::path::PathBuf> {
    let base = std::env::var_os("APPDATA")
        .or_else(|| std::env::var_os("XDG_CONFIG_HOME"))
        .or_else(|| std::env::var_os("HOME"))?;
    Some(
        std::path::PathBuf::from(base)
            .join("green_circle_td")
            .join("save.json"),
    )
}

#[cfg(not(target_arch = "wasm32"))]
fn write(text: &str) {
    let Some(p) = path() else { return };
    if let Some(dir) = p.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::write(p, text);
}

#[cfg(not(target_arch = "wasm32"))]
fn read() -> Option<String> {
    std::fs::read_to_string(path()?).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn played_game() -> Game {
        let mut g = Game::new();
        g.start_run_with_difficulty(0x5A7E, Difficulty::Veteran);
        g.gold = 5_000_000;
        // Every shop root, each pushed a few steps up its own path, so the
        // fixture covers forks as well as straight ladders.
        let shop = crate::game::defs::shop_order();
        for (n, slot) in (0..g.board.slots.len()).step_by(3).enumerate().take(12) {
            let def = shop[n % shop.len()];
            g.build_choice = Some((def, 1));
            if g.try_build(slot) {
                let ti = g.towers.len() - 1;
                for _ in 0..4 {
                    let before = g.towers[ti].def;
                    let choices = g.upgrade_choices(ti);
                    match choices.first() {
                        Some(&(into, _)) => g.upgrade_into(ti, into),
                        None => break,
                    }
                    if g.towers[ti].def == before {
                        break;
                    }
                }
            }
        }
        g.build_choice = None;
        g.selected = None;
        g.wave = 23;
        g.gold = 4_321;
        g.doctrines = [1, 1, 0];
        g.rebuild_auras();
        g.phase = Phase::Combat;
        g.prep = false;
        g.time = 731.25;
        g.wave_timer = 17.75;
        g.spawn_timer = 0.19;
        g.spawn_left = 7;
        let w = g.wave_def(g.wave);
        g.spawn_creep(&w, w.hp, 1.0, 13.5);
        g.spawn_creep(&w, w.hp * 0.65, 0.92, 8.0);
        g.creeps[0].hp *= 0.73;
        g.creeps[0].laps = 2;
        g.creeps[0].slow = Timed { amt: 0.34, t: 1.7 };
        g.creeps[0].poison = Timed { amt: 21.0, t: 3.2 };
        g.creeps[0].stun_dr = 0.38;
        g.creeps[0].push_left = 2.4;
        g.creeps[1].burn = Timed { amt: 13.0, t: 2.1 };
        g.creeps[1].suppress = 1.25;
        if let Some(t) = g.towers.first_mut() {
            t.cooldown = 0.37;
            t.angle = 1.23;
            t.target_uid = g.creeps[0].uid;
            t.ramp = 0.44;
            t.frenzy_cd = 2.6;
            t.aura_timer = 0.18;
            t.flash = 0.09;
            t.built_at = 42.0;
        }
        g.stats.kills = 987;
        g.stats.clean_sweeps = 6;
        g.stats.rush_gold = 432;
        g
    }

    fn active_campaign_game() -> Game {
        let mut g = Game::new();
        g.start_campaign(0xCA11_A11, Difficulty::Veteran);
        // Verify that the actual player-facing 10x tempo survives the same
        // Campaign envelope that preserves packets, traits and rewards.
        g.speed = crate::game::MAX_CAMPAIGN_SPEED;
        g.send_wave();
        // Enter part of the first packet so this covers the persisted queue,
        // live trait/body fields, speed, and Combat phase—not only a clean
        // encounter boundary.
        g.update(0.125);
        g
    }

    fn campaign_build_save_after(encounter: u32, doctrines: [u8; 3], pending: bool) -> Save {
        assert!(encounter.is_multiple_of(10) && encounter < 600);
        let mut game = Game::new();
        game.start_campaign(0xC0AA_A000 + encounter as u64, Difficulty::Veteran);
        game.wave = encounter;
        game.phase = Phase::Build;
        game.prep = false;
        game.pending_doctrine = pending;
        game.paused = pending;
        game.doctrines = doctrines;
        game.campaign.as_mut().unwrap().encounter = (encounter + 1) as u16;
        Save::capture(&game)
    }

    #[test]
    fn campaign_reinforcement_ranks_round_trip_beyond_legacy_caps() {
        // Concentrating six legal C1 choices in Arsenal and spreading seven
        // across C1 + C2 are both valid Campaign states; Legacy's per-rank
        // cap must not make either save unresumable.
        for (encounter, doctrines, pending) in [
            (60, [6, 0, 0], false),
            (70, [3, 2, 2], false),
            // E60 is defeated but its sixth reinforcement is still awaiting
            // a choice, so only five saved ranks are legal here.
            (60, [5, 0, 0], true),
        ] {
            let save = campaign_build_save_after(encounter, doctrines, pending);
            let mut restored = Game::new();
            assert!(save.restore(&mut restored), "legal Campaign ranks at E{encounter} were rejected");
            assert_eq!(restored.doctrines, doctrines);
            assert_eq!(restored.pending_doctrine, pending);
        }

        let forged = campaign_build_save_after(60, [7, 0, 0], false);
        assert!(!forged.restore(&mut Game::new()), "forged Campaign reward count was accepted");

        // E10 is an active commander fight, not a completed commander reward.
        // A hand-edited Combat save may not claim its first perk early.
        let mut active_e10 = Game::new();
        active_e10.start_campaign(0xC0AA_E010, Difficulty::Veteran);
        active_e10.phase = Phase::Combat;
        active_e10.prep = false;
        active_e10.wave = 10;
        active_e10.campaign.as_mut().unwrap().encounter = 10;
        active_e10.doctrines = [1, 0, 0];
        let forged_active_e10 = Save::capture(&active_e10);
        assert!(
            !forged_active_e10.restore(&mut Game::new()),
            "Combat E10 accepted an unearned commander perk"
        );

        let mut legacy_rank = Save::capture(&played_game());
        legacy_rank.doctrines = [4, 0, 0];
        assert!(!legacy_rank.restore(&mut Game::new()), "Legacy per-rank cap changed");
        let mut legacy_total = Save::capture(&played_game());
        legacy_total.doctrines = [3, 1, 0];
        assert!(!legacy_total.restore(&mut Game::new()), "Legacy doctrine total cap changed");
    }

    fn overlapping_campaign_game() -> Game {
        let mut g = Game::new();
        g.start_campaign(0x0FEA_1A9, Difficulty::Classic);
        g.speed = crate::game::MAX_CAMPAIGN_SPEED;
        // This fixture is about the persistence envelope, not whether an
        // intentionally undefended board loses. Let the real scheduler run
        // C1 into C2 while retaining every physical C1 survivor.
        g.campaign_pressure_grace = 1_000_000.0;
        g.send_wave();
        for _ in 0..150 {
            g.update(0.10);
            if g.phase == Phase::Combat
                && g.campaign_encounter() == Some(2)
                && g.creeps.iter().any(|c| c.campaign_encounter == 1)
                && g.creeps.iter().any(|c| c.campaign_encounter == 2)
            {
                g.campaign_pressure_grace = crate::game::campaign::BREACH_SECONDS;
                return g;
            }
        }
        panic!("Campaign did not produce an overlapping C1/C2 persistence fixture");
    }

    /// A resumed run has to be the run that was saved, down to the fork every
    /// tower took - restoring a board the player did not build is worse than
    /// not restoring at all.
    #[test]
    fn a_run_survives_a_round_trip() {
        let before = played_game();
        let save = Save::capture(&before);
        let text = serde_json::to_string(&save).expect("serialises");

        let back: Save = serde_json::from_str(&text).expect("deserialises");
        let mut after = Game::new();
        assert!(
            back.restore(&mut after),
            "a save this build wrote must restore"
        );

        assert_eq!(after.wave, before.wave);
        assert_eq!(after.difficulty, Difficulty::Veteran);
        assert_eq!(after.gold, before.gold);
        assert_eq!(after.creeps.len(), before.creeps.len());
        assert_eq!(
            after.seed, before.seed,
            "the seed is what makes the waves match"
        );
        assert_eq!(after.stats.kills, before.stats.kills);
        assert_eq!(after.stats.clean_sweeps, before.stats.clean_sweeps);
        assert_eq!(after.stats.rush_gold, before.stats.rush_gold);
        assert_eq!(after.doctrines, before.doctrines);
        assert_eq!(after.towers.len(), before.towers.len());
        assert_eq!(after.rng.state(), before.rng.state());
        assert_eq!(after.time, before.time);
        assert_eq!(after.wave_timer, before.wave_timer);
        assert_eq!(after.spawn_timer, before.spawn_timer);
        assert_eq!(after.spawn_left, before.spawn_left);
        assert_eq!(after.next_uid, before.next_uid);

        for (a, b) in after.creeps.iter().zip(&before.creeps) {
            assert_eq!(a.uid, b.uid);
            assert_eq!(a.dist, b.dist);
            assert_eq!(a.pos, b.pos);
            assert_eq!(a.hp, b.hp);
            assert_eq!(a.max_hp, b.max_hp);
            assert_eq!(a.model, b.model);
            assert_eq!(a.armour_type, b.armour_type);
            assert_eq!(a.laps, b.laps);
            assert_eq!((a.slow.amt, a.slow.t), (b.slow.amt, b.slow.t));
            assert_eq!((a.burn.amt, a.burn.t), (b.burn.amt, b.burn.t));
            assert_eq!((a.poison.amt, a.poison.t), (b.poison.amt, b.poison.t));
            assert_eq!(a.stun_dr, b.stun_dr);
            assert_eq!(a.suppress, b.suppress);
            assert_eq!(a.push_left, b.push_left);
        }

        let mut a: Vec<_> = after.towers.iter().map(|t| (t.slot, t.def)).collect();
        let mut b: Vec<_> = before.towers.iter().map(|t| (t.slot, t.def)).collect();
        a.sort_unstable();
        b.sort_unstable();
        assert_eq!(a, b, "the restored board is not the board that was saved");
        assert_eq!(after.towers[0].cooldown, before.towers[0].cooldown);
        assert_eq!(after.towers[0].angle, before.towers[0].angle);
        assert_eq!(after.towers[0].target_uid, before.towers[0].target_uid);
        assert_eq!(after.towers[0].ramp, before.towers[0].ramp);
        assert_eq!(after.towers[0].built_at, before.towers[0].built_at);

        // The pads have to agree with the towers, or selling one corrupts the board.
        for (i, t) in after.towers.iter().enumerate() {
            assert_eq!(
                after.board.slots[t.slot].tower,
                Some(i),
                "pad {} disagrees",
                t.slot
            );
        }

        // And the next wave must be the same one the saved run was facing.
        assert_eq!(after.next_wave_def().name, before.next_wave_def().name);
    }

    #[test]
    fn campaign_mid_packet_save_resumes_the_same_mode_and_cursor() {
        let before = active_campaign_game();
        let state_before = before.campaign.clone().expect("live campaign state");
        assert!(before.is_campaign());
        assert!(before.spawn_left > 0 || !before.creeps.is_empty());
        let encoded = serde_json::to_string(&Save::capture(&before)).expect("campaign serializes");
        let saved: Save = serde_json::from_str(&encoded).expect("campaign deserializes");
        let mut after = Game::new();
        assert!(saved.restore(&mut after), "a Campaign save written by this build restores");
        assert_eq!(after.mode, RunMode::Campaign);
        assert_eq!(after.phase, Phase::Combat);
        assert_eq!(after.speed, crate::game::MAX_CAMPAIGN_SPEED);
        let state_after = after.campaign.as_ref().expect("restored campaign state");
        assert_eq!(state_after.encounter, state_before.encounter);
        assert_eq!(state_after.deployed_packets, state_before.deployed_packets);
        assert_eq!(state_after.queued_packet, state_before.queued_packet);
        assert_eq!(state_after.queued_bodies_left, state_before.queued_bodies_left);
        assert_eq!(state_after.spawned_bodies, state_before.spawned_bodies);
        assert_eq!(state_after.reward_issued, state_before.reward_issued);
        assert_eq!(after.spawn_left, before.spawn_left);
        assert_eq!(after.creeps.len(), before.creeps.len());
        assert!(after
            .creeps
            .iter()
            .all(|c| c.campaign_encounter == state_after.encounter));
    }

    #[test]
    fn campaign_old_missing_field_save_halfway_first_packet_preserves_policy_and_converges() {
        let mut before = Game::new();
        before.start_campaign(0xCA11_A11, Difficulty::Veteran);
        before.speed = 1.0;
        before.send_wave();

        // Step until halfway through first packet (first packet has 39 bodies; spawn 20)
        while before.campaign.as_ref().unwrap().spawned_bodies < 20 {
            before.update(0.05);
        }
        let spawned = before.campaign.as_ref().unwrap().spawned_bodies;
        assert_eq!(spawned, 20);
        let gold_before = before.gold;

        // Serialize and strip in_flight_budget to simulate genuine old save
        let mut save_val = serde_json::to_value(Save::capture(&before)).expect("serializes to json");
        let camp_obj = save_val
            .get_mut("campaign")
            .expect("campaign field")
            .as_object_mut()
            .expect("campaign object");
        camp_obj.remove("in_flight_budget");

        // Old raw ledger: raw deployment is 93 * 40 / 100 = 37.
        // Raw kill budget is 93 - 37 = 56. 20 bodies spawned -> 37 + 20 = 57.
        camp_obj.insert("reward_issued".to_string(), serde_json::Value::from(57));
        camp_obj.insert("reward_paid".to_string(), serde_json::Value::from(57));

        let legacy_save: Save = serde_json::from_value(save_val).expect("deserializes legacy save");
        let mut after = Game::new();
        assert!(
            legacy_save.restore(&mut after),
            "old missing-field save must restore cleanly"
        );

        // Verify that original raw budget (93) is preserved and stored ledger/gold are intact
        let camp_after = after.campaign.as_ref().unwrap();
        assert_eq!(
            camp_after.in_flight_budget,
            Some(93),
            "Old save must finish under original raw encounter.reward budget policy (93)"
        );
        assert_eq!(camp_after.reward_issued, 57, "reward_issued must be preserved untouched");
        assert_eq!(after.gold, gold_before, "gold must not be retroactively altered");
        assert_eq!(camp_after.spawned_bodies, 20);

        // Run until encounter 1 completes and transitions to encounter 2
        after.campaign_pressure_grace = 1_000_000.0;
        let mut steps = 0;
        while after.campaign_encounter() == Some(1) && steps < 3_000 {
            for ci in 0..after.creeps.len() {
                crate::game::combat::damage_creep(&mut after, ci, 1_000_000.0, usize::MAX, false);
            }
            after.update(0.1);
            steps += 1;
        }

        assert_eq!(
            after.campaign_encounter(),
            Some(2),
            "Encounter 1 must finish under original policy and advance to encounter 2"
        );
        assert_eq!(after.phase, Phase::Build);
        assert_eq!(after.campaign.as_ref().unwrap().in_flight_budget, None, "budget resets for next encounter");

        // Starting encounter 2 now uses the newly scaled Veteran budget
        after.send_wave();
        let enc2_budget = Difficulty::Veteran.campaign_encounter_budget(
            after.campaign.as_ref().unwrap().current().reward, 1, 2
        );
        assert_eq!(
            after.campaign.as_ref().unwrap().in_flight_budget,
            Some(enc2_budget),
            "Encounter 2 must use the new scaled difficulty budget"
        );
    }

    #[test]
    fn campaign_old_missing_field_save_between_packets_with_dead_bodies_preserves_policy_and_converges() {
        let mut before = Game::new();
        before.start_campaign(0xCA11_A11, Difficulty::Veteran);
        before.speed = 1.0;
        before.send_wave();

        // Wait until packet 1 (39 bodies) finishes spawning, but before packet 2 at 10s
        while before.campaign.as_ref().unwrap().spawned_bodies < 39 {
            before.update(0.05);
        }
        assert_eq!(before.campaign.as_ref().unwrap().spawned_bodies, 39);
        assert_eq!(before.campaign.as_ref().unwrap().deployed_packets, 1);
        assert!(before.campaign.as_ref().unwrap().queued_packet.is_none());

        // Kill half the creeps from packet 1 so there are dead bodies
        let initial_creeps = before.creeps.len();
        assert!(initial_creeps >= 30);
        for ci in 0..15 {
            crate::game::combat::damage_creep(&mut before, ci, 1_000_000.0, usize::MAX, false);
        }
        before.update(0.01);
        assert!(before.creeps.len() < initial_creeps, "some bodies are now dead");

        let gold_before = before.gold;
        let mut save_val = serde_json::to_value(Save::capture(&before)).expect("serializes to json");
        let camp_obj = save_val
            .get_mut("campaign")
            .expect("campaign field")
            .as_object_mut()
            .expect("campaign object");
        camp_obj.remove("in_flight_budget");
        // 39 bodies spawned under raw calculation: 37 deployment + 39 kills = 76
        camp_obj.insert("reward_issued".to_string(), serde_json::Value::from(76));
        camp_obj.insert("reward_paid".to_string(), serde_json::Value::from(76));

        let legacy_save: Save = serde_json::from_value(save_val).expect("deserializes legacy save");
        let mut after = Game::new();
        assert!(
            legacy_save.restore(&mut after),
            "old save between packets with dead bodies must restore cleanly"
        );

        let camp_after = after.campaign.as_ref().unwrap();
        assert_eq!(
            camp_after.in_flight_budget,
            Some(93),
            "Original raw encounter budget (93) preserved"
        );
        assert_eq!(camp_after.reward_issued, 76);
        assert_eq!(after.gold, gold_before);

        // Step until encounter 1 finishes
        after.campaign_pressure_grace = 1_000_000.0;
        let mut steps = 0;
        while after.campaign_encounter() == Some(1) && steps < 3_000 {
            for ci in 0..after.creeps.len() {
                crate::game::combat::damage_creep(&mut after, ci, 1_000_000.0, usize::MAX, false);
            }
            after.update(0.1);
            steps += 1;
        }

        assert_eq!(
            after.campaign_encounter(),
            Some(2),
            "Encounter 1 must complete and advance to encounter 2 without stalling"
        );
        assert_eq!(after.phase, Phase::Build);
        assert_eq!(after.campaign.as_ref().unwrap().in_flight_budget, None);
    }

    #[test]
    fn campaign_explicit_budget_exact_resume_converges() {
        let mut before = Game::new();
        before.start_campaign(0xCA11_A11, Difficulty::Veteran);
        before.speed = 2.0;
        before.send_wave();

        // Advance into combat: 50 bodies spawned
        while before.campaign.as_ref().unwrap().spawned_bodies < 50 {
            before.update(0.05);
        }
        let state_before = before.campaign.clone().unwrap();
        assert_eq!(state_before.in_flight_budget, Some(76));

        let save = Save::capture(&before);
        let save_text = serde_json::to_string(&save).expect("serializes");
        assert!(save_text.contains("\"in_flight_budget\":76"));

        let restored_save: Save = serde_json::from_str(&save_text).expect("deserializes");
        let mut after = Game::new();
        assert!(
            restored_save.restore(&mut after),
            "explicit budget save must restore"
        );

        let state_after = after.campaign.as_ref().unwrap();
        assert_eq!(state_after.in_flight_budget, Some(76));
        assert_eq!(state_after.reward_issued, state_before.reward_issued);
        assert_eq!(state_after.spawned_bodies, state_before.spawned_bodies);

        // Run until encounter 1 completes
        after.campaign_pressure_grace = 1_000_000.0;
        let mut steps = 0;
        while after.campaign_encounter() == Some(1) && steps < 3_000 {
            for ci in 0..after.creeps.len() {
                crate::game::combat::damage_creep(&mut after, ci, 1_000_000.0, usize::MAX, false);
            }
            after.update(0.1);
            steps += 1;
        }

        assert_eq!(
            after.campaign_encounter(),
            Some(2),
            "Explicit budget encounter must transition to encounter 2"
        );
        assert_eq!(after.phase, Phase::Build);
        assert_eq!(after.campaign.as_ref().unwrap().in_flight_budget, None);
    }

    #[test]
    fn campaign_intermediate_scaled_v12_save_detects_budget_and_converges() {
        let mut before = Game::new();
        before.start_campaign(0xCA11_A11, Difficulty::Veteran);
        before.speed = 2.0;
        before.send_wave();

        while before.campaign.as_ref().unwrap().spawned_bodies < 50 {
            before.update(0.05);
        }
        let spawned = before.campaign.as_ref().unwrap().spawned_bodies;
        assert!(spawned >= 50);

        let mut save_val = serde_json::to_value(Save::capture(&before)).expect("serializes to json");
        let camp_obj = save_val
            .get_mut("campaign")
            .expect("campaign field")
            .as_object_mut()
            .expect("campaign object");
        camp_obj.remove("in_flight_budget");

        let legacy_save: Save = serde_json::from_value(save_val).expect("deserializes intermediate save");
        let mut after = Game::new();
        assert!(legacy_save.restore(&mut after));

        let camp_after = after.campaign.as_ref().unwrap();
        assert_eq!(camp_after.in_flight_budget, Some(76), "Intermediate scaled save must be detected as 76");

        after.campaign_pressure_grace = 1_000_000.0;
        let mut steps = 0;
        while after.campaign_encounter() == Some(1) && steps < 3_000 {
            for ci in 0..after.creeps.len() {
                crate::game::combat::damage_creep(&mut after, ci, 1_000_000.0, usize::MAX, false);
            }
            after.update(0.1);
            steps += 1;
        }

        assert_eq!(after.campaign_encounter(), Some(2));
        assert_eq!(after.phase, Phase::Build);
        assert_eq!(after.campaign.as_ref().unwrap().in_flight_budget, None);
    }

    #[test]
    fn campaign_overlap_save_preserves_historical_survivors_and_current_cursor() {
        let before = overlapping_campaign_game();
        let before_ids: Vec<u16> = before.creeps.iter().map(|c| c.campaign_encounter).collect();
        assert!(before_ids.contains(&1) && before_ids.contains(&2));
        let saved = Save::capture(&before);
        let mut after = Game::new();
        assert!(saved.restore(&mut after), "overlapping Campaign save was rejected");
        let after_ids: Vec<u16> = after.creeps.iter().map(|c| c.campaign_encounter).collect();
        assert_eq!(after.mode, RunMode::Campaign);
        assert_eq!(after.phase, Phase::Combat);
        assert_eq!(after.campaign_encounter(), Some(2));
        assert_eq!(after_ids, before_ids, "resume dropped historical bodies");
        assert!(after_ids.contains(&1) && after_ids.contains(&2));
    }

    #[test]
    fn free_grass_tower_position_survives_save_resume_without_a_hidden_pad() {
        let mut before = Game::new();
        before.gold = 50_000;
        let point = before.first_clear_grass().expect("clear grass exists");
        before.build_choice = Some((crate::game::defs::family_start(crate::game::defs::Family::Single).unwrap(), 1));
        assert!(before.try_build_at(point));
        let saved = Save::capture(&before);
        assert_eq!(saved.towers.len(), 1);
        assert_eq!(saved.towers[0].pos, Some(point));
        assert_eq!(saved.towers[0].slot, SAVED_FREE_SLOT);

        let mut after = Game::new();
        assert!(saved.restore(&mut after));
        assert_eq!(after.towers.len(), 1);
        assert_eq!(after.towers[0].pos, point);
        assert_eq!(after.towers[0].slot, FREE_TOWER_SLOT);
        assert!(after.tower_at(point).is_some(), "world-position selection was not restored");
    }

    #[test]
    fn final_campaign_cleanup_checkpoint_keeps_prior_survivors_resumable() {
        let mut before = Game::new();
        before.start_campaign(0xF1AA_1C1E, Difficulty::Classic);
        before.speed = 1.0;
        let state = before.campaign.as_mut().expect("Campaign state");
        state.encounter = crate::game::campaign::REALISTIC_ENCOUNTERS;
        let final_encounter = state.current();
        assert!(final_encounter.commander.is_some(), "final encounter lost its objective");
        state.elapsed_seconds = final_encounter.duration_seconds as f32;
        state.deployed_packets = final_encounter.packets.len() as u8;
        state.queued_packet = None;
        state.queued_bodies_left = 0;
        state.packet_spawn_timer = 0.0;
        state.spawned_bodies = final_encounter
            .packets
            .iter()
            .map(|packet| packet.bodies as u32)
            .sum();
        state.reward_issued = final_encounter.reward;
        state.reward_paid = final_encounter.reward;
        state.deployment_paid = true;
        before.wave = crate::game::campaign::REALISTIC_ENCOUNTERS as u32;
        before.phase = Phase::Combat;
        before.prep = false;
        let w = before.next_wave_def();
        before.spawn_creep(&w, 10.0, 1.0, 1.0);
        before.creeps[0].campaign_encounter = 1; // a genuine older survivor
        before.update(1.0 / 120.0);
        assert!(before.campaign.as_ref().is_some_and(|state| state.complete));
        assert_eq!(before.phase, Phase::Combat, "historical survivor was not kept for cleanup");

        let saved = Save::capture(&before);
        let mut after = Game::new();
        assert!(saved.restore(&mut after), "final cleanup checkpoint was rejected");
        assert!(after.campaign.as_ref().is_some_and(|state| state.complete));
        assert_eq!(after.phase, Phase::Combat);
        assert_eq!(after.creeps.len(), 1);
        assert_eq!(after.creeps[0].campaign_encounter, 1);
    }

    #[test]
    fn v10_legacy_save_without_campaign_fields_stays_legacy() {
        let before = played_game();
        let mut value = serde_json::to_value(Save::capture(&before)).expect("legacy value");
        let object = value.as_object_mut().expect("save object");
        object.insert("version".to_owned(), serde_json::Value::from(LEGACY_VERSION));
        for key in [
            "mode",
            "campaign",
            "campaign_pressure_grace",
            "speed",
            "phase",
        ] {
            object.remove(key);
        }
        for creep in object
            .get_mut("creeps")
            .and_then(serde_json::Value::as_array_mut)
            .expect("creep array")
        {
            let creep = creep.as_object_mut().expect("creep object");
            for key in [
                "shield",
                "max_shield",
                "regen_per_second",
                "resistant",
                "campaign_encounter",
                "pressure",
                "death_killer",
            ] {
                creep.remove(key);
            }
        }
        let legacy: Save = serde_json::from_value(value).expect("v10 shape still decodes");
        let mut after = Game::new();
        assert!(legacy.restore(&mut after));
        assert_eq!(after.mode, RunMode::Legacy);
        assert!(after.campaign.is_none());
        assert_eq!(after.wave, before.wave);
    }

    /// A save is a file on disk that anyone can edit. It must never be able to
    /// produce a board the game itself would refuse to build.
    #[test]
    fn a_corrupt_save_is_refused_rather_than_half_applied() {
        let mut save = Save::capture(&played_game());
        let good = save.clone();

        for break_it in [
            (|s: &mut Save| s.version = 999) as fn(&mut Save),
            |s: &mut Save| s.towers[0].def = 9_999,
            |s: &mut Save| s.towers[0].slot = 60_000,
            |s: &mut Save| s.doctrines = [3, 3, 3],
            |s: &mut Save| s.rng_state = 0,
            |s: &mut Save| s.wave_timer = f32::NAN,
            |s: &mut Save| s.creeps[0].model = u8::MAX,
            |s: &mut Save| s.creeps[1].uid = s.creeps[0].uid,
            |s: &mut Save| s.spawn_left = u32::MAX,
            |s: &mut Save| s.circling = s.circling.saturating_sub(1),
            // More monsters circling than the ring can hold: a board that
            // had already lost.
            |s: &mut Save| s.circling = u16::MAX,
        ] {
            save = good.clone();
            break_it(&mut save);
            let mut g = Game::new();
            let before = (g.wave, g.gold, g.towers.len());
            assert!(
                !save.restore(&mut g),
                "a broken save was accepted: {save:?}"
            );
            assert_eq!(
                (g.wave, g.gold, g.towers.len()),
                before,
                "a rejected save still changed the game"
            );
        }

        // The unmodified one still works, so the test is not passing by accident.
        let mut g = Game::new();
        assert!(good.restore(&mut g));
    }

    /// Two towers must never end up on one pad, however the save was written.
    #[test]
    fn duplicate_pads_in_a_save_do_not_stack_towers() {
        let mut save = Save::capture(&played_game());
        let first = save.towers[0].clone();
        save.towers.push(first);

        let mut g = Game::new();
        assert!(save.restore(&mut g));
        let mut slots: Vec<u16> = g.towers.iter().map(|t| t.slot as u16).collect();
        slots.sort_unstable();
        let n = slots.len();
        slots.dedup();
        assert_eq!(slots.len(), n, "two towers were restored onto one pad");
    }
}
