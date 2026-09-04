//! Targeting, firing, projectile flight and damage resolution.
//!
//! Every rule here is one of the map's. A tower fires a missile on a cooldown;
//! some missiles splash, some leap on to the next target, some are fired at
//! several targets at once. Riders - crit, poison, roots, the outright kill,
//! the armour a Corruption Tower strips - come straight out of [`Abil`], which
//! is generated from the map's own ability data.

use super::board::ROAD_HALF;
use super::defs::*;
use super::fx::ParticleStyle;
use super::{
    Beam, FloatText, Game, KNOCKBACK_CD, MAX_BEAMS, MAX_FLOAT_TEXTS, MAX_PROJECTILES, Proj,
    ProjKind, STUN_DR_MAX, STUN_DR_STEP, TargetMode, TextKind,
};

/// How fast a tower's missile travels, in tiles per second.
///
/// The map gives every tower a Warcraft III missile art and speed; they are all
/// fast, and the differences between them are decoration. One number keeps the
/// projectiles readable and the maths honest.
const MISSILE_SPEED: f32 = 22.0;

/// How much a bouncing shot loses at each leap - the Moon Glaive's own falloff.
const BOUNCE_FALLOFF: f32 = 0.7;
/// How far a bouncing shot will reach for its next target, in tiles.
const BOUNCE_HOP: f32 = 4.0;

/// How often a standing aura - a Slow Tower's chill, a Fire Tower's
/// immolation - is applied. Every step would be a hundred floating numbers a
/// second for the same total damage.
const AURA_TICK: f32 = 0.25;

// ---------------------------------------------------------------- towers

pub fn step_towers(g: &mut Game, dt: f32) {
    let mut scratch = std::mem::take(&mut g.scratch);
    // Tracking between shots is visual polish, not a combat rule. Preserve it
    // on every economically plausible board; on a pathological fully packed
    // arena it would spend thousands of spatial queries per frame merely
    // turning barrels that are still reloading.
    let animate_tracking = g.towers.len() <= 512;
    for ti in 0..g.towers.len() {
        g.towers[ti].flash = (g.towers[ti].flash - dt * 5.0).max(0.0);

        // A large share of this map's buildable interior is farther from the
        // ring than any tower can reach. The distance never changes, while an
        // upgrade's range can, so cache the former and compare it with the
        // latter. This turns an impossible packed arena from thousands of
        // empty spatial queries into a cheap scalar check without changing a
        // single target a real tower could acquire.
        let a = g.towers[ti].abil();
        let active_reach = g.towers[ti].range().max(a.slow_range).max(a.burn_range) + ROAD_HALF;
        if g.towers[ti].road_dist > active_reach {
            continue;
        }

        // Standing auras first: a Slow Tower and a Frost Tower never fire, and
        // a Fire Tower burns everything near it as well as shooting.
        aura_tick(g, ti, dt, &mut scratch);

        if g.towers[ti].is_support() {
            continue;
        }

        // The Troll Tower works itself into a frenzy on its own timer.
        frenzy_tick(g, ti, dt);

        g.towers[ti].cooldown -= dt;
        if g.towers[ti].cooldown > 0.0 {
            // Keep the barrel tracking even while reloading.
            if animate_tracking {
                if let Some(ci) = live_target(g, ti) {
                    aim(g, ti, g.creeps[ci].pos, dt);
                }
            }
            continue;
        }

        let Some(ci) = acquire(g, ti, &mut scratch) else {
            continue;
        };
        let tgt_uid = g.creeps[ci].uid;
        let tgt_pos = g.creeps[ci].pos;
        aim(g, ti, tgt_pos, 1.0);

        g.towers[ti].target_uid = tgt_uid;
        g.towers[ti].cooldown = 1.0 / g.towers[ti].rate().max(0.05);
        g.towers[ti].flash = 1.0;
        fire(g, ti, ci);

        // Multishot: the same attack at several targets at once, which is the
        // whole of the Multi Tower - ten of them at the top of its ladder.
        let extra = g.towers[ti].abil().multishot.saturating_sub(1) as usize;
        if extra > 0 {
            let others = nearby_targets(g, ti, ci, extra, &mut scratch);
            for oi in others {
                if oi < g.creeps.len() {
                    fire(g, ti, oi);
                }
            }
        }
    }
    g.scratch = scratch;
}

/// The Slow Tower's chill and the Fire Tower's immolation: no shot, no target,
/// just everything standing too close.
fn aura_tick(g: &mut Game, ti: usize, dt: f32, scratch: &mut Vec<usize>) {
    let a = g.towers[ti].abil();
    let slow = a.slow_amt;
    let slow_r = if a.slow_range > 0.0 {
        a.slow_range + g.towers[ti].buff_range
    } else {
        0.0
    };
    let burn = a.burn_dps * (1.0 + g.towers[ti].buff_dmg);
    let burn_r = if a.burn_range > 0.0 {
        a.burn_range + g.towers[ti].buff_range
    } else {
        0.0
    };
    if (slow <= 0.0 || slow_r <= 0.0) && (burn <= 0.0 || burn_r <= 0.0) {
        return;
    }
    g.towers[ti].aura_timer -= dt;
    if g.towers[ti].aura_timer > 0.0 {
        return;
    }
    g.towers[ti].aura_timer += AURA_TICK;

    let pos = g.towers[ti].pos;
    let reach = slow_r.max(burn_r);
    scratch.clear();
    g.spatial.query(pos, reach, |i| scratch.push(i));
    scratch.sort_unstable();
    scratch.dedup();
    scratch.retain(|&i| i < g.creeps.len());

    // A tower that never attacks - a Slow Tower, the Snowman - has no attack
    // targeting to read, and in Warcraft III its aura covers both layers. Only
    // a tower that *does* attack is limited to what it can shoot.
    let reaches_air = match g.towers[ti].targets() {
        Targets::Nothing => true,
        t => t.can_hit(true),
    };
    for k in 0..scratch.len() {
        let ci = scratch[k];
        if g.creeps[ci].hp <= 0.0 {
            continue;
        }
        let d2 = dist2(g.creeps[ci].pos, pos);
        // Immolation is fire on the ground; a Fire Tower that cannot reach the
        // air does not burn what is flying over it. A slow cloud is a cloud,
        // and it catches everything.
        if burn > 0.0 && g.creeps[ci].flying && !reaches_air && d2 <= burn_r * burn_r {
            continue;
        }
        if slow > 0.0 && d2 <= slow_r * slow_r {
            // Refreshed a little longer than the tick, so walking through the
            // cloud is a continuous slow rather than a flicker.
            g.creeps[ci].slow.apply(slow, AURA_TICK * 2.0);
        }
        if burn > 0.0 && d2 <= burn_r * burn_r {
            let hurt = burn * AURA_TICK;
            damage_creep(g, ci, hurt, ti, false);
        }
    }
}

