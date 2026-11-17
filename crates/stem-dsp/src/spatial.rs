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
        let sr = sample_rate.max(1.0);
        let max_samples = ((max_delay_ms.max(1.0) * 0.001) * sr).ceil() as usize + 4;
        Self {
            buffer: vec![0.0; max_samples.max(2)],
            write_pos: 0,
            sample_rate: sr,
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
        let buf_len = self.buffer.len();
        let max_delay = (buf_len.saturating_sub(2)) as f32;
        let delay_samples = ((self.time_ms * 0.001) * self.sample_rate).clamp(0.0, max_delay);
        let delay_int = delay_samples.floor() as usize;
        let frac = delay_samples - delay_int as f32;

        let idx0 = (self.write_pos + buf_len - delay_int) % buf_len;
        let idx1 = (self.write_pos + buf_len - (delay_int + 1)) % buf_len;
        let delayed_sample = self.buffer[idx0] * (1.0 - frac) + self.buffer[idx1] * frac;

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
            buffer: vec![0.0; size.max(1)],
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
            buffer: vec![0.0; size.max(1)],
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
        Self::new_with_spread(sample_rate, 0)
    }

    /// Creates a reverberator with an optional stereo decorrelation spread (in 44.1 kHz reference samples).
    pub fn new_with_spread(sample_rate: f32, spread_samples: usize) -> Self {
        let scale = sample_rate.max(1.0) / 44100.0;
        let comb_tunings = [1116, 1188, 1277, 1356, 1422, 1491, 1557, 1617];
        let allpass_tunings = [556, 441, 341, 225];

        let combs = comb_tunings
            .iter()
            .map(|&t| CombFilter::new(((t + spread_samples) as f32 * scale).round() as usize))
            .collect();
        let allpasses = allpass_tunings
            .iter()
            .map(|&t| AllPassFilter::new(((t + spread_samples) as f32 * scale).round() as usize))
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

/// Stereo width & Mid/Side processor (`width` in `[0.0, 2.0]`, gains in dB).
#[derive(Debug, Clone, Copy)]
pub struct StereoProcessor {
    pub width: f32,
    pub mid_gain_db: f32,
    pub side_gain_db: f32,
}

impl Default for StereoProcessor {
    fn default() -> Self {
        Self {
            width: 1.0,
            mid_gain_db: 0.0,
            side_gain_db: 0.0,
        }
    }
}

impl StereoProcessor {
    pub fn new(width: f32, mid_gain_db: f32, side_gain_db: f32) -> Self {
        Self {
            width: width.clamp(0.0, 2.0),
            mid_gain_db,
            side_gain_db,
        }
    }

    /// Processes a stereo sample pair `(left, right)` using Mid/Side matrixing.
    #[inline]
    pub fn process_stereo(&self, left: f32, right: f32) -> (f32, f32) {
        let mid = 0.5 * (left + right);
        let side = 0.5 * (left - right);
        let mid_lin = 10.0f32.powf(self.mid_gain_db / 20.0);
        let side_lin = 10.0f32.powf(self.side_gain_db / 20.0) * self.width.clamp(0.0, 2.0);
        let m = mid * mid_lin;
        let s = side * side_lin;
        (m + s, m - s)
    }
}

/// 5.1 surround panner & ITU-R BS.775 downmix helper.
/// Channel order: `[L, R, C, LFE, Ls, Rs]`.
#[derive(Debug, Clone, Copy)]
pub struct Surround51Pan {
    /// Front/rear bias in `[0.0, 1.0]` (0.0 = front, 1.0 = surround rear).
    pub rear_mix: f32,
    /// Center channel feed `[0.0, 1.0]`.
    pub center_mix: f32,
    /// LFE subwoofer feed `[0.0, 1.0]`.
    pub lfe_mix: f32,
}

impl Default for Surround51Pan {
    fn default() -> Self {
        Self {
            rear_mix: 0.0,
            center_mix: 0.5,
            lfe_mix: 0.25,
        }
    }
}

impl Surround51Pan {
    /// Encodes a stereo `(left, right)` frame into 5.1 `[L, R, C, LFE, Ls, Rs]`.
    #[inline]
    pub fn encode_stereo(&self, left: f32, right: f32) -> [f32; 6] {
        let rear = self.rear_mix.clamp(0.0, 1.0);
        let front = (1.0 - rear).sqrt();
        let surr = rear.sqrt();
        let mono = 0.5 * (left + right);
        let c = mono * self.center_mix.clamp(0.0, 1.0) * 0.707_106_77;
        let lfe = mono * self.lfe_mix.clamp(0.0, 1.0);
        [
            left * front,
            right * front,
            c,
            lfe,
            left * surr,
            right * surr,
        ]
    }

    /// ITU-R BS.775 5.1-to-stereo downmix (`L' = L + 0.707*C + 0.707*Ls`).
    #[inline]
    pub fn downmix_to_stereo(ch: [f32; 6]) -> (f32, f32) {
        const INV_SQRT2: f32 = std::f32::consts::FRAC_1_SQRT_2;
        let l = ch[0] + INV_SQRT2 * ch[2] + INV_SQRT2 * ch[4];
        let r = ch[1] + INV_SQRT2 * ch[2] + INV_SQRT2 * ch[5];
        (l, r)
    }
}
