//! Small, original Web Audio soundscape.
//!
//! The browser build is the primary playable target. Tones are synthesised at
//! runtime, so there are no borrowed samples, no decode stalls, and no extra
//! download. Combat is deliberately rate-limited: clarity beats adding the
//! sound of every projectile to a late-game barrage.

use crate::game::Cue;

#[cfg(target_arch = "wasm32")]
use crate::game::ProjKind;

#[cfg(target_arch = "wasm32")]
pub struct Audio {
    ctx: Option<web_sys::AudioContext>,
    /// One deterministic broadband buffer, filtered differently for muzzle
    /// blasts, explosions, flame and creature bodies. Reusing it avoids a JS
    /// allocation for every impact in a dense wave.
    noise: Option<web_sys::AudioBuffer>,
    last_shot: f64,
    last_impact: f64,
    last_death: f64,
}

#[cfg(not(target_arch = "wasm32"))]
#[derive(Default)]
pub struct Audio;

#[cfg(not(target_arch = "wasm32"))]
impl Audio {
    pub fn new() -> Self {
        Self
    }

    pub fn play_cues(&mut self, cues: &mut Vec<Cue>) {
        cues.clear();
    }
}

#[cfg(target_arch = "wasm32")]
impl Audio {
    pub fn new() -> Self {
        let ctx = web_sys::AudioContext::new().ok();
        let noise = ctx.as_ref().and_then(make_noise);
        Self {
            // Creating the context is permitted before interaction; Edge will
            // keep it suspended until the first deliberate game command.
            ctx,
            noise,
            last_shot: -10.0,
            last_impact: -10.0,
            last_death: -10.0,
        }
    }