/// The Troll Tower's self-buff: a burst of attack speed on a fixed cycle.
fn frenzy_tick(g: &mut Game, ti: usize, dt: f32) {
    let a = g.towers[ti].abil();
    if a.frenzy <= 0.0 {
        return;
    }
    let (dur, cd) = (a.frenzy_dur, a.frenzy_cd.max(a.frenzy_dur + 1.0));
    let t = &mut g.towers[ti];
    t.ramp = (t.ramp - dt).max(0.0);
    t.frenzy_cd = (t.frenzy_cd - dt).max(0.0);
    if t.ramp <= 0.0 && t.frenzy_cd <= 0.0 {
        t.ramp = dur;
        t.frenzy_cd = cd;
    }
}

fn aim(g: &mut Game, ti: usize, at: [f32; 2], k: f32) {
    let t = &mut g.towers[ti];
    let want = (at[1] - t.pos[1]).atan2(at[0] - t.pos[0]);
    let mut d = want - t.angle;
    while d > std::f32::consts::PI {
        d -= std::f32::consts::TAU;
    }
    while d < -std::f32::consts::PI {
        d += std::f32::consts::TAU;
    }
    t.angle += d * (k * 14.0).min(1.0);
}

/// The target a tower held last frame, if it is still alive, in range, and on a
/// layer this tower can reach.
///
/// Through the spatial hash rather than a scan of every monster on the board.
/// A packed field is a thousand towers, and a thousand linear scans of seven
/// hundred creeps - just to keep a barrel pointing the right way while the
/// tower reloads - was more than half of one frame's simulation.
fn live_target(g: &Game, ti: usize) -> Option<usize> {
    let uid = g.towers[ti].target_uid;
    if uid == 0 {
        return None;
    }
    let r = g.towers[ti].range();
    let pos = g.towers[ti].pos;
    let targets = g.towers[ti].targets();
    let mut found = None;
    g.spatial.query(pos, r, |i| {
        if found.is_some() || i >= g.creeps.len() {
            return;
        }
        let c = &g.creeps[i];
        if c.hp > 0.0 && c.uid == uid && targets.can_hit(c.flying) && dist2(c.pos, pos) <= r * r {
            found = Some(i);
        }
    });
    found
}

fn acquire(g: &Game, ti: usize, scratch: &mut Vec<usize>) -> Option<usize> {
    let pos = g.towers[ti].pos;
    let range = g.towers[ti].range();
    let r2 = range * range;
    scratch.clear();
    g.spatial.query(pos, range, |i| scratch.push(i));
    scratch.sort_unstable();
    scratch.dedup();

    let mode = g.towers[ti].mode;
    let targets = g.towers[ti].targets();
    let mut best: Option<usize> = None;
    let mut best_score = f32::MAX;
    for &i in scratch.iter() {
        if i >= g.creeps.len() {
            continue;
        }
        let c = &g.creeps[i];
        if c.hp <= 0.0 {
            continue;
        }
        // Nine of the ten Air Towers see nothing but what flies, and the Siege
        // ladder never elevates. Both are the map's own targeting.
        if !targets.can_hit(c.flying) {
            continue;
        }
        let d2 = dist2(c.pos, pos);
        if d2 > r2 {
            continue;
        }
        // On a circuit raw `dist` wraps to zero. Include completed laps or a
        // dangerous survivor suddenly becomes "last" exactly when it reaches
        // the start again and default-targeting towers abandon it.
        let progress = c.laps as f32 * g.board.total + c.dist;
        let score = match mode {
            TargetMode::First => -progress,
            TargetMode::Last => progress,
            TargetMode::Strongest => -c.hp,
            TargetMode::Closest => d2,
        };
        if score < best_score {
            best_score = score;
            best = Some(i);
        }
    }
    best
}

/// Up to `want` other targets a multishot tower can also hit this shot.
///
/// Returned high-index-first so the caller can fire at each without a
/// `swap_remove` elsewhere invalidating the ones it has not used yet.
fn nearby_targets(
    g: &Game,
    ti: usize,
    skip: usize,
    want: usize,
    scratch: &mut Vec<usize>,
) -> Vec<usize> {
    if want == 0 {
        return Vec::new();
    }
    let pos = g.towers[ti].pos;
    let range = g.towers[ti].range();
    let r2 = range * range;
    let targets = g.towers[ti].targets();
    scratch.clear();
    g.spatial.query(pos, range, |i| scratch.push(i));
    scratch.sort_unstable();
    scratch.dedup();
    let mut out: Vec<usize> = scratch
        .iter()
        .copied()
        .filter(|&i| {
            i != skip
                && i < g.creeps.len()
                && g.creeps[i].hp > 0.0
                && targets.can_hit(g.creeps[i].flying)
                && dist2(g.creeps[i].pos, pos) <= r2
        })
        .collect();
    out.sort_unstable_by(|a, b| b.cmp(a));
    out.truncate(want);
    out
}

