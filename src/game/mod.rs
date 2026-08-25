//! Game simulation: the road, the monsters walking it, the towers on the pads.

pub mod board;
pub mod combat;
pub mod defs;
pub mod fx;
#[cfg(test)]
mod tests;

use board::{BH, BW, Board};
use defs::*;
use fx::Fx;

use crate::rng::Rng;

pub const MAX_CREEPS: usize = 4000;
pub const START_GOLD: i64 = 260;
pub const BUILD_TIME_FIRST: f32 = 25.0;
pub const BUILD_TIME: f32 = 16.0;
pub const SELL_REFUND: f32 = 0.75;
/// How much stun resistance one stun adds, and the ceiling it climbs to.
/// At the ceiling a stun still lands, but briefly - hard control should be
/// strong, never absolute.
pub const STUN_DR_STEP: f32 = 0.34;
pub const STUN_DR_MAX: f32 = 0.85;
/// How fast stun resistance bleeds off, per second.
pub const STUN_DR_DECAY: f32 = 0.30;
/// The shortest gap between two knockbacks on the same monster.
pub const KNOCKBACK_CD: f32 = 0.75;

/// Interest paid on gold in hand at the end of every wave.
pub const INTEREST_RATE: f32 = 0.05;
/// The most interest can ever reach, however many Treasuries are built.
///
/// Compound interest with no ceiling is not an economy, it is a runaway. Each
/// Treasury used to add `0.04 * utility_scale(tier)`, which at level 10 is
/// +23.8% *each* - four of them put the rate over 100% and gold doubled every
/// wave. A real game reached 813 billion gold at wave 89 and, with every tower
/// maxed forever, ran to wave 136 without difficulty. Infinite money is the
/// same thing as no game.
pub const INTEREST_MAX: f32 = 0.20;

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

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TargetMode {
    First,
    Last,
    Strongest,
    Closest,
}

impl TargetMode {
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
}

// ---------------------------------------------------------------- entities

#[derive(Clone)]
pub struct Creep {
    pub uid: u32,
    /// How far along the road, in tiles. This is the creep's real position.
    pub dist: f32,
    /// Sideways offset from the road centre so a pack does not walk in a line.
    pub lane: f32,
    /// Cached world position, refreshed every step.
    pub pos: [f32; 2],
    pub facing: f32,
    pub hp: f32,
    pub max_hp: f32,
    pub base_speed: f32,
    pub armor: Armor,
    pub kind: Kind,
    pub radius: f32,
    pub bounty: u32,
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
    pub regen: f32,
    pub splits: u8,
    /// Absorbs damage before health is touched (Bulwark).
    pub shield: f32,
    pub max_shield: f32,
    /// Heals nearby monsters this fraction of max health per second (Mender).
    pub heal: f32,
    /// Ignores slows for half of every second (Phaser).
    pub phasing: bool,
    /// Set each step while phasing is suppressing slows.
    pub slow_off: bool,
    pub flash: f32,
    pub bob: f32,
}

impl Creep {
    #[inline]
    pub fn speed(&self) -> f32 {
        if self.stun > 0.0 {
            return 0.0;
        }
        let slow = if self.slow.active() && !self.slow_off { self.slow.amt } else { 0.0 };
        self.base_speed * (1.0 - slow).max(0.15)
    }
    #[inline]
    pub fn hp_frac(&self) -> f32 {
        (self.hp / self.max_hp).clamp(0.0, 1.0)
    }
    /// Height of the body's centre above the ground, for the 3D view.
    #[inline]
    pub fn height(&self) -> f32 {
        // Flyers ride at their kind's altitude, drifting gently so a formation
        // of them does not look like a decal sheet.
        let alt = self.kind.altitude();
        let drift = if alt > 0.0 { (self.bob * 0.9).sin() * 0.16 } else { 0.0 };
        alt + drift + self.radius * 1.4 + (self.bob.sin() * 0.5 + 0.5) * 0.10
    }
    pub fn flying(&self) -> bool {
        self.kind.flying()
    }
}

