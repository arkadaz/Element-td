//! Targeting, firing, projectile flight and damage resolution.
//!
//! Every rule here is one of the map's. A tower fires a missile on a cooldown;
//! some missiles splash, some leap on to the next target, some are fired at
//! several targets at once. Riders - crit, poison, roots, the outright kill,
//! the armour a Corruption Tower strips - come straight out of [`Abil`], which
//! is generated from the map's own ability data.

use super::defs::*;
use super::{
    Beam, FloatText, Game, KNOCKBACK_CD, Proj, ProjKind, STUN_DR_MAX, STUN_DR_STEP, TargetMode,
    TextKind,
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
    for ti in 0..g.towers.len() {
        g.towers[ti].flash = (g.towers[ti].flash - dt * 5.0).max(0.0);

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
            if let Some(ci) = live_target(g, ti) {
                aim(g, ti, g.creeps[ci].pos, dt);
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
    let slow_r = a.slow_range;
    let burn = a.burn_dps * (1.0 + g.towers[ti].buff_dmg);
    let burn_r = a.burn_range;
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
    let list: Vec<usize> = scratch
        .iter()
        .copied()
        .filter(|&i| i < g.creeps.len())
        .collect();

    // A tower that never attacks - a Slow Tower, the Snowman - has no attack
    // targeting to read, and in Warcraft III its aura covers both layers. Only
    // a tower that *does* attack is limited to what it can shoot.
    let reaches_air = match g.towers[ti].targets() {
        Targets::Nothing => true,
        t => t.can_hit(true),
    };
    let mut dead: Vec<usize> = Vec::new();
    for ci in list {
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
            if damage_creep(g, ci, hurt, ti, false) {
                dead.push(ci);
            }
        }
    }
    let _ = dead;
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
        if c.uid == uid && targets.can_hit(c.flying) && dist2(c.pos, pos) <= r * r {
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
        // Nine of the ten Air Towers see nothing but what flies, and the Siege
        // ladder never elevates. Both are the map's own targeting.
        if !targets.can_hit(c.flying) {
            continue;
        }
        let d2 = dist2(c.pos, pos);
        if d2 > r2 {
            continue;
        }
        // "First" means furthest along the road, which is now just `dist`.
        let score = match mode {
            TargetMode::First => -c.dist,
            TargetMode::Last => c.dist,
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
                && targets.can_hit(g.creeps[i].flying)
                && dist2(g.creeps[i].pos, pos) <= r2
        })
        .collect();
    out.sort_unstable_by(|a, b| b.cmp(a));
    out.truncate(want);
    out
}

fn fire(g: &mut Game, ti: usize, ci: usize) {
    let def = g.towers[ti].def();
    let abil = g.towers[ti].abil();
    let pos = g.towers[ti].pos;
    let mz = g.towers[ti].muzzle_height();
    let col = tower_color(def);
    let tgt = g.creeps[ci].pos;
    let dir = norm([tgt[0] - pos[0], tgt[1] - pos[1]]);
    let muzzle = [pos[0] + dir[0] * 0.30, pos[1] + dir[1] * 0.30];

    let mut dmg = g.towers[ti].dmg();
    let mut crit = false;
    if abil.crit_chance > 0.0 && g.rng.chance(abil.crit_chance) {
        dmg *= abil.crit_mult.max(1.0);
        crit = true;
    }

    g.fx.cone(
        &mut g.rng,
        [muzzle[0], muzzle[1], mz],
        dir,
        5,
        4.0,
        [col[0], col[1], col[2], 1.0],
        0.20,
        0.12,
    );

    g.projs.push(Proj {
        pos: muzzle,
        z: mz,
        vel: [dir[0] * MISSILE_SPEED, dir[1] * MISSILE_SPEED],
        kind: ProjKind::Homing,
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
            let valid = ti < g.creeps.len() && g.creeps[ti].uid == uid;
            let found = if valid {
                Some(ti)
            } else {
                g.creeps.iter().position(|c| c.uid == uid)
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
            let p = &g.projs[i];
            let col = tower_color(&TOWERS[p.def]);
            let pos = [p.pos[0], p.pos[1], p.z];
            let vel = [-p.vel[0] * 0.12, -p.vel[1] * 0.12, 0.05];
            g.projs[i].trail = 0.035;
            g.fx.mote(pos, vel, 0.28, 0.09, [col[0], col[1], col[2], 0.85]);
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
}

fn detonate(g: &mut Game, d: &Detonation) {
    let (dmg, splash, tower, def, at, crit) = (d.dmg, d.splash, d.tower, d.def, d.at, d.crit);
    let ground = [at[0], at[1]];
    let primary = if d.target_uid == 0 {
        None
    } else {
        g.creeps.iter().position(|c| c.uid == d.target_uid)
    };
    let col = tower_color(&TOWERS[def]);
    if splash > 0.0 {
        g.fx.burst_at(
            &mut g.rng,
            at,
            18,
            splash * 3.0,
            [col[0], col[1], col[2], 1.0],
            0.32,
            splash * 0.40,
        );
    } else {
        g.fx.burst_at(
            &mut g.rng,
            at,
            7,
            2.4,
            [col[0], col[1], col[2], 1.0],
            0.24,
            0.13,
        );
    }

    if let Some(ci) = primary {
        if ci < g.creeps.len() {
            on_hit_riders(g, tower, ci);
            damage_creep(g, ci, dmg, tower, crit);
        }
    }

    if splash > 0.0 {
        // Splash obeys the same layers the shot did. Without this a Siege
        // Tower - which cannot see the air at all - still shot down every
        // flyer on the ring by landing a shell under one, and a board with no
        // anti-air quietly cleared every air wave in the campaign.
        let targets = if tower < g.towers.len() {
            g.towers[tower].targets()
        } else {
            Targets::Both
        };
        let mut scratch = std::mem::take(&mut g.scratch);
        scratch.clear();
        g.spatial.query(ground, splash, |i| scratch.push(i));
        scratch.sort_unstable();
        scratch.dedup();
        let mut list: Vec<usize> = scratch
            .iter()
            .copied()
            .filter(|&i| i < g.creeps.len() && Some(i) != primary)
            .filter(|&i| targets.can_hit(g.creeps[i].flying))
            .filter(|&i| dist2(g.creeps[i].pos, ground) <= (splash + g.creeps[i].radius).powi(2))
            .collect();
        list.sort_unstable_by(|a, b| b.cmp(a));
        for ci in list {
            if ci >= g.creeps.len() {
                continue;
            }
            let dd = dist2(g.creeps[ci].pos, ground).sqrt();
            let f = (1.0 - (dd / splash.max(0.001)) * 0.55).clamp(0.35, 1.0);
            on_hit_riders(g, tower, ci);
            damage_creep(g, ci, dmg * f, tower, false);
        }
        g.scratch = scratch;
    }

    // A bouncing shot leaps on from where it landed, weaker each time.
    if d.bounces > 0 {
        bounce(g, tower, ground, at[2], dmg, d.bounces, d.target_uid, col);
    }
}

/// The Moon Glaive: the shot carries on to the next thing standing near it.
#[allow(clippy::too_many_arguments)]
fn bounce(
    g: &mut Game,
    ti: usize,
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
        let targets = if ti < g.towers.len() {
            g.towers[ti].targets()
        } else {
            Targets::Both
        };
        let mut best: Option<usize> = None;
        let mut best_d = BOUNCE_HOP * BOUNCE_HOP;
        for (i, c) in g.creeps.iter().enumerate() {
            if hit.contains(&c.uid) || !targets.can_hit(c.flying) {
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
        g.beams.push(Beam {
            from: [origin[0], origin[1], oz],
            to,
            color: col,
            t: 1.0,
            width: 0.07,
        });
        hit.push(g.creeps[ci].uid);
        origin = [to[0], to[1]];
        oz = to[2];
        on_hit_riders(g, ti, ci);
        damage_creep(g, ci, power, ti, false);
        power *= BOUNCE_FALLOFF;
    }
}

// ---------------------------------------------------------------- damage

/// Applies the on-hit riders - poison, roots - to one monster.
pub fn on_hit_riders(g: &mut Game, ti: usize, ci: usize) {
    if ti >= g.towers.len() || ci >= g.creeps.len() {
        return;
    }
    let a = g.towers[ti].abil();

    // The Poison Tower's sting: damage over time and a heavy slow, both of
    // which the map states outright on the ability - 500 a second and 40% for
    // twelve seconds, on up to 8000 and 80% for thirty at the top of the
    // ladder. It stacks rather than refreshing, which is why the family scales
    // on one big target.
    if a.poison_dps > 0.0 {
        let cap = a.poison_dps * 12.0;
        let c = &mut g.creeps[ci];
        c.poison.amt = (c.poison.amt + a.poison_dps).min(cap);
        c.poison.t = c.poison.t.max(a.poison_dur);
    }
    if a.poison_slow > 0.0 {
        g.creeps[ci]
            .slow
            .apply(a.poison_slow, a.poison_dur.max(1.0));
    }

    // The Troll Tower's roots. Bosses stand through them, and nothing can be
    // rooted again inside its post-root window - see `STUN_IMMUNE`.
    if a.root_chance > 0.0 {
        let locked = g.creeps[ci].stun > 0.0 || g.creeps[ci].stun_immune > 0.0;
        if !g.creeps[ci].is_boss() && !locked && g.rng.chance(a.root_chance) {
            let c = &mut g.creeps[ci];
            // Diminishing returns. Without them, enough rooting towers freeze a
            // wave permanently: nothing dies, nothing leaks, and the wave simply
            // never ends.
            let effective = a.root_dur * (1.0 - c.stun_dr);
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
    let moved = dist.min(c.push_left);
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
    if ci >= g.creeps.len() {
        return false;
    }
    let (attack, pen) = if ti < g.towers.len() {
        (g.towers[ti].attack(), g.towers[ti].abil().armour_pen)
    } else {
        (Attack::Normal, 0)
    };

    // The Corruption Tower strips armour off whatever it hits - fifteen points
    // at the first level, seventy-five at the last. On a wave carrying seven
    // hundred that is small; on the early waves it is most of their defence.
    let armour = g.creeps[ci].armour - pen;
    let armour_type = g.creeps[ci].armour_type;

    // The outright kill. Rolled before anything else, because a monster this
    // lands on does not care about armour or health. Never on a boss: a boss
    // deleted by a coin flip is not a boss.
    if ti < g.towers.len() {
        let chance = g.towers[ti].abil().kill_chance;
        if chance > 0.0 && !g.creeps[ci].is_boss() && g.rng.chance(chance) {
            let pos = g.creeps[ci].pos;
            let z = g.creeps[ci].height();
            let hp = g.creeps[ci].hp;
            g.towers[ti].damage += hp as f64;
            g.stats.damage += hp as f64;
            g.texts.push(FloatText {
                pos: [pos[0], pos[1], z + 0.35],
                value: hp,
                kind: TextKind::Crit,
                t: 1.1,
            });
            g.fx.burst(&mut g.rng, pos, 20, 2.4, [1.0, 0.95, 0.55, 1.0], 0.45, 0.22);
            let c = g.creeps[ci].clone();
            g.on_creep_died(&c, Some(ti));
            g.creeps.swap_remove(ci);
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
    if crit || dealt >= floor {
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
        g.creeps.swap_remove(ci);
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