fn fire(g: &mut Game, ti: usize, ci: usize) {
    if ti >= g.towers.len() || ci >= g.creeps.len() || g.creeps[ci].hp <= 0.0 {
        return;
    }
    let def = g.towers[ti].def();
    let abil = g.towers[ti].abil();
    let pos = g.towers[ti].pos;
    let mz = g.towers[ti].muzzle_height();
    let col = def.family.fx_color();
    let kind = projectile_kind(def.family);
    let tgt = g.creeps[ci].pos;
    // Launch along the barrel the player can see. Guided and multishot rounds
    // may curve toward their own targets on later simulation ticks, but their
    // first frame must agree with the weapon model instead of firing sideways.
    let barrel_dir = [g.towers[ti].angle.cos(), g.towers[ti].angle.sin()];
    let to_target = [tgt[0] - pos[0], tgt[1] - pos[1]];
    let target_forward = to_target[0] * barrel_dir[0] + to_target[1] * barrel_dir[1];
    // A lane-jittered creep can get closer than a long Seed barrel. Pull that
    // one flash inward rather than placing the projectile beyond its target
    // and making the homing step visibly reverse it through the weapon.
    const TARGET_MARGIN: f32 = 0.08;
    let muzzle_reach = g.towers[ti]
        .muzzle_reach()
        .min((target_forward - TARGET_MARGIN).max(0.0));
    let muzzle = [
        pos[0] + barrel_dir[0] * muzzle_reach,
        pos[1] + barrel_dir[1] * muzzle_reach,
    ];
    let dir = barrel_dir;

    let mut dmg = g.towers[ti].dmg();
    let mut crit = false;
    if abil.crit_chance > 0.0 && g.rng.chance(abil.crit_chance) {
        dmg *= abil.crit_mult.max(1.0);
        crit = true;
    }

    let (muzzle_count, muzzle_speed, muzzle_life, muzzle_size) = match kind {
        ProjKind::Shell | ProjKind::Flame => (6, 3.2, 0.20, 0.105),
        ProjKind::Royal | ProjKind::Chaos => (7, 5.0, 0.18, 0.09),
        ProjKind::Missile => (5, 4.5, 0.16, 0.075),
        _ => (3, 4.0, 0.14, 0.065),
    };
    let muzzle_style = match kind {
        ProjKind::Flame | ProjKind::Missile => ParticleStyle::Ember,
        ProjKind::Chaos | ProjKind::Orb | ProjKind::Royal | ProjKind::Acid => ParticleStyle::Magic,
        _ => ParticleStyle::Spark,
    };
    g.fx.cone_styled(
        &mut g.rng,
        [muzzle[0], muzzle[1], mz],
        barrel_dir,
        muzzle_count,
        muzzle_speed,
        [col[0], col[1], col[2], 1.0],
        muzzle_life,
        [muzzle_size, muzzle_size * 0.12],
        muzzle_style,
    );
    // A brief hot core gives every barrel a readable firing frame, even when
    // a very fast projectile reaches the target inside the same render tick.
    g.fx.mote_styled(
        [muzzle[0], muzzle[1], mz],
        [barrel_dir[0] * 0.4, barrel_dir[1] * 0.4, 0.08],
        0.10,
        [muzzle_size * 2.5, 0.0],
        [col[0], col[1], col[2], 0.82],
        ParticleStyle::Soft,
        0.0,
    );
    if matches!(kind, ProjKind::Shell | ProjKind::Missile) {
        g.fx.mote_styled(
            [
                muzzle[0] - barrel_dir[0] * 0.12,
                muzzle[1] - barrel_dir[1] * 0.12,
                mz,
            ],
            [-barrel_dir[0] * 0.8, -barrel_dir[1] * 0.8, 0.65],
            0.42,
            [muzzle_size * 1.25, muzzle_size * 3.4],
            [0.34, 0.31, 0.28, 0.36],
            ParticleStyle::Smoke,
            g.time,
        );
    }
    if matches!(kind, ProjKind::Chaos | ProjKind::Orb | ProjKind::Royal) {
        g.fx.ring(
            [muzzle[0], muzzle[1], mz],
            0.16,
            [muzzle_size * 0.7, muzzle_size * 3.2],
            [col[0], col[1], col[2], 0.72],
        );
    }
    if !g
        .sound_cues
        .iter()
        .any(|cue| matches!(cue, crate::game::Cue::Shot(_)))
    {
        g.sound_cues.push(crate::game::Cue::Shot(kind));
    }

    // Preserve combat when the visual missile pool is saturated. A packed
    // late-game board can legally fire faster than a browser should retain
    // three seconds of individual missiles. Past the ceiling the shot resolves
    // immediately, including splash and riders, instead of allocating without
    // bound or silently deleting the player's damage.
    if g.projs.len() >= MAX_PROJECTILES {
        let at = [
            g.creeps[ci].pos[0],
            g.creeps[ci].pos[1],
            g.creeps[ci].height(),
        ];
        let hit = Detonation {
            dmg,
            splash: def.splash,
            bounces: abil.bounce,
            tower: ti,
            def: g.towers[ti].def,
            at,
            target_uid: g.creeps[ci].uid,
            crit,
        };
        detonate(g, &hit);
        return;
    }

    g.projs.push(Proj {
        pos: muzzle,
        z: mz,
        vel: [dir[0] * MISSILE_SPEED, dir[1] * MISSILE_SPEED],
        kind,
        tower: ti,
        def: g.towers[ti].def,
        dmg,
        splash: def.splash,
        bounces: abil.bounce,
        crit,
        target_idx: ci,
        target_uid: g.creeps[ci].uid,
        life: 3.0,
        trail: 0.0,
    });
}

