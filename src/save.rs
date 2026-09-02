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
//! What is stored is small and declarative - the seed, the wave number, the
//! purse, and one line per tower. Waves are generated from their number, so
//! replaying the seed reproduces the run exactly without storing any of it.

use serde::{Deserialize, Serialize};

use crate::game::defs::{ESSENCE_WAVES, MAX_TIER, tier_cap};
use crate::game::{Game, Phase, TargetMode};

/// Bumped whenever the shape below changes. An older save is discarded rather
/// than half-read, because a half-restored board is worse than a fresh start.
const VERSION: u16 = 2;

const KEY: &str = "elemental_td_save_v2";

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SavedTower {
    pub def: u16,
    pub tier: u8,
    pub slot: u16,
    pub invested: u32,
    pub kills: u32,
    pub damage: f64,
    pub gold_earned: u64,
    pub mode: u8,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Save {
    pub version: u16,
    pub seed: u64,
    pub wave: u32,
    pub gold: i64,
    pub lives: i32,
    pub endless: bool,
    /// Essences held, by element index. Without these a resumed run could not
    /// rebuild the board it saved, let alone upgrade it.
    pub essence: [u8; 6],
    pub drafts_taken: u16,
    pub kills: u64,
    pub leaked: u32,
    pub gold_earned: u64,
    pub gold_spent: u64,
    pub damage: f64,
    pub towers_built: u32,
    pub towers: Vec<SavedTower>,
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
            wave: g.wave,
            gold: g.gold,
            lives: g.lives,
            endless: g.endless,
            essence: g.essence,
            drafts_taken: g.drafts_taken.min(u16::MAX as usize) as u16,
            kills: g.stats.kills,
            leaked: g.stats.leaked,
            gold_earned: g.stats.gold_earned,
            gold_spent: g.stats.gold_spent,
            damage: g.stats.damage,
            towers_built: g.stats.towers_built,
            towers: g
                .towers
                .iter()
                .map(|t| SavedTower {
                    def: t.def as u16,
                    tier: t.tier as u8,
                    slot: t.slot as u16,
                    invested: t.invested,
                    kills: t.kills,
                    damage: t.damage,
                    gold_earned: t.gold_earned,
                    mode: mode_to_u8(t.mode),
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
        // A saved tower must be one this build can construct *at the level it
        // was saved at*, which now depends on the essences saved beside it. A
        // save claiming a level-8 Bastion with one Light essence describes a
        // board the game would never have allowed, so it is refused whole.
        let valid = self.towers.iter().all(|t| {
            (t.def as usize) < towers
                && (1..=MAX_TIER).contains(&(t.tier as u32))
                && (t.slot as usize) < g.board.slots.len()
                && t.tier as u32
                    <= tier_cap(&self.essence, &crate::game::defs::TOWERS[t.def as usize])
        });
        let essence_sane = self.drafts_taken as usize <= ESSENCE_WAVES.len()
            && self.essence.iter().map(|&n| n as usize).sum::<usize>()
                == self.drafts_taken as usize;
        if !valid || !essence_sane || self.lives <= 0 {
            return false;
        }

        g.start_run(self.seed);
        g.wave = self.wave;
        g.gold = self.gold;
        g.lives = self.lives;
        g.endless = self.endless;
        // Restored before the towers, because try_build reads the essence pool
        // to decide whether each one is allowed to exist.
        g.essence = self.essence;
        g.drafts_taken = self.drafts_taken as usize;
        g.pending_draft = None;
        g.stats.kills = self.kills;
        g.stats.leaked = self.leaked;
        g.stats.gold_earned = self.gold_earned;
        g.stats.gold_spent = self.gold_spent;
        g.stats.damage = self.damage;
        g.stats.towers_built = self.towers_built;

        // Rebuilt through the same path a click takes, so a restored board can
        // never be one the game would refuse to build.
        for t in &self.towers {
            let slot = t.slot as usize;
            if g.board.slots[slot].tower.is_some() {
                continue;
            }
            g.build_choice = Some((t.def as usize, t.tier as u32));
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
        }
        g.build_choice = None;
        g.selected = None;
        g.stats.towers_built = self.towers_built;
        g.rebuild_auras();
        g.phase = Phase::Build;
        g.build_timer = crate::game::BUILD_TIME;
        // A run saved mid-draft comes back owing the same draft.
        g.offer_draft_if_due();
        true
    }

    /// A one-line summary for the menu button.
    pub fn label(&self) -> String {
        format!(
            "Wave {} · {} towers · {} essences · {} lives",
            self.wave,
            self.towers.len(),
            self.drafts_taken,
            self.lives
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
    (save.version == VERSION && save.lives > 0).then_some(save)
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
            .join("elemental_td")
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
    use crate::game::defs::TOWERS;

    fn played_game() -> Game {
        let mut g = Game::new();
        g.gold = 500_000;
        // A lopsided pool: deep in two elements, thin in two more, none at
        // all in the last two - so the fixture exercises the essence ceiling
        // rather than a board where everything happens to be legal.
        g.essence = [4, 4, 2, 2, 0, 0];
        g.drafts_taken = 12;
        for (n, slot) in (0..g.board.slots.len()).step_by(3).enumerate().take(12) {
            let def = n % TOWERS.len();
            if !g.unlocked(def) {
                continue;
            }
            g.build_choice = Some((def, 1));
            if g.try_build(slot) {
                let ti = g.towers.len() - 1;
                while g.towers[ti].tier < 5 {
                    let before = g.towers[ti].tier;
                    g.upgrade(ti);
                    if g.towers[ti].tier == before {
                        break;
                    }
                }
            }
        }
        g.build_choice = None;
        g.selected = None;
        g.wave = 23;
        g.gold = 4_321;
        g.lives = 11;
        g.stats.kills = 987;
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
        assert_eq!(after.gold, before.gold);
        assert_eq!(after.lives, before.lives);
        assert_eq!(
            after.seed, before.seed,
            "the seed is what makes the waves match"
        );
        assert_eq!(after.stats.kills, before.stats.kills);
        assert_eq!(
            after.essence, before.essence,
            "a resumed run lost its essences"
        );
        assert_eq!(after.drafts_taken, before.drafts_taken);
        assert_eq!(after.towers.len(), before.towers.len());

        let mut a: Vec<_> = after
            .towers
            .iter()
            .map(|t| (t.slot, t.def, t.tier))
            .collect();
        let mut b: Vec<_> = before
            .towers
            .iter()
            .map(|t| (t.slot, t.def, t.tier))
            .collect();
        a.sort_unstable();
        b.sort_unstable();
        assert_eq!(a, b, "the restored board is not the board that was saved");

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
        assert_eq!(after.next_wave_def().kind, before.next_wave_def().kind);
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
            |s: &mut Save| s.towers[0].tier = 0,
            |s: &mut Save| s.towers[0].tier = 99,
            |s: &mut Save| s.towers[0].slot = 60_000,
            // Essences that could not have built this board.
            |s: &mut Save| s.essence = [0; 6],
            // A pool whose total does not match the number of drafts taken.
            |s: &mut Save| s.drafts_taken = 99,
            |s: &mut Save| s.lives = 0,
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
