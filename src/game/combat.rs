//! Targeting, firing, projectile flight and damage resolution.

use super::defs::*;
use super::{
    Beam, FloatText, Game, KNOCKBACK_CD, Proj, ProjKind, STUN_DR_MAX, STUN_DR_STEP, SUPPRESS_TIME,
    TargetMode, TextKind, Zone,
};

// ---------------------------------------------------------------- towers

pub fn step_towers(g: &mut Game, dt: f32) {
    let mut scratch = std::mem::take(&mut g.scratch);
    for ti in 0..g.towers.len() {
        g.towers[ti].flash = (g.towers[ti].flash - dt * 5.0).max(0.0);

        // Support towers never attack; their aura is applied when the board changes.
        if g.towers[ti].is_support() {
            continue;
        }
        g.towers[ti].cooldown -= dt;

        let stats = g.towers[ti].stats();
        let range = g.towers[ti].range();
        let rate = g.towers[ti].rate().max(0.05);
        let pos = g.towers[ti].pos;

        if stats.delivery == Delivery::Nova {
            if g.towers[ti].cooldown <= 0.0 && !g.creeps.is_empty() {
                scratch.clear();
                g.spatial.query(pos, range, |i| scratch.push(i));
                scratch.sort_unstable();
                scratch.dedup();
                scratch.retain(|&i| {
                    i < g.creeps.len() && dist2(g.creeps[i].pos, pos) <= range * range
                });
                if !scratch.is_empty() {
                    g.towers[ti].cooldown = 1.0 / rate;
                    g.towers[ti].flash = 1.0;
                    let col = tower_color(g.towers[ti].def());
                    g.beams.push(Beam {
                        from: [pos[0], pos[1], 0.12],
                        to: [pos[0] + range, pos[1], 0.12],
                        color: col,
                        t: 1.0,
                        width: 0.0,
                    });
                    g.fx.burst(
                        &mut g.rng,
                        pos,
                        30,
                        range * 2.0,
                        [col[0], col[1], col[2], 1.0],
                        0.35,
                        0.22,
                    );
                    let dmg = g.towers[ti].dmg();
                    let mut list = scratch.clone();
                    list.sort_unstable_by(|a, b| b.cmp(a));
                    for ci in list {
                        if ci >= g.creeps.len() {
                            continue;
                        }
                        on_hit_specials(g, ti, ci, false);
                        damage_creep(g, ci, dmg, ti, false);
                    }
                }
            }
            continue;
        }

        if g.towers[ti].cooldown > 0.0 {
            // Keep the barrel tracking even while reloading.
            if let Some(ci) = live_target(g, ti) {
                aim(g, ti, g.creeps[ci].pos, dt);
            }
            continue;
        }

        let Some(ci) = acquire(g, ti, &mut scratch) else {
            g.towers[ti].ramp = 0.0;
            continue;
        };
        let tgt_uid = g.creeps[ci].uid;
        let tgt_pos = g.creeps[ci].pos;
        aim(g, ti, tgt_pos, 1.0);

        // Ramp resets whenever the tower switches target.
        if g.towers[ti].target_uid != tgt_uid {
            g.towers[ti].ramp = 0.0;
        }
        g.towers[ti].target_uid = tgt_uid;
        g.towers[ti].cooldown = 1.0 / rate;
        g.towers[ti].flash = 1.0;
        fire(g, ti, ci);

        // Multishot: the same shot again at other targets in range. Each is a
        // full hit, so this is a straight multiplier on a tower's throughput -
        // which is why only two towers in the roster have it.
        let extra = g.towers[ti].specials().iter().find_map(|s| match *s {
            Special::Multishot { extra } => Some(extra),
            _ => None,
        });
        if let Some(extra) = extra {
            let others = nearby_targets(g, ti, ci, extra as usize, &mut scratch);
            for oi in others {
                if oi < g.creeps.len() {
                    fire(g, ti, oi);
                }
            }
        }
    }
    g.scratch = scratch;
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
fn live_target(g: &Game, ti: usize) -> Option<usize> {
    let uid = g.towers[ti].target_uid;
    if uid == 0 {
        return None;
    }
    let r = g.towers[ti].range();
    let pos = g.towers[ti].pos;
    let targets = g.towers[ti].def().targets;
    g.creeps
        .iter()
        .position(|c| c.uid == uid && targets.can_hit(c.kind.layer()) && dist2(c.pos, pos) <= r * r)
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
    let targets = g.towers[ti].def().targets;
    let mut best: Option<usize> = None;
    let mut best_score = f32::MAX;
    for &i in scratch.iter() {
        if i >= g.creeps.len() {
            continue;
        }
        let c = &g.creeps[i];
        // A mortar cannot elevate and a fire pool cannot leave the road, so
        // ground-only towers simply do not see what is flying over them.
        if !targets.can_hit(c.kind.layer()) {
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
    let targets = g.towers[ti].def().targets;
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
                && targets.can_hit(g.creeps[i].kind.layer())
                && dist2(g.creeps[i].pos, pos) <= r2
        })
        .collect();
    out.sort_unstable_by(|a, b| b.cmp(a));
    out.truncate(want);
    out
}

fn fire(g: &mut Game, ti: usize, ci: usize) {
    let def = g.towers[ti].def();
    let stats = g.towers[ti].stats();
    let specials = g.towers[ti].specials();
    let pos = g.towers[ti].pos;
    let mz = g.towers[ti].muzzle_height();
    let col = tower_color(def);
    let tgt = g.creeps[ci].pos;
    let tgt_z = g.creeps[ci].height();
    let dir = norm([tgt[0] - pos[0], tgt[1] - pos[1]]);
    let muzzle = [pos[0] + dir[0] * 0.30, pos[1] + dir[1] * 0.30];

    let mut dmg = g.towers[ti].dmg() * (1.0 + g.towers[ti].ramp);
    let mut crit = false;

    for s in specials.iter() {
        match *s {
            Special::Crit { chance, mult } => {
                if g.rng.chance(chance) {
                    dmg *= mult;
                    crit = true;
                }
            }
            Special::Ramp { per_hit, max } => {
                g.towers[ti].ramp = (g.towers[ti].ramp + per_hit).min(max);
            }
            _ => {}
        }
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

    match stats.delivery {
        Delivery::Zone { radius, dur } => {
            // Aimed at the road under the target, not at the target itself: the
            // fire stays where it is put, and whatever walks through it burns.
            g.zones.push(Zone {
                pos: tgt,
                radius,
                life: dur,
                max_life: dur,
                dps: dmg,
                shred: specials.shred_amt(),
                tower: ti,
                def: g.towers[ti].def,
                tick: 0.0,
            });
            g.fx.burst(
                &mut g.rng,
                tgt,
                14,
                2.2,
                [col[0], col[1], col[2], 1.0],
                0.6,
                0.26,
            );
        }
        Delivery::Shot { speed } => {
            g.projs.push(Proj {
                pos: muzzle,
                z: mz,
                vel: [dir[0] * speed, dir[1] * speed],
                kind: ProjKind::Homing,
                tower: ti,
                def: g.towers[ti].def,
                tier: g.towers[ti].tier,
                dmg,
                splash: stats.splash,
                crit,
                target_idx: ci,
                target_uid: g.creeps[ci].uid,
                life: 3.0,
                trail: 0.0,
                hit: [0; 16],
                hit_n: 0,
            });
        }
        Delivery::Lance { speed } => {
            g.projs.push(Proj {
                pos: muzzle,
                z: mz * 0.8,
                vel: [dir[0] * speed, dir[1] * speed],
                kind: ProjKind::Lance,
                tower: ti,
                def: g.towers[ti].def,
                tier: g.towers[ti].tier,
                dmg,
                splash: 0.0,
                crit,
                target_idx: usize::MAX,
                target_uid: 0,
                life: stats.range / speed * 1.35,
                trail: 0.0,
                hit: [0; 16],
                hit_n: 0,
            });
        }
        Delivery::Beam { pierce } => {
            g.beams.push(Beam {
                from: [muzzle[0], muzzle[1], mz],
                to: [tgt[0], tgt[1], tgt_z],
                color: col,
                t: 1.0,
                width: 0.08,
            });
            on_hit_specials(g, ti, ci, crit);
            damage_creep(g, ci, dmg, ti, crit);
            if pierce > 0 {
                let primary_uid = if ci < g.creeps.len() {
                    g.creeps[ci].uid
                } else {
                    0
                };
                let mut list: Vec<usize> = Vec::new();
                for (i, c) in g.creeps.iter().enumerate() {
                    if c.uid != primary_uid
                        && point_seg_dist2(c.pos, muzzle, tgt) < (c.radius + 0.16).powi(2)
                    {
                        list.push(i);
                    }
                }
                list.sort_unstable_by(|a, b| b.cmp(a));
                let mut extra = pierce;
                for i in list {
                    if extra == 0 {
                        break;
                    }
                    if i >= g.creeps.len() {
                        continue;
                    }
                    on_hit_specials(g, ti, i, false);
                    damage_creep(g, i, dmg * 0.6, ti, false);
                    extra -= 1;
                }
            }
        }
        Delivery::Chain {
            bounces,
            falloff,
            hop,
        } => {
            chain(
                g,
                ti,
                ci,
                dmg,
                bounces,
                falloff,
                hop,
                crit,
                [muzzle[0], muzzle[1], mz],
                col,
            );
        }
        Delivery::Nova | Delivery::Aura => {}
    }
}

/// Lightning leaping from one monster to the next, losing power each hop.
#[allow(clippy::too_many_arguments)]
#[allow(clippy::too_many_arguments)]
fn chain(
    g: &mut Game,
    ti: usize,
    first: usize,
    dmg: f32,
    bounces: u32,
    falloff: f32,
    hop: f32,
    crit: bool,
    from: [f32; 3],
    col: [f32; 3],
) {
    let mut hit_uids: Vec<u32> = Vec::with_capacity(bounces as usize + 1);
    let mut cur = first;
    let mut power = dmg;
    let mut origin = from;

    for leap in 0..=bounces {
        if cur >= g.creeps.len() {
            break;
        }
        let uid = g.creeps[cur].uid;
        let to = [
            g.creeps[cur].pos[0],
            g.creeps[cur].pos[1],
            g.creeps[cur].height(),
        ];
        hit_uids.push(uid);

        g.beams.push(Beam {
            from: origin,
            to,
            color: col,
            t: 1.0,
            width: 0.08 * (1.0 - leap as f32 * 0.08).max(0.45),
        });

        on_hit_specials(g, ti, cur, crit && leap == 0);
        damage_creep(g, cur, power, ti, crit && leap == 0);

        if leap == bounces {
            break;
        }
        power *= falloff;
        origin = to;

        // Nearest monster not already struck by this bolt.
        let mut best: Option<usize> = None;
        let mut best_d = hop * hop;
        for (i, c) in g.creeps.iter().enumerate() {
            if hit_uids.contains(&c.uid) {
                continue;
            }
            let d = dist2(c.pos, [origin[0], origin[1]]);
            if d < best_d {
                best_d = d;
                best = Some(i);
            }
        }
        match best {
            Some(n) => cur = n,
            None => break,
        }
    }
}

// ---------------------------------------------------------------- projectiles

/// Everything needed to resolve an impact, captured before the projectile dies.
struct Detonation {
    dmg: f32,
    splash: f32,
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
                detonate_now = p.kind == ProjKind::Homing && p.splash > 0.0;
            }
        }

        if !remove && g.projs[i].kind == ProjKind::Homing {
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
            if p.pos[0] < -4.0 || p.pos[0] > 40.0 || p.pos[1] < -4.0 || p.pos[1] > 28.0 {
                remove = true;
            }
        }

        // Lances damage everything they pass through and keep going.
        if !remove && g.projs[i].kind == ProjKind::Lance {
            let ppos = g.projs[i].pos;
            let mut hits: Vec<usize> = Vec::new();
            for (ci, c) in g.creeps.iter().enumerate() {
                if dist2(c.pos, ppos) <= (c.radius + 0.2).powi(2)
                    && !g.projs[i].hit[..g.projs[i].hit_n as usize].contains(&c.uid)
                {
                    hits.push(ci);
                }
            }
            hits.sort_unstable_by(|a, b| b.cmp(a));
            for ci in hits {
                if ci >= g.creeps.len() {
                    continue;
                }
                let uid = g.creeps[ci].uid;
                let p = &mut g.projs[i];
                if (p.hit_n as usize) < p.hit.len() {
                    p.hit[p.hit_n as usize] = uid;
                    p.hit_n += 1;
                }
                let (dmg, tower, crit) = (p.dmg, p.tower, p.crit);
                on_hit_specials(g, tower, ci, crit);
                damage_creep(g, ci, dmg, tower, crit);
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
            on_hit_specials(g, tower, ci, crit);
            damage_creep(g, ci, dmg, tower, crit);
        }
    }

    if splash > 0.0 {
        let mut scratch = std::mem::take(&mut g.scratch);
        scratch.clear();
        g.spatial.query(ground, splash, |i| scratch.push(i));
        scratch.sort_unstable();
        scratch.dedup();
        let mut list: Vec<usize> = scratch
            .iter()
            .copied()
            .filter(|&i| i < g.creeps.len() && Some(i) != primary)
            .filter(|&i| dist2(g.creeps[i].pos, ground) <= (splash + g.creeps[i].radius).powi(2))
            .collect();
        list.sort_unstable_by(|a, b| b.cmp(a));
        for ci in list {
            if ci >= g.creeps.len() {
                continue;
            }
            let dd = dist2(g.creeps[ci].pos, ground).sqrt();
            let f = (1.0 - (dd / splash.max(0.001)) * 0.55).clamp(0.35, 1.0);
            on_hit_specials(g, tower, ci, false);
            damage_creep(g, ci, dmg * f, tower, false);
        }
        g.scratch = scratch;
    }
}