    pub fn play_cues(&mut self, cues: &mut Vec<Cue>) {
        if cues.is_empty() {
            return;
        }
        let Some(ctx) = self.ctx.as_ref() else {
            cues.clear();
            return;
        };
        if ctx.state() == web_sys::AudioContextState::Suspended {
            let _ = ctx.resume();
        }

        let now = ctx.current_time();
        let mut shot = None;
        let mut impact = None;
        let mut death = None;
        for cue in cues.drain(..) {
            match cue {
                Cue::Select => tone(ctx, now, 510.0, 680.0, 0.055, 0.018, Wave::Sine),
                Cue::Build => {
                    tone(ctx, now, 245.0, 390.0, 0.13, 0.055, Wave::Triangle);
                    tone(ctx, now + 0.055, 390.0, 520.0, 0.11, 0.035, Wave::Sine);
                }
                Cue::Sell => tone(ctx, now, 420.0, 210.0, 0.16, 0.045, Wave::Triangle),
                Cue::Error => tone(ctx, now, 132.0, 92.0, 0.18, 0.065, Wave::Square),
                Cue::Shot(kind) => shot = Some(kind),
                Cue::Impact(kind) => impact = Some(kind),
                Cue::MonsterDeath { boss, mechanical } => {
                    death = Some((boss, mechanical));
                }
                Cue::WaveStart => {
                    tone(ctx, now, 196.0, 294.0, 0.18, 0.045, Wave::Triangle);
                    tone(ctx, now + 0.10, 294.0, 392.0, 0.18, 0.040, Wave::Triangle);
                }
                Cue::Boss => {
                    tone(ctx, now, 82.0, 55.0, 0.55, 0.080, Wave::Saw);
                    tone(ctx, now + 0.16, 110.0, 73.0, 0.45, 0.060, Wave::Square);
                }
                Cue::Victory => {
                    for (i, hz) in [262.0, 330.0, 392.0, 523.0].iter().enumerate() {
                        tone(
                            ctx,
                            now + i as f64 * 0.105,
                            *hz,
                            *hz * 1.01,
                            0.24,
                            0.045,
                            Wave::Triangle,
                        );
                    }
                }
                Cue::Defeat => {
                    tone(ctx, now, 220.0, 82.0, 0.85, 0.075, Wave::Saw);
                    tone(ctx, now + 0.18, 110.0, 55.0, 0.70, 0.050, Wave::Sine);
                }
            }
        }

        // Launch and impact have separate clocks, so a slow siege shell has a
        // beginning and an end. Both are aggressively rate-limited: late game
        // should sound like a battle rhythm, not digital clipping.
        if let Some(kind) = shot.filter(|_| now - self.last_shot >= 0.075) {
            self.last_shot = now;
            match kind {
                ProjKind::Dart | ProjKind::Glaive => {
                    tone(ctx, now, 920.0, 510.0, 0.038, 0.010, Wave::Triangle)
                }
                ProjKind::Bolt => tone(ctx, now, 1380.0, 680.0, 0.045, 0.012, Wave::Square),
                ProjKind::Acid => tone(ctx, now, 410.0, 245.0, 0.075, 0.012, Wave::Sine),
                ProjKind::Shell => {
                    tone(ctx, now, 150.0, 74.0, 0.095, 0.022, Wave::Triangle);
                    noise_burst(ctx, self.noise.as_ref(), now, 0.055, 0.012, 900.0, false);
                }
                ProjKind::Missile => {
                    tone(ctx, now, 260.0, 145.0, 0.10, 0.016, Wave::Saw);
                    noise_burst(ctx, self.noise.as_ref(), now, 0.085, 0.009, 1800.0, true);
                }
                ProjKind::Flame => {
                    tone(ctx, now, 235.0, 105.0, 0.085, 0.016, Wave::Saw);
                    noise_burst(ctx, self.noise.as_ref(), now, 0.075, 0.010, 1200.0, false);
                }
                ProjKind::Chaos => {
                    tone(ctx, now, 620.0, 155.0, 0.11, 0.018, Wave::Square);
                    tone(ctx, now, 315.0, 94.0, 0.13, 0.010, Wave::Sine);
                }
                ProjKind::Orb => {
                    tone(ctx, now, 720.0, 280.0, 0.095, 0.014, Wave::Sine);
                    tone(ctx, now + 0.012, 1080.0, 410.0, 0.08, 0.007, Wave::Triangle);
                }
                ProjKind::Royal => {
                    tone(ctx, now, 980.0, 122.0, 0.15, 0.022, Wave::Triangle);
                    tone(ctx, now + 0.018, 1470.0, 244.0, 0.12, 0.010, Wave::Sine);
                }
            }
        }

        if let Some(kind) = impact.filter(|_| now - self.last_impact >= 0.095) {
            self.last_impact = now;
            let (from, to, dur, gain, wave) = match kind {
                ProjKind::Dart | ProjKind::Glaive => (760.0, 420.0, 0.045, 0.013, Wave::Triangle),
                ProjKind::Bolt => (980.0, 560.0, 0.055, 0.014, Wave::Square),
                ProjKind::Acid => (330.0, 145.0, 0.10, 0.019, Wave::Sine),
                ProjKind::Shell => (126.0, 48.0, 0.16, 0.040, Wave::Saw),
                ProjKind::Missile => (185.0, 62.0, 0.14, 0.034, Wave::Triangle),
                ProjKind::Flame => (210.0, 72.0, 0.13, 0.032, Wave::Saw),
                ProjKind::Chaos => (440.0, 86.0, 0.16, 0.030, Wave::Square),
                ProjKind::Orb => (520.0, 220.0, 0.10, 0.020, Wave::Sine),
                ProjKind::Royal => (660.0, 110.0, 0.19, 0.040, Wave::Triangle),
            };
            tone(ctx, now, from, to, dur, gain, wave);
            match kind {
                ProjKind::Shell | ProjKind::Missile => {
                    noise_burst(ctx, self.noise.as_ref(), now, 0.16, 0.032, 620.0, false);
                }
                ProjKind::Flame => {
                    noise_burst(ctx, self.noise.as_ref(), now, 0.12, 0.018, 1450.0, false);
                }
                ProjKind::Dart | ProjKind::Glaive | ProjKind::Bolt => {
                    noise_burst(ctx, self.noise.as_ref(), now, 0.032, 0.007, 2600.0, true);
                }
                ProjKind::Acid | ProjKind::Chaos | ProjKind::Orb | ProjKind::Royal => {}
            }
        }

        if let Some((boss, mechanical)) =
            death.filter(|(boss, _)| *boss || now - self.last_death >= 0.16)
        {
            self.last_death = now;
            if boss {
                // Long, layered and unmistakable, but still below the UI and
                // victory cues in volume.
                tone(ctx, now, 92.0, 38.0, 0.72, 0.050, Wave::Saw);
                tone(ctx, now + 0.045, 138.0, 46.0, 0.62, 0.025, Wave::Triangle);
                noise_burst(ctx, self.noise.as_ref(), now, 0.48, 0.040, 430.0, false);
            } else if mechanical {
                tone(ctx, now, 520.0, 118.0, 0.13, 0.017, Wave::Square);
                noise_burst(ctx, self.noise.as_ref(), now, 0.12, 0.018, 1800.0, true);
            } else {
                // A compact downward formant reads as a creature falling
                // without replaying a stock scream hundreds of times.
                tone(ctx, now, 176.0, 68.0, 0.19, 0.018, Wave::Saw);
                tone(ctx, now + 0.020, 112.0, 52.0, 0.16, 0.010, Wave::Triangle);
                noise_burst(ctx, self.noise.as_ref(), now, 0.13, 0.009, 760.0, false);
            }
        }
    }
}

