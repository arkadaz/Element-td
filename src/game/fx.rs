//! Particle spawn queue.
//!
//! The simulation only ever *describes* a particle: where it starts, how fast it
//! is going, how long it lives. The GPU integrates the motion in the vertex
//! shader, so nothing here is touched again after it is queued.

use crate::rng::Rng;

/// Height particles spawn at when only a ground position is given.
const GROUND_Z: f32 = 0.25;

#[derive(Clone, Copy)]
pub struct ParticleSpawn {
    pub pos: [f32; 3],
    pub vel: [f32; 3],
    pub life: f32,
    /// Start and end radius, in tiles.
    pub size: [f32; 2],
    pub color: [f32; 4],
}

#[derive(Default)]
pub struct Fx {
    pub particles: Vec<ParticleSpawn>,
}

impl Fx {
    pub fn push(&mut self, p: ParticleSpawn) {
        // Hard cap: a frame that queues more than this is already off-screen chaos.
        if self.particles.len() < 8192 {
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
        for _ in 0..n {
            let d = rng.dir();
            let s = rng.range(0.25, 1.0) * spread;
            self.push(ParticleSpawn {
                pos,
                vel: [d[0] * s, d[1] * s, rng.range(0.1, 1.0) * spread * 0.55],
                life: life * rng.range(0.6, 1.25),
                size: [size * rng.range(0.5, 1.1), 0.0],
                color,
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
        let base = dir[1].atan2(dir[0]);
        for _ in 0..n {
            let a = base + rng.range(-0.5, 0.5);
            let s = rng.range(0.4, 1.0) * speed;
            self.push(ParticleSpawn {
                pos,
                vel: [a.cos() * s, a.sin() * s, rng.range(-0.2, 0.5)],
                life: life * rng.range(0.6, 1.2),
                size: [size * rng.range(0.6, 1.15), 0.0],
                color,
            });
        }
    }

    /// A single drifting ember (projectile trails, ambient motes).
    pub fn mote(&mut self, pos: [f32; 3], vel: [f32; 3], life: f32, size: f32, color: [f32; 4]) {
        self.push(ParticleSpawn {
            pos,
            vel,
            life,
            size: [size, 0.0],
            color,
        });
    }
}