// ---------------------------------------------------------------- damage

/// Applies the on-hit riders (burn, slow, stun, ...) to one monster.
pub fn on_hit_specials(g: &mut Game, ti: usize, ci: usize, _crit: bool) {
    if ti >= g.towers.len() || ci >= g.creeps.len() {
        return;
    }
    let specials = g.towers[ti].specials();
    let k = g.towers[ti].scale();
    for s in specials.iter() {
        match *s {
            Special::Burn { dps, dur } => {
                g.creeps[ci].burn.apply(dps * k, dur);
            }
            Special::Poison { dps, dur } => {
                let c = &mut g.creeps[ci];
                // Venom stacks instead of refreshing - that is Bramble and Blight's
                // whole identity, and the reason they scale on one big target.
                c.poison.amt = (c.poison.amt + dps * k).min(dps * k * 12.0);
                c.poison.t = c.poison.t.max(dur);
            }
            Special::Slow { amt, dur } => {
                g.creeps[ci].slow.apply(amt, dur);
            }
            Special::Stun { chance, dur } => {
                // Bosses are immune to hard control by design, and nothing can
                // be stunned again inside its post-stun window.
                let locked = g.creeps[ci].stun > 0.0 || g.creeps[ci].stun_immune > 0.0;
                if g.creeps[ci].armor != Armor::Boss && !locked && g.rng.chance(chance) {
                    let c = &mut g.creeps[ci];
                    // Diminishing returns. Without them, enough Eclipse towers
                    // freeze a wave permanently: nothing dies, nothing leaks,
                    // and the wave simply never ends. A full campaign got stuck
                    // on wave 76 that way. Each stun in quick succession lands
                    // shorter, and the resistance bleeds off once the target is
                    // left alone.
                    let effective = dur * (1.0 - c.stun_dr);
                    if effective > 0.05 {
                        c.stun = c.stun.max(effective);
                    }
                    c.stun_dr = (c.stun_dr + STUN_DR_STEP).min(STUN_DR_MAX);
                }
            }
            Special::Shred { amt, dur } => {
                g.creeps[ci].shred.apply(amt, dur);
            }
            // Thornwall shoves, Abyss drags. Both spend from the same
            // per-monster budget and share one cooldown, because two towers
            // that each move a monster backwards faster than it walks forwards
            // is a wave that never arrives.
            //
            // Neither scales with the tower's level. Displacement is measured
            // in tiles of road, and the road does not get longer as a tower
            // gets stronger - Pull briefly scaled with the damage curve, which
            // at level eight dragged a monster six tiles per hit and stalled
            // the game outright.
            Special::Knockback { dist } => push_back(&mut g.creeps[ci], dist),
            Special::Pull { dist } => push_back(&mut g.creeps[ci], dist),
            Special::Suppress => {
                // Mire. Regeneration and Mender healing both stop while this
                // holds, which is the only counter in the game to a wave that
                // out-heals a board rather than out-tanking it.
                g.creeps[ci].suppress = g.creeps[ci].suppress.max(SUPPRESS_TIME);
            }
            _ => {}
        }
    }
}