fn projectile_kind(family: Family) -> ProjKind {
    use Family::*;
    match family {
        Single => ProjKind::Dart,
        Siege => ProjKind::Shell,
        Bouncing | SuperBounce => ProjKind::Glaive,
        Multi | Critical | SuperMulti => ProjKind::Bolt,
        Corruption | Poison => ProjKind::Acid,
        Air | Frost => ProjKind::Missile,
        Chaos | SuperChaos => ProjKind::Chaos,
        Destruction | SuperDestruct | Fire => ProjKind::Flame,
        Demon | Troll => ProjKind::Orb,
        King | OneStrike => ProjKind::Royal,
        // These towers do not fire, but keeping the mapping exhaustive makes
        // adding damage to one later produce a coherent effect immediately.
        Aura | Damage | Speed | Slow => ProjKind::Orb,
    }
}

// ---------------------------------------------------------------- projectiles

/// Everything needed to resolve an impact, captured before the projectile dies.
struct Detonation {
    dmg: f32,
    splash: f32,
    bounces: u32,
    tower: usize,
    def: usize,
    at: [f32; 3],
    target_uid: u32,
    crit: bool,
}

pub fn step_projectiles(g: &mut Game, dt: f32) {
    let mut pending: Vec<Detonation> = Vec::new();
    // swap_remove keeps creep storage compact but invalidates cached projectile
    // indices. Resolve those misses through one frame-local table instead of a
    // linear scan per projectile; crowded waves can carry thousands of shots.
    let uid_index: std::collections::HashMap<u32, usize> = g
        .creeps
        .iter()
        .enumerate()
        .filter(|(_, c)| c.hp > 0.0)
        .map(|(i, c)| (c.uid, i))
        .collect();
    let mut i = 0;
    while i < g.projs.len() {
        let mut remove = false;
        let mut detonate_now = false;
        {
            let p = &mut g.projs[i];
            p.life -= dt;
            p.trail -= dt;
            if p.life <= 0.0 {
                remove = true;
                detonate_now = p.splash > 0.0;
            }
        }

        if !remove {
            // Re-acquire by uid; the index may have shifted or the creep may be gone.
            let uid = g.projs[i].target_uid;
            let ti = g.projs[i].target_idx;
            let valid = ti < g.creeps.len() && g.creeps[ti].hp > 0.0 && g.creeps[ti].uid == uid;
            let found = if valid {
                Some(ti)
            } else {
                uid_index.get(&uid).copied()
            };
            if let Some(ci) = found {
                g.projs[i].target_idx = ci;
                let tp = g.creeps[ci].pos;
                let tz = g.creeps[ci].height();
                let hit_r = (g.creeps[ci].radius + 0.16).powi(2);
                let p = &mut g.projs[i];
                let speed = mag(p.vel).max(0.001);
                let d = norm([tp[0] - p.pos[0], tp[1] - p.pos[1]]);
                let blend = (dt * 14.0).min(1.0);
                p.vel[0] += (d[0] * speed - p.vel[0]) * blend;
                p.vel[1] += (d[1] * speed - p.vel[1]) * blend;
                let v = norm(p.vel);
                p.vel = [v[0] * speed, v[1] * speed];
                p.z += (tz - p.z) * blend;
                if dist2(p.pos, tp) <= hit_r {
                    p.pos = tp;
                    p.z = tz;
                    remove = true;
                    detonate_now = true;
                }
            } else {
                g.projs[i].target_uid = 0;
            }
        }

        if !remove {
            let p = &mut g.projs[i];
            p.pos[0] += p.vel[0] * dt;
            p.pos[1] += p.vel[1] * dt;
            // Off the map entirely. This is a safety net for a shot whose
            // target died mid-flight, not a play boundary - it used to be a
            // hard-coded forty by twenty-eight box, which on the real map is a
            // corner of the field, and every projectile fired was deleted the
            // instant it left the barrel.
            if p.pos[0] < -4.0
                || p.pos[0] > super::board::BW + 4.0
                || p.pos[1] < -4.0
                || p.pos[1] > super::board::BH + 4.0
            {
                remove = true;
            }
        }

        // Sparse trail: one mote every few steps keeps the particle budget sane.
        if !remove && g.projs[i].trail <= 0.0 {
            g.projs[i].trail = 0.065;
            if g.fx.has_capacity() {
                let p = &g.projs[i];
                let col = TOWERS[p.def].family.fx_color();
                let pos = [p.pos[0], p.pos[1], p.z];
                let (drag, lift, life, size, end, alpha, trail_col, style) = match p.kind {
                    ProjKind::Shell => (
                        0.05,
                        0.38,
                        0.48,
                        0.075,
                        0.24,
                        0.30,
                        [0.30, 0.28, 0.25],
                        ParticleStyle::Smoke,
                    ),
                    ProjKind::Missile => (
                        0.10,
                        0.10,
                        0.25,
                        0.075,
                        0.018,
                        0.68,
                        [1.00, 0.55, 0.16],
                        ParticleStyle::Ember,
                    ),
                    ProjKind::Flame => (
                        0.10,
                        0.18,
                        0.30,
                        0.12,
                        0.018,
                        0.62,
                        [1.00, 0.28, 0.06],
                        ParticleStyle::Ember,
                    ),
                    ProjKind::Acid => (
                        0.08,
                        -0.06,
                        0.26,
                        0.085,
                        0.025,
                        0.56,
                        col,
                        ParticleStyle::Soft,
                    ),
                    ProjKind::Royal => (
                        0.16,
                        0.02,
                        0.18,
                        0.065,
                        0.0,
                        0.76,
                        col,
                        ParticleStyle::Magic,
                    ),
                    ProjKind::Chaos | ProjKind::Orb => {
                        (0.12, 0.05, 0.22, 0.06, 0.0, 0.55, col, ParticleStyle::Magic)
                    }
                    _ => (0.12, 0.05, 0.18, 0.05, 0.0, 0.48, col, ParticleStyle::Spark),
                };
                let vel = [-p.vel[0] * drag, -p.vel[1] * drag, lift];
                g.fx.mote_styled(
                    pos,
                    vel,
                    life,
                    [size, end],
                    [trail_col[0], trail_col[1], trail_col[2], alpha],
                    style,
                    p.life,
                );
                // A missile carries a clean smoke ribbon behind its hot motor.
                if p.kind == ProjKind::Missile && g.fx.has_capacity() {
                    g.fx.mote_styled(
                        pos,
                        [-p.vel[0] * 0.035, -p.vel[1] * 0.035, 0.32],
                        0.45,
                        [0.055, 0.20],
                        [0.32, 0.30, 0.28, 0.24],
                        ParticleStyle::Smoke,
                        p.life * 2.0,
                    );
                }
            }
        }

        if remove {
            if detonate_now {
                let p = &g.projs[i];
                pending.push(Detonation {
                    dmg: p.dmg,
                    splash: p.splash,
                    bounces: p.bounces,
                    tower: p.tower,
                    def: p.def,
                    at: [p.pos[0], p.pos[1], p.z],
                    target_uid: p.target_uid,
                    crit: p.crit,
                });
            }
            g.projs.swap_remove(i);
        } else {
            i += 1;
        }
    }

    for d in pending {
        detonate(g, &d);
    }

    // Damage resolution deliberately leaves dead creeps in place until every
    // impact from this fixed step is complete. That keeps spatial-hash and
    // multishot indices stable; removing during splash used to redirect later
    // hits onto whichever creep `swap_remove` moved into the dead one's slot.
    g.creeps.retain(|c| c.hp > 0.0);
}