#[derive(Clone)]
pub struct Tower {
    pub def: usize,
    pub tier: u32,
    /// Which tier-3 specialisation was chosen.
    pub fork: Option<usize>,
    pub slot: usize,
    pub pos: [f32; 2],
    pub cooldown: f32,
    pub angle: f32,
    pub target_uid: u32,
    pub ramp: f32,
    pub kills: u32,
    pub damage: f64,
    pub invested: u32,
    pub mode: TargetMode,
    pub flash: f32,
    pub built_at: f32,
    /// Aura bonuses from nearby Beacons, recomputed when the board changes.
    pub buff_dmg: f32,
    pub buff_rate: f32,
    pub buff_range: f32,
    /// Gold this tower has personally generated - income and kill bounties.
    pub gold_earned: u64,
}

impl Tower {
    pub fn def(&self) -> &'static TowerDef {
        &TOWERS[self.def]
    }
    pub fn stats(&self) -> Stats {
        self.def().stats(self.tier, self.fork)
    }
    pub fn specials(&self) -> SpecialSet {
        self.def().specials_for(self.fork)
    }
    pub fn dmg(&self) -> f32 {
        self.stats().dmg * (1.0 + self.buff_dmg)
    }
    pub fn rate(&self) -> f32 {
        self.stats().rate * (1.0 + self.buff_rate)
    }
    pub fn range(&self) -> f32 {
        self.stats().range + self.buff_range
    }
    pub fn splash(&self) -> f32 {
        self.stats().splash
    }
    pub fn scale(&self) -> f32 {
        TowerDef::scale(self.tier)
    }
    /// Multiplier applied to auras, income and interest at this level.
    pub fn utility(&self) -> f32 {
        TowerDef::utility_scale(self.tier)
    }
    pub fn dtype(&self) -> Damage {
        self.def().dtype
    }
    /// Display name including the tier-3 specialisation.
    pub fn full_name(&self) -> &'static str {
        match self.fork {
            Some(i) => self.def().forks[i].name,
            None => self.def().name,
        }
    }
    pub fn sell_value(&self) -> u32 {
        (self.invested as f32 * SELL_REFUND).round() as u32
    }
    pub fn upgrade_cost(&self) -> Option<u32> {
        if self.tier >= MAX_TIER {
            return None;
        }
        let d = self.def();
        Some(d.cost_at(self.tier + 1) - d.cost_at(self.tier))
    }
    /// Reaching the fork level means choosing a specialisation.
    pub fn needs_fork_choice(&self) -> bool {
        self.tier + 1 == FORK_TIER
    }
    /// How tall the tower stands, in tiles.
    pub fn height(&self) -> f32 {
        0.55 + 0.30 * (self.tier - 1) as f32
    }
    pub fn is_support(&self) -> bool {
        self.def().dtype == Damage::None
    }
    /// Where shots leave the tower.
    pub fn muzzle_height(&self) -> f32 {
        self.height() + 0.16
    }
}

#[derive(Clone, Copy, PartialEq)]
pub enum ProjKind {
    Homing,
    Lance,
}

#[derive(Clone)]
pub struct Proj {
    pub pos: [f32; 2],
    pub z: f32,
    pub vel: [f32; 2],
    pub kind: ProjKind,
    pub tower: usize,
    pub def: usize,
    pub tier: u32,
    pub dmg: f32,
    pub splash: f32,
    pub crit: bool,
    pub target_idx: usize,
    pub target_uid: u32,
    pub life: f32,
    pub trail: f32,
    /// Creeps a lance has already passed through.
    pub hit: [u32; 16],
    pub hit_n: u8,
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
    Life,
    Leak,
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
        let mut cursor: Vec<u32> = self.starts[..self.counts.len()].to_vec();
        for (i, c) in creeps.iter().enumerate() {
            let (cx, cy) = self.cell_of(c.pos);
            let k = cy * self.cols + cx;
            self.items[cursor[k] as usize] = i as u32;
            cursor[k] += 1;
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
}

/// A patch of burning road left behind by a Pyre.
///
/// Zones are the only thing in the game that damages by *position* rather than
/// by targeting, which is what makes where a Pyre stands matter more than what
/// its stats say.
#[derive(Clone)]
pub struct Zone {
    pub pos: [f32; 2],
    pub radius: f32,
    pub life: f32,
    pub max_life: f32,
    /// Damage per second to everything standing in it.
    pub dps: f32,
    /// Extra damage everything inside takes from every other source.
    pub shred: f32,
    pub tower: usize,
    pub def: usize,
    /// Counts down to the next damage tick.
    pub tick: f32,
}

#[derive(Clone, Copy, PartialEq)]
pub enum Cue {
    Build,
    Sell,
    Error,
    WaveStart,
    Leak,
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
    /// Burning patches of road. See [`Zone`].
    pub zones: Vec<Zone>,
    pub texts: Vec<FloatText>,
    pub fx: Fx,
    pub rng: Rng,
    /// The seed this run was started from. Saved, so a resumed run faces the
    /// same waves - they are generated, never stored.
    pub seed: u64,
    pub spatial: SpatialHash,

