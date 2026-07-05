//! Speaker sound, port of `snd.c` (#188): the core's cycle-stamped `$C030`
//! toggles become a square wave in a sample buffer that is queued to SDL
//! once per frame.

use sdl2::AudioSubsystem;
use sdl2::audio::{AudioQueue, AudioSpecDesired};

pub const SND_SAMPLE_RATE: u32 = 44100;
const SND_BUFFER_SIZE: usize = 4096;
const SND_AMPLITUDE: i16 = 8000;
const SND_CPU_FREQUENCY: u64 = 1_023_000;

pub struct Snd {
    device: AudioQueue<i16>,
    speaker_state: bool,
    buffer: Vec<i16>,
    frame_start_cycle: u64,
}

impl Snd {
    /// Port of `ewm_snd_init`.
    pub fn new(audio: &AudioSubsystem) -> Result<Snd, String> {
        let desired = AudioSpecDesired {
            freq: Some(SND_SAMPLE_RATE as i32),
            channels: Some(1),
            samples: Some(512),
        };
        let device = audio.open_queue::<i16, _>(None, &desired)?;
        device.resume();
        Ok(Snd {
            device,
            speaker_state: false,
            buffer: Vec::with_capacity(SND_BUFFER_SIZE),
            frame_start_cycle: 0,
        })
    }

    /// Port of `ewm_snd_toggle_speaker`, replayed from the core's stamped
    /// toggle events.
    fn toggle_speaker(&mut self, cpu_counter: u64) {
        let cycles_since_frame_start = cpu_counter.saturating_sub(self.frame_start_cycle);
        let sample_index =
            (cycles_since_frame_start * SND_SAMPLE_RATE as u64 / SND_CPU_FREQUENCY) as usize;

        let amplitude = if self.speaker_state {
            SND_AMPLITUDE
        } else {
            -SND_AMPLITUDE
        };
        while self.buffer.len() < sample_index && self.buffer.len() < SND_BUFFER_SIZE {
            self.buffer.push(amplitude);
        }

        self.speaker_state = !self.speaker_state;
    }

    /// Port of `ewm_snd_update`: fill the rest of the frame at the current
    /// speaker level and queue it, unless SDL already has 100ms buffered.
    pub fn update(&mut self, toggles: &[u64], cpu_counter: u64) {
        for &cycle in toggles {
            self.toggle_speaker(cycle);
        }

        let cycles_this_frame = cpu_counter.saturating_sub(self.frame_start_cycle);
        let mut samples_needed =
            (cycles_this_frame * SND_SAMPLE_RATE as u64 / SND_CPU_FREQUENCY) as usize;
        samples_needed = samples_needed.min(SND_BUFFER_SIZE);

        let amplitude = if self.speaker_state {
            SND_AMPLITUDE
        } else {
            -SND_AMPLITUDE
        };
        while self.buffer.len() < samples_needed {
            self.buffer.push(amplitude);
        }

        if !self.buffer.is_empty() {
            let queued = self.device.size();
            if queued < SND_SAMPLE_RATE * 2 / 10 {
                let _ = self.device.queue_audio(&self.buffer);
            }
        }

        self.buffer.clear();
        self.frame_start_cycle = cpu_counter;
    }
}