fn detonate(g: &mut Game, d: &Detonation) {
    let (dmg, splash, tower, def, at, crit) = (d.dmg, d.splash, d.tower, d.def, d.at, d.crit);
    let ground = [at[0], at[1]];
    let primary = if d.target_uid == 0 {
        None
    } else {
        g.creeps
            .iter()
            .position(|c| c.hp > 0.0 && c.uid == d.target_uid)
    };
    let col = TOWERS[def].family.fx_color();
    impact_fx(g, def, at, splash, crit);
    // Aggregate the fixed-step storm into one representative sound per render
    // frame. `Audio` applies a second time gate, keeping hundreds of towers
    // from becoming a wall of clicks while preserving the weapon character.
    if !g
        .sound_cues
        .iter()
        .any(|cue| matches!(cue, crate::game::Cue::Impact(_)))
    {
        g.sound_cues.push(crate::game::Cue::Impact(projectile_kind(
            TOWERS[def].family,
        )));
    }

    if let Some(ci) = primary {
        if ci < g.creeps.len() {
            on_hit_riders_from_def(g, def, ci);
            damage_creep_from_def(g, ci, dmg, tower, def, crit);
        }
    }

    if splash > 0.0 {
        // Splash obeys the same layers the shot did. Without this a Siege
        // Tower - which cannot see the air at all - still shot down every
        // flyer on the ring by landing a shell under one, and a board with no
        // anti-air quietly cleared every air wave in the campaign.
        let targets = TOWERS[def].targets;
        let mut scratch = std::mem::take(&mut g.scratch);
        scratch.clear();
        g.spatial.query(ground, splash, |i| scratch.push(i));
        scratch.sort_unstable();
        scratch.dedup();
        scratch.retain(|&i| {
            i < g.creeps.len()
                && g.creeps[i].hp > 0.0
                && Some(i) != primary
                && targets.can_hit(g.creeps[i].flying)
                && dist2(g.creeps[i].pos, ground) <= (splash + g.creeps[i].radius).powi(2)
        });
        scratch.sort_unstable_by(|a, b| b.cmp(a));
        for k in 0..scratch.len() {
            let ci = scratch[k];
            if ci >= g.creeps.len() {
                continue;
            }
            let dd = dist2(g.creeps[ci].pos, ground).sqrt();
            let f = (1.0 - (dd / splash.max(0.001)) * 0.55).clamp(0.35, 1.0);
            on_hit_riders_from_def(g, def, ci);
            damage_creep_from_def(g, ci, dmg * f, tower, def, false);
        }
        g.scratch = scratch;
    }

    // A bouncing shot leaps on from where it landed, weaker each time.
    if d.bounces > 0 {
        bounce(
            g,
            tower,
            def,
            ground,
            at[2],
            dmg,
            d.bounces,
            d.target_uid,
            col,
        );
    }
}