    /// True once the campaign has been cleared and the run has continued.
    pub endless: bool,
    pub wave: u32,
    pub phase: Phase,
    pub gold: i64,
    pub lives: i32,

    pub build_timer: f32,
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
    pub speed: f32,
    pub paused: bool,
    pub shake: f32,
    pub stats: RunStats,
    /// Interest paid at the end of the last wave, for the scoreboard.
    pub last_interest: i64,
    pub toast: Option<(String, f32)>,
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
            zones: Vec::with_capacity(32),
            texts: Vec::with_capacity(64),
            fx: Fx::default(),
            rng: Rng::new(0x5eed_1234_abcd_9876),
            seed: 0x5eed_1234_abcd_9876,
            spatial: SpatialHash::new(),
            endless: false,
            wave: 0,
            phase: Phase::Build,
            gold: START_GOLD,
            lives: START_LIVES,
            build_timer: BUILD_TIME_FIRST,
            spawn_left: 0,
            escort_left: 0,
            escort_timer: 0.0,
            spawn_timer: 0.0,
            time: 0.0,
            next_uid: 1,
            selected: None,
            build_choice: None,
            hover_slot: None,
            speed: 1.0,
            paused: false,
            shake: 0.0,
            stats: RunStats::default(),
            last_interest: 0,
            toast: None,
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
        wave_at(wave)
    }

    /// Restarts the run on a fresh road.
    pub fn restart(&mut self) {
        let seed = self.rng.next_u64() ^ 0x51ED_2A17_9C3B_44D1;
        self.start_run(seed);
    }

    /// Restarts from an exact seed.
    ///
    /// This is what makes multiplayer work without the server simulating
    /// anything: every client in a room is handed the same seed, so everyone
    /// faces byte-identical waves on their own board.
    pub fn start_run(&mut self, seed: u64) {
        *self = Game::new();
        self.rng = Rng::new(seed);
        self.seed = seed;
    }

    /// The scoreboard line shared with the rest of the room.
    pub fn snapshot(&self) -> td_proto::Snapshot {
        td_proto::Snapshot {
            wave: self.wave.min(u16::MAX as u32) as u16,
            lives: self.lives.clamp(i16::MIN as i32, i16::MAX as i32) as i16,
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
            self.phase = Phase::Build;
            self.build_timer = BUILD_TIME;
        }
    }

    pub fn next_wave_def(&self) -> WaveDef {
        self.wave_def(self.wave + 1)
    }

    pub fn max_tier_of(&self, _def: usize) -> u32 {
        MAX_TIER
    }

    /// Gold in hand plus everything sunk into towers.
    pub fn net_worth(&self) -> i64 {
        self.gold + self.towers.iter().map(|t| t.invested as i64).sum::<i64>()
    }

    /// What interest will pay if the wave ended right now.
    pub fn projected_interest(&self) -> i64 {
        let earning = self.gold.clamp(0, self.interest_ceiling());
        (earning as f64 * self.interest_rate() as f64).floor() as i64
    }

    /// What the Mints will pay at the end of this wave.
    pub fn projected_income(&self) -> i64 {
        self.tower_income()
    }

    pub fn can_afford(&self, cost: u32) -> bool {
        self.gold >= cost as i64
    }

    /// The tower standing on a pad, if any.
    pub fn tower_in_slot(&self, slot: usize) -> Option<usize> {
        self.board.slots.get(slot).and_then(|s| s.tower)
    }

