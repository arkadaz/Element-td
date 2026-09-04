//! Particle spawn queue.
//!
//! The simulation only ever *describes* a particle: where it starts, how fast it
//! is going, how long it lives. The GPU integrates the motion in the vertex
//! shader, so nothing here is touched again after it is queued.

use crate::rng::Rng;

/// Height particles spawn at when only a ground position is given.
const GROUND_Z: f32 = 0.25;
const PARTICLE_CAP: usize = 8192;

/// Shape language understood by the billboard shader.
///
/// Keeping this as a tiny numeric vocabulary lets one instanced draw render
/// hot cores, sparks, smoke, embers, spell rings and physical fragments.  A
/// texture atlas would add another download and sampling cost; these analytic
/// silhouettes stay sharp at every browser resolution.
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParticleStyle {
    Soft = 0,
    Spark = 1,
    Smoke = 2,
    Ember = 3,
    Magic = 4,
    Shard = 5,
}

impl ParticleStyle {
    #[inline]
    fn shader_id(self) -> f32 {
        self as u8 as f32
    }
}

#[derive(Clone, Copy)]
pub struct ParticleSpawn {
    pub pos: [f32; 3],
    pub vel: [f32; 3],
    pub life: f32,
    /// Start and end radius, in tiles.
    pub size: [f32; 2],
    pub color: [f32; 4],
    /// x = [`ParticleStyle`] shader id, y = per-particle rotation seed.
    pub style: [f32; 2],
}

#[derive(Default)]
pub struct Fx {
    pub particles: Vec<ParticleSpawn>,
}

impl Fx {
    #[inline]
    pub fn has_capacity(&self) -> bool {
        self.particles.len() < PARTICLE_CAP
    }

    pub fn push(&mut self, p: ParticleSpawn) {
        // Hard cap: a frame that queues more than this is already off-screen chaos.
        if self.has_capacity() {
            self.particles.push(p);
        }
    }

    /// Radial spray from a ground position.
    pub fn burst(
        &mut self,
        rng: &mut Rng,
        pos: [f32; 2],
        n: u32,
        spread: f32,
        color: [f32; 4],
        life: f32,
        size: f32,
    ) {
        self.burst_at(
            rng,
            [pos[0], pos[1], GROUND_Z],
            n,
            spread,
            color,
            life,
            size,
        );
    }

    /// Radial spray in three dimensions, biased upwards so debris arcs.
    #[allow(clippy::too_many_arguments)]
    pub fn burst_at(
        &mut self,
        rng: &mut Rng,
        pos: [f32; 3],
        n: u32,
        spread: f32,
        color: [f32; 4],
        life: f32,
        size: f32,
    ) {
        self.burst_styled_at(
            rng,
            pos,
            n,
            spread,
            color,
            life,
            [size, 0.0],
            ParticleStyle::Spark,
        );
    }

    /// Radial spray with a deliberate screen-space silhouette.  Smoke grows,
    /// embers rise, and magic stays close to the impact plane; those small
    /// motion differences do more for material identity than simply changing
    /// every effect's colour.
    #[allow(clippy::too_many_arguments)]
    pub fn burst_styled_at(
        &mut self,
        rng: &mut Rng,
        pos: [f32; 3],
        n: u32,
        spread: f32,
        color: [f32; 4],
        life: f32,
        size: [f32; 2],
        style: ParticleStyle,
    ) {
        // Once the frame queue is full, even generating discarded directions,
        // speeds and lifetimes is wasted CPU work. Dense/endless boards can
        // request tens of thousands of cosmetic particles in one update.
        if self.particles.len() >= PARTICLE_CAP {
            return;
        }
        for _ in 0..n {
            let d = rng.dir();
            let s = rng.range(0.25, 1.0) * spread;
            let (planar, vertical) = match style {
                ParticleStyle::Smoke => (s * 0.30, rng.range(0.35, 0.85) * spread),
                ParticleStyle::Ember => (s * 0.62, rng.range(0.25, 0.95) * spread),
                ParticleStyle::Magic => (s * 0.72, rng.range(-0.08, 0.22) * spread),
                ParticleStyle::Shard => (s, rng.range(0.25, 1.05) * spread * 0.70),
                ParticleStyle::Soft | ParticleStyle::Spark => {
                    (s, rng.range(0.1, 1.0) * spread * 0.55)
                }
            };
            self.push(ParticleSpawn {
                pos,
                vel: [d[0] * planar, d[1] * planar, vertical],
                life: life * rng.range(0.6, 1.25),
                size: [
                    size[0] * rng.range(0.5, 1.1),
                    size[1] * rng.range(0.8, 1.25),
                ],
                color,
                style: [style.shader_id(), rng.range(0.0, std::f32::consts::TAU)],
            });
        }
    }