/// A hit should explain which tower caused it even in a crowded wave. Keep the
/// grammar compact: one primary burst, an optional material/debris burst, and
/// shockwaves only for attacks whose gameplay actually covers an area.
fn impact_fx(g: &mut Game, def_i: usize, at: [f32; 3], splash: f32, crit: bool) {
    let def = &TOWERS[def_i];
    let kind = projectile_kind(def.family);
    let col = def.family.fx_color();
    let (n, spread, life, size) = match kind {
        ProjKind::Dart => (5, 1.5, 0.18, 0.065),
        ProjKind::Shell => (12, (2.0 + splash).min(4.2), 0.34, 0.13),
        ProjKind::Glaive => (7, 2.2, 0.20, 0.075),
        ProjKind::Bolt => (5, 2.4, 0.16, 0.055),
        ProjKind::Acid => (9, 1.8, 0.34, 0.10),
        ProjKind::Missile => (10, 2.8, 0.24, 0.09),
        ProjKind::Chaos => (11, 3.0, 0.28, 0.11),
        ProjKind::Flame => (14, (2.3 + splash).min(4.8), 0.36, 0.14),
        ProjKind::Orb => (8, 2.0, 0.30, 0.10),
        ProjKind::Royal => (16, 4.0, 0.30, 0.12),
    };
    let primary_style = match kind {
        ProjKind::Flame | ProjKind::Missile => ParticleStyle::Ember,
        ProjKind::Acid | ProjKind::Chaos | ProjKind::Orb | ProjKind::Royal => ParticleStyle::Magic,
        ProjKind::Shell | ProjKind::Glaive => ParticleStyle::Shard,
        ProjKind::Dart | ProjKind::Bolt => ParticleStyle::Spark,
    };
    g.fx.burst_styled_at(
        &mut g.rng,
        at,
        n + u32::from(crit) * 4,
        spread,
        [col[0], col[1], col[2], 1.0],
        life,
        [size, size * 0.08],
        primary_style,
    );

    match kind {
        ProjKind::Shell | ProjKind::Missile => {
            // A fast hot fracture, followed by a slower expanding smoke body.
            g.fx.burst_styled_at(
                &mut g.rng,
                at,
                7,
                spread * 0.78,
                [0.46, 0.36, 0.24, 0.72],
                life * 1.05,
                [size * 0.72, size * 0.18],
                ParticleStyle::Shard,
            );
            g.fx.burst_styled_at(
                &mut g.rng,
                at,
                if kind == ProjKind::Shell { 6 } else { 4 },
                spread * 0.30,
                [0.30, 0.28, 0.26, 0.34],
                life * 2.3,
                [size * 0.85, size * 3.1],
                ParticleStyle::Smoke,
            );
        }
        ProjKind::Flame => {
            g.fx.burst_styled_at(
                &mut g.rng,
                at,
                9,
                spread * 0.80,
                [1.0, 0.72, 0.16, 0.88],
                life * 1.45,
                [size * 0.82, size * 0.08],
                ParticleStyle::Ember,
            );
            g.fx.burst_styled_at(
                &mut g.rng,
                at,
                3,
                spread * 0.22,
                [0.24, 0.20, 0.18, 0.24],
                life * 2.1,
                [size * 0.72, size * 2.7],
                ParticleStyle::Smoke,
            );
        }
        ProjKind::Acid => {
            g.fx.ring(
                at,
                0.30,
                [size * 0.6, size * 4.4],
                [col[0], col[1], col[2], 0.66],
            );
        }
        ProjKind::Chaos | ProjKind::Orb | ProjKind::Royal => {
            g.fx.ring(
                at,
                if kind == ProjKind::Royal { 0.42 } else { 0.30 },
                [
                    size * 0.5,
                    size * if kind == ProjKind::Royal { 6.5 } else { 4.4 },
                ],
                [col[0], col[1], col[2], 0.82],
            );
        }
        ProjKind::Dart | ProjKind::Glaive | ProjKind::Bolt => {}
    }
    if crit {
        g.fx.ring(at, 0.24, [size * 0.45, size * 5.0], [1.0, 0.94, 0.68, 0.88]);
        g.fx.burst_styled_at(
            &mut g.rng,
            at,
            6,
            spread * 1.25,
            [1.0, 0.96, 0.78, 0.95],
            life * 0.85,
            [size * 0.58, 0.0],
            ParticleStyle::Spark,
        );
    }

    if splash > 0.0 {
        // The largest reference-map splashes are wider than the tactical
        // viewport. Showing that literal radius hid the lane under overlapping
        // gold circles; a capped shock front communicates AOE while particles
        // carry the impact itself. Merge near-simultaneous hits at one spot.
        let radius = splash.min(3.2);
        let nearby_wave = g
            .beams
            .iter()
            .any(|b| b.width <= 0.0 && dist2([b.from[0], b.from[1]], [at[0], at[1]]) < 0.8 * 0.8);
        if nearby_wave || g.beams.len() >= MAX_BEAMS.min(96) {
            return;
        }
        g.beams.push(Beam {
            from: [at[0], at[1], 0.18],
            to: [at[0] + radius, at[1], 0.18],
            color: col,
            t: 1.0,
            width: 0.0,
        });
    }
}

/// The Moon Glaive: the shot carries on to the next thing standing near it.
#[allow(clippy::too_many_arguments)]
fn bounce(
    g: &mut Game,
    ti: usize,
    def: usize,
    from: [f32; 2],
    z: f32,
    dmg: f32,
    hops: u32,
    first_uid: u32,
    col: [f32; 3],
) {
    let mut hit: Vec<u32> = vec![first_uid];
    let mut origin = from;
    let mut power = dmg * BOUNCE_FALLOFF;
    let mut oz = z;

    for _ in 0..hops {
        let targets = TOWERS[def].targets;
        let mut best: Option<usize> = None;
        let mut best_d = BOUNCE_HOP * BOUNCE_HOP;
        for (i, c) in g.creeps.iter().enumerate() {
            if c.hp <= 0.0 || hit.contains(&c.uid) || !targets.can_hit(c.flying) {
                continue;
            }
            let d = dist2(c.pos, origin);
            if d < best_d {
                best_d = d;
                best = Some(i);
            }
        }
        let Some(ci) = best else { break };
        let to = [
            g.creeps[ci].pos[0],
            g.creeps[ci].pos[1],
            g.creeps[ci].height(),
        ];
        if g.beams.len() < MAX_BEAMS {
            g.beams.push(Beam {
                from: [origin[0], origin[1], oz],
                to,
                color: col,
                t: 1.0,
                width: 0.07,
            });
        }
        hit.push(g.creeps[ci].uid);
        origin = [to[0], to[1]];
        oz = to[2];
        on_hit_riders_from_def(g, def, ci);
        damage_creep_from_def(g, ci, power, ti, def, false);
        power *= BOUNCE_FALLOFF;
    }
}

// ---------------------------------------------------------------- damage

/// Applies the on-hit riders - poison, roots - to one monster.
pub fn on_hit_riders(g: &mut Game, ti: usize, ci: usize) {
    let Some(def) = g.towers.get(ti).map(|tower| tower.def) else {
        return;
    };
    on_hit_riders_from_def(g, def, ci);
}