    pub fn toast(&mut self, msg: impl Into<String>) {
        self.toast = Some((msg.into(), 2.2));
    }

    // ------------------------------------------------ player actions

    pub fn try_build(&mut self, slot: usize) -> bool {
        let Some((def, tier)) = self.build_choice else { return false };
        if tier == 0 || tier > MAX_TIER {
            return false;
        }
        let Some(s) = self.board.slots.get(slot) else { return false };
        if s.tower.is_some() {
            self.toast("That pad is taken");
            self.sound_cues.push(Cue::Error);
            return false;
        }
        let pos = s.pos;
        let cost = TOWERS[def].cost_at(tier);
        if !self.can_afford(cost) {
            self.toast("Not enough gold");
            self.sound_cues.push(Cue::Error);
            return false;
        }

        self.gold -= cost as i64;
        self.stats.gold_spent += cost as u64;
        self.stats.towers_built += 1;
        let ti = self.towers.len();
        self.towers.push(Tower {
            def,
            tier,
            fork: None,
            slot,
            pos,
            cooldown: 0.0,
            angle: 0.0,
            target_uid: 0,
            ramp: 0.0,
            kills: 0,
            damage: 0.0,
            invested: cost,
            mode: TargetMode::First,
            flash: 0.0,
            built_at: self.time,
            buff_dmg: 0.0,
            buff_rate: 0.0,
            buff_range: 0.0,
            gold_earned: 0,
        });
        self.board.slots[slot].tower = Some(ti);
        self.rebuild_auras();
        self.selected = Some(ti);
        self.sound_cues.push(Cue::Build);
        let c = tower_color(&TOWERS[def]);
        self.fx.burst(&mut self.rng, pos, 22, 3.0, [c[0], c[1], c[2], 1.0], 0.5, 0.30);
        true
    }