#[cfg(target_arch = "wasm32")]
#[derive(Clone, Copy)]
enum Wave {
    Sine,
    Triangle,
    Square,
    Saw,
}

#[cfg(target_arch = "wasm32")]
fn tone(
    ctx: &web_sys::AudioContext,
    at: f64,
    from_hz: f32,
    to_hz: f32,
    seconds: f64,
    volume: f32,
    wave: Wave,
) {
    let Ok(osc) = ctx.create_oscillator() else {
        return;
    };
    let Ok(gain) = ctx.create_gain() else { return };
    osc.set_type(match wave {
        Wave::Sine => web_sys::OscillatorType::Sine,
        Wave::Triangle => web_sys::OscillatorType::Triangle,
        Wave::Square => web_sys::OscillatorType::Square,
        Wave::Saw => web_sys::OscillatorType::Sawtooth,
    });
    let end = at + seconds;
    let _ = osc.frequency().set_value_at_time(from_hz, at);
    let _ = osc
        .frequency()
        .exponential_ramp_to_value_at_time(to_hz.max(1.0), end);
    let _ = gain.gain().set_value_at_time(0.0001, at);
    let _ = gain.gain().linear_ramp_to_value_at_time(volume, at + 0.008);
    let _ = gain.gain().exponential_ramp_to_value_at_time(0.0001, end);
    let _ = osc.connect_with_audio_node(&gain);
    let _ = gain.connect_with_audio_node(&ctx.destination());
    let _ = osc.start_with_when(at);
    let _ = osc.stop_with_when(end + 0.01);
}

/// Builds a one-second, deterministic noise bed. The xorshift sequence avoids
/// pulling JavaScript randomness into every combat frame and makes audio QA
/// reproducible.
#[cfg(target_arch = "wasm32")]
fn make_noise(ctx: &web_sys::AudioContext) -> Option<web_sys::AudioBuffer> {
    let rate = ctx.sample_rate();
    let len = rate.max(8_000.0).round() as u32;
    let buffer = ctx.create_buffer(1, len, rate).ok()?;
    let mut seed = 0x6d2b_79f5u32;
    let mut samples = Vec::with_capacity(len as usize);
    for _ in 0..len {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        samples.push((seed as f32 / u32::MAX as f32) * 2.0 - 1.0);
    }
    buffer.copy_to_channel(&samples, 0).ok()?;
    Some(buffer)
}

/// Filtered noise transient for material detail: low-pass for explosions and
/// bodies, high-pass for metal fractures and arrow strikes.
#[cfg(target_arch = "wasm32")]
fn noise_burst(
    ctx: &web_sys::AudioContext,
    noise: Option<&web_sys::AudioBuffer>,
    at: f64,
    seconds: f64,
    volume: f32,
    cutoff: f32,
    high_pass: bool,
) {
    let Some(noise) = noise else { return };
    let Ok(src) = ctx.create_buffer_source() else {
        return;
    };
    let Ok(filter) = ctx.create_biquad_filter() else {
        return;
    };
    let Ok(gain) = ctx.create_gain() else { return };
    src.set_buffer(Some(noise));
    filter.set_type(if high_pass {
        web_sys::BiquadFilterType::Highpass
    } else {
        web_sys::BiquadFilterType::Lowpass
    });
    let _ = filter.frequency().set_value_at_time(cutoff, at);
    let _ = filter
        .frequency()
        .exponential_ramp_to_value_at_time((cutoff * 0.42).max(40.0), at + seconds);
    let _ = gain.gain().set_value_at_time(0.0001, at);
    let _ = gain.gain().linear_ramp_to_value_at_time(volume, at + 0.004);
    let _ = gain
        .gain()
        .exponential_ramp_to_value_at_time(0.0001, at + seconds);
    let _ = src.connect_with_audio_node(&filter);
    let _ = filter.connect_with_audio_node(&gain);
    let _ = gain.connect_with_audio_node(&ctx.destination());
    let _ = src.start_with_when_and_grain_offset_and_grain_duration(at, 0.0, seconds);
}