/// Resolve riders from the projectile's firing definition. The owning tower
/// may have been upgraded or sold while the shot was in flight.
fn on_hit_riders_from_def(g: &mut Game, def: usize, ci: usize) {
    if def >= TOWERS.len() || ci >= g.creeps.len() || g.creeps[ci].hp <= 0.0 {
        return;
    }
    let a = &TOWERS[def].abil;

    // Corruption is the readable counter to a commander's Mender aura. The
    // armour strip already identifies this family as anti-defence; suppressing
    // repair for two seconds gives that identity a tactical purpose on boss
    // waves without adding another shop button.
    if a.armour_pen > 0 {
        g.creeps[ci].suppress = g.creeps[ci].suppress.max(2.0);
    }

    // The Poison Tower's sting: damage over time and a heavy slow, both of
    // which the map states outright on the ability - 500 a second and 40% for
    // twelve seconds, on up to 8000 and 80% for thirty at the top of the
    // ladder. It stacks rather than refreshing, which is why the family scales
    // on one big target.
    if a.poison_dps > 0.0 {
        let c = &mut g.creeps[ci];
        // A low-tier sting must never truncate a stronger stack. Its own
        // twelve-hit cap may contribute only when that cap is above the
        // strength already present; the existing stack is always a floor.
        let cap = c.poison.amt.max(a.poison_dps * 12.0);
        c.poison.amt = (c.poison.amt + a.poison_dps).min(cap);
        c.poison.t = c.poison.t.max(a.poison_dur);
    }
    if a.poison_slow > 0.0 {
        let control = g.creeps[ci].control_scale();
        g.creeps[ci]
            .slow
            .apply(a.poison_slow * control, a.poison_dur.max(1.0) * control);
    }

    // The Troll Tower's roots. Bosses stand through them, and nothing can be
    // rooted again inside its post-root window - see `STUN_IMMUNE`.
    if a.root_chance > 0.0 {
        let locked = g.creeps[ci].stun > 0.0 || g.creeps[ci].stun_immune > 0.0;
        let control = g.creeps[ci].control_scale();
        if !g.creeps[ci].is_boss() && !locked && g.rng.chance(a.root_chance * control) {
            let c = &mut g.creeps[ci];
            // Diminishing returns. Without them, enough rooting towers freeze a
            // wave permanently: nothing dies, nothing leaks, and the wave simply
            // never ends.
            let effective = a.root_dur * control * (1.0 - c.stun_dr);
            if effective > 0.05 {
                c.stun = c.stun.max(effective);
            }
            c.stun_dr = (c.stun_dr + STUN_DR_STEP).min(STUN_DR_MAX);
        }
    }
}

/// Moves one monster back down the road, within its cooldown and its budget.
#[allow(dead_code)]
fn push_back(c: &mut crate::game::Creep, dist: f32) {
    if c.is_boss() || c.kb_cd > 0.0 || c.push_left <= 0.0 {
        return;
    }
    let moved = (dist * c.control_scale()).min(c.push_left);
    c.dist = (c.dist - moved).max(0.0);
    c.push_left -= moved;
    c.kb_cd = KNOCKBACK_CD;
}

/// Deals `base` damage and cleans up if the monster dies.
///
/// This is where the map's two rules meet: the attack-versus-armour table,
/// which after `war3mapMisc.txt` is only "Immune takes five percent, except
/// from Chaos and Hero", and the armour *value*, which climbs to seven hundred
/// and is the whole of the difficulty curve.
pub fn damage_creep(g: &mut Game, ci: usize, base: f32, ti: usize, crit: bool) -> bool {
    let Some(def) = g.towers.get(ti).map(|tower| tower.def) else {
        return damage_creep_snapshot(g, ci, base, ti, Attack::Normal, 0, 0.0, crit);
    };
    damage_creep_from_def(g, ci, base, ti, def, crit)
}

/// Resolve damage using the definition captured when a projectile fired.
fn damage_creep_from_def(
    g: &mut Game,
    ci: usize,
    base: f32,
    ti: usize,
    def: usize,
    crit: bool,
) -> bool {
    let Some(tower) = TOWERS.get(def) else {
        return false;
    };
    damage_creep_snapshot(
        g,
        ci,
        base,
        ti,
        tower.attack,
        tower.abil.armour_pen,
        tower.abil.kill_chance,
        crit,
    )
}

#[allow(clippy::too_many_arguments)]
fn damage_creep_snapshot(
    g: &mut Game,
    ci: usize,
    base: f32,
    ti: usize,
    attack: Attack,
    pen: i32,
    kill_chance: f32,
    crit: bool,
) -> bool {
    if ci >= g.creeps.len() {
        return false;
    }
    if g.creeps[ci].hp <= 0.0 {
        return true;
    }

    // The Corruption Tower strips armour off whatever it hits - fifteen points
    // at the first level, seventy-five at the last. On a wave carrying seven
    // hundred that is small; on the early waves it is most of their defence.
    let armour = g.creeps[ci].armour - pen;
    let armour_type = g.creeps[ci].armour_type;

    // The outright kill. Rolled before anything else, because a monster this
    // lands on does not care about armour or health. Never on a boss: a boss
    // deleted by a coin flip is not a boss.
    if ti < g.towers.len() {
        if kill_chance > 0.0 && !g.creeps[ci].is_boss() && g.rng.chance(kill_chance) {
            let pos = g.creeps[ci].pos;
            let z = g.creeps[ci].height();
            let hp = g.creeps[ci].hp;
            g.towers[ti].damage += hp as f64;
            g.stats.damage += hp as f64;
            if g.texts.len() < MAX_FLOAT_TEXTS {
                g.texts.push(FloatText {
                    pos: [pos[0], pos[1], z + 0.35],
                    value: hp,
                    kind: TextKind::Crit,
                    t: 1.1,
                });
            }
            g.fx.burst(&mut g.rng, pos, 20, 2.4, [1.0, 0.95, 0.55, 1.0], 0.45, 0.22);
            let c = g.creeps[ci].clone();
            g.on_creep_died(&c, Some(ti));
            g.creeps[ci].hp = 0.0;
            return true;
        }
    }

    let dealt = damage_taken(base, attack, armour, armour_type);
    let c = &mut g.creeps[ci];
    c.hp -= dealt;
    c.flash = 1.0;
    let dead = c.hp <= 0.0;
    let pos = c.pos;
    let z = c.height();

    if ti < g.towers.len() {
        g.towers[ti].damage += dealt as f64;
    }
    g.stats.damage += dealt as f64;

    // Only the loud hits get a number, otherwise the board is unreadable. The
    // threshold rises with the wave, because a hit that is worth reading on
    // wave three is noise on wave thirty.
    let floor = (g.wave as f32).powi(2) * 12.0 + 40.0;
    if (crit || dealt >= floor) && g.texts.len() < MAX_FLOAT_TEXTS {
        g.texts.push(FloatText {
            pos: [pos[0], pos[1], z + 0.35],
            value: dealt,
            kind: if crit {
                TextKind::Crit
            } else {
                TextKind::Damage
            },
            t: 0.9,
        });
    }

    if dead {
        let c = g.creeps[ci].clone();
        g.on_creep_died(&c, Some(ti));
    }
    dead
}

