//! Speaker sound. The core's cycle-stamped `$C030` toggles become a
//! waveform that is queued to SDL once per frame.
//!
//! This deliberately diverges from `snd.c` (#188): the C held the output
//! at a DC rail (±amplitude forever after the last toggle), so any queue
//! underrun — a late frame during game loading, say — snapped the output
//! to silence and back, which was audible as clicks that real hardware
//! never makes. Here the level decays toward zero like the real AC-coupled
//! speaker cone relaxing to rest: silence is actually silence, and
//! underruns during silence are inaudible.

use sdl3::AudioSubsystem;
use sdl3::audio::{AudioFormat, AudioSpec, AudioStreamOwner};

pub const SND_SAMPLE_RATE: u32 = 44100;
const SND_BUFFER_SIZE: usize = 4096;
const SND_AMPLITUDE: f32 = 8000.0;
const SND_CPU_FREQUENCY: u64 = 1_023_000;

/// Per-sample decay factor for the speaker level, ≈ a 4 ms time constant
/// at 44.1 kHz (exp(-1 / (0.004 * 44100))): fast enough that silence
/// follows within ~25 ms of the last toggle, slow enough that a 1 kHz
/// beep keeps its square timbre.
const SND_DECAY: f32 = 0.9943;

/// Prime the queue with this much silence at startup, as headroom against
/// late frames.
const SND_PRIME_SAMPLES: usize = (SND_SAMPLE_RATE as usize) * 64 / 1000;

/// Skip queueing an all-silent frame once this many bytes (100 ms of mono
/// i16 samples) are already buffered.
const SND_QUEUE_CAP_BYTES: i32 = (SND_SAMPLE_RATE * 2 / 10) as i32;

/// The pure synthesis half: toggles in, samples out. Kept free of SDL so
/// the waveform is unit-testable.
struct Wave {
    /// Current output level; jumps to a rail on a toggle and decays toward
    /// zero between them, like the AC-coupled cone.
    level: f32,
    /// Which rail the next toggle jumps to.
    polarity: bool,
    frame_start_cycle: u64,
    /// Emulated CPU cycles per second, used to map cycle timestamps to
    /// sample positions. Scales with the accelerator speed so a frame's
    /// worth of samples stays real-time even when the machine runs faster —
    /// the sound then pitches up, as a real accelerator card does, instead
    /// of overflowing the audio queue.
    cpu_frequency: u64,
    buffer: Vec<i16>,
}

impl Wave {
    fn new() -> Wave {
        Wave {
            level: 0.0,
            polarity: false,
            frame_start_cycle: 0,
            cpu_frequency: SND_CPU_FREQUENCY,
            buffer: Vec::with_capacity(SND_BUFFER_SIZE),
        }
    }

    fn sample_index(&self, cpu_counter: u64) -> usize {
        let cycles = cpu_counter.saturating_sub(self.frame_start_cycle);
        ((cycles * SND_SAMPLE_RATE as u64 / self.cpu_frequency) as usize).min(SND_BUFFER_SIZE)
    }

    fn emit_until(&mut self, sample_index: usize) {
        while self.buffer.len() < sample_index {
            self.buffer.push(self.level as i16);
            self.level *= SND_DECAY;
            if self.level.abs() < 1.0 {
                self.level = 0.0;
            }
        }
    }

    /// Render one frame: replay the cycle-stamped toggles into samples up
    /// to `cpu_counter`, decaying between edges.
    fn render(&mut self, toggles: &[u64], cpu_counter: u64) -> &[i16] {
        self.buffer.clear();
        for &cycle in toggles {
            let index = self.sample_index(cycle);
            self.emit_until(index);
            self.polarity = !self.polarity;
            self.level = if self.polarity {
                SND_AMPLITUDE
            } else {
                -SND_AMPLITUDE
            };
        }
        let end = self.sample_index(cpu_counter);
        self.emit_until(end);
        self.frame_start_cycle = cpu_counter;
        &self.buffer
    }
}

pub struct Snd {
    stream: AudioStreamOwner,
    wave: Wave,
}

impl Snd {
    pub fn new(audio: &AudioSubsystem) -> Result<Snd, String> {
        // SDL3 has no per-device sample-buffer request (the SDL2 `samples:
        // 512`); SDL picks the device buffer size.
        let spec = AudioSpec {
            freq: Some(SND_SAMPLE_RATE as i32),
            channels: Some(1),
            format: Some(AudioFormat::s16_sys()),
        };
        let device = audio
            .open_playback_device(&spec)
            .map_err(|e| e.to_string())?;
        let stream = device
            .open_device_stream(Some(&spec))
            .map_err(|e| e.to_string())?;
        // Prime the queue so a late frame does not immediately underrun.
        // The device starts paused in SDL3, so the silence sits buffered
        // until resume.
        let _ = stream.put_data_i16(&vec![0i16; SND_PRIME_SAMPLES]);
        stream.resume().map_err(|e| e.to_string())?;
        Ok(Snd {
            stream,
            wave: Wave::new(),
        })
    }

