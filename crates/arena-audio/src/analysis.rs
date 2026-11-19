//! Measurements of a rendered buffer, for the tests and the preview, since
//! nobody can listen in CI: peak, RMS, DC, where the energy sits in the
//! spectrum, and how long it takes to ring out.

use crate::dsp::SR;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Report {
    pub seconds: f32,
    pub peak: f32,
    pub rms: f32,
    /// Mean sample value.
    pub dc: f32,
    /// Spectral centroid, Hz (energy-weighted mean frequency).
    pub centroid_hz: f32,
    /// Loudest bin of the averaged spectrum, Hz.
    pub dominant_hz: f32,
    /// Share of energy below 200 Hz, 200 Hz to 2 kHz, above 2 kHz.
    pub bands: [f32; 3],
    /// Share of energy above 8 kHz.
    pub air: f32,
    /// Seconds until the level stays 40 dB under the peak.
    pub ring_out: f32,
    /// Largest sample-to-sample step: a click detector.
    pub max_step: f32,
}

#[must_use]
pub fn db(x: f32) -> f32 {
    20.0 * x.max(1e-9).log10()
}

#[must_use]
pub fn analyse(x: &[f32]) -> Report {
    let n = x.len().max(1);
    let peak = x.iter().fold(0.0f32, |m, s| m.max(s.abs()));
    let rms = (x.iter().map(|s| f64::from(*s) * f64::from(*s)).sum::<f64>() / n as f64).sqrt() as f32;
    let dc = (x.iter().map(|s| f64::from(*s)).sum::<f64>() / n as f64) as f32;
    let max_step = x.windows(2).fold(0.0f32, |m, w| m.max((w[1] - w[0]).abs()));
    let floor = peak * 0.01;
    let ring_out = x.iter().rposition(|s| s.abs() > floor).map_or(0.0, |i| i as f32 / SR);

    // Averaged power spectrum over Hann-windowed frames.
    const N: usize = 2048;
    let mut power = vec![0.0f64; N / 2];
    let mut start = 0;
    let hann: Vec<f32> = (0..N).map(|i| 0.5 - 0.5 * (std::f32::consts::TAU * i as f32 / N as f32).cos()).collect();
    while start < x.len() {
        let mut re = vec![0.0f32; N];
        let mut im = vec![0.0f32; N];
        for i in 0..N {
            re[i] = x.get(start + i).copied().unwrap_or(0.0) * hann[i];
        }
        fft(&mut re, &mut im);
        for k in 0..N / 2 {
            power[k] += f64::from(re[k] * re[k] + im[k] * im[k]);
        }
        start += N / 2;
    }
    let bin = SR / N as f32;
    let total: f64 = power.iter().skip(1).sum::<f64>().max(1e-30);
    let mut centroid = 0.0f64;
    let mut bands = [0.0f64; 3];
    let mut air = 0.0f64;
    let mut dominant = (0usize, 0.0f64);
    for (k, p) in power.iter().enumerate().skip(1) {
        let f = k as f32 * bin;
        centroid += f64::from(f) * p;
        let band = if f < 200.0 { 0 } else if f < 2000.0 { 1 } else { 2 };
        bands[band] += p;
        if f > 8000.0 {
            air += p;
        }
        if *p > dominant.1 {
            dominant = (k, *p);
        }
    }
    Report {
        seconds: x.len() as f32 / SR,
        peak,
        rms,
        dc,
        centroid_hz: (centroid / total) as f32,
        dominant_hz: dominant.0 as f32 * bin,
        bands: [(bands[0] / total) as f32, (bands[1] / total) as f32, (bands[2] / total) as f32],
        air: (air / total) as f32,
        ring_out,
        max_step,
    }
}

/// In-place radix-2 FFT; `re.len()` must be a power of two.
pub fn fft(re: &mut [f32], im: &mut [f32]) {
    let n = re.len();
    debug_assert!(n.is_power_of_two() && im.len() == n);
    let mut j = 0;
    for i in 1..n {
        let mut bit = n >> 1;
        while j & bit != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j |= bit;
        if i < j {
            re.swap(i, j);
            im.swap(i, j);
        }
    }
    let mut len = 2;
    while len <= n {
        let ang = -std::f32::consts::TAU / len as f32;
        let (wr, wi) = (ang.cos(), ang.sin());
        for start in (0..n).step_by(len) {
            let (mut cr, mut ci) = (1.0f32, 0.0f32);
            for k in 0..len / 2 {
                let a = start + k;
                let b = a + len / 2;
                let tr = re[b] * cr - im[b] * ci;
                let ti = re[b] * ci + im[b] * cr;
                re[b] = re[a] - tr;
                im[b] = im[a] - ti;
                re[a] += tr;
                im[a] += ti;
                let nr = cr * wr - ci * wi;
                ci = cr * wi + ci * wr;
                cr = nr;
            }
        }
        len <<= 1;
    }
}