/// Moves one monster back down the road, within its cooldown and its budget.
fn push_back(c: &mut crate::game::Creep, dist: f32) {
    if c.armor == Armor::Boss || c.kb_cd > 0.0 || c.push_left <= 0.0 {
        return;
    }
    let moved = dist.min(c.push_left);
    c.dist = (c.dist - moved).max(0.0);
    c.push_left -= moved;
    c.kb_cd = KNOCKBACK_CD;
}

/// Deals `base` damage (before armour) and cleans up if the monster dies.
pub fn damage_creep(g: &mut Game, ci: usize, base: f32, ti: usize, crit: bool) -> bool {
    if ci >= g.creeps.len() {
        return false;
    }
    let dtype = if ti < g.towers.len() {
        g.towers[ti].dtype()
    } else {
        Damage::Physical
    };
    let mult = armor_mult(dtype, g.creeps[ci].armor);
    let shred = if g.creeps[ci].shred.active() {
        g.creeps[ci].shred.amt
    } else {
        0.0
    };
    // Hellfire: the multiplier is read from the health bar at the moment of the
    // hit, so a board that chips a target down hands it a finisher.
    let execute = if ti < g.towers.len() {
        let frac = g.creeps[ci].hp_frac();
        g.towers[ti]
            .specials()
            .iter()
            .find_map(|s| match *s {
                Special::Execute { below, mult } if frac <= below => Some(mult),
                _ => None,
            })
            .unwrap_or(1.0)
    } else {
        1.0
    };
    let mut dealt = base * mult * (1.0 + shred) * execute;

    // One-strike kill. Rolled before anything else, because a monster this
    // lands on does not care about armour, shields or health. Never on a boss:
    // a boss deleted by a coin flip is not a boss.
    if ti < g.towers.len() && g.creeps[ci].armor != Armor::Boss {
        let chance = g.towers[ti]
            .specials()
            .iter()
            .find_map(|s| match *s {
                Special::Instakill { chance } => Some(chance),
                _ => None,
            })
            .unwrap_or(0.0);
        if chance > 0.0 && g.rng.chance(chance) {
            let pos = g.creeps[ci].pos;
            let z = g.creeps[ci].height();
            let hp = g.creeps[ci].hp;
            g.creeps[ci].hp = 0.0;
            g.towers[ti].damage += hp as f64;
            g.stats.damage += hp as f64;
            g.texts.push(FloatText {
                pos: [pos[0], pos[1], z + 0.35],
                value: hp,
                kind: TextKind::Crit,
                t: 1.1,
            });
            g.fx.burst(&mut g.rng, pos, 20, 2.4, [1.0, 0.95, 0.55, 1.0], 0.45, 0.22);
            contagion(g, ci, ti);
            let c = g.creeps[ci].clone();
            g.on_creep_died(&c, Some(ti));
            g.creeps.swap_remove(ci);
            return true;
        }
    }

    let c = &mut g.creeps[ci];
    // Shields soak everything except Toxic, which is the point of Toxic.
    if c.shield > 0.0 && dtype != Damage::Toxic {
        let absorbed = dealt.min(c.shield);
        c.shield -= absorbed;
        dealt -= absorbed;
    }
    c.hp -= dealt;
    c.flash = 1.0;
    let dead = c.hp <= 0.0;
    let pos = c.pos;
    let z = c.height();

    if ti < g.towers.len() {
        g.towers[ti].damage += dealt as f64;
    }
    g.stats.damage += dealt as f64;

    // Only the loud hits get a number, otherwise the board is unreadable.
    if crit || dealt >= 60.0 {
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
        contagion(g, ci, ti);
        let c = g.creeps[ci].clone();
        g.on_creep_died(&c, Some(ti));
        g.creeps.swap_remove(ci);
    }
    dead
}