// ---------------------------------------------------------------- math

#[inline]
pub fn dist2(a: [f32; 2], b: [f32; 2]) -> f32 {
    let dx = a[0] - b[0];
    let dy = a[1] - b[1];
    dx * dx + dy * dy
}

#[inline]
fn mag(v: [f32; 2]) -> f32 {
    (v[0] * v[0] + v[1] * v[1]).sqrt()
}

#[inline]
fn norm(v: [f32; 2]) -> [f32; 2] {
    let m = mag(v).max(1e-5);
    [v[0] / m, v[1] / m]
}

#[cfg(test)]
mod visual_tests {
    use super::*;

    /// The ten attacking command cards must not collapse back into one generic
    /// glowing ball. This is the visual grammar a new player learns.
    #[test]
    fn every_attacking_command_family_has_its_own_projectile() {
        let families = [
            Family::Single,
            Family::Siege,
            Family::Bouncing,
            Family::Multi,
            Family::Corruption,
            Family::Air,
            Family::Chaos,
            Family::Destruction,
            Family::Demon,
            Family::King,
        ];
        let mut kinds: Vec<ProjKind> = families.into_iter().map(projectile_kind).collect();
        kinds.sort_unstable_by_key(|kind| *kind as u8);
        kinds.dedup();
        assert_eq!(kinds.len(), families.len());
    }

    #[test]
    fn first_targeting_keeps_a_lapped_survivor_at_the_front() {
        let mut g = Game::new();
        g.gold = 10_000;
        let def = family_start(Family::Single).expect("single root");
        g.build_choice = Some((def, 1));
        assert!(g.try_build(0));
        let tower_pos = g.towers[0].pos;
        let w = g.wave_def(1);
        g.wave = 1;
        g.spawn_creep(&w, w.hp, 1.0, 3.0);
        g.spawn_creep(&w, w.hp, 1.0, 0.2);
        g.creeps[0].pos = tower_pos;
        g.creeps[1].pos = tower_pos;
        g.creeps[1].laps = 1;
        g.spatial.rebuild(&g.creeps);

        let mut scratch = Vec::new();
        assert_eq!(acquire(&g, 0, &mut scratch), Some(1));
    }

    #[test]
    fn every_shot_launches_forward_from_the_tracked_barrel() {
        let mut g = Game::new();
        g.gold = 10_000;
        let def = family_start(Family::Single).expect("single root");
        g.build_choice = Some((def, 1));
        assert!(g.try_build(0));
        let origin = g.towers[0].pos;
        // Deliberately point the tracked barrel north while a secondary
        // multishot-style target sits east.
        g.towers[0].angle = std::f32::consts::FRAC_PI_2;
        let w = g.wave_def(1);
        g.wave = 1;
        g.spawn_creep(&w, w.hp, 1.0, 3.0);
        g.creeps[0].pos = [origin[0] + 2.0, origin[1] + 2.0];

        let reach = g.towers[0].muzzle_reach();
        let height = g.towers[0].muzzle_height();
        fire(&mut g, 0, 0);
        let shot = g.projs.last().expect("projectile");
        assert!((shot.pos[0] - origin[0]).abs() < 1e-5);
        assert!((shot.pos[1] - origin[1] - reach).abs() < 1e-5);
        assert!((shot.z - height).abs() < 1e-5);
        assert!(shot.vel[0].abs() < 1e-5);
        assert!(
            shot.vel[1] > 0.0,
            "shot did not leave the north-facing barrel"
        );
    }

    #[test]
    fn a_close_target_never_sits_behind_the_projectile_spawn() {
        let mut g = Game::new();
        g.gold = 10_000;
        let def = family_start(Family::Single).expect("single root");
        g.build_choice = Some((def, 1));
        assert!(g.try_build(0));
        let origin = g.towers[0].pos;
        g.towers[0].angle = 0.0;
        let authored_reach = g.towers[0].muzzle_reach();
        let w = g.wave_def(1);
        g.wave = 1;
        g.spawn_creep(&w, w.hp, 1.0, 3.0);
        g.creeps[0].pos = [origin[0] + authored_reach * 0.5, origin[1]];

        fire(&mut g, 0, 0);
        let shot = g.projs.last().expect("projectile");
        assert!(shot.pos[0] >= origin[0]);
        assert!(shot.pos[0] < g.creeps[0].pos[0]);
        assert!(shot.vel[0] > 0.0);
        assert!(shot.vel[1].abs() < 1e-5);
    }
}
