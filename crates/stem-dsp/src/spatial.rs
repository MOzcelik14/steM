//! Spatial audio effects: Delay line and Algorithmic Reverb

pub struct Delay {
    buffer: Vec<f32>,
    write_pos: usize,
    sample_rate: f32,
    time_ms: f32,
    feedback: f32,
    mix: f32,
}

impl Delay {
    pub fn new(sample_rate: f32, max_delay_ms: f32) -> Self {
        let max_samples = ((max_delay_ms * 0.001) * sample_rate).ceil() as usize + 2;
        Self {
            buffer: vec![0.0; max_samples],
            write_pos: 0,
            sample_rate,
            time_ms: 250.0,
            feedback: 0.35,
            mix: 0.3,
        }
    }

    pub fn set_params(&mut self, time_ms: f32, feedback: f32, mix: f32) {
        self.time_ms = time_ms.clamp(1.0, 2000.0);
        self.feedback = feedback.clamp(0.0, 0.95);
        self.mix = mix.clamp(0.0, 1.0);
    }

    pub fn reset(&mut self) {
        self.buffer.fill(0.0);
        self.write_pos = 0;
    }

    #[inline]
    pub fn process_sample(&mut self, input: f32) -> f32 {
        let delay_samples = ((self.time_ms * 0.001) * self.sample_rate).round() as usize;
        let buf_len = self.buffer.len();
        let read_pos = (self.write_pos + buf_len - delay_samples.min(buf_len - 1)) % buf_len;

        let delayed_sample = self.buffer[read_pos];
        self.buffer[self.write_pos] = input + delayed_sample * self.feedback;
        self.write_pos = (self.write_pos + 1) % buf_len;

        input * (1.0 - self.mix) + delayed_sample * self.mix
    }

    pub fn process_buffer(&mut self, buffer: &mut [f32]) {
        for s in buffer.iter_mut() {
            *s = self.process_sample(*s);
        }
    }
}

/// Comb filter used in algorithmic reverberation.
struct CombFilter {
    buffer: Vec<f32>,
    index: usize,
    feedback: f32,
    damp: f32,
    filter_store: f32,
}

impl CombFilter {
    fn new(size: usize) -> Self {
        Self {
            buffer: vec![0.0; size],
            index: 0,
            feedback: 0.8,
            damp: 0.2,
            filter_store: 0.0,
        }
    }

    #[inline]
    fn process(&mut self, input: f32) -> f32 {
        let output = self.buffer[self.index];
        self.filter_store = output * (1.0 - self.damp) + self.filter_store * self.damp;
        self.buffer[self.index] = input + self.filter_store * self.feedback;
        self.index = (self.index + 1) % self.buffer.len();
        output
    }

    fn reset(&mut self) {
        self.buffer.fill(0.0);
        self.index = 0;
        self.filter_store = 0.0;
    }
}

/// Allpass filter used for diffusing reverb reflections.
struct AllPassFilter {
    buffer: Vec<f32>,
    index: usize,
    feedback: f32,
}

impl AllPassFilter {
    fn new(size: usize) -> Self {
        Self {
            buffer: vec![0.0; size],
            index: 0,
            feedback: 0.5,
        }
    }

    fn reset(&mut self) {
        self.buffer.fill(0.0);
        self.index = 0;
    }

    #[inline]
    fn process(&mut self, input: f32) -> f32 {
        let buf_out = self.buffer[self.index];
        let output = -input + buf_out;
        self.buffer[self.index] = input + buf_out * self.feedback;
        self.index = (self.index + 1) % self.buffer.len();
        output
    }
}

/// Schroeder / Freeverb style algorithmic stereo reverberator.
pub struct Reverb {
    combs: Vec<CombFilter>,
    allpasses: Vec<AllPassFilter>,
    pub room_size: f32,
    pub damping: f32,
    pub mix: f32,
}

impl Reverb {
    pub fn new(sample_rate: f32) -> Self {
        let scale = sample_rate / 44100.0;
        let comb_tunings = [1116, 1188, 1277, 1356, 1422, 1491, 1557, 1617];
        let allpass_tunings = [556, 441, 341, 225];

        let combs = comb_tunings
            .iter()
            .map(|&t| CombFilter::new((t as f32 * scale).round() as usize))
            .collect();
        let allpasses = allpass_tunings
            .iter()
            .map(|&t| AllPassFilter::new((t as f32 * scale).round() as usize))
            .collect();

        Self {
            combs,
            allpasses,
            room_size: 0.7,
            damping: 0.25,
            mix: 0.2,
        }
    }

    pub fn set_params(&mut self, room_size: f32, damping: f32, mix: f32) {
        self.room_size = room_size.clamp(0.0, 0.98);
        self.damping = damping.clamp(0.0, 1.0);
        self.mix = mix.clamp(0.0, 1.0);

        for comb in self.combs.iter_mut() {
            comb.feedback = self.room_size;
            comb.damp = self.damping;
        }
    }

    pub fn reset(&mut self) {
        for comb in &mut self.combs {
            comb.reset();
        }
        for allpass in &mut self.allpasses {
            allpass.reset();
        }
    }

    #[inline]
    pub fn process_sample(&mut self, input: f32) -> f32 {
        let mut out = 0.0;
        for comb in self.combs.iter_mut() {
            out += comb.process(input);
        }
        for allpass in self.allpasses.iter_mut() {
            out = allpass.process(out);
        }
        input * (1.0 - self.mix) + out * (self.mix * 0.2)
    }

    pub fn process_buffer(&mut self, buffer: &mut [f32]) {
        for s in buffer.iter_mut() {
            *s = self.process_sample(*s);
        }
    }
}