/// Blight: the damage-over-time jumps to whatever is standing near the corpse,
/// which is what turns one kill in a packed lane into a chain of them.
fn contagion(g: &mut Game, ci: usize, ti: usize) {
    if ti >= g.towers.len() || ci >= g.creeps.len() {
        return;
    }
    let Some(radius) = g.towers[ti].specials().iter().find_map(|s| match *s {
        Special::Contagion { radius } => Some(radius),
        _ => None,
    }) else {
        return;
    };
    let (pos, burn, poison) = {
        let c = &g.creeps[ci];
        (c.pos, c.burn, c.poison)
    };
    if burn.t <= 0.0 && poison.t <= 0.0 {
        return;
    }
    let uid = g.creeps[ci].uid;
    for c in g.creeps.iter_mut() {
        if c.uid == uid || dist2(c.pos, pos) > radius * radius {
            continue;
        }
        if burn.t > 0.0 {
            c.burn.apply(burn.amt * 0.75, burn.t.max(1.5));
        }
        if poison.t > 0.0 {
            c.poison.amt = (c.poison.amt + poison.amt * 0.6).min(poison.amt * 4.0);
            c.poison.t = c.poison.t.max(poison.t);
        }
    }
    g.fx.burst_at(
        &mut g.rng,
        [pos[0], pos[1], 0.4],
        22,
        radius * 2.2,
        [0.7, 1.0, 0.4, 1.0],
        0.45,
        0.20,
    );
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

/// Squared distance from point `p` to segment `a`-`b`.
fn point_seg_dist2(p: [f32; 2], a: [f32; 2], b: [f32; 2]) -> f32 {
    let abx = b[0] - a[0];
    let aby = b[1] - a[1];
    let len2 = abx * abx + aby * aby;
    if len2 < 1e-6 {
        return dist2(p, a);
    }
    let t = (((p[0] - a[0]) * abx + (p[1] - a[1]) * aby) / len2).clamp(0.0, 1.0);
    dist2(p, [a[0] + abx * t, a[1] + aby * t])
}