    /// Track the emulated CPU speed (Hz). At an accelerator speed the same
    /// wall-clock frame spans more emulated cycles, so scaling the mapping
    /// keeps each frame's sample count real-time and pitches the sound up
    /// instead of flooding the queue.
    pub fn set_cpu_frequency(&mut self, hz: u64) {
        self.wave.cpu_frequency = hz.max(1);
    }

    /// Queue one frame of samples. When the queue is comfortably full an
    /// all-silent frame is skipped (inaudible); a frame carrying signal is
    /// always queued — dropping samples mid-tone is what clicks.
    pub fn update(&mut self, toggles: &[u64], cpu_counter: u64) {
        let samples = self.wave.render(toggles, cpu_counter);
        if samples.is_empty() {
            return;
        }
        let silent = samples.iter().all(|&s| s == 0);
        if silent && self.stream.queued_bytes().unwrap_or(0) >= SND_QUEUE_CAP_BYTES {
            return;
        }
        let _ = self.stream.put_data_i16(samples);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Cycles for `n` samples, rounded up (the inverse of sample_index).
    fn cycles_for_samples(n: u64) -> u64 {
        n * SND_CPU_FREQUENCY / SND_SAMPLE_RATE as u64 + 1
    }

    #[test]
    fn idle_wave_is_silent() {
        let mut wave = Wave::new();
        let samples = wave.render(&[], cycles_for_samples(1000));
        assert_eq!(samples.len(), 1000);
        assert!(samples.iter().all(|&s| s == 0), "idle output must be zero");
    }

    #[test]
    fn higher_cpu_frequency_keeps_the_frame_real_time() {
        // Accelerator pacing: one real frame is a fixed slice of wall time,
        // so it must yield the same number of samples no matter how many
        // emulated cycles pass in it. At 7x the frequency the frame spans 7x
        // the cycles but still renders ~one frame of samples (the sound then
        // pitches up), which is what stops the audio queue from overflowing.
        let normal_cycles = 1_023_000 / 40; // one 40fps frame at 1x
        let mut base = Wave::new();
        let base_len = base.render(&[], normal_cycles).len();

        let mut fast = Wave::new();
        fast.cpu_frequency = 7 * SND_CPU_FREQUENCY;
        let fast_len = fast.render(&[], 7 * normal_cycles).len();

        assert_eq!(
            fast_len, base_len,
            "a real frame renders the same sample count at any speed"
        );
    }

    #[test]
    fn single_toggle_clicks_then_decays_to_silence() {
        // The regression test for the loading clicks: after a toggle the
        // level must return to zero instead of holding a DC rail.
        let mut wave = Wave::new();
        let samples = wave.render(&[0], cycles_for_samples(4410)).to_vec(); // 100ms

        assert_eq!(samples[0], SND_AMPLITUDE as i16, "full-amplitude edge");
        // Monotonic decay in magnitude...
        for pair in samples.windows(2) {
            assert!(pair[1].abs() <= pair[0].abs(), "decay must be monotonic");
        }
        // ...reaching true zero within 50 ms.
        assert!(
            samples[2205..].iter().all(|&s| s == 0),
            "level must decay to silence within 50ms"
        );
    }

    #[test]
    fn toggle_burst_returns_to_silence() {
        // The power-on beep: toggles at ~1 kHz, then nothing. The C
        // implementation held ±8000 forever afterwards. Frames are 50ms
        // (2205 samples), within the buffer cap.
        let mut wave = Wave::new();
        let toggles: Vec<u64> = (0..64).map(|i| i * 512).collect(); // ends ~31.5ms in
        let first = wave.render(&toggles, cycles_for_samples(2205)).to_vec();
        assert_eq!(first.len(), 2205);
        assert!(
            first.iter().any(|&s| s.abs() > 4000),
            "the beep itself must be audible"
        );

        let second = wave.render(&[], 2 * cycles_for_samples(2205)).to_vec();
        assert_eq!(second.len(), 2205);
        assert!(
            second[1102..].iter().all(|&s| s == 0),
            "silence after the beep, not a held rail"
        );
    }

    #[test]
    fn sample_count_matches_cycle_span() {
        let mut wave = Wave::new();
        // One 40fps frame: 1023000/40 cycles -> 44100/40 samples.
        let samples = wave.render(&[], 1_023_000 / 40);
        assert_eq!(samples.len(), 44100 / 40);
        // The next frame is relative to the new start cycle.
        let samples = wave.render(&[], 2 * (1_023_000 / 40));
        assert_eq!(samples.len(), 44100 / 40);
    }

    #[test]
    fn square_wave_alternates_rails() {
        let mut wave = Wave::new();
        // Toggle every ~46 cycles* 512 apart = 1kHz-ish; check both rails
        // appear with full amplitude while the tone plays.
        let toggles: Vec<u64> = (0..64).map(|i| i * 512).collect();
        let samples = wave.render(&toggles, 64 * 512).to_vec();
        assert!(samples.iter().any(|&s| s > 7000));
        assert!(samples.iter().any(|&s| s < -7000));
    }
}
