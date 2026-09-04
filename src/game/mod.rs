//! Game simulation: the road, the monsters walking it, the towers on the pads.

pub mod board;
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

use board::{BH, BW, Board};
use defs::*;
use fx::Fx;

use crate::rng::Rng;

pub const MAX_CREEPS: usize = 4000;

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
    /// Complete laps of the circuit. Nothing in the simulation reads it - it is
    /// there so the player can see which monsters have been round before, and
    /// so `Strongest`-mode towers have something to sort veterans by.
    pub laps: u32,
    pub flash: f32,
    pub bob: f32,
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
    /// Whether this is one of the map's banner waves - Immune, Hero or Boss.
    pub fn is_boss(&self) -> bool {
        self.max_hp >= 100_000.0
    }
}

#[derive(Clone)]
pub struct Tower {
    /// Index into [`TOWERS`] - a family *and* a rung, together. Upgrading moves
    /// this to the next rung rather than bumping a separate tier counter.
    pub def: usize,
    pub slot: usize,
    pub pos: [f32; 2],
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
    /// What selling pays. The map's own point value, which is nearly always
    /// everything sunk into the tower - `UpgradeRefundRate=1.0` - and less at
    /// the very top of a path.
    pub fn sell_value(&self) -> u32 {
        self.def().refund
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
    /// How tall it stands, in tiles. The map scales its models up as they
    /// climb, so this follows the map rather than the level number.
    pub fn height(&self) -> f32 {
        0.55 + 0.14 * (self.def().step.min(10) as f32)
    }
    pub fn muzzle_height(&self) -> f32 {
        self.height() + 0.16
    }
    #[allow(dead_code)]
    pub fn scale(&self) -> f32 {
        (0.85 + 0.06 * self.def().step as f32) * self.def().scale.clamp(0.6, 2.0)
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
    /// The most monsters ever circling at once.
    ///
    /// The run's high-water mark, and the only honest measure of how close it
    /// came to ending - a run that finished with an empty ring might have been
    /// one monster from drowning at wave 60, and nothing else records that.
    pub peak_circling: u32,
}

#[derive(Clone, Copy, PartialEq)]
pub enum Cue {
    Build,
    Sell,
    Error,
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
            texts: Vec::with_capacity(64),
            fx: Fx::default(),
            rng: Rng::new(0x5eed_1234_abcd_9876),
            seed: 0x5eed_1234_abcd_9876,
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
            // The scoreboard still speaks in "lives", so the flood gauge is
            // reported as headroom: how many more monsters the ring will take.
            lives: (FLOOD_LIMIT.saturating_sub(self.creeps.len())).min(i16::MAX as usize) as i16,
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

    pub fn next_wave_def(&self) -> WaveDef {
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

    /// The tower standing on a pad, if any.
    pub fn tower_in_slot(&self, slot: usize) -> Option<usize> {
        self.board.slots.get(slot).and_then(|s| s.tower)
    }

    pub fn toast(&mut self, msg: impl Into<String>) {
        self.toast = Some((msg.into(), 2.2));
    }

    // ------------------------------------------------ player actions

    pub fn try_build(&mut self, slot: usize) -> bool {
        let Some((def, _)) = self.build_choice else {
            return false;
        };
        let Some(level) = TOWERS.get(def) else {
            return false;
        };
        let Some(s) = self.board.slots.get(slot) else {
            return false;
        };
        if s.tower.is_some() {
            self.toast("That pad is taken");
            self.sound_cues.push(Cue::Error);
            return false;
        }
        let pos = s.pos;
        let cost = level.gold;
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
            slot,
            pos,
            cooldown: 0.0,
            angle: 0.0,
            target_uid: 0,
            ramp: 0.0,
            frenzy_cd: 0.0,
            aura_timer: 0.0,
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
            self.toast("Choose what it becomes");
            self.sound_cues.push(Cue::Error);
            return;
        }
        let Some((next, cost)) = self.towers[ti].upgrade_target() else {
            self.toast("Nothing above this");
            self.sound_cues.push(Cue::Error);
            return;
        };
        if !self.can_afford(cost) {
            self.toast("Not enough gold");
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
            self.toast("Not enough gold");
            self.sound_cues.push(Cue::Error);
            return;
        }
        self.pay_and_replace(ti, into, cost);
    }

    fn pay_and_replace(&mut self, ti: usize, into: usize, cost: u32) {
        self.gold -= cost as i64;
        self.stats.gold_spent += cost as u64;
        let t = &mut self.towers[ti];
        t.def = into;
        t.invested += cost;
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
        for t in &mut self.towers {
            t.buff_dmg = 0.0;
            t.buff_rate = 0.0;
            t.buff_range = 0.0;
        }
        // Collect the auras first so the loop below can stay a simple scan.
        let beacons: Vec<([f32; 2], f32, f32, f32)> = self
            .towers
            .iter()
            .filter(|t| t.abil().is_aura())
            .map(|t| {
                let a = t.abil();
                (
                    t.pos,
                    a.aura_range.max(t.def().range),
                    a.dmg_aura,
                    a.speed_aura,
                )
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
            for (bp, br, dmg, rate) in &beacons {
                let d2 = (bp[0] - p[0]).powi(2) + (bp[1] - p[1]).powi(2);
                if d2 <= br * br {
                    self.towers[i].buff_dmg += dmg;
                    self.towers[i].buff_rate += rate;
                }
            }
        }
    }

    /// Call the next wave now; pays a bonus for the time skipped.
    /// Calls the next wave now, and pays for the time skipped.
    ///
    /// On a circuit this is a real gamble rather than a free speed-up: the wave
    /// you were still killing does not go anywhere, so calling early stacks the
    /// new stream on top of the old one. The gold is the reward for judging
    /// that your board can take it.
    pub fn send_wave(&mut self) {
        if matches!(self.phase, Phase::Defeat | Phase::Victory) {
            return;
        }
        if self.wave >= self.last_wave() {
            self.toast("No more waves to call");
            return;
        }
        let bonus = (self.wave_timer * EARLY_BONUS_PER_SEC).round().max(0.0) as i64;
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

    /// The last wave the campaign will send. Endless has no last wave.
    pub fn last_wave(&self) -> u32 {
        if self.endless {
            u32::MAX
        } else {
            CAMPAIGN_WAVES
        }
    }

    /// Starts the next wave streaming, and pays the stipend for it.
    ///
    /// Whatever the previous wave had left to spawn is dropped: a wave's count
    /// is spread across exactly one [`WAVE_PERIOD`], so if the clock has come
    /// round then the wave has finished arriving. Carrying a backlog of *spawns*
    /// as well as a backlog of live monsters would compound twice over.
    fn begin_wave(&mut self) {
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
        let stipend = wave_clear_bonus(self.wave) as i64;
        self.last_interest = 0;
        self.gold += stipend;
        self.stats.gold_earned += stipend.max(0) as u64;
    }

    /// Checks the two ways a run can end.
    fn check_end(&mut self) {
        if matches!(self.phase, Phase::Defeat | Phase::Victory) {
            return;
        }
        self.stats.peak_circling = self.stats.peak_circling.max(self.creeps.len() as u32);
        if self.creeps.len() > FLOOD_LIMIT {
            self.phase = Phase::Defeat;
            self.sound_cues.push(Cue::Defeat);
            return;
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
        self.creeps.len() as f32 / FLOOD_LIMIT as f32
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

        // The wave clock never stops. There is no build phase to hide in: the
        // only quiet stretch in a run is before wave one, exactly as the map
        // plays it.
        if self.wave < self.last_wave() {
            self.wave_timer -= dt;
            if self.wave_timer <= 0.0 {
                self.begin_wave();
            }
        }
        if !self.prep {
            self.spawn_step(dt);
        }

        self.spatial.rebuild(&self.creeps);
        self.step_creeps(dt);
        combat::step_towers(self, dt);
        combat::step_projectiles(self, dt);
        self.check_end();
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
            self.spawn_creep(&w, w.hp, 1.0, 0.0);
            self.spawn_left -= 1;
            self.spawn_timer += gap;
        }
    }

    pub(crate) fn spawn_creep(&mut self, w: &WaveDef, hp: f32, scale: f32, at_dist: f32) {
        if self.creeps.len() >= MAX_CREEPS {
            return;
        }
        let uid = self.next_uid;
        self.next_uid = self.next_uid.wrapping_add(1).max(1);
        let lane = self.rng.range(-0.28, 0.28);
        let dist = at_dist - self.rng.range(0.0, 0.35);
        let mut c = Creep {
            uid,
            dist,
            lane,
            pos: [0.0, 0.0],
            facing: 0.0,
            hp,
            max_hp: hp,
            base_speed: w.speed,
            armour: w.armour,
            armour_type: w.armour_type,
            model: w.model,
            flying: w.flying,
            radius: w.model.radius() * w.scale.clamp(0.7, 1.35) * scale,
            bounty: bounty_of(w),
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
        };
        place(&self.board, &mut c);
        self.creeps.push(c);
    }

    fn step_creeps(&mut self, dt: f32) {
        let mut died: Vec<usize> = Vec::new();

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
                c.dist -= total;
                c.laps += 1;
                // A pushback budget that never refreshed meant a monster on its
                // fifth lap could not be slowed by a Thornwall at all. One lap
                // survived is worth a fresh shove.
                c.push_left = PUSHBACK_BUDGET;
            }
            place(&self.board, &mut self.creeps[i]);
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
        let n = if big { 120 } else { 20 };
        let spread = if big { 6.0 } else { 4.0 };
        self.fx.burst_at(
            &mut self.rng,
            [c.pos[0], c.pos[1], c.height()],
            n,
            spread,
            [col[0], col[1], col[2], 1.0],
            0.6,
            c.radius * 1.1,
        );
        if big {
            self.shake = 1.0;
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
    let lane = if c.flying { c.lane * 2.2 } else { c.lane };
    c.pos = [p[0] - h[1] * lane, p[1] + h[0] * lane];
}
