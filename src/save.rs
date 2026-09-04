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
use crate::game::{Creep, Difficulty, Game, MAX_CREEPS, Phase, TargetMode, Timed};
use crate::rng::Rng;

/// Bumped whenever the shape below changes. An older save is discarded rather
/// than half-read, because a half-restored board is worse than a fresh start.
// Version 7 persists command doctrines, tempo rewards and the exact pressure
// state. Restoring a v6 run without its enemies and global bonuses would
// quietly change the build, so older saves are rejected instead.
const VERSION: u16 = 7;

const KEY: &str = "green_td_save_v7";

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SavedTower {
    /// Index into `defs::TOWERS`, which is family *and* level together, so
    /// nothing else about the tower's position on its path has to be stored.
    pub def: u16,
    pub slot: u16,
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
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Save {
    pub version: u16,
    pub seed: u64,
    pub difficulty: u8,
    pub wave: u32,
    pub gold: i64,
    pub rng_state: u64,
    pub time: f32,
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
        })
    }
}

fn forward_route() -> f32 {
    1.0
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
            seed: g.seed,
            difficulty: g.difficulty.as_u8(),
            wave: g.wave,
            gold: g.gold,
            rng_state: g.rng.state(),
            time: g.time,
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
                    slot: t.slot as u16,
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
        if self.version != VERSION {
            return false;
        }
        let towers = crate::game::defs::TOWERS.len();
        // Every tower in the roster can be reached from the shop, so the only
        // things worth refusing are indices this build does not have.
        let valid = self.towers.iter().all(|t| {
            (t.def as usize) < towers
                && (t.slot as usize) < g.board.slots.len()
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
        let difficulty = Difficulty::from_u8(self.difficulty);
        let valid_doctrines = self.doctrines.iter().all(|&rank| rank <= 3)
            && self.doctrines.iter().copied().sum::<u8>() <= 3;
        let pending_ok = !self.pending_doctrine
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
        let finite_clock = [self.time, self.wave_timer, self.spawn_timer]
            .into_iter()
            .all(f32::is_finite);
        let sane_positions = restored_creeps.iter().all(|c| {
            (-1.0..=g.board.total + 1.0).contains(&c.dist)
                && c.lane.abs() <= 3.0
                && c.laps <= 1_000_000
        });
        let wave_count = if self.wave == 0 {
            0
        } else {
            crate::game::defs::wave_at(self.wave).count
        };
        if !valid
            || !valid_doctrines
            || !pending_ok
            || !finite_clock
            || self.rng_state == 0
            || self.time < 0.0
            || self.wave_timer < 0.0
            || self.spawn_timer < 0.0
            || !sane_positions
            || restored_creeps.len() != self.creeps.len()
            || restored_creeps.len() != self.circling as usize
            || restored_creeps.len() > MAX_CREEPS
            || restored_creeps.len() > difficulty.flood_limit(self.wave)
            || !unique_uids
            || self.spawn_left > wave_count
        {
            return false;
        }

        g.start_run_with_difficulty(self.seed, difficulty);
        g.wave = self.wave;
        g.gold = self.gold;
        g.time = self.time;
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
        g.endless = self.endless;
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

        // Rebuilt through the same path a click takes, so a restored board can
        // never be one the game would refuse to build.
        for t in &self.towers {
            let slot = t.slot as usize;
            if g.board.slots[slot].tower.is_some() {
                continue;
            }
            g.build_choice = Some((t.def as usize, 1));
            let had = g.gold;
            g.gold = i64::MAX / 4; // the cost was already paid, before the save
            let ok = g.try_build(slot);
            g.gold = had;
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
        g.phase = if self.wave == 0 && self.prep {
            Phase::Build
        } else {
            Phase::Combat
        };
        g.wants_save = false;
        true
    }

    /// A one-line summary for the menu button.
    pub fn label(&self) -> String {
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
    (save.version == VERSION && save.circling as usize <= difficulty.flood_limit(save.wave))
        .then_some(save)
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
