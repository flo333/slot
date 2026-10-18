const TAPS: usize = 48;
const HALF: usize = TAPS / 2;
// Nearest phase instead of blending two: 512 steps put the timing error near -56 dB at 15 kHz,
// under the GBA's own ~9-bit output, at half the work and none of the per-tap blend.
const PHASES: usize = 512;
const CUTOFF: f64 = 22_000.0 / 24_000.0;
const BETA: f64 = 7.0;
// Independent sums a tap row is split over, so the adds overlap and vectorise.
const LANES: usize = 8;
// The read position is fixed point, frames above FRAC_BITS and the fraction below, so stepping and
// picking a phase stay integer work.
const FRAC_BITS: u32 = 32;
const PHASE_SHIFT: u32 = FRAC_BITS - PHASES.trailing_zeros();

pub struct Resampler {
    step: f64,
    scaled: u64,
    pos: u64,
    kernel: Vec<[f32; TAPS]>,
    // Left and right apart, so each output frame is two contiguous dot products.
    left: Vec<f32>,
    right: Vec<f32>,
}

impl Resampler {
    pub fn new(src_hz: f64, dst_hz: f64) -> Self {
        let usable = |hz: f64| hz.is_finite() && hz > 0.0;
        let step = if usable(src_hz) && usable(dst_hz) {
            src_hz / dst_hz
        } else {
            1.0
        };
        Resampler {
            step,
            scaled: fixed(step),
            pos: (HALF as u64) << FRAC_BITS,
            kernel: kernel(CUTOFF * (1.0 / step).min(1.0)),
            left: vec![0.0; TAPS],
            right: vec![0.0; TAPS],
        }
    }

    pub fn set_ratio(&mut self, ratio: f64) {
        if ratio > 0.0 {
            self.scaled = fixed(self.step / ratio);
        }
    }

    pub fn process(&mut self, src: &[i16], out: &mut Vec<i16>) {
        out.clear();
        if src.len() < 2 {
            return;
        }
        let (frames, _) = src.as_chunks::<2>();
        self.left.extend(frames.iter().map(|f| f32::from(f[0])));
        self.right.extend(frames.iter().map(|f| f32::from(f[1])));
        let end = ((self.left.len() - HALF) as u64) << FRAC_BITS;
        out.reserve((end.saturating_sub(self.pos) / self.scaled) as usize * 2 + 2);
        while self.pos < end {
            let i = (self.pos >> FRAC_BITS) as usize;
            let frac = self.pos & ((1 << FRAC_BITS) - 1);
            let phase = (frac + (1 << (PHASE_SHIFT - 1))) >> PHASE_SHIFT;
            let taps = &self.kernel[phase as usize];
            let start = i + 1 - HALF;
            let (l, r) = dot(
                taps,
                self.left[start..start + TAPS].try_into().unwrap(),
                self.right[start..start + TAPS].try_into().unwrap(),
            );
            out.push(l.round().clamp(-32768.0, 32767.0) as i16);
            out.push(r.round().clamp(-32768.0, 32767.0) as i16);
            self.pos += self.scaled;
        }
        let drop = (self.pos >> FRAC_BITS) as usize + 1 - HALF;
        self.left.drain(..drop);
        self.right.drain(..drop);
        self.pos -= (drop as u64) << FRAC_BITS;
    }
}

fn fixed(step: f64) -> u64 {
    ((step * (1u64 << FRAC_BITS) as f64).round() as u64).max(1)
}

fn dot(taps: &[f32; TAPS], left: &[f32; TAPS], right: &[f32; TAPS]) -> (f32, f32) {
    let (mut l, mut r) = ([0.0f32; LANES], [0.0f32; LANES]);
    let (t, a, b) = (
        taps.as_chunks::<LANES>().0,
        left.as_chunks::<LANES>().0,
        right.as_chunks::<LANES>().0,
    );
    for ((t, a), b) in t.iter().zip(a).zip(b) {
        for k in 0..LANES {
            l[k] += t[k] * a[k];
            r[k] += t[k] * b[k];
        }
    }
    (reduce(l), reduce(r))
}

fn reduce(sums: [f32; LANES]) -> f32 {
    let quads = [0, 1, 2, 3].map(|k| sums[k] + sums[k + 4]);
    (quads[0] + quads[2]) + (quads[1] + quads[3])
}

fn kernel(cutoff: f64) -> Vec<[f32; TAPS]> {
    (0..=PHASES)
        .map(|phase| {
            let frac = phase as f64 / PHASES as f64;
            let mut taps = [0.0f64; TAPS];
            for (t, tap) in taps.iter_mut().enumerate() {
                let x = t as f64 + 1.0 - HALF as f64 - frac;
                let window = bessel_i0(BETA * (1.0 - (x / HALF as f64).powi(2)).max(0.0).sqrt())
                    / bessel_i0(BETA);
                *tap = cutoff * sinc(cutoff * x) * window;
            }
            let sum: f64 = taps.iter().sum();
            taps.map(|tap| (tap / sum) as f32)
        })
        .collect()
}

fn sinc(x: f64) -> f64 {
    if x.abs() < 1e-12 {
        1.0
    } else {
        let px = std::f64::consts::PI * x;
        px.sin() / px
    }
}

fn bessel_i0(x: f64) -> f64 {
    let mut sum = 1.0;
    let mut term = 1.0;
    for k in 1..32 {
        term *= (x / (2.0 * k as f64)).powi(2);
        sum += term;
    }
    sum
}