    /// Narrow cone, used for muzzle flashes and directed impacts.
    #[allow(clippy::too_many_arguments)]
    pub fn cone(
        &mut self,
        rng: &mut Rng,
        pos: [f32; 3],
        dir: [f32; 2],
        n: u32,
        speed: f32,
        color: [f32; 4],
        life: f32,
        size: f32,
    ) {
        self.cone_styled(
            rng,
            pos,
            dir,
            n,
            speed,
            color,
            life,
            [size, 0.0],
            ParticleStyle::Spark,
        );
    }

    #[allow(clippy::too_many_arguments)]
    pub fn cone_styled(
        &mut self,
        rng: &mut Rng,
        pos: [f32; 3],
        dir: [f32; 2],
        n: u32,
        speed: f32,
        color: [f32; 4],
        life: f32,
        size: [f32; 2],
        style: ParticleStyle,
    ) {
        if self.particles.len() >= PARTICLE_CAP {
            return;
        }
        let base = dir[1].atan2(dir[0]);
        for _ in 0..n {
            let a = base + rng.range(-0.5, 0.5);
            let s = rng.range(0.4, 1.0) * speed;
            self.push(ParticleSpawn {
                pos,
                vel: [a.cos() * s, a.sin() * s, rng.range(-0.2, 0.5)],
                life: life * rng.range(0.6, 1.2),
                size: [
                    size[0] * rng.range(0.6, 1.15),
                    size[1] * rng.range(0.85, 1.15),
                ],
                color,
                style: [style.shader_id(), rng.range(0.0, std::f32::consts::TAU)],
            });
        }
    }

    /// A single drifting ember (projectile trails, ambient motes).
    pub fn mote(&mut self, pos: [f32; 3], vel: [f32; 3], life: f32, size: f32, color: [f32; 4]) {
        self.mote_styled(pos, vel, life, [size, 0.0], color, ParticleStyle::Soft, 0.0);
    }

    #[allow(clippy::too_many_arguments)]
    pub fn mote_styled(
        &mut self,
        pos: [f32; 3],
        vel: [f32; 3],
        life: f32,
        size: [f32; 2],
        color: [f32; 4],
        style: ParticleStyle,
        rotation: f32,
    ) {
        self.push(ParticleSpawn {
            pos,
            vel,
            life,
            size,
            color,
            style: [style.shader_id(), rotation],
        });
    }

    /// One expanding analytic ring.  Used at spell impacts and boss deaths;
    /// unlike a ground mesh it can face the camera and bloom without adding a
    /// separate render pass.
    pub fn ring(&mut self, pos: [f32; 3], life: f32, size: [f32; 2], color: [f32; 4]) {
        self.mote_styled(
            pos,
            [0.0, 0.0, 0.08],
            life,
            size,
            color,
            ParticleStyle::Magic,
            0.0,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn analytic_particle_styles_keep_the_shader_abi() {
        assert_eq!(ParticleStyle::Soft.shader_id(), 0.0);
        assert_eq!(ParticleStyle::Spark.shader_id(), 1.0);
        assert_eq!(ParticleStyle::Smoke.shader_id(), 2.0);
        assert_eq!(ParticleStyle::Ember.shader_id(), 3.0);
        assert_eq!(ParticleStyle::Magic.shader_id(), 4.0);
        assert_eq!(ParticleStyle::Shard.shader_id(), 5.0);
    }

    #[test]
    fn ring_expands_without_ballistic_motion() {
        let mut fx = Fx::default();
        fx.ring([1.0, 2.0, 3.0], 0.4, [0.1, 0.9], [1.0; 4]);
        assert_eq!(fx.particles.len(), 1);
        let p = fx.particles[0];
        assert_eq!(p.style[0], ParticleStyle::Magic.shader_id());
        assert!(p.size[1] > p.size[0]);
        assert_eq!(p.vel[..2], [0.0, 0.0]);
    }
}