    pub fn sell(&mut self, ti: usize) {
        if ti >= self.towers.len() {
            return;
        }
        let t = self.towers[ti].clone();
        self.gold += t.sell_value() as i64;
        self.board.slots[t.slot].tower = None;
        self.towers.swap_remove(ti);
        // swap_remove moved the last tower into `ti`; repoint its pad and shots.
        if ti < self.towers.len() {
            let moved = self.towers[ti].slot;
            self.board.slots[moved].tower = Some(ti);
        }
        self.repoint_projectiles(ti);
        self.rebuild_auras();
        self.selected = None;
        self.sound_cues.push(Cue::Sell);
        self.fx.burst(&mut self.rng, t.pos, 16, 2.5, [1.0, 0.85, 0.35, 1.0], 0.5, 0.25);
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

    /// Upgrades one tier. Reaching tier 3 requires choosing a specialisation.
    pub fn upgrade(&mut self, ti: usize, fork: Option<usize>) {
        if ti >= self.towers.len() {
            return;
        }
        let Some(cost) = self.towers[ti].upgrade_cost() else {
            self.toast("Already at max tier");
            self.sound_cues.push(Cue::Error);
            return;
        };
        if self.towers[ti].needs_fork_choice() && fork.is_none() {
            self.toast("Pick a specialisation");
            return;
        }
        if !self.can_afford(cost) {
            self.toast("Not enough gold");
            self.sound_cues.push(Cue::Error);
            return;
        }
        self.gold -= cost as i64;
        self.stats.gold_spent += cost as u64;
        let def = self.towers[ti].def;
        let t = &mut self.towers[ti];
        t.tier += 1;
        if t.tier == FORK_TIER {
            t.fork = fork;
        }
        t.invested += cost;
        t.flash = 1.0;
        let pos = t.pos;
        let c = tower_color(&TOWERS[def]);
        self.rebuild_auras();
        self.sound_cues.push(Cue::Build);
        self.fx.burst(&mut self.rng, pos, 30, 3.6, [c[0], c[1], c[2], 1.0], 0.6, 0.34);
    }

    /// Recomputes every tower's aura bonus. Only runs when the board changes.
    pub fn rebuild_auras(&mut self) {
        for i in 0..self.towers.len() {
            self.towers[i].buff_dmg = 0.0;
            self.towers[i].buff_rate = 0.0;
            self.towers[i].buff_range = 0.0;
        }
        // Collect the beacons first so the loop below can stay a simple scan.
        let beacons: Vec<([f32; 2], f32, f32, f32, f32)> = self
            .towers
            .iter()
            .filter_map(|t| {
                let u = t.utility();
                t.specials()
                    .find_buff()
                    .map(|(dmg, rate, range)| {
                        (t.pos, t.stats().range, dmg * u, rate * u, range * u)
                    })
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
            for (bp, br, dmg, rate, range) in &beacons {
                let d2 = (bp[0] - p[0]).powi(2) + (bp[1] - p[1]).powi(2);
                if d2 <= br * br {
                    self.towers[i].buff_dmg += dmg;
                    self.towers[i].buff_rate += rate;
                    self.towers[i].buff_range += range;
                }
            }
        }
    }

    /// Gold every Mint pays at the end of a wave.
    fn tower_income(&self) -> i64 {
        self.towers
            .iter()
            .map(|t| {
                let u = t.utility();
                t.specials()
                    .iter()
                    .filter_map(|s| match *s {
                        Special::Income { per_wave } => Some((per_wave as f32 * u) as i64),
                        _ => None,
                    })
                    .sum::<i64>()
            })
            .sum()
    }

    /// Base interest plus anything a Treasury adds, capped at [`INTEREST_MAX`].
    ///
    /// Deliberately *not* scaled by tier. Interest compounds, so anything that
    /// multiplies the rate multiplies an exponential - a Treasury pays for
    /// itself through its flat income and a modest rate bump, not by bending
    /// the curve.
    pub fn interest_rate(&self) -> f32 {
        let extra: f32 = self
            .towers
            .iter()
            .map(|t| {
                t.specials()
                    .iter()
                    .filter_map(|s| match *s {
                        Special::Interest { extra } => Some(extra),
                        _ => None,
                    })
                    .sum::<f32>()
            })
            .sum();
        (INTEREST_RATE + extra).min(INTEREST_MAX)
    }

    /// The most gold that earns interest.
    ///
    /// Banking a wave or two of income is a real strategy and should pay. An
    /// unbounded pile earning compound interest is an exponential with nothing
    /// on the other side of it, so above this ceiling gold simply sits there.
    /// The ceiling rises with the wave, so it never stops being relevant.
    pub fn interest_ceiling(&self) -> i64 {
        let purse = wave_clear_bonus(self.wave.max(1)) as i64;
        (purse * 12).max(2_000)
    }

    /// Call the next wave now; pays a bonus for the time skipped.
    pub fn send_wave(&mut self) {
        if self.phase != Phase::Build {
            return;
        }
        let bonus = (self.build_timer * EARLY_BONUS_PER_SEC).round().max(0.0) as i64;
        if bonus > 0 {
            self.gold += bonus;
            self.stats.gold_earned += bonus as u64;
            let s = self.board.start();
            self.texts.push(FloatText {
                pos: [s[0] + 1.5, s[1], 1.2],
                value: bonus as f32,
                kind: TextKind::Gold,
                t: 1.6,
            });
        }
        self.begin_wave();
    }

    fn begin_wave(&mut self) {
        self.wave += 1;
        self.phase = Phase::Combat;
        let w = self.wave_def(self.wave);
        self.spawn_left = w.count;
        self.escort_left = w.escort.map_or(0, |e| e.count);
        self.spawn_timer = 0.0;
        self.escort_timer = 0.0;
        self.build_timer = 0.0;
        self.sound_cues
            .push(if w.kind.is_boss() { Cue::Boss } else { Cue::WaveStart });
    }

    /// Runs one wave boundary's payout. Tests only.
    #[cfg(test)]
    pub fn end_wave_for_test(&mut self) {
        self.end_wave();
    }

    fn end_wave(&mut self) {
        let bonus = wave_clear_bonus(self.wave) as i64;
        // Interest rewards holding gold, exactly like the original.
        let earning = self.gold.clamp(0, self.interest_ceiling());
        let interest = (earning as f64 * self.interest_rate() as f64).floor() as i64;
        let income = self.tower_income();
        for i in 0..self.towers.len() {
            let u = self.towers[i].utility();
            let paid: i64 = self.towers[i]
                .specials()
                .iter()
                .filter_map(|s| match *s {
                    Special::Income { per_wave } => Some((per_wave as f32 * u) as i64),
                    _ => None,
                })
                .sum();
            self.towers[i].gold_earned += paid.max(0) as u64;
        }
        self.last_interest = interest;
        self.gold += bonus + interest + income;
        self.stats.gold_earned += (bonus + interest + income) as u64;
        // Clearing the campaign is a win the first time. After that the waves
        // keep coming until the player finally runs out of lives.
        if self.wave >= CAMPAIGN_WAVES && !self.endless {
            self.phase = Phase::Victory;
            self.sound_cues.push(Cue::Victory);
            return;
        }
        self.phase = Phase::Build;
        self.build_timer = BUILD_TIME;
        // A wave boundary is the only moment worth checkpointing: nothing is in
        // flight, so a resumed run never starts mid-projectile.
        self.wants_save = true;
    }

    // ------------------------------------------------ update

    pub fn update(&mut self, real_dt: f32) {
        self.tick_ui(real_dt);
        if self.paused || matches!(self.phase, Phase::Defeat | Phase::Victory) {
            return;
        }
        // Fixed steps keep behaviour identical at any game speed or frame rate.
        const STEP: f32 = 1.0 / 120.0;
        let scaled = (real_dt * self.speed).min(0.25);
        let mut left = scaled;
        let mut guard = 0;
        while left > 0.0 && guard < 48 {
            let dt = left.min(STEP);
            self.step(dt);
            left -= dt;
            guard += 1;
        }
    }

    fn tick_ui(&mut self, dt: f32) {
        if let Some((_, t)) = &mut self.toast {
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

        match self.phase {
            Phase::Build => {
                self.build_timer -= dt;
                if self.build_timer <= 0.0 {
                    self.begin_wave();
                }
            }
            Phase::Combat => self.spawn_step(dt),
            _ => {}
        }

        self.spatial.rebuild(&self.creeps);
        self.step_menders(dt);
        self.step_creeps(dt);
        combat::step_towers(self, dt);
        combat::step_projectiles(self, dt);
        self.step_zones(dt);

        if self.phase == Phase::Combat
            && self.spawn_left == 0
            && self.escort_left == 0
            && self.creeps.is_empty()
        {
            self.end_wave();
        }
    }

    /// Burning road. Zones tick a few times a second rather than every step -
    /// a hundred floating combat numbers a second is unreadable, and the total
    /// damage is identical either way.
    fn step_zones(&mut self, dt: f32) {
        const TICK: f32 = 0.25;
        if self.zones.is_empty() {
            return;
        }
        for z in &mut self.zones {
            z.life -= dt;
            z.tick -= dt;
        }
        let mut hits: Vec<(usize, usize, f32)> = Vec::new();
        for zi in 0..self.zones.len() {
            if self.zones[zi].tick > 0.0 {
                continue;
            }
            self.zones[zi].tick += TICK;
            let z = self.zones[zi].clone();
            let r2 = z.radius * z.radius;
            for (ci, c) in self.creeps.iter_mut().enumerate() {
                // The fire is on the road. Anything above it is untouched.
                if c.kind.flying() {
                    continue;
                }
                let dx = c.pos[0] - z.pos[0];
                let dy = c.pos[1] - z.pos[1];
                if dx * dx + dy * dy > r2 {
                    continue;
                }
                // Shred is refreshed while they stand in it, and lingers just
                // long enough after they leave to be worth chasing them out.
                if z.shred > 0.0 {
                    c.shred.apply(z.shred, TICK + 0.9);
                }
                hits.push((ci, z.tower, z.dps * TICK));
            }
        }
        for (ci, ti, dmg) in hits {
            if ci < self.creeps.len() && ti < self.towers.len() {
                combat::damage_creep(self, ci, dmg, ti, false);
            }
        }
        self.zones.retain(|z| z.life > 0.0);
    }

    fn spawn_step(&mut self, dt: f32) {
        if self.spawn_left == 0 && self.escort_left == 0 {
            return;
        }
        let w = self.wave_def(self.wave);

        self.spawn_timer -= dt;
        while self.spawn_left > 0 && self.spawn_timer <= 0.0 {
            self.spawn_creep(&w, w.hp, 1.0, 0.0);
            self.spawn_left -= 1;
            self.spawn_timer += w.gap.max(0.05);
            if w.gap <= 0.0 {
                break;
            }
        }

        // The escort arrives on its own clock, spread across the same window,
        // so the two types are genuinely mixed rather than queued one after the
        // other - which would just be two easier waves.
        let Some(e) = w.escort else { return };
        self.escort_timer -= dt;
        while self.escort_left > 0 && self.escort_timer <= 0.0 {
            let mut sub = w;
            sub.kind = e.kind;
            sub.speed = kind_speed(e.kind);
            sub.shield = 0.0;
            sub.heal = if e.kind == Kind::Mender { w.heal } else { 0.0 };
            sub.phasing = e.kind == Kind::Phaser;
            sub.split = false;
            sub.escort = None;
            self.spawn_creep(&sub, e.hp, 1.0, 0.0);
            self.escort_left -= 1;
            let span = (w.gap.max(0.05) * w.count as f32).max(2.0);
            self.escort_timer += (span / e.count as f32).max(0.12);
        }
    }

    fn spawn_creep(&mut self, w: &WaveDef, hp: f32, scale: f32, at_dist: f32) {
        if self.creeps.len() >= MAX_CREEPS {
            return;
        }
        let uid = self.next_uid;
        self.next_uid = self.next_uid.wrapping_add(1).max(1);
        let lane = self.rng.range(-0.28, 0.28);
        let dist = at_dist - self.rng.range(0.0, 0.35);
        let shield = w.shield * scale;
        let mut c = Creep {
            uid,
            dist,
            lane,
            pos: [0.0, 0.0],
            facing: 0.0,
            hp,
            max_hp: hp,
            base_speed: w.speed,
            armor: w.armor(),
            kind: w.kind,
            radius: w.kind.radius() * scale,
            bounty: w.bounty,
            slow: Timed::default(),
            burn: Timed::default(),
            poison: Timed::default(),
            shred: Timed::default(),
            stun: 0.0,
            stun_dr: 0.0,
            kb_cd: 0.0,
            regen: if w.regen { hp * 0.02 } else { 0.0 },
            splits: if w.split { 1 } else { 0 },
            shield,
            max_shield: shield,
            heal: w.heal,
            phasing: w.phasing,
            slow_off: false,
            flash: 0.0,
            bob: self.rng.range(0.0, 6.28),
        };
        place(&self.board, &mut c);
        self.creeps.push(c);
    }

    /// Menders top up everything around them, so they have to die first.
    fn step_menders(&mut self, dt: f32) {
        let healers: Vec<([f32; 2], f32)> = self
            .creeps
            .iter()
            .filter(|c| c.heal > 0.0)
            .map(|c| (c.pos, c.heal))
            .collect();
        if healers.is_empty() {
            return;
        }
        const RADIUS: f32 = 2.6;
        for c in &mut self.creeps {
            for (hp_pos, rate) in &healers {
                let d2 = (hp_pos[0] - c.pos[0]).powi(2) + (hp_pos[1] - c.pos[1]).powi(2);
                if d2 <= RADIUS * RADIUS {
                    c.hp = (c.hp + c.max_hp * rate * dt).min(c.max_hp);
                }
            }
        }
    }

    fn step_creeps(&mut self, dt: f32) {
        let mut died: Vec<usize> = Vec::new();
        let mut leaked: Vec<usize> = Vec::new();

        for i in 0..self.creeps.len() {
            {
                let phase_window = (self.time * 1.4).fract() < 0.5;
                let c = &mut self.creeps[i];
                c.slow_off = c.phasing && phase_window;
                c.flash = (c.flash - dt * 6.0).max(0.0);
                c.bob += dt * 6.0;
                if c.stun > 0.0 {
                    c.stun -= dt;
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
                if c.regen > 0.0 && c.hp > 0.0 {
                    c.hp = (c.hp + c.regen * dt).min(c.max_hp);
                }
                if c.hp <= 0.0 {
                    died.push(i);
                    continue;
                }
                c.dist += c.speed() * dt;
            }
            place(&self.board, &mut self.creeps[i]);
            if self.creeps[i].dist >= self.board.total {
                leaked.push(i);
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

        leaked.sort_unstable();
        leaked.dedup();
        for &i in leaked.iter().rev() {
            if i >= self.creeps.len() {
                continue;
            }
            let c = self.creeps[i].clone();
            let cost = if c.kind == Kind::Boss { 10 } else { 1 };
            self.lives -= cost;
            self.stats.leaked += 1;
            self.shake = (self.shake + 0.5).min(1.0);
            self.sound_cues.push(Cue::Leak);
            self.texts.push(FloatText {
                pos: [c.pos[0], c.pos[1], 1.0],
                value: cost as f32,
                kind: TextKind::Leak,
                t: 1.6,
            });
            self.fx.burst(&mut self.rng, c.pos, 26, 5.0, [1.0, 0.25, 0.35, 1.0], 0.55, 0.35);
            self.creeps.swap_remove(i);
            if self.lives <= 0 {
                self.lives = 0;
                self.phase = Phase::Defeat;
                self.sound_cues.push(Cue::Defeat);
            }
        }
    }

    pub(crate) fn on_creep_died(&mut self, c: &Creep, killer: Option<usize>) {
        self.stats.kills += 1;
        let mut bounty = c.bounty as i64;

        if let Some(ti) = killer {
            if ti < self.towers.len() {
                self.towers[ti].kills += 1;
                for s in self.towers[ti].specials().iter() {
                    match *s {
                        Special::Bounty { flat, chance, bonus } => {
                            let mut extra = flat as i64;
                            if self.rng.chance(chance) {
                                extra += bonus as i64;
                            }
                            bounty += extra;
                            self.towers[ti].gold_earned += extra.max(0) as u64;
                        }
                        _ => {}
                    }
                }
            }
        }

        self.gold += bounty;
        self.stats.gold_earned += bounty.max(0) as u64;

        let col = c.armor.color();
        let n = match c.kind {
            Kind::Boss => 140,
            Kind::Brute | Kind::Bulwark => 46,
            Kind::Swarm => 12,
            _ => 20,
        };
        let spread = if c.kind == Kind::Boss { 7.0 } else { 4.0 };
        self.fx.burst_at(
            &mut self.rng,
            [c.pos[0], c.pos[1], c.height()],
            n,
            spread,
            [col[0], col[1], col[2], 1.0],
            0.6,
            c.radius * 1.1,
        );
        if c.kind == Kind::Boss {
            self.shake = 1.0;
        }

        // Splitters leave two smaller copies behind.
        if c.splits > 0 {
            let w = WaveDef {
                kind: Kind::Swarm,
                count: 2,
                hp: c.max_hp * 0.35,
                speed: c.base_speed * 1.2,
                bounty: (c.bounty / 2).max(1),
                gap: 0.0,
                shield: 0.0,
                heal: 0.0,
                phasing: c.phasing,
                regen: false,
                split: false,
                escort: None,
            };
            for k in 0..2 {
                if self.creeps.len() >= MAX_CREEPS {
                    break;
                }
                self.spawn_creep(&w, w.hp, 0.7, c.dist);
                if let Some(nc) = self.creeps.last_mut() {
                    nc.lane = if k == 0 { -0.26 } else { 0.26 };
                }
            }
        }
    }
}

/// Refreshes a creep's world position and facing from its distance along the road.
#[inline]
fn place(board: &Board, c: &mut Creep) {
    let p = board.sample(c.dist);
    let h = board.heading(c.dist);
    c.facing = h[1].atan2(h[0]);
    // Lane offset is perpendicular to the heading. Flyers follow the same road
    // - this is a fixed-path game and a straight line over the walls would make
    // the whole board meaningless - they simply do it out of reach.
    let lane = if c.kind.flying() { c.lane * 2.2 } else { c.lane };
    c.pos = [p[0] - h[1] * lane, p[1] + h[0] * lane];
}
