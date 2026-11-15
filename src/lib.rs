// ============================================================================
// OMORE PUNCH - STEREO DRUM BUS PARALLEL COMPRESSOR & CLIPPER
// Monolithic Architecture (src/lib.rs) - 100% Pure ASCII
// Compatible with Reaper on Linux (Debian) and Windows 11 (VST3 & CLAP)
// ============================================================================

// Suppress future-incompatibility lint triggered inside nih_plug::nih_export_vst3!
// by transitive macro vst3_com::vtable! (Rust issue #79813).
#![allow(semicolon_in_expressions_from_non_local_macros)]

use nih_plug::prelude::*;
use nih_plug_egui::{create_egui_editor, egui, EguiState};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;

const SAFETY_CLAMP_LIMIT: f32 = 4.0;
const MIN_DETECTOR_PEAK: f32 = 1.0e-6;
const MIN_FLOOR_DB: f32 = -60.0;
const MAX_GR_METER_DB: f32 = 24.0;

// ----------------------------------------------------------------------------
// Helper Functions for Lock-Free Atomic f32 Storage
// ----------------------------------------------------------------------------

#[inline(always)]
fn store_f32_relaxed(atom: &AtomicU32, val: f32) {
    atom.store(val.to_bits(), Ordering::Relaxed);
}

#[inline(always)]
fn load_f32_relaxed(atom: &AtomicU32) -> f32 {
    f32::from_bits(atom.load(Ordering::Relaxed))
}

#[inline(always)]
pub fn sanitize_sample(x: f32) -> f32 {
    if x.is_finite() {
        x.clamp(-SAFETY_CLAMP_LIMIT, SAFETY_CLAMP_LIMIT)
    } else {
        0.0
    }
}

#[inline(always)]
pub fn db_to_lin(db: f32) -> f32 {
    10.0_f32.powf(db * 0.05)
}

#[inline(always)]
pub fn lin_to_db(lin: f32) -> f32 {
    20.0 * lin.max(MIN_DETECTOR_PEAK).log10()
}

// ----------------------------------------------------------------------------
// Clipper Mode Enumeration
// ----------------------------------------------------------------------------

#[derive(Enum, PartialEq, Eq, Clone, Copy, Debug)]
pub enum ClipperMode {
    #[id = "soft_clip"]
    #[name = "Soft Clip (Omore Cubic/Tanh)"]
    SoftClip,

    #[id = "hard_clip"]
    #[name = "Hard Clip"]
    HardClip,

    #[id = "bypass_clip"]
    #[name = "Bypass Clipper"]
    Bypass,
}

// ----------------------------------------------------------------------------
// Pure Real-Time Stereo DSP Engine
// ----------------------------------------------------------------------------

#[derive(Clone, Copy, Debug)]
struct BiquadCoeffs {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
    bypass: bool,
}

impl Default for BiquadCoeffs {
    fn default() -> Self {
        Self {
            b0: 1.0,
            b1: 0.0,
            b2: 0.0,
            a1: 0.0,
            a2: 0.0,
            bypass: true,
        }
    }
}

#[inline(always)]
fn sanitize_biquad_state(s: f32) -> f32 {
    if s.is_finite() && s.abs() > 1.0e-20 {
        s
    } else {
        0.0
    }
}

/// Stereo Direct Form II Transposed biquad with per-sample NaN/Inf and denormal protection.
#[derive(Clone, Debug, Default)]
struct StereoBiquad {
    coeffs: BiquadCoeffs,
    z1_l: f32,
    z2_l: f32,
    z1_r: f32,
    z2_r: f32,
}

impl StereoBiquad {
    #[inline(always)]
    fn reset(&mut self) {
        self.z1_l = 0.0;
        self.z2_l = 0.0;
        self.z1_r = 0.0;
        self.z2_r = 0.0;
    }

    #[inline(always)]
    fn process_channel(x: f32, coeffs: &BiquadCoeffs, z1: &mut f32, z2: &mut f32) -> f32 {
        let clean_x = if x.is_finite() { x } else { 0.0 };
        let raw_y = coeffs.b0 * clean_x + *z1;
        let y = if raw_y.is_finite() { raw_y } else { 0.0 };
        let next_z1 = coeffs.b1 * clean_x - coeffs.a1 * y + *z2;
        let next_z2 = coeffs.b2 * clean_x - coeffs.a2 * y;
        *z1 = sanitize_biquad_state(next_z1);
        *z2 = sanitize_biquad_state(next_z2);
        y
    }

    #[inline(always)]
    fn process_sample(&mut self, in_l: f32, in_r: f32) -> (f32, f32) {
        if self.coeffs.bypass {
            self.reset();
            return (in_l, in_r);
        }

        let out_l = Self::process_channel(in_l, &self.coeffs, &mut self.z1_l, &mut self.z2_l);
        let out_r = Self::process_channel(in_r, &self.coeffs, &mut self.z1_r, &mut self.z2_r);
        (out_l, out_r)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct DspConfig {
    pub input_db: f32,
    pub threshold_db: f32,
    pub ratio: f32,
    pub attack_ms: f32,
    pub release_ms: f32,
    pub sc_hpf_hz: f32,
    pub auto_makeup: bool,
    pub makeup_db: f32,
    pub eq_low_cut_hz: f32,
    pub eq_mid_cut_db: f32,
    pub eq_high_shelf_db: f32,
    pub clipper_mode: ClipperMode,
    pub ceiling_db: f32,
    pub mix_pct: f32,
}

impl Default for DspConfig {
    fn default() -> Self {
        Self {
            input_db: 0.0,
            threshold_db: 0.0,
            ratio: 4.0,
            attack_ms: 3.0,
            release_ms: 80.0,
            sc_hpf_hz: 20.0,
            auto_makeup: true,
            makeup_db: 0.0,
            eq_low_cut_hz: 20.0,
            eq_mid_cut_db: 0.0,
            eq_high_shelf_db: 0.0,
            clipper_mode: ClipperMode::SoftClip,
            ceiling_db: 0.0,
            mix_pct: 100.0,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct SampleOutput {
    pub out_l: f32,
    pub out_r: f32,
    pub gr_db: f32,
    pub clipped: bool,
}

#[derive(Clone, Debug)]
pub struct StereoPunchDsp {
    sample_rate: f32,
    cached_attack_ms: f32,
    cached_release_ms: f32,
    attack_coeff: f32,
    release_coeff: f32,
    cached_sc_hpf_hz: f32,
    sc_hpf_coeff: f32,
    sc_prev_in_l: f32,
    sc_prev_in_r: f32,
    sc_prev_out_l: f32,
    sc_prev_out_r: f32,
    eq_low_cut: StereoBiquad,
    eq_mid_cut: StereoBiquad,
    eq_high_shelf: StereoBiquad,
    cached_eq_low_cut_hz: f32,
    cached_eq_mid_cut_db: f32,
    cached_eq_high_shelf_db: f32,
    pub env_gr_db: f32,
}

impl Default for StereoPunchDsp {
    fn default() -> Self {
        let mut dsp = Self {
            sample_rate: 48000.0,
            cached_attack_ms: -1.0,
            cached_release_ms: -1.0,
            attack_coeff: 0.0,
            release_coeff: 0.0,
            cached_sc_hpf_hz: -1.0,
            sc_hpf_coeff: 0.0,
            sc_prev_in_l: 0.0,
            sc_prev_in_r: 0.0,
            sc_prev_out_l: 0.0,
            sc_prev_out_r: 0.0,
            eq_low_cut: StereoBiquad::default(),
            eq_mid_cut: StereoBiquad::default(),
            eq_high_shelf: StereoBiquad::default(),
            cached_eq_low_cut_hz: -1.0,
            cached_eq_mid_cut_db: -999.0,
            cached_eq_high_shelf_db: -999.0,
            env_gr_db: 0.0,
        };
        dsp.update_ballistics(48000.0, 3.0, 80.0);
        dsp.update_sidechain_hpf(48000.0, 20.0);
        dsp.update_eq(48000.0, 20.0, 0.0, 0.0);
        dsp
    }
}

impl StereoPunchDsp {
    pub fn new(sample_rate: f32) -> Self {
        let mut dsp = Self::default();
        dsp.set_sample_rate(sample_rate);
        dsp
    }

    #[inline(always)]
    pub fn reset(&mut self) {
        self.env_gr_db = 0.0;
        self.sc_prev_in_l = 0.0;
        self.sc_prev_in_r = 0.0;
        self.sc_prev_out_l = 0.0;
        self.sc_prev_out_r = 0.0;
        self.eq_low_cut.reset();
        self.eq_mid_cut.reset();
        self.eq_high_shelf.reset();
    }

    #[inline(always)]
    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        let sr = sample_rate.max(8000.0);
        if (sr - self.sample_rate).abs() > 1e-3 {
            self.sample_rate = sr;
            let att = self.cached_attack_ms.max(0.1);
            let rel = self.cached_release_ms.max(10.0);
            let hpf = self.cached_sc_hpf_hz.clamp(20.0, 250.0);
            let eq_lc = self.cached_eq_low_cut_hz.clamp(20.0, 160.0);
            let eq_mc = if self.cached_eq_mid_cut_db < -100.0 {
                0.0
            } else {
                self.cached_eq_mid_cut_db.clamp(-12.0, 0.0)
            };
            let eq_hs = if self.cached_eq_high_shelf_db < -100.0 {
                0.0
            } else {
                self.cached_eq_high_shelf_db.clamp(-6.0, 12.0)
            };
            self.cached_attack_ms = -1.0;
            self.cached_release_ms = -1.0;
            self.cached_sc_hpf_hz = -1.0;
            self.cached_eq_low_cut_hz = -1.0;
            self.cached_eq_mid_cut_db = -999.0;
            self.cached_eq_high_shelf_db = -999.0;
            self.update_ballistics(sr, att, rel);
            self.update_sidechain_hpf(sr, hpf);
            self.update_eq(sr, eq_lc, eq_mc, eq_hs);
        }
    }

    #[inline(always)]
    pub fn update_ballistics(&mut self, sample_rate: f32, attack_ms: f32, release_ms: f32) {
        let sr = sample_rate.max(8000.0);
        let sr_changed = (sr - self.sample_rate).abs() > 1e-3;
        let att_changed = (attack_ms - self.cached_attack_ms).abs() > 1e-5;
        let rel_changed = (release_ms - self.cached_release_ms).abs() > 1e-5;

        if sr_changed {
            self.cached_sc_hpf_hz = -1.0;
            self.cached_eq_low_cut_hz = -1.0;
            self.cached_eq_mid_cut_db = -999.0;
            self.cached_eq_high_shelf_db = -999.0;
        }

        if sr_changed || att_changed {
            self.sample_rate = sr;
            self.cached_attack_ms = attack_ms;
            let attack_sec = (attack_ms * 0.001).max(0.0001);
            self.attack_coeff = (-1.0 / (attack_sec * sr)).exp();
        }

        if sr_changed || rel_changed {
            self.sample_rate = sr;
            self.cached_release_ms = release_ms;
            let release_sec = (release_ms * 0.001).max(0.001);
            self.release_coeff = (-1.0 / (release_sec * sr)).exp();
        }
    }

    #[inline(always)]
    pub fn update_sidechain_hpf(&mut self, sample_rate: f32, sc_hpf_hz: f32) {
        let sr = sample_rate.max(8000.0);
        let clamped_hz = sc_hpf_hz.clamp(20.0, 250.0);
        let sr_changed = (sr - self.sample_rate).abs() > 1e-3;
        let hpf_changed = (clamped_hz - self.cached_sc_hpf_hz).abs() > 1e-4;

        if sr_changed {
            self.cached_attack_ms = -1.0;
            self.cached_release_ms = -1.0;
            self.cached_eq_low_cut_hz = -1.0;
            self.cached_eq_mid_cut_db = -999.0;
            self.cached_eq_high_shelf_db = -999.0;
        }

        if sr_changed || hpf_changed {
            self.sample_rate = sr;
            self.cached_sc_hpf_hz = clamped_hz;
            let rc = 1.0 / (2.0 * std::f32::consts::PI * clamped_hz);
            let dt = 1.0 / sr;
            self.sc_hpf_coeff = rc / (rc + dt);
        }
    }

    #[inline(always)]
    fn normalize_biquad_coeffs(
        b0: f32,
        b1: f32,
        b2: f32,
        a0: f32,
        a1: f32,
        a2: f32,
    ) -> BiquadCoeffs {
        if !a0.is_finite() || a0.abs() < 1.0e-12 {
            return BiquadCoeffs::default();
        }
        let inv_a0 = 1.0 / a0;
        BiquadCoeffs {
            b0: b0 * inv_a0,
            b1: b1 * inv_a0,
            b2: b2 * inv_a0,
            a1: a1 * inv_a0,
            a2: a2 * inv_a0,
            bypass: false,
        }
    }

    /// 2nd-order Butterworth high-pass filter (12 dB/oct, Q = 1/sqrt(2)), bypassed <= 20.5 Hz.
    #[inline(always)]
    fn calc_low_cut_coeffs(sample_rate: f32, low_cut_hz: f32) -> BiquadCoeffs {
        if low_cut_hz <= 20.5 {
            return BiquadCoeffs::default();
        }
        let fc = low_cut_hz.min(0.45 * sample_rate);
        let w0 = 2.0 * std::f32::consts::PI * fc / sample_rate;
        let cos_w0 = w0.cos();
        let sin_w0 = w0.sin();
        let q = std::f32::consts::FRAC_1_SQRT_2;
        let alpha = sin_w0 / (2.0 * q);
        let one_plus_cos = 1.0 + cos_w0;
        Self::normalize_biquad_coeffs(
            0.5 * one_plus_cos,
            -one_plus_cos,
            0.5 * one_plus_cos,
            1.0 + alpha,
            -2.0 * cos_w0,
            1.0 - alpha,
        )
    }

    /// Peaking EQ cut centered at 500 Hz (Q = 1.0), bypassed when |gain| < 1e-3 dB.
    #[inline(always)]
    fn calc_mid_cut_coeffs(sample_rate: f32, mid_cut_db: f32) -> BiquadCoeffs {
        if mid_cut_db.abs() < 1.0e-3 {
            return BiquadCoeffs::default();
        }
        let fc = 500.0_f32.min(0.45 * sample_rate);
        let q = 1.0_f32;
        let a = 10.0_f32.powf(mid_cut_db * 0.025);
        let w0 = 2.0 * std::f32::consts::PI * fc / sample_rate;
        let cos_w0 = w0.cos();
        let sin_w0 = w0.sin();
        let alpha = sin_w0 / (2.0 * q);
        Self::normalize_biquad_coeffs(
            1.0 + alpha * a,
            -2.0 * cos_w0,
            1.0 - alpha * a,
            1.0 + alpha / a,
            -2.0 * cos_w0,
            1.0 - alpha / a,
        )
    }

    /// 2nd-order High-Shelf filter centered at 10 kHz (S = 1.0), bypassed when |gain| < 1e-3 dB.
    #[inline(always)]
    fn calc_high_shelf_coeffs(sample_rate: f32, high_shelf_db: f32) -> BiquadCoeffs {
        if high_shelf_db.abs() < 1.0e-3 {
            return BiquadCoeffs::default();
        }
        let fc = 10000.0_f32.min(0.45 * sample_rate);
        let s = 1.0_f32;
        let a = 10.0_f32.powf(high_shelf_db * 0.025);
        let w0 = 2.0 * std::f32::consts::PI * fc / sample_rate;
        let cos_w0 = w0.cos();
        let sin_w0 = w0.sin();
        let slope_term = ((a + 1.0 / a) * (1.0 / s - 1.0) + 2.0).max(0.0);
        let alpha = 0.5 * sin_w0 * slope_term.sqrt();
        let two_sqrt_a_alpha = 2.0 * a.sqrt() * alpha;
        let a_plus_1 = a + 1.0;
        let a_minus_1 = a - 1.0;
        Self::normalize_biquad_coeffs(
            a * (a_plus_1 + a_minus_1 * cos_w0 + two_sqrt_a_alpha),
            -2.0 * a * (a_minus_1 + a_plus_1 * cos_w0),
            a * (a_plus_1 + a_minus_1 * cos_w0 - two_sqrt_a_alpha),
            a_plus_1 - a_minus_1 * cos_w0 + two_sqrt_a_alpha,
            2.0 * (a_minus_1 - a_plus_1 * cos_w0),
            a_plus_1 - a_minus_1 * cos_w0 - two_sqrt_a_alpha,
        )
    }

    /// Updates the 3-band Drum Bus EQ coefficients only when sample rate or a parameter changes.
    #[inline(always)]
    pub fn update_eq(
        &mut self,
        sample_rate: f32,
        low_cut_hz: f32,
        mid_cut_db: f32,
        high_shelf_db: f32,
    ) {
        let sr = sample_rate.max(8000.0);
        let sr_changed = (sr - self.sample_rate).abs() > 1e-3;
        if sr_changed {
            self.sample_rate = sr;
            self.cached_attack_ms = -1.0;
            self.cached_release_ms = -1.0;
            self.cached_sc_hpf_hz = -1.0;
            self.cached_eq_low_cut_hz = -1.0;
            self.cached_eq_mid_cut_db = -999.0;
            self.cached_eq_high_shelf_db = -999.0;
        }

        let lc_hz = if low_cut_hz.is_finite() {
            low_cut_hz.clamp(20.0, 160.0)
        } else {
            20.0
        };
        let mc_db = if mid_cut_db.is_finite() {
            mid_cut_db.clamp(-12.0, 0.0)
        } else {
            0.0
        };
        let hs_db = if high_shelf_db.is_finite() {
            high_shelf_db.clamp(-6.0, 12.0)
        } else {
            0.0
        };

        if (lc_hz - self.cached_eq_low_cut_hz).abs() > 1.0e-4 {
            self.cached_eq_low_cut_hz = lc_hz;
            self.eq_low_cut.coeffs = Self::calc_low_cut_coeffs(sr, lc_hz);
        }

        if (mc_db - self.cached_eq_mid_cut_db).abs() > 1.0e-4 {
            self.cached_eq_mid_cut_db = mc_db;
            self.eq_mid_cut.coeffs = Self::calc_mid_cut_coeffs(sr, mc_db);
        }

        if (hs_db - self.cached_eq_high_shelf_db).abs() > 1.0e-4 {
            self.cached_eq_high_shelf_db = hs_db;
            self.eq_high_shelf.coeffs = Self::calc_high_shelf_coeffs(sr, hs_db);
        }
    }

    /// Soft knee saturator (unity gain below 50% of ceiling, C1 continuous tanh saturation
    /// above 50% of ceiling, followed by strict hard ceiling clamp).
    #[inline(always)]
    fn soft_clip_channel(sample: f32, ceiling_lin: f32) -> (f32, bool) {
        let ceil = ceiling_lin.max(1.0e-4);
        let is_over = sample.abs() > ceil;
        let x = sample / ceil;
        let abs_x = x.abs();

        let shaped_norm = if abs_x <= 0.5 {
            x
        } else {
            // C1 continuous transition at |x| = 0.5:
            // f(0.5) = 0.5, f'(0.5) = 0.5 * 2.0 * sech^2(0) = 1.0
            let excess = (abs_x - 0.5) * 2.0;
            let sat = 0.5 + 0.5 * excess.tanh();
            x.signum() * sat
        };

        let clamped = shaped_norm.clamp(-1.0, 1.0) * ceil;
        (clamped, is_over)
    }

    /// Processes a single stereo sample frame without allocations or locks.
    #[inline(always)]
    pub fn process_sample(&mut self, raw_l: f32, raw_r: f32, cfg: &DspConfig) -> SampleOutput {
        // 0. Sanitize inputs so NaN / Inf can never enter the detector or dry path
        let dry_l = sanitize_sample(raw_l);
        let dry_r = sanitize_sample(raw_r);

        // Update envelope, sidechain filter and EQ coefficients only if parameters changed
        self.update_ballistics(self.sample_rate, cfg.attack_ms, cfg.release_ms);
        self.update_sidechain_hpf(self.sample_rate, cfg.sc_hpf_hz);
        self.update_eq(
            self.sample_rate,
            cfg.eq_low_cut_hz,
            cfg.eq_mid_cut_db,
            cfg.eq_high_shelf_db,
        );

        // 1. Input Trim (Drive)
        let input_gain = db_to_lin(cfg.input_db);
        let driven_l = dry_l * input_gain;
        let driven_r = dry_r * input_gain;

        // 1b. Internal Sidechain High-Pass Filter (20 Hz to 250 Hz, 1st-order 6 dB/oct)
        let (det_l, det_r) = if cfg.sc_hpf_hz <= 20.5 {
            self.sc_prev_in_l = 0.0;
            self.sc_prev_in_r = 0.0;
            self.sc_prev_out_l = 0.0;
            self.sc_prev_out_r = 0.0;
            (driven_l, driven_r)
        } else {
            let raw_det_l = self.sc_hpf_coeff * (self.sc_prev_out_l + driven_l - self.sc_prev_in_l);
            let raw_det_r = self.sc_hpf_coeff * (self.sc_prev_out_r + driven_r - self.sc_prev_in_r);

            let clean_det_l = if raw_det_l.is_finite() && raw_det_l.abs() > 1.0e-20 {
                raw_det_l
            } else {
                0.0
            };
            let clean_det_r = if raw_det_r.is_finite() && raw_det_r.abs() > 1.0e-20 {
                raw_det_r
            } else {
                0.0
            };

            self.sc_prev_in_l = if driven_l.is_finite() { driven_l } else { 0.0 };
            self.sc_prev_in_r = if driven_r.is_finite() { driven_r } else { 0.0 };
            self.sc_prev_out_l = clean_det_l;
            self.sc_prev_out_r = clean_det_r;

            (clean_det_l, clean_det_r)
        };

        // 2. Stereo-Linked Peak Detector
        let peak_in = det_l.abs().max(det_r.abs()).max(MIN_DETECTOR_PEAK);
        let in_db = 20.0 * peak_in.log10();

        // 3. Feed-Forward Gain Reduction Calculation in dB
        let target_gr_db = if in_db > cfg.threshold_db && cfg.ratio > 1.01 {
            (in_db - cfg.threshold_db) * (1.0 - 1.0 / cfg.ratio)
        } else {
            0.0
        };

        // 4. Omore Punch Ballistics (3ms / 80ms default)
        if target_gr_db > self.env_gr_db {
            self.env_gr_db =
                self.attack_coeff * self.env_gr_db + (1.0 - self.attack_coeff) * target_gr_db;
        } else {
            self.env_gr_db =
                self.release_coeff * self.env_gr_db + (1.0 - self.release_coeff) * target_gr_db;
        }

        if !self.env_gr_db.is_finite() || self.env_gr_db < 0.0 {
            self.env_gr_db = 0.0;
        }

        // 5. Gain Compensation (Omore Auto-Makeup + Manual Makeup)
        let auto_makeup_db = if cfg.auto_makeup && cfg.ratio > 1.01 {
            (-cfg.threshold_db) * (1.0 - 1.0 / cfg.ratio) * 0.35
        } else {
            0.0
        };
        let total_makeup_db = auto_makeup_db + cfg.makeup_db;
        let comp_gain = 10.0_f32.powf((-self.env_gr_db + total_makeup_db) * 0.05);

        let comp_l = driven_l * comp_gain;
        let comp_r = driven_r * comp_gain;

        // 5b. Drum Bus EQ (Low Cut -> 500Hz Cut -> High Shelf, immediately pre-clipper)
        let (eq_l, eq_r) = self.eq_low_cut.process_sample(comp_l, comp_r);
        let (eq_l, eq_r) = self.eq_mid_cut.process_sample(eq_l, eq_r);
        let (eq_l, eq_r) = self.eq_high_shelf.process_sample(eq_l, eq_r);

        // 6. Final Clipper Stage (Soft Knee Saturation + Hard Ceiling)
        let ceiling_lin = db_to_lin(cfg.ceiling_db).max(1.0e-4);
        let (wet_l, wet_r, clipped) = match cfg.clipper_mode {
            ClipperMode::SoftClip => {
                let (cl_l, over_l) = Self::soft_clip_channel(eq_l, ceiling_lin);
                let (cl_r, over_r) = Self::soft_clip_channel(eq_r, ceiling_lin);
                (cl_l, cl_r, over_l || over_r)
            }
            ClipperMode::HardClip => {
                let over = eq_l.abs() > ceiling_lin || eq_r.abs() > ceiling_lin;
                (
                    eq_l.clamp(-ceiling_lin, ceiling_lin),
                    eq_r.clamp(-ceiling_lin, ceiling_lin),
                    over,
                )
            }
            ClipperMode::Bypass => (eq_l, eq_r, false),
        };

        // 7. Parallel Mix (Dry / Wet %)
        // Bit-exact dry path at 0.0% mix
        let wet = (cfg.mix_pct * 0.01).clamp(0.0, 1.0);
        let (mixed_l, mixed_r) = if wet <= 0.0 {
            (dry_l, dry_r)
        } else if wet >= 1.0 {
            (wet_l, wet_r)
        } else {
            let dry_weight = 1.0 - wet;
            (
                dry_l * dry_weight + wet_l * wet,
                dry_r * dry_weight + wet_r * wet,
            )
        };

        // Final Safety Sanitization & Clamp [-4.0, 4.0]
        let out_l = sanitize_sample(mixed_l);
        let out_r = sanitize_sample(mixed_r);

        SampleOutput {
            out_l,
            out_r,
            gr_db: self.env_gr_db,
            clipped,
        }
    }
}

// ----------------------------------------------------------------------------
// Shared Atomic Telemetry for GUI and Host Metering
// ----------------------------------------------------------------------------

#[derive(Default)]
pub struct MeterTelemetry {
    pub in_peak_l: AtomicU32,
    pub in_peak_r: AtomicU32,
    pub out_peak_l: AtomicU32,
    pub out_peak_r: AtomicU32,
    /// Instantaneous gain reduction in positive dB (e.g. 4.8 means -4.8 dB GR)
    pub current_gr_db: AtomicU32,
    /// Maximum peak gain reduction held in positive dB
    pub max_gr_db: AtomicU32,
    /// Clipper activity indicator latch
    pub clip_latched: AtomicBool,
}

impl MeterTelemetry {
    pub fn reset_peaks(&self) {
        store_f32_relaxed(&self.max_gr_db, 0.0);
        self.clip_latched.store(false, Ordering::Relaxed);
    }
}

// ----------------------------------------------------------------------------
// Plugin Parameters Definition
// ----------------------------------------------------------------------------

#[derive(Params)]
pub struct OmorePunchParams {
    #[persist = "editor-state"]
    pub editor_state: Arc<EguiState>,

    #[id = "input_db"]
    pub input_db: FloatParam,

    #[id = "threshold_db"]
    pub threshold_db: FloatParam,

    #[id = "ratio"]
    pub ratio: FloatParam,

    #[id = "attack_ms"]
    pub attack_ms: FloatParam,

    #[id = "release_ms"]
    pub release_ms: FloatParam,

    #[id = "sc_hpf_hz"]
    pub sc_hpf_hz: FloatParam,

    #[id = "auto_makeup"]
    pub auto_makeup: BoolParam,

    #[id = "makeup_db"]
    pub makeup_db: FloatParam,

    #[id = "eq_low_cut_hz"]
    pub eq_low_cut_hz: FloatParam,

    #[id = "eq_mid_cut_db"]
    pub eq_mid_cut_db: FloatParam,

    #[id = "eq_high_shelf_db"]
    pub eq_high_shelf_db: FloatParam,

    #[id = "clipper_mode"]
    pub clipper_mode: EnumParam<ClipperMode>,

    #[id = "ceiling_db"]
    pub ceiling_db: FloatParam,

    #[id = "mix_pct"]
    pub mix_pct: FloatParam,

    /// Readout parameter exposed to Reaper TCP/MCP so the user can also embed
    /// or monitor Gain Reduction (dB) as a track control / automation lane.
    #[id = "gr_meter_db"]
    pub gr_meter_db: FloatParam,
}

impl Default for OmorePunchParams {
    fn default() -> Self {
        Self {
            editor_state: EguiState::from_size(UI_WINDOW_WIDTH, UI_WINDOW_HEIGHT),

            input_db: FloatParam::new(
                "Input Trim",
                0.0,
                FloatRange::Linear {
                    min: -12.0,
                    max: 12.0,
                },
            )
            .with_smoother(SmoothingStyle::Linear(5.0))
            .with_unit(" dB")
            .with_value_to_string(formatters::v2s_f32_rounded(2)),

            threshold_db: FloatParam::new(
                "Threshold",
                0.0,
                FloatRange::Linear {
                    min: -40.0,
                    max: 0.0,
                },
            )
            .with_smoother(SmoothingStyle::Linear(5.0))
            .with_unit(" dB")
            .with_value_to_string(formatters::v2s_f32_rounded(2)),

            ratio: FloatParam::new(
                "Ratio",
                4.0,
                FloatRange::Skewed {
                    min: 1.0,
                    max: 20.0,
                    factor: FloatRange::skew_factor(-1.5),
                },
            )
            .with_smoother(SmoothingStyle::Linear(5.0))
            .with_unit(":1")
            .with_value_to_string(formatters::v2s_f32_rounded(2)),

            attack_ms: FloatParam::new(
                "Attack",
                3.0,
                FloatRange::Skewed {
                    min: 0.1,
                    max: 100.0,
                    factor: FloatRange::skew_factor(-1.5),
                },
            )
            .with_smoother(SmoothingStyle::Linear(5.0))
            .with_unit(" ms")
            .with_value_to_string(formatters::v2s_f32_rounded(2)),

            release_ms: FloatParam::new(
                "Release",
                80.0,
                FloatRange::Skewed {
                    min: 10.0,
                    max: 500.0,
                    factor: FloatRange::skew_factor(-1.2),
                },
            )
            .with_smoother(SmoothingStyle::Linear(5.0))
            .with_unit(" ms")
            .with_value_to_string(formatters::v2s_f32_rounded(1)),

            sc_hpf_hz: FloatParam::new(
                "SC HPF",
                20.0,
                FloatRange::Skewed {
                    min: 20.0,
                    max: 250.0,
                    factor: FloatRange::skew_factor(-1.0),
                },
            )
            .with_smoother(SmoothingStyle::Linear(5.0))
            .with_unit(" Hz")
            .with_value_to_string(formatters::v2s_f32_rounded(1)),

            auto_makeup: BoolParam::new("Auto Makeup", true),

            makeup_db: FloatParam::new(
                "Makeup Gain",
                0.0,
                FloatRange::Linear {
                    min: -12.0,
                    max: 24.0,
                },
            )
            .with_smoother(SmoothingStyle::Linear(5.0))
            .with_unit(" dB")
            .with_value_to_string(formatters::v2s_f32_rounded(2)),

            eq_low_cut_hz: FloatParam::new(
                "EQ Low Cut",
                20.0,
                FloatRange::Skewed {
                    min: 20.0,
                    max: 160.0,
                    factor: FloatRange::skew_factor(-1.0),
                },
            )
            .with_smoother(SmoothingStyle::Linear(5.0))
            .with_unit(" Hz")
            .with_value_to_string(formatters::v2s_f32_rounded(1)),

            eq_mid_cut_db: FloatParam::new(
                "EQ 500Hz Cut",
                0.0,
                FloatRange::Linear {
                    min: -12.0,
                    max: 0.0,
                },
            )
            .with_smoother(SmoothingStyle::Linear(5.0))
            .with_unit(" dB")
            .with_value_to_string(formatters::v2s_f32_rounded(2)),

            eq_high_shelf_db: FloatParam::new(
                "EQ High Shelf",
                0.0,
                FloatRange::Linear {
                    min: -6.0,
                    max: 12.0,
                },
            )
            .with_smoother(SmoothingStyle::Linear(5.0))
            .with_unit(" dB")
            .with_value_to_string(formatters::v2s_f32_rounded(2)),

            clipper_mode: EnumParam::new("Clipper Mode", ClipperMode::SoftClip),

            ceiling_db: FloatParam::new(
                "Clip Ceiling",
                0.0,
                FloatRange::Linear {
                    min: -6.0,
                    max: 0.0,
                },
            )
            .with_smoother(SmoothingStyle::Linear(5.0))
            .with_unit(" dB")
            .with_value_to_string(formatters::v2s_f32_rounded(2)),

            mix_pct: FloatParam::new(
                "Mix Dry/Wet",
                100.0,
                FloatRange::Linear {
                    min: 0.0,
                    max: 100.0,
                },
            )
            .with_smoother(SmoothingStyle::Linear(5.0))
            .with_unit(" %")
            .with_value_to_string(formatters::v2s_f32_rounded(1)),

            gr_meter_db: FloatParam::new(
                "Gain Reduction (dB)",
                0.0,
                FloatRange::Linear {
                    min: -MAX_GR_METER_DB,
                    max: 0.0,
                },
            )
            .with_unit(" dB")
            .with_value_to_string(formatters::v2s_f32_rounded(2))
            .non_automatable(),
        }
    }
}

// ----------------------------------------------------------------------------
// Main Plugin Struct
// ----------------------------------------------------------------------------

pub struct OmorePunch {
    params: Arc<OmorePunchParams>,
    dsp: StereoPunchDsp,
    telemetry: Arc<MeterTelemetry>,
}

impl Default for OmorePunch {
    fn default() -> Self {
        Self {
            params: Arc::new(OmorePunchParams::default()),
            dsp: StereoPunchDsp::default(),
            telemetry: Arc::new(MeterTelemetry::default()),
        }
    }
}

impl OmorePunch {
    /// Returns the instantaneous gain adjustment in dB (negative when attenuating,
    /// e.g. -4.8) for CLAP `clap.gain-adjustment-metering/0` and VST3 `IGainReductionInfo`.
    #[inline(always)]
    pub fn current_gain_adjustment_db(&self) -> f64 {
        -(load_f32_relaxed(&self.telemetry.current_gr_db) as f64)
    }
}

impl Plugin for OmorePunch {
    const NAME: &'static str = "Omore Punch";
    const VENDOR: &'static str = "Omore Audio";
    const URL: &'static str = "https://omore-audio.local";
    const EMAIL: &'static str = "contact@omore-audio.local";
    const VERSION: &'static str = env!("CARGO_PKG_VERSION");

    const AUDIO_IO_LAYOUTS: &'static [AudioIOLayout] = &[AudioIOLayout {
        main_input_channels: NonZeroU32::new(2),
        main_output_channels: NonZeroU32::new(2),
        aux_input_ports: &[],
        aux_output_ports: &[],
        names: PortNames::const_default(),
    }];

    const MIDI_INPUT: MidiConfig = MidiConfig::None;
    const MIDI_OUTPUT: MidiConfig = MidiConfig::None;
    const SAMPLE_ACCURATE_AUTOMATION: bool = true;

    type SysExMessage = ();
    type BackgroundTask = ();

    fn params(&self) -> Arc<dyn Params> {
        self.params.clone()
    }

    fn initialize(
        &mut self,
        _audio_io_layout: &AudioIOLayout,
        buffer_config: &BufferConfig,
        _context: &mut impl InitContext<Self>,
    ) -> bool {
        self.dsp.set_sample_rate(buffer_config.sample_rate);
        self.dsp.reset();
        self.telemetry.reset_peaks();
        true
    }

    fn reset(&mut self) {
        self.dsp.reset();
    }

    fn editor(&mut self, _async_executor: AsyncExecutor<Self>) -> Option<Box<dyn Editor>> {
        let params = self.params.clone();
        let telemetry = self.telemetry.clone();

        create_egui_editor(
            self.params.editor_state.clone(),
            (),
            |_, _| {},
            move |egui_ctx, setter, _state| {
                draw_omore_punch_ui(egui_ctx, setter, &params, &telemetry);
            },
        )
    }

    fn process(
        &mut self,
        buffer: &mut Buffer,
        _aux: &mut AuxiliaryBuffers,
        _context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        let mut block_in_l = 0.0_f32;
        let mut block_in_r = 0.0_f32;
        let mut block_out_l = 0.0_f32;
        let mut block_out_r = 0.0_f32;
        let mut block_max_gr = 0.0_f32;
        let mut block_clipped = false;

        let auto_makeup = self.params.auto_makeup.value();
        let clipper_mode = self.params.clipper_mode.value();

        for mut channel_samples in buffer.iter_samples() {
            if channel_samples.len() < 2 {
                continue;
            }

            let cfg = DspConfig {
                input_db: self.params.input_db.smoothed.next(),
                threshold_db: self.params.threshold_db.smoothed.next(),
                ratio: self.params.ratio.smoothed.next(),
                attack_ms: self.params.attack_ms.smoothed.next(),
                release_ms: self.params.release_ms.smoothed.next(),
                sc_hpf_hz: self.params.sc_hpf_hz.smoothed.next(),
                auto_makeup,
                makeup_db: self.params.makeup_db.smoothed.next(),
                eq_low_cut_hz: self.params.eq_low_cut_hz.smoothed.next(),
                eq_mid_cut_db: self.params.eq_mid_cut_db.smoothed.next(),
                eq_high_shelf_db: self.params.eq_high_shelf_db.smoothed.next(),
                clipper_mode,
                ceiling_db: self.params.ceiling_db.smoothed.next(),
                mix_pct: self.params.mix_pct.smoothed.next(),
            };

            let raw_l = *channel_samples.get_mut(0).unwrap();
            let raw_r = *channel_samples.get_mut(1).unwrap();

            let clean_in_l = sanitize_sample(raw_l);
            let clean_in_r = sanitize_sample(raw_r);

            block_in_l = block_in_l.max(clean_in_l.abs());
            block_in_r = block_in_r.max(clean_in_r.abs());

            let out = self.dsp.process_sample(clean_in_l, clean_in_r, &cfg);

            *channel_samples.get_mut(0).unwrap() = out.out_l;
            *channel_samples.get_mut(1).unwrap() = out.out_r;

            block_out_l = block_out_l.max(out.out_l.abs());
            block_out_r = block_out_r.max(out.out_r.abs());
            block_max_gr = block_max_gr.max(out.gr_db);
            if out.clipped {
                block_clipped = true;
            }
        }

        // Smooth decay for UI meters between audio blocks
        let prev_in_l = load_f32_relaxed(&self.telemetry.in_peak_l) * 0.88;
        let prev_in_r = load_f32_relaxed(&self.telemetry.in_peak_r) * 0.88;
        let prev_out_l = load_f32_relaxed(&self.telemetry.out_peak_l) * 0.88;
        let prev_out_r = load_f32_relaxed(&self.telemetry.out_peak_r) * 0.88;

        store_f32_relaxed(&self.telemetry.in_peak_l, block_in_l.max(prev_in_l));
        store_f32_relaxed(&self.telemetry.in_peak_r, block_in_r.max(prev_in_r));
        store_f32_relaxed(&self.telemetry.out_peak_l, block_out_l.max(prev_out_l));
        store_f32_relaxed(&self.telemetry.out_peak_r, block_out_r.max(prev_out_r));
        store_f32_relaxed(&self.telemetry.current_gr_db, self.dsp.env_gr_db);

        let held_max_gr = load_f32_relaxed(&self.telemetry.max_gr_db);
        if block_max_gr > held_max_gr {
            store_f32_relaxed(&self.telemetry.max_gr_db, block_max_gr);
        }

        if block_clipped {
            self.telemetry.clip_latched.store(true, Ordering::Relaxed);
        }

        ProcessStatus::Normal
    }
}

// ----------------------------------------------------------------------------
// EGUI User Interface (Dark Omore Style - 100% ASCII)
// ----------------------------------------------------------------------------

const UI_WINDOW_WIDTH: u32 = 820;
const UI_WINDOW_HEIGHT: u32 = 620;
const UI_LEFT_COL_WIDTH: f32 = 480.0;
const UI_RIGHT_COL_WIDTH: f32 = 310.0;
const UI_COL_GAP: f32 = 10.0;
const UI_KNOB_COLS_MAX: usize = 4;
const UI_KNOB_CELL_WIDTH: f32 = 110.0;
const UI_KNOB_CELL_HEIGHT: f32 = 94.0;
const UI_KNOB_DIAMETER: f32 = 44.0;
const UI_KNOB_GRID_SPACING_X: f32 = 8.0;
const UI_KNOB_GRID_SPACING_Y: f32 = 4.0;
const UI_KNOB_STEP_BTN_WIDTH: f32 = 22.0;
const UI_KNOB_RESET_BTN_WIDTH: f32 = 46.0;
const UI_KNOB_BTN_HEIGHT: f32 = 16.0;
const UI_METER_HEIGHT: f32 = 390.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SslKnobColor {
    GainDrive,
    Dynamics,
    Envelope,
    Filter,
    ClipperMix,
}

impl SslKnobColor {
    fn cap_color(self, active_or_hovered: bool) -> egui::Color32 {
        let (r, g, b) = match self {
            Self::GainDrive => (215_u8, 145_u8, 30_u8),
            Self::Dynamics => (150_u8, 158_u8, 170_u8),
            Self::Envelope => (55_u8, 155_u8, 95_u8),
            Self::Filter => (55_u8, 125_u8, 195_u8),
            Self::ClipperMix => (195_u8, 65_u8, 60_u8),
        };
        if active_or_hovered {
            egui::Color32::from_rgb(
                r.saturating_add(24),
                g.saturating_add(24),
                b.saturating_add(24),
            )
        } else {
            egui::Color32::from_rgb(r, g, b)
        }
    }
}

fn apply_omore_dark_theme(ctx: &egui::Context) {
    let mut visuals = egui::Visuals::dark();
    visuals.panel_fill = egui::Color32::from_rgb(18, 19, 24);
    visuals.window_fill = egui::Color32::from_rgb(24, 26, 32);
    visuals.extreme_bg_color = egui::Color32::from_rgb(10, 11, 14);
    visuals.widgets.noninteractive.bg_fill = egui::Color32::from_rgb(28, 30, 38);
    visuals.widgets.inactive.bg_fill = egui::Color32::from_rgb(38, 41, 52);
    visuals.widgets.hovered.bg_fill = egui::Color32::from_rgb(56, 61, 78);
    visuals.widgets.active.bg_fill = egui::Color32::from_rgb(225, 145, 25);
    visuals.selection.bg_fill = egui::Color32::from_rgb(215, 135, 20);
    ctx.set_visuals(visuals);
}

#[allow(clippy::too_many_arguments)]
fn draw_ssl_knob_cell(
    ui: &mut egui::Ui,
    setter: &ParamSetter,
    param: &FloatParam,
    label: &str,
    step: f32,
    min_val: f32,
    max_val: f32,
    color_family: SslKnobColor,
) {
    let current_val = param.value();
    let default_val = param.default_plain_value();
    let formatted_val = param.to_string();

    ui.allocate_ui_with_layout(
        egui::vec2(UI_KNOB_CELL_WIDTH, UI_KNOB_CELL_HEIGHT),
        egui::Layout::top_down(egui::Align::Center),
        |ui| {
            ui.set_width(UI_KNOB_CELL_WIDTH);
            ui.spacing_mut().item_spacing.y = 2.0;

            ui.label(
                egui::RichText::new(label)
                    .monospace()
                    .strong()
                    .size(11.0)
                    .color(egui::Color32::from_rgb(220, 224, 232)),
            );

            let (knob_rect, resp) = ui.allocate_exact_size(
                egui::vec2(UI_KNOB_DIAMETER, UI_KNOB_DIAMETER),
                egui::Sense::click_and_drag(),
            );
            let resp = resp.on_hover_text(
                "Drag vertically to adjust (hold Shift for fine control, double-click to reset)",
            );

            if resp.drag_started() {
                setter.begin_set_parameter(param);
            }
            if resp.dragged() {
                let dy = resp.drag_delta().y;
                if dy.abs() > f32::EPSILON {
                    let fine = ui.input(|i| i.modifiers.shift);
                    let sensitivity = if fine { 600.0_f32 } else { 150.0_f32 };
                    let current_norm = param.modulated_normalized_value();
                    let next_norm = (current_norm - dy / sensitivity).clamp(0.0, 1.0);
                    let next_plain = param.preview_plain(next_norm).clamp(min_val, max_val);
                    setter.set_parameter(param, next_plain);
                }
            }
            if resp.drag_stopped() {
                setter.end_set_parameter(param);
            }
            if resp.double_clicked() {
                setter.begin_set_parameter(param);
                setter.set_parameter(param, default_val);
                setter.end_set_parameter(param);
            }

            let norm = param.modulated_normalized_value().clamp(0.0, 1.0);
            let painter = ui.painter();
            let center = knob_rect.center();
            let outer_r = UI_KNOB_DIAMETER * 0.5;

            // 11 radial graduation ticks over 270 deg (-135 deg to +135 deg from top vertical)
            for idx in 0..=10 {
                let tick_norm = (idx as f32) / 10.0;
                let tick_angle = (-135.0_f32 + tick_norm * 270.0_f32).to_radians();
                let dir = egui::vec2(tick_angle.sin(), -tick_angle.cos());
                let p_in = center + dir * (outer_r - 4.0);
                let p_out = center + dir * (outer_r - 0.5);
                let tick_color = if tick_norm <= norm + 0.001 {
                    egui::Color32::from_rgb(245, 185, 45)
                } else {
                    egui::Color32::from_rgb(85, 92, 108)
                };
                painter.line_segment([p_in, p_out], egui::Stroke::new(1.5_f32, tick_color));
            }

            // Outer dark skirt
            let skirt_r = outer_r - 5.5;
            painter.circle_filled(center, skirt_r, egui::Color32::from_rgb(16, 18, 22));
            painter.circle_stroke(
                center,
                skirt_r,
                egui::Stroke::new(1.2_f32, egui::Color32::from_rgb(62, 68, 82)),
            );

            // Inner SSL colored cap
            let cap_r = outer_r - 8.5;
            let active_or_hovered = resp.hovered() || resp.dragged();
            painter.circle_filled(center, cap_r, color_family.cap_color(active_or_hovered));
            painter.circle_stroke(
                center,
                cap_r,
                egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(25, 28, 34)),
            );

            // Angular pointer line
            let val_angle = (-135.0_f32 + norm * 270.0_f32).to_radians();
            let val_dir = egui::vec2(val_angle.sin(), -val_angle.cos());
            painter.line_segment(
                [center + val_dir * 2.5, center + val_dir * (skirt_r - 1.5)],
                egui::Stroke::new(2.2_f32, egui::Color32::from_rgb(250, 250, 245)),
            );
            painter.circle_filled(center, 2.0, egui::Color32::from_rgb(25, 28, 34));

            ui.label(
                egui::RichText::new(formatted_val)
                    .monospace()
                    .size(11.0)
                    .color(egui::Color32::from_rgb(245, 185, 45)),
            );

            ui.horizontal(|ui| {
                let btn_gap = 3.0_f32;
                let total_btns_w =
                    2.0 * UI_KNOB_STEP_BTN_WIDTH + UI_KNOB_RESET_BTN_WIDTH + 2.0 * btn_gap;
                let pad = ((UI_KNOB_CELL_WIDTH - total_btns_w) * 0.5).max(0.0);
                ui.add_space(pad);
                ui.spacing_mut().item_spacing.x = btn_gap;
                ui.spacing_mut().button_padding = egui::vec2(2.0, 1.0);
                ui.spacing_mut().interact_size.y = UI_KNOB_BTN_HEIGHT;

                if ui
                    .add_sized(
                        [UI_KNOB_STEP_BTN_WIDTH, UI_KNOB_BTN_HEIGHT],
                        egui::Button::new(egui::RichText::new("-").monospace().size(11.0)),
                    )
                    .on_hover_text("Decrease value")
                    .clicked()
                {
                    let next_val = (current_val - step).clamp(min_val, max_val);
                    setter.begin_set_parameter(param);
                    setter.set_parameter(param, next_val);
                    setter.end_set_parameter(param);
                }

                if ui
                    .add_sized(
                        [UI_KNOB_STEP_BTN_WIDTH, UI_KNOB_BTN_HEIGHT],
                        egui::Button::new(egui::RichText::new("+").monospace().size(11.0)),
                    )
                    .on_hover_text("Increase value")
                    .clicked()
                {
                    let next_val = (current_val + step).clamp(min_val, max_val);
                    setter.begin_set_parameter(param);
                    setter.set_parameter(param, next_val);
                    setter.end_set_parameter(param);
                }

                if ui
                    .add_sized(
                        [UI_KNOB_RESET_BTN_WIDTH, UI_KNOB_BTN_HEIGHT],
                        egui::Button::new(egui::RichText::new("Reset").monospace().size(10.5)),
                    )
                    .on_hover_text("Reset to default value")
                    .clicked()
                {
                    setter.begin_set_parameter(param);
                    setter.set_parameter(param, default_val);
                    setter.end_set_parameter(param);
                }
            });
        },
    );
}

fn draw_vertical_level_meter(
    ui: &mut egui::Ui,
    title: &str,
    peak_l: f32,
    peak_r: f32,
    height: f32,
) {
    let db_l = lin_to_db(peak_l).clamp(MIN_FLOOR_DB, 6.0);
    let db_r = lin_to_db(peak_r).clamp(MIN_FLOOR_DB, 6.0);
    let norm_l = ((db_l - MIN_FLOOR_DB) / (0.0 - MIN_FLOOR_DB)).clamp(0.0, 1.0);
    let norm_r = ((db_r - MIN_FLOOR_DB) / (0.0 - MIN_FLOOR_DB)).clamp(0.0, 1.0);

    ui.vertical(|ui| {
        ui.label(
            egui::RichText::new(title)
                .monospace()
                .strong()
                .color(egui::Color32::from_rgb(200, 205, 215)),
        );

        let (rect, _resp) =
            ui.allocate_exact_size(egui::vec2(48.0, height), egui::Sense::hover());
        let painter = ui.painter();

        painter.rect_filled(rect, 3.0, egui::Color32::from_rgb(12, 13, 16));
        painter.rect_stroke(
            rect,
            3.0,
            egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(55, 60, 72)),
            egui::StrokeKind::Inside,
        );

        let inner_margin = 4.0;
        let bar_width = (rect.width() - inner_margin * 3.0) * 0.5;
        let max_bar_h = rect.height() - inner_margin * 2.0;

        for (idx, norm) in [norm_l, norm_r].iter().enumerate() {
            let x0 = rect.left() + inner_margin + (idx as f32) * (bar_width + inner_margin);
            let bar_h = max_bar_h * (*norm);
            let y1 = rect.bottom() - inner_margin;
            let y0 = y1 - bar_h;

            let bar_rect = egui::Rect::from_min_max(egui::pos2(x0, y0), egui::pos2(x0 + bar_width, y1));
            let color = if *norm > 0.98 {
                egui::Color32::from_rgb(235, 65, 55)
            } else if *norm > 0.80 {
                egui::Color32::from_rgb(240, 190, 40)
            } else {
                egui::Color32::from_rgb(55, 200, 110)
            };
            painter.rect_filled(bar_rect, 1.5, color);
        }

        let max_db = db_l.max(db_r);
        let readout = if max_db <= MIN_FLOOR_DB + 0.1 {
            "-inf dB".to_string()
        } else {
            format!("{:+5.1} dB", max_db)
        };
        ui.label(
            egui::RichText::new(readout)
                .monospace()
                .size(11.0)
                .color(egui::Color32::from_rgb(175, 180, 190)),
        );
    });
}

fn draw_reaper_style_gr_meter(
    ui: &mut egui::Ui,
    current_gr_db: f32,
    max_gr_db: f32,
    height: f32,
) {
    let clamped_gr = current_gr_db.clamp(0.0, MAX_GR_METER_DB);
    let clamped_max = max_gr_db.clamp(0.0, MAX_GR_METER_DB);
    let norm_gr = (clamped_gr / MAX_GR_METER_DB).clamp(0.0, 1.0);
    let norm_max = (clamped_max / MAX_GR_METER_DB).clamp(0.0, 1.0);

    ui.vertical(|ui| {
        ui.label(
            egui::RichText::new("GR (0 to -24 dB)")
                .monospace()
                .strong()
                .color(egui::Color32::from_rgb(245, 190, 40)),
        );

        ui.horizontal(|ui| {
            let (rect, _resp) =
                ui.allocate_exact_size(egui::vec2(58.0, height), egui::Sense::hover());
            let painter = ui.painter();

            painter.rect_filled(rect, 3.0, egui::Color32::from_rgb(12, 13, 16));
            painter.rect_stroke(
                rect,
                3.0,
                egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(80, 70, 35)),
                egui::StrokeKind::Inside,
            );

            let pad = 4.0;
            let inner_top = rect.top() + pad;
            let inner_bottom = rect.bottom() - pad;
            let inner_height = (inner_bottom - inner_top).max(1.0);

            // Top-down descending yellow/amber bar (0.0 dB at top -> -24.0 dB at bottom)
            let bar_bottom = inner_top + inner_height * norm_gr;
            let gr_rect = egui::Rect::from_min_max(
                egui::pos2(rect.left() + pad, inner_top),
                egui::pos2(rect.right() - pad, bar_bottom),
            );
            painter.rect_filled(gr_rect, 2.0, egui::Color32::from_rgb(245, 180, 25));

            // Peak GR Hold horizontal marker line
            if norm_max > 0.005 {
                let peak_y = inner_top + inner_height * norm_max;
                painter.line_segment(
                    [
                        egui::pos2(rect.left() + 2.0, peak_y),
                        egui::pos2(rect.right() - 2.0, peak_y),
                    ],
                    egui::Stroke::new(2.0_f32, egui::Color32::from_rgb(255, 95, 45)),
                );
            }

            // Scale tick marks (-3, -6, -12, -18, -24 dB)
            for db_mark in [0.0_f32, 3.0, 6.0, 12.0, 18.0, 24.0] {
                let y = inner_top + inner_height * (db_mark / MAX_GR_METER_DB);
                painter.line_segment(
                    [egui::pos2(rect.left(), y), egui::pos2(rect.left() + 6.0, y)],
                    egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(110, 115, 125)),
                );
            }

            // Scale labels column
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = (height - 110.0).max(4.0) / 5.0;
                for label in [" 0 dB", "-3 dB", "-6 dB", "-12dB", "-18dB", "-24dB"] {
                    ui.label(
                        egui::RichText::new(label)
                            .monospace()
                            .size(10.5)
                            .color(egui::Color32::from_rgb(150, 155, 165)),
                    );
                }
            });
        });
    });
}

/// Splits a package version string into `(base_version, is_dev)`.
/// Only the `-dev` suffix triggers the distinct `[DEV ...]` header badge;
/// other pre-release suffixes (e.g. `-rc.1`) remain intact with `is_dev = false`.
fn parse_display_version(version: &str) -> (&str, bool) {
    match version.strip_suffix("-dev") {
        Some(base) if !base.is_empty() => (base, true),
        _ => (version, false),
    }
}

/// Formats the development build badge label from the compile-time Git tag.
/// Falls back to `"[DEV]"` when no valid ASCII Git tag is available.
fn format_dev_badge_label(git_tag: &str) -> String {
    let trimmed = git_tag.trim();
    if trimmed.is_empty() || !trimmed.is_ascii() {
        "[DEV]".to_string()
    } else {
        format!("[DEV {}]", trimmed)
    }
}

fn draw_header_bar(
    egui_ctx: &egui::Context,
    telemetry: &MeterTelemetry,
    raw_version: &str,
    git_tag: &str,
) {
    let is_clipped = telemetry.clip_latched.load(Ordering::Relaxed);
    let (base_version, is_dev) = parse_display_version(raw_version);
    let header_amber = egui::Color32::from_rgb(245, 185, 35);
    let header_text = |text: String| {
        egui::RichText::new(text)
            .monospace()
            .strong()
            .size(13.5)
            .color(header_amber)
    };

    egui::TopBottomPanel::top("omore_punch_header").show(egui_ctx, |ui| {
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 5.0;

            if is_dev {
                ui.label(header_text(format!("OMORE PUNCH v{}", base_version)));
                egui::Frame::new()
                    .fill(egui::Color32::from_rgb(28, 78, 120))
                    .stroke(egui::Stroke::new(
                        1.0_f32,
                        egui::Color32::from_rgb(70, 155, 220),
                    ))
                    .corner_radius(3.0)
                    .inner_margin(egui::Margin::symmetric(5, 1))
                    .show(ui, |ui| {
                        ui.label(
                            egui::RichText::new(format_dev_badge_label(git_tag))
                                .monospace()
                                .strong()
                                .size(11.5)
                                .color(egui::Color32::from_rgb(235, 248, 255)),
                        );
                    });
                ui.label(header_text(
                    "|  STEREO DRUM BUS COMPRESSOR & CLIPPER".to_string(),
                ));
            } else {
                ui.label(header_text(format!(
                    "OMORE PUNCH v{}  |  STEREO DRUM BUS COMPRESSOR & CLIPPER",
                    base_version
                )));
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let clip_text = if is_clipped {
                    "[CLIP: ACTIVE - CLICK TO RESET]"
                } else {
                    "[CLIP: OK]"
                };
                let clip_color = if is_clipped {
                    egui::Color32::from_rgb(220, 45, 45)
                } else {
                    egui::Color32::from_rgb(40, 55, 45)
                };
                if ui
                    .add(
                        egui::Button::new(
                            egui::RichText::new(clip_text)
                                .monospace()
                                .strong()
                                .size(12.5)
                                .color(egui::Color32::WHITE),
                        )
                        .fill(clip_color),
                    )
                    .clicked()
                {
                    telemetry.clip_latched.store(false, Ordering::Relaxed);
                }
            });
        });
        ui.add_space(4.0);
    });
}

fn draw_omore_punch_ui(
    egui_ctx: &egui::Context,
    setter: &ParamSetter,
    params: &Arc<OmorePunchParams>,
    telemetry: &Arc<MeterTelemetry>,
) {
    apply_omore_dark_theme(egui_ctx);

    let in_l = load_f32_relaxed(&telemetry.in_peak_l);
    let in_r = load_f32_relaxed(&telemetry.in_peak_r);
    let out_l = load_f32_relaxed(&telemetry.out_peak_l);
    let out_r = load_f32_relaxed(&telemetry.out_peak_r);
    let current_gr_db = load_f32_relaxed(&telemetry.current_gr_db);
    let max_gr_db = load_f32_relaxed(&telemetry.max_gr_db);

    // Keep Reaper's readable GR parameter synchronized while UI is open
    let target_gr_param = (-current_gr_db).clamp(-MAX_GR_METER_DB, 0.0);
    if (params.gr_meter_db.value() - target_gr_param).abs() > 0.05 {
        setter.set_parameter(&params.gr_meter_db, target_gr_param);
    }

    draw_header_bar(
        egui_ctx,
        telemetry,
        env!("CARGO_PKG_VERSION"),
        option_env!("OMORE_GIT_TAG").unwrap_or(""),
    );

    egui::CentralPanel::default().show(egui_ctx, |ui| {
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = UI_COL_GAP;

            // ================================================================
            // LEFT COLUMN: COMPRESSOR, CLIPPER, PARALLEL MIX & DRUM BUS EQ
            // ================================================================
            ui.allocate_ui_with_layout(
                egui::vec2(UI_LEFT_COL_WIDTH, ui.available_height()),
                egui::Layout::top_down(egui::Align::Min),
                |left_col| {
                    left_col.set_width(UI_LEFT_COL_WIDTH);

                    left_col.group(|ui| {
                        ui.set_width(ui.available_width());
                        ui.label(
                            egui::RichText::new("1. COMPRESSOR & ENVELOPE BALLISTICS")
                                .monospace()
                                .strong()
                                .color(egui::Color32::from_rgb(245, 185, 35)),
                        );
                        ui.separator();

                        let comp_knobs = [
                            (
                                &params.input_db,
                                "Input Drive",
                                0.5_f32,
                                -12.0_f32,
                                12.0_f32,
                                SslKnobColor::GainDrive,
                            ),
                            (
                                &params.threshold_db,
                                "Threshold",
                                0.5,
                                -40.0,
                                0.0,
                                SslKnobColor::Dynamics,
                            ),
                            (
                                &params.ratio,
                                "Ratio",
                                0.25,
                                1.0,
                                20.0,
                                SslKnobColor::Dynamics,
                            ),
                            (
                                &params.attack_ms,
                                "Attack",
                                0.5,
                                0.1,
                                100.0,
                                SslKnobColor::Envelope,
                            ),
                            (
                                &params.release_ms,
                                "Release",
                                5.0,
                                10.0,
                                500.0,
                                SslKnobColor::Envelope,
                            ),
                            (
                                &params.sc_hpf_hz,
                                "SC HPF",
                                5.0,
                                20.0,
                                250.0,
                                SslKnobColor::Filter,
                            ),
                            (
                                &params.makeup_db,
                                "Makeup Gain",
                                0.5,
                                -12.0,
                                24.0,
                                SslKnobColor::GainDrive,
                            ),
                        ];

                        for (row_idx, chunk) in comp_knobs.chunks(UI_KNOB_COLS_MAX).enumerate() {
                            if row_idx > 0 {
                                ui.add_space(UI_KNOB_GRID_SPACING_Y);
                            }
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = UI_KNOB_GRID_SPACING_X;
                                for &(param, label, step, min_val, max_val, color_family) in chunk {
                                    draw_ssl_knob_cell(
                                        ui,
                                        setter,
                                        param,
                                        label,
                                        step,
                                        min_val,
                                        max_val,
                                        color_family,
                                    );
                                }
                            });
                        }

                        ui.add_space(2.0);
                        ui.horizontal(|ui| {
                            if ui
                                .button(
                                    egui::RichText::new("[Punch 3ms / 80ms]")
                                        .monospace()
                                        .strong(),
                                )
                                .on_hover_text(
                                    "Set Omore Sampler signature Drum Bus Attack (3.0 ms) and Release (80.0 ms)",
                                )
                                .clicked()
                            {
                                setter.begin_set_parameter(&params.attack_ms);
                                setter.set_parameter(&params.attack_ms, 3.0);
                                setter.end_set_parameter(&params.attack_ms);

                                setter.begin_set_parameter(&params.release_ms);
                                setter.set_parameter(&params.release_ms, 80.0);
                                setter.end_set_parameter(&params.release_ms);
                            }

                            if ui
                                .button(
                                    egui::RichText::new("[Snare Safe 200Hz]")
                                        .monospace()
                                        .strong(),
                                )
                                .on_hover_text(
                                    "Set Sidechain HPF to 200.0 Hz so Kick and Snare body (up to 200 Hz) do not pump the Drum Bus",
                                )
                                .clicked()
                            {
                                setter.begin_set_parameter(&params.sc_hpf_hz);
                                setter.set_parameter(&params.sc_hpf_hz, 200.0);
                                setter.end_set_parameter(&params.sc_hpf_hz);
                            }

                            let auto_mk = params.auto_makeup.value();
                            let auto_label = if auto_mk {
                                "[Auto Makeup: ON]"
                            } else {
                                "[Auto Makeup: OFF]"
                            };
                            let auto_fill = if auto_mk {
                                egui::Color32::from_rgb(195, 125, 20)
                            } else {
                                egui::Color32::from_rgb(45, 48, 58)
                            };
                            if ui
                                .add(
                                    egui::Button::new(
                                        egui::RichText::new(auto_label).monospace().strong(),
                                    )
                                    .fill(auto_fill),
                                )
                                .on_hover_text(
                                    "Enable 35% automatic threshold/ratio gain compensation from Omore Sampler",
                                )
                                .clicked()
                            {
                                setter.begin_set_parameter(&params.auto_makeup);
                                setter.set_parameter(&params.auto_makeup, !auto_mk);
                                setter.end_set_parameter(&params.auto_makeup);
                            }
                        });
                    });

                    left_col.add_space(6.0);

                    left_col.group(|ui| {
                        ui.set_width(ui.available_width());
                        ui.label(
                            egui::RichText::new("2. OUTPUT CLIPPER & PARALLEL MIX")
                                .monospace()
                                .strong()
                                .color(egui::Color32::from_rgb(245, 185, 35)),
                        );
                        ui.separator();

                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new("Clipper Mode:")
                                    .monospace()
                                    .strong(),
                            );
                            let current_mode = params.clipper_mode.value();
                            for (mode, text) in [
                                (ClipperMode::SoftClip, "[Soft Clip]"),
                                (ClipperMode::HardClip, "[Hard Clip]"),
                                (ClipperMode::Bypass, "[Bypass]"),
                            ] {
                                let active = current_mode == mode;
                                let fill = if active {
                                    egui::Color32::from_rgb(195, 125, 20)
                                } else {
                                    egui::Color32::from_rgb(45, 48, 58)
                                };
                                if ui
                                    .add(
                                        egui::Button::new(
                                            egui::RichText::new(text).monospace().strong(),
                                        )
                                        .fill(fill),
                                    )
                                    .clicked()
                                {
                                    setter.begin_set_parameter(&params.clipper_mode);
                                    setter.set_parameter(&params.clipper_mode, mode);
                                    setter.end_set_parameter(&params.clipper_mode);
                                }
                            }
                        });

                        ui.add_space(2.0);
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = UI_KNOB_GRID_SPACING_X;
                            draw_ssl_knob_cell(
                                ui,
                                setter,
                                &params.ceiling_db,
                                "Clip Ceiling",
                                0.25,
                                -6.0,
                                0.0,
                                SslKnobColor::ClipperMix,
                            );
                            draw_ssl_knob_cell(
                                ui,
                                setter,
                                &params.mix_pct,
                                "Mix Dry/Wet",
                                5.0,
                                0.0,
                                100.0,
                                SslKnobColor::ClipperMix,
                            );
                        });
                    });

                    left_col.add_space(6.0);

                    left_col.group(|ui| {
                        ui.set_width(ui.available_width());
                        ui.label(
                            egui::RichText::new("3. DRUM BUS EQ")
                                .monospace()
                                .strong()
                                .color(egui::Color32::from_rgb(245, 185, 35)),
                        );
                        ui.separator();

                        let eq_knobs = [
                            (&params.eq_low_cut_hz, "Low Cut", 5.0_f32, 20.0_f32, 160.0_f32),
                            (&params.eq_mid_cut_db, "500Hz Cut", 0.5_f32, -12.0_f32, 0.0_f32),
                            (
                                &params.eq_high_shelf_db,
                                "High Shelf",
                                0.5_f32,
                                -6.0_f32,
                                12.0_f32,
                            ),
                        ];
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = UI_KNOB_GRID_SPACING_X;
                            for &(param, label, step, min_val, max_val) in &eq_knobs {
                                draw_ssl_knob_cell(
                                    ui,
                                    setter,
                                    param,
                                    label,
                                    step,
                                    min_val,
                                    max_val,
                                    SslKnobColor::Filter,
                                );
                            }
                        });
                    });
                },
            );

            // ================================================================
            // RIGHT COLUMN: GAIN REDUCTION & STEREO IN/OUT METERS
            // ================================================================
            ui.allocate_ui_with_layout(
                egui::vec2(UI_RIGHT_COL_WIDTH, ui.available_height()),
                egui::Layout::top_down(egui::Align::Min),
                |right_col| {
                    right_col.set_width(UI_RIGHT_COL_WIDTH);

                    right_col.group(|ui| {
                        ui.set_width(ui.available_width());
                        ui.label(
                            egui::RichText::new("4. GAIN REDUCTION & STEREO BUS METERING")
                                .monospace()
                                .strong()
                                .color(egui::Color32::from_rgb(245, 185, 35)),
                        );
                        ui.separator();

                        // Large ASCII Digital Readouts for Instant GR and Peak GR Hold
                        ui.horizontal(|ui| {
                            ui.group(|ui| {
                                ui.vertical(|ui| {
                                    ui.label(
                                        egui::RichText::new(format!(
                                            "GR: -{:04.1} dB",
                                            current_gr_db
                                        ))
                                        .monospace()
                                        .strong()
                                        .size(20.0)
                                        .color(egui::Color32::from_rgb(250, 195, 40)),
                                    );
                                    ui.label(
                                        egui::RichText::new(format!(
                                            "Max GR: -{:04.1} dB",
                                            max_gr_db
                                        ))
                                        .monospace()
                                        .strong()
                                        .size(15.0)
                                        .color(egui::Color32::from_rgb(255, 125, 65)),
                                    );
                                });
                            });

                            ui.vertical(|ui| {
                                if ui
                                    .add_sized(
                                        [112.0, 32.0],
                                        egui::Button::new(
                                            egui::RichText::new("[Reset Peak]")
                                                .monospace()
                                                .strong(),
                                        ),
                                    )
                                    .on_hover_text(
                                        "Reset Max Gain Reduction Hold and Clipper Latch",
                                    )
                                    .clicked()
                                {
                                    telemetry.reset_peaks();
                                }
                            });
                        });

                        ui.add_space(8.0);

                        // Vertical Meters Row: In L/R, Out L/R, and Reaper-Style Top-Down GR Bar
                        ui.horizontal(|ui| {
                            ui.add_space(8.0);
                            draw_vertical_level_meter(ui, "In L/R", in_l, in_r, UI_METER_HEIGHT);
                            ui.add_space(18.0);
                            draw_reaper_style_gr_meter(
                                ui,
                                current_gr_db,
                                max_gr_db,
                                UI_METER_HEIGHT,
                            );
                            ui.add_space(18.0);
                            draw_vertical_level_meter(ui, "Out L/R", out_l, out_r, UI_METER_HEIGHT);
                        });
                    });
                },
            );
        });
    });

    // Request continuous 60 FPS repaint for smooth metering while editor is open
    egui_ctx.request_repaint();
}

// ----------------------------------------------------------------------------
// CLAP and VST3 Format Exports
// ----------------------------------------------------------------------------

impl ClapPlugin for OmorePunch {
    const CLAP_ID: &'static str = "local.omore-audio.omore-punch";
    const CLAP_DESCRIPTION: Option<&'static str> =
        Some("Stereo Drum Bus Parallel Compressor and Clipper");
    const CLAP_MANUAL_URL: Option<&'static str> = Some(Self::URL);
    const CLAP_SUPPORT_URL: Option<&'static str> = None;
    const CLAP_FEATURES: &'static [ClapFeature] = &[
        ClapFeature::AudioEffect,
        ClapFeature::Compressor,
        ClapFeature::Limiter,
        ClapFeature::Distortion,
        ClapFeature::Stereo,
    ];
}

impl Vst3Plugin for OmorePunch {
    const VST3_CLASS_ID: [u8; 16] = *b"OmorePunchBusV01";
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] = &[
        Vst3SubCategory::Fx,
        Vst3SubCategory::Dynamics,
        Vst3SubCategory::Distortion,
        Vst3SubCategory::Stereo,
    ];
}

nih_export_clap!(OmorePunch);
nih_export_vst3!(OmorePunch);

// ============================================================================
// UNIT TESTS (cargo test)
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compressor_stereo_linked_reduction() {
        let mut dsp = StereoPunchDsp::new(48000.0);
        let cfg = DspConfig {
            input_db: 0.0,
            threshold_db: -20.0,
            ratio: 4.0,
            attack_ms: 3.0,
            release_ms: 80.0,
            sc_hpf_hz: 20.0,
            auto_makeup: false,
            makeup_db: 0.0,
            eq_low_cut_hz: 20.0,
            eq_mid_cut_db: 0.0,
            eq_high_shelf_db: 0.0,
            clipper_mode: ClipperMode::Bypass,
            ceiling_db: 0.0,
            mix_pct: 100.0,
        };

        // Left channel is at 0.5 (-6.0206 dBFS), Right channel is at 0.1 (-20.0 dBFS).
        // Expected target GR = (-6.0206 - (-20.0)) * (1.0 - 0.25) = 10.48455 dB.
        let in_l = 0.5_f32;
        let in_r = 0.1_f32;
        let mut last = SampleOutput {
            out_l: 0.0,
            out_r: 0.0,
            gr_db: 0.0,
            clipped: false,
        };

        for _ in 0..4000 {
            last = dsp.process_sample(in_l, in_r, &cfg);
        }

        let expected_gr_db = (20.0 * in_l.log10() - cfg.threshold_db) * (1.0 - 1.0 / cfg.ratio);
        assert!(
            (last.gr_db - expected_gr_db).abs() < 0.05,
            "Expected GR ~ {:.3} dB, got {:.3} dB",
            expected_gr_db,
            last.gr_db
        );

        // Verify Stereo Link: L and R must be attenuated by the exact same linear gain factor
        let gain_l = last.out_l / in_l;
        let gain_r = last.out_r / in_r;
        assert!(
            (gain_l - gain_r).abs() < 1.0e-6,
            "Stereo link mismatch: gain_l={} vs gain_r={}",
            gain_l,
            gain_r
        );
        assert!(gain_l < 0.35, "Expected significant attenuation on both channels");
    }

    #[test]
    fn test_sidechain_hpf_kick_immunity() {
        let sample_rate = 48000.0_f32;
        let kick_freq_hz = 50.0_f32;
        let kick_amp = 0.8_f32;

        let mut dsp_unfiltered = StereoPunchDsp::new(sample_rate);
        let mut dsp_filtered = StereoPunchDsp::new(sample_rate);

        let cfg_unfiltered = DspConfig {
            input_db: 0.0,
            threshold_db: -18.0,
            ratio: 4.0,
            attack_ms: 3.0,
            release_ms: 80.0,
            sc_hpf_hz: 20.0,
            auto_makeup: false,
            makeup_db: 0.0,
            eq_low_cut_hz: 20.0,
            eq_mid_cut_db: 0.0,
            eq_high_shelf_db: 0.0,
            clipper_mode: ClipperMode::Bypass,
            ceiling_db: 0.0,
            mix_pct: 100.0,
        };

        let cfg_filtered = DspConfig {
            sc_hpf_hz: 150.0,
            ..cfg_unfiltered
        };

        let mut gr_unfiltered = 0.0_f32;
        let mut gr_filtered = 0.0_f32;
        let mut peak_out_unfiltered = 0.0_f32;
        let mut peak_out_filtered = 0.0_f32;

        let total_samples = 24000;
        for n in 0..total_samples {
            let phase = 2.0 * std::f32::consts::PI * kick_freq_hz * (n as f32) / sample_rate;
            let s = kick_amp * phase.sin();

            let out_u = dsp_unfiltered.process_sample(s, s, &cfg_unfiltered);
            let out_f = dsp_filtered.process_sample(s, s, &cfg_filtered);

            if n > total_samples / 2 {
                gr_unfiltered = gr_unfiltered.max(out_u.gr_db);
                gr_filtered = gr_filtered.max(out_f.gr_db);
                peak_out_unfiltered = peak_out_unfiltered.max(out_u.out_l.abs());
                peak_out_filtered = peak_out_filtered.max(out_f.out_l.abs());
            }
        }

        assert!(
            gr_filtered < gr_unfiltered - 4.0,
            "Expected SC HPF @ 150 Hz to significantly reduce Kick GR: gr_filtered={:.2} dB vs gr_unfiltered={:.2} dB",
            gr_filtered,
            gr_unfiltered
        );
        assert!(
            peak_out_filtered > peak_out_unfiltered * 1.5,
            "Expected SC HPF @ 150 Hz to preserve much more sub-bass amplitude in main audio path: peak_out_filtered={:.3} vs peak_out_unfiltered={:.3}",
            peak_out_filtered,
            peak_out_unfiltered
        );
    }

    #[test]
    fn test_sidechain_hpf_snare_safe_200hz() {
        let sample_rate = 48000.0_f32;
        let snare_body_freq_hz = 100.0_f32;
        let snare_amp = 0.8_f32;

        let mut dsp_unfiltered = StereoPunchDsp::new(sample_rate);
        let mut dsp_snare_safe = StereoPunchDsp::new(sample_rate);

        let cfg_unfiltered = DspConfig {
            input_db: 0.0,
            threshold_db: -18.0,
            ratio: 4.0,
            attack_ms: 3.0,
            release_ms: 80.0,
            sc_hpf_hz: 20.0,
            auto_makeup: false,
            makeup_db: 0.0,
            eq_low_cut_hz: 20.0,
            eq_mid_cut_db: 0.0,
            eq_high_shelf_db: 0.0,
            clipper_mode: ClipperMode::Bypass,
            ceiling_db: 0.0,
            mix_pct: 100.0,
        };

        let cfg_snare_safe = DspConfig {
            sc_hpf_hz: 200.0,
            ..cfg_unfiltered
        };

        let mut gr_unfiltered = 0.0_f32;
        let mut gr_snare_safe = 0.0_f32;
        let mut peak_out_unfiltered = 0.0_f32;
        let mut peak_out_snare_safe = 0.0_f32;

        let total_samples = 24000;
        for n in 0..total_samples {
            let phase =
                2.0 * std::f32::consts::PI * snare_body_freq_hz * (n as f32) / sample_rate;
            let s = snare_amp * phase.sin();

            let out_u = dsp_unfiltered.process_sample(s, s, &cfg_unfiltered);
            let out_s = dsp_snare_safe.process_sample(s, s, &cfg_snare_safe);

            if n > total_samples / 2 {
                gr_unfiltered = gr_unfiltered.max(out_u.gr_db);
                gr_snare_safe = gr_snare_safe.max(out_s.gr_db);
                peak_out_unfiltered = peak_out_unfiltered.max(out_u.out_l.abs());
                peak_out_snare_safe = peak_out_snare_safe.max(out_s.out_l.abs());
            }
        }

        assert!(
            gr_snare_safe < gr_unfiltered - 4.0,
            "Expected SC HPF @ 200 Hz (Snare Safe) to significantly reduce low-mid GR: gr_snare_safe={:.2} dB vs gr_unfiltered={:.2} dB",
            gr_snare_safe,
            gr_unfiltered
        );
        assert!(
            peak_out_snare_safe > peak_out_unfiltered * 1.5,
            "Expected SC HPF @ 200 Hz (Snare Safe) to preserve much more body amplitude in main audio path: peak_out_snare_safe={:.3} vs peak_out_unfiltered={:.3}",
            peak_out_snare_safe,
            peak_out_unfiltered
        );
    }

    #[test]
    fn test_clipper_strict_ceiling() {
        let mut dsp = StereoPunchDsp::new(48000.0);
        let ceiling_db = -3.0_f32;
        let ceiling_lin = db_to_lin(ceiling_db);

        for mode in [ClipperMode::SoftClip, ClipperMode::HardClip] {
            let cfg = DspConfig {
                input_db: 6.0,
                threshold_db: 0.0, // Compressor inactive
                ratio: 1.0,
                attack_ms: 3.0,
                release_ms: 80.0,
                sc_hpf_hz: 20.0,
                auto_makeup: false,
                makeup_db: 0.0,
                eq_low_cut_hz: 20.0,
                eq_mid_cut_db: 0.0,
                eq_high_shelf_db: 0.0,
                clipper_mode: mode,
                ceiling_db,
                mix_pct: 100.0,
            };

            let out = dsp.process_sample(1.2, -1.8, &cfg);
            assert!(
                out.out_l.abs() <= ceiling_lin + 1.0e-6,
                "Mode {:?}: Left output {} exceeded ceiling {}",
                mode,
                out.out_l,
                ceiling_lin
            );
            assert!(
                out.out_r.abs() <= ceiling_lin + 1.0e-6,
                "Mode {:?}: Right output {} exceeded ceiling {}",
                mode,
                out.out_r,
                ceiling_lin
            );
            assert!(out.clipped, "Mode {:?}: Should flag clipping activity", mode);
        }

        // Verify SoftClip preserves unity gain at low levels (below 50% of ceiling)
        let soft_cfg = DspConfig {
            input_db: 0.0,
            threshold_db: 0.0,
            ratio: 1.0,
            auto_makeup: false,
            clipper_mode: ClipperMode::SoftClip,
            ceiling_db: 0.0,
            mix_pct: 100.0,
            ..DspConfig::default()
        };
        let low_out = dsp.process_sample(0.25, -0.25, &soft_cfg);
        assert!((low_out.out_l - 0.25).abs() < 1.0e-6);
        assert!((low_out.out_r + 0.25).abs() < 1.0e-6);
    }

    #[test]
    fn test_parallel_mix_dry_wet() {
        let dry_l = 0.623456_f32;
        let dry_r = -0.412345_f32;

        // 1. Test 0% Mix: Must restore Dry signal bit-for-bit even with heavy settings
        let mut dsp_dry = StereoPunchDsp::new(48000.0);
        let cfg_zero_mix = DspConfig {
            input_db: 9.0,
            threshold_db: -30.0,
            ratio: 12.0,
            attack_ms: 1.0,
            release_ms: 50.0,
            sc_hpf_hz: 90.0,
            auto_makeup: true,
            makeup_db: 6.0,
            eq_low_cut_hz: 20.0,
            eq_mid_cut_db: 0.0,
            eq_high_shelf_db: 0.0,
            clipper_mode: ClipperMode::HardClip,
            ceiling_db: -4.0,
            mix_pct: 0.0,
        };
        let out_zero = dsp_dry.process_sample(dry_l, dry_r, &cfg_zero_mix);
        assert_eq!(
            out_zero.out_l.to_bits(),
            dry_l.to_bits(),
            "0% Mix must be bit-identical to Dry on Left channel"
        );
        assert_eq!(
            out_zero.out_r.to_bits(),
            dry_r.to_bits(),
            "0% Mix must be bit-identical to Dry on Right channel"
        );

        // 2. Test 50% Mix: Must equal 0.5 * Dry + 0.5 * Wet
        let mut dsp_wet = StereoPunchDsp::new(48000.0);
        let mut dsp_half = StereoPunchDsp::new(48000.0);

        let cfg_wet = DspConfig {
            mix_pct: 100.0,
            ..cfg_zero_mix
        };
        let cfg_half = DspConfig {
            mix_pct: 50.0,
            ..cfg_zero_mix
        };

        let out_wet = dsp_wet.process_sample(dry_l, dry_r, &cfg_wet);
        let out_half = dsp_half.process_sample(dry_l, dry_r, &cfg_half);

        let expected_half_l = 0.5 * dry_l + 0.5 * out_wet.out_l;
        let expected_half_r = 0.5 * dry_r + 0.5 * out_wet.out_r;

        assert!((out_half.out_l - expected_half_l).abs() < 1.0e-6);
        assert!((out_half.out_r - expected_half_r).abs() < 1.0e-6);
    }

    #[test]
    fn test_nan_inf_and_safety_clamp() {
        let mut dsp = StereoPunchDsp::new(48000.0);
        let cfg = DspConfig {
            input_db: 12.0,
            threshold_db: -24.0,
            ratio: 8.0,
            attack_ms: 3.0,
            release_ms: 80.0,
            sc_hpf_hz: 90.0,
            auto_makeup: true,
            makeup_db: 24.0,
            eq_low_cut_hz: 120.0,
            eq_mid_cut_db: -9.0,
            eq_high_shelf_db: 12.0,
            clipper_mode: ClipperMode::Bypass,
            ceiling_db: 0.0,
            mix_pct: 100.0,
        };

        let pathological_inputs = [
            (f32::NAN, f32::NAN),
            (f32::INFINITY, f32::NEG_INFINITY),
            (1.0e30, -1.0e30),
            (0.5, -0.5),
        ];

        for (in_l, in_r) in pathological_inputs {
            let out = dsp.process_sample(in_l, in_r, &cfg);
            assert!(out.out_l.is_finite(), "Left output is not finite: {}", out.out_l);
            assert!(out.out_r.is_finite(), "Right output is not finite: {}", out.out_r);
            assert!(
                out.out_l >= -SAFETY_CLAMP_LIMIT && out.out_l <= SAFETY_CLAMP_LIMIT,
                "Left output {} out of [-4.0, 4.0] safety bounds",
                out.out_l
            );
            assert!(
                out.out_r >= -SAFETY_CLAMP_LIMIT && out.out_r <= SAFETY_CLAMP_LIMIT,
                "Right output {} out of [-4.0, 4.0] safety bounds",
                out.out_r
            );
            assert!(
                dsp.env_gr_db.is_finite(),
                "Internal GR envelope became non-finite"
            );
            for (name, biquad) in [
                ("eq_low_cut", &dsp.eq_low_cut),
                ("eq_mid_cut", &dsp.eq_mid_cut),
                ("eq_high_shelf", &dsp.eq_high_shelf),
            ] {
                assert!(
                    biquad.z1_l.is_finite()
                        && biquad.z2_l.is_finite()
                        && biquad.z1_r.is_finite()
                        && biquad.z2_r.is_finite(),
                    "Internal EQ biquad states for {} became non-finite",
                    name
                );
            }
        }
    }

    fn measure_steady_sine_rms(
        sample_rate: f32,
        cfg: &DspConfig,
        freq_hz: f32,
        amp_l: f32,
        amp_r: f32,
    ) -> (f32, f32) {
        let mut dsp = StereoPunchDsp::new(sample_rate);
        let total_samples = sample_rate.round() as usize;
        let warmup_samples = total_samples / 2;
        let mut sum_sq_l = 0.0_f64;
        let mut sum_sq_r = 0.0_f64;
        let mut count = 0_usize;

        for n in 0..total_samples {
            let phase = 2.0 * std::f32::consts::PI * freq_hz * (n as f32) / sample_rate;
            let s = phase.sin();
            let out = dsp.process_sample(amp_l * s, amp_r * s, cfg);
            assert!(out.out_l.is_finite() && out.out_r.is_finite());
            if n >= warmup_samples {
                sum_sq_l += (out.out_l as f64) * (out.out_l as f64);
                sum_sq_r += (out.out_r as f64) * (out.out_r as f64);
                count += 1;
            }
        }

        let inv_n = 1.0 / (count as f64);
        ((sum_sq_l * inv_n).sqrt() as f32, (sum_sq_r * inv_n).sqrt() as f32)
    }

    #[test]
    fn test_eq_neutral_bypass_transparency() {
        let mut dsp = StereoPunchDsp::new(48000.0);
        let neutral_cfg = DspConfig {
            ratio: 1.0,
            auto_makeup: false,
            eq_low_cut_hz: 20.0,
            eq_mid_cut_db: 0.0,
            eq_high_shelf_db: 0.0,
            clipper_mode: ClipperMode::Bypass,
            ..DspConfig::default()
        };

        for n in 0..512 {
            let t = (n as f32) / 48000.0;
            let in_l = 0.3 * (2.0 * std::f32::consts::PI * 50.0 * t).sin()
                + 0.25 * (2.0 * std::f32::consts::PI * 500.0 * t).sin();
            let in_r = -0.2 * (2.0 * std::f32::consts::PI * 12000.0 * t).sin()
                + 0.35 * (2.0 * std::f32::consts::PI * 500.0 * t).sin();
            let out = dsp.process_sample(in_l, in_r, &neutral_cfg);
            assert_eq!(out.out_l.to_bits(), in_l.to_bits());
            assert_eq!(out.out_r.to_bits(), in_r.to_bits());
        }

        // Verify engaging EQ then returning to neutral (including 20.5 Hz threshold)
        // resets biquad states and restores bit-exact transparency.
        let active_cfg = DspConfig {
            eq_low_cut_hz: 120.0,
            eq_mid_cut_db: -9.0,
            eq_high_shelf_db: 9.0,
            ..neutral_cfg
        };
        for _ in 0..128 {
            dsp.process_sample(0.4, -0.3, &active_cfg);
        }

        let boundary_neutral_cfg = DspConfig {
            eq_low_cut_hz: 20.5,
            ..neutral_cfg
        };
        let out_after_reset = dsp.process_sample(0.42, -0.17, &boundary_neutral_cfg);
        assert_eq!(out_after_reset.out_l.to_bits(), 0.42_f32.to_bits());
        assert_eq!(out_after_reset.out_r.to_bits(), (-0.17_f32).to_bits());
        assert_eq!(dsp.eq_low_cut.z1_l, 0.0);
        assert_eq!(dsp.eq_mid_cut.z1_r, 0.0);
        assert_eq!(dsp.eq_high_shelf.z2_l, 0.0);
    }

    #[test]
    fn test_eq_low_cut_sub_attenuation() {
        let base_cfg = DspConfig {
            ratio: 1.0,
            auto_makeup: false,
            clipper_mode: ClipperMode::Bypass,
            ..DspConfig::default()
        };
        let cut_cfg = DspConfig {
            eq_low_cut_hz: 120.0,
            ..base_cfg
        };

        for sample_rate in [44100.0_f32, 48000.0_f32, 96000.0_f32] {
            let (base_sub_l, base_sub_r) =
                measure_steady_sine_rms(sample_rate, &base_cfg, 40.0, 0.5, 0.35);
            let (cut_sub_l, cut_sub_r) =
                measure_steady_sine_rms(sample_rate, &cut_cfg, 40.0, 0.5, 0.35);

            let sub_att_db_l = 20.0 * (cut_sub_l / base_sub_l).log10();
            let sub_att_db_r = 20.0 * (cut_sub_r / base_sub_r).log10();
            assert!(
                sub_att_db_l < -16.0 && sub_att_db_l > -21.0,
                "sr={}: expected ~ -19 dB @ 40 Hz with 120 Hz Low Cut, got {:.2} dB",
                sample_rate,
                sub_att_db_l
            );
            assert!(
                (sub_att_db_l - sub_att_db_r).abs() < 1.0e-3,
                "sr={}: stereo Low Cut mismatch L={:.4} dB vs R={:.4} dB",
                sample_rate,
                sub_att_db_l,
                sub_att_db_r
            );

            let (base_mid_l, _) = measure_steady_sine_rms(sample_rate, &base_cfg, 1000.0, 0.5, 0.5);
            let (cut_mid_l, _) = measure_steady_sine_rms(sample_rate, &cut_cfg, 1000.0, 0.5, 0.5);
            let mid_diff_db = 20.0 * (cut_mid_l / base_mid_l).log10();
            assert!(
                mid_diff_db.abs() < 0.1,
                "sr={}: expected 1 kHz preserved by 120 Hz Low Cut, got {:.3} dB",
                sample_rate,
                mid_diff_db
            );
        }

        // Verify set_sample_rate() invalidates and recalculates EQ filter tuning accurately
        let mut dsp = StereoPunchDsp::new(44100.0);
        dsp.process_sample(0.1, 0.1, &cut_cfg);
        dsp.set_sample_rate(96000.0);
        let mut sum_sq = 0.0_f64;
        let total = 96000_usize;
        for n in 0..total {
            let phase = 2.0 * std::f32::consts::PI * 40.0 * (n as f32) / 96000.0;
            let out = dsp.process_sample(0.5 * phase.sin(), 0.5 * phase.sin(), &cut_cfg);
            if n >= total / 2 {
                sum_sq += (out.out_l as f64) * (out.out_l as f64);
            }
        }
        let rms_after_sr_change = (sum_sq / ((total / 2) as f64)).sqrt() as f32;
        let db_after_sr_change = 20.0 * (rms_after_sr_change / (0.5 / std::f32::consts::SQRT_2)).log10();
        assert!(
            db_after_sr_change < -16.0 && db_after_sr_change > -21.0,
            "After set_sample_rate(96000), expected ~ -19 dB @ 40 Hz, got {:.2} dB",
            db_after_sr_change
        );
    }

    #[test]
    fn test_eq_500hz_cut_attenuation() {
        let base_cfg = DspConfig {
            ratio: 1.0,
            auto_makeup: false,
            clipper_mode: ClipperMode::Bypass,
            ..DspConfig::default()
        };
        let cut_12_cfg = DspConfig {
            eq_mid_cut_db: -12.0,
            ..base_cfg
        };
        let cut_6_cfg = DspConfig {
            eq_mid_cut_db: -6.0,
            ..base_cfg
        };

        for sr in [44100.0_f32, 48000.0_f32, 96000.0_f32] {
            let (base_500_l, base_500_r) =
                measure_steady_sine_rms(sr, &base_cfg, 500.0, 0.5, 0.25);
            let (cut12_500_l, cut12_500_r) =
                measure_steady_sine_rms(sr, &cut_12_cfg, 500.0, 0.5, 0.25);
            let (cut6_500_l, _) = measure_steady_sine_rms(sr, &cut_6_cfg, 500.0, 0.5, 0.25);

            let att12_l = 20.0 * (cut12_500_l / base_500_l).log10();
            let att12_r = 20.0 * (cut12_500_r / base_500_r).log10();
            let att6_l = 20.0 * (cut6_500_l / base_500_l).log10();

            assert!(
                (att12_l - (-12.0)).abs() < 0.35,
                "sr={}: expected -12.0 dB +/- 0.35 dB @ 500 Hz, got {:.3} dB",
                sr,
                att12_l
            );
            assert!(
                (att12_l - att12_r).abs() < 1.0e-3,
                "sr={}: stereo 500 Hz Cut mismatch L={:.4} dB vs R={:.4} dB",
                sr,
                att12_l,
                att12_r
            );
            assert!(
                (att6_l - (-6.0)).abs() < 0.35,
                "sr={}: expected -6.0 dB +/- 0.35 dB @ 500 Hz, got {:.3} dB",
                sr,
                att6_l
            );

            // Unrelated frequency bands (60 Hz and 8 kHz) remain nearly intact
            for freq in [60.0_f32, 8000.0_f32] {
                let (base_other, _) = measure_steady_sine_rms(sr, &base_cfg, freq, 0.5, 0.5);
                let (cut_other, _) = measure_steady_sine_rms(sr, &cut_12_cfg, freq, 0.5, 0.5);
                let diff_db = 20.0 * (cut_other / base_other).log10();
                assert!(
                    diff_db.abs() < 0.5,
                    "sr={}: expected {} Hz untouched by 500 Hz Cut, got {:.3} dB",
                    sr,
                    freq,
                    diff_db
                );
            }
        }
    }

    #[test]
    fn test_eq_high_shelf_boost_and_cut() {
        let base_cfg = DspConfig {
            ratio: 1.0,
            auto_makeup: false,
            clipper_mode: ClipperMode::Bypass,
            ..DspConfig::default()
        };
        let boost_cfg = DspConfig {
            eq_high_shelf_db: 12.0,
            ..base_cfg
        };
        let cut_cfg = DspConfig {
            eq_high_shelf_db: -6.0,
            ..base_cfg
        };

        for sr in [44100.0_f32, 48000.0_f32, 96000.0_f32] {
            let (base_hi_l, base_hi_r) =
                measure_steady_sine_rms(sr, &base_cfg, 12000.0, 0.2, 0.15);
            let (boost_hi_l, boost_hi_r) =
                measure_steady_sine_rms(sr, &boost_cfg, 12000.0, 0.2, 0.15);
            let (cut_hi_l, cut_hi_r) =
                measure_steady_sine_rms(sr, &cut_cfg, 12000.0, 0.2, 0.15);

            let boost_db_l = 20.0 * (boost_hi_l / base_hi_l).log10();
            let boost_db_r = 20.0 * (boost_hi_r / base_hi_r).log10();
            let cut_db_l = 20.0 * (cut_hi_l / base_hi_l).log10();
            let cut_db_r = 20.0 * (cut_hi_r / base_hi_r).log10();

            assert!(
                boost_db_l > 6.5 && boost_db_l < 12.2,
                "sr={}: expected strong +12 dB High Shelf boost @ 12 kHz, got {:.2} dB",
                sr,
                boost_db_l
            );
            assert!(
                (boost_db_l - boost_db_r).abs() < 1.0e-3,
                "sr={}: stereo High Shelf boost mismatch",
                sr
            );
            assert!(
                cut_db_l < -3.0 && cut_db_l > -6.2,
                "sr={}: expected clear -6 dB High Shelf cut @ 12 kHz, got {:.2} dB",
                sr,
                cut_db_l
            );
            assert!(
                (cut_db_l - cut_db_r).abs() < 1.0e-3,
                "sr={}: stereo High Shelf cut mismatch",
                sr
            );

            // Low frequency (100 Hz) must stay unaffected by 10 kHz High Shelf
            let (base_lo, _) = measure_steady_sine_rms(sr, &base_cfg, 100.0, 0.5, 0.5);
            let (boost_lo, _) = measure_steady_sine_rms(sr, &boost_cfg, 100.0, 0.5, 0.5);
            let lo_diff_db = 20.0 * (boost_lo / base_lo).log10();
            assert!(
                lo_diff_db.abs() < 0.25,
                "sr={}: expected 100 Hz untouched by 10 kHz High Shelf, got {:.3} dB",
                sr,
                lo_diff_db
            );
        }

        // Post-EQ clipper ceiling enforcement with +12 dB High Shelf boost
        let sr = 48000.0_f32;
        let ceiling_db = -3.0_f32;
        let ceiling_lin = db_to_lin(ceiling_db);
        for mode in [ClipperMode::SoftClip, ClipperMode::HardClip] {
            let mut dsp = StereoPunchDsp::new(sr);
            let clipped_cfg = DspConfig {
                clipper_mode: mode,
                ceiling_db,
                ..boost_cfg
            };
            let mut any_clipped = false;
            for n in 0..4096 {
                let phase = 2.0 * std::f32::consts::PI * 12000.0 * (n as f32) / sr;
                let s = 0.7 * phase.sin();
                let out = dsp.process_sample(s, -s, &clipped_cfg);
                assert!(
                    out.out_l.abs() <= ceiling_lin + 1.0e-6,
                    "Mode {:?}: post-EQ Left {} exceeded ceiling {}",
                    mode,
                    out.out_l,
                    ceiling_lin
                );
                assert!(
                    out.out_r.abs() <= ceiling_lin + 1.0e-6,
                    "Mode {:?}: post-EQ Right {} exceeded ceiling {}",
                    mode,
                    out.out_r,
                    ceiling_lin
                );
                if out.clipped {
                    any_clipped = true;
                }
            }
            assert!(
                any_clipped,
                "Mode {:?}: expected +12 dB High Shelf peaks to engage post-EQ clipper",
                mode
            );
        }

        // 0% Parallel Mix remains bit-exact to dry input even with extreme EQ settings
        let mut dsp_dry = StereoPunchDsp::new(sr);
        let dry_mix_cfg = DspConfig {
            eq_low_cut_hz: 160.0,
            eq_mid_cut_db: -12.0,
            eq_high_shelf_db: 12.0,
            clipper_mode: ClipperMode::HardClip,
            ceiling_db: -6.0,
            mix_pct: 0.0,
            ..DspConfig::default()
        };
        let dry_out = dsp_dry.process_sample(0.6123, -0.4567, &dry_mix_cfg);
        assert_eq!(dry_out.out_l.to_bits(), 0.6123_f32.to_bits());
        assert_eq!(dry_out.out_r.to_bits(), (-0.4567_f32).to_bits());
    }

    #[test]
    fn test_ui_layout_column_widths_fit_window() {
        // CentralPanel inner margins (8.0 px left + 8.0 px right)
        let central_panel_margins = 16.0_f32;
        let total_columns_width =
            UI_LEFT_COL_WIDTH + UI_COL_GAP + UI_RIGHT_COL_WIDTH + central_panel_margins;
        assert!(
            total_columns_width <= UI_WINDOW_WIDTH as f32,
            "Total columns width ({}) exceeds window width ({})",
            total_columns_width,
            UI_WINDOW_WIDTH
        );

        // 4 SSL knob cells per row inside Group 1 + 3 grid gaps + group margins (12 px)
        let group_margins_px = 12.0_f32;
        let knob_row_width = (UI_KNOB_COLS_MAX as f32) * UI_KNOB_CELL_WIDTH
            + ((UI_KNOB_COLS_MAX - 1) as f32) * UI_KNOB_GRID_SPACING_X
            + group_margins_px;
        assert!(
            knob_row_width <= UI_LEFT_COL_WIDTH,
            "Knob row width ({}) overflows left column width ({})",
            knob_row_width,
            UI_LEFT_COL_WIDTH
        );

        // Compact button row ('-', '+', 'Reset') fits inside each knob cell
        let cell_buttons_width =
            2.0 * UI_KNOB_STEP_BTN_WIDTH + UI_KNOB_RESET_BTN_WIDTH + 2.0 * 3.0;
        assert!(
            cell_buttons_width <= UI_KNOB_CELL_WIDTH,
            "Cell compact buttons width ({}) overflow knob cell width ({})",
            cell_buttons_width,
            UI_KNOB_CELL_WIDTH
        );

        // Combined left-column sections (Section 1: 2 knob rows + quick buttons; Section 2:
        // clipper mode row + 1 knob row; Section 3: 1 EQ knob row; group headers, separators,
        // margins and gaps) fit within CentralPanel height
        // (UI_WINDOW_HEIGHT - header_and_panel_margins_px = 572.0 px).
        let header_and_panel_margins_px = 48.0_f32;
        let available_left_col_height =
            UI_WINDOW_HEIGHT as f32 - header_and_panel_margins_px;
        let group_header_and_margins_px = 36.0_f32;
        let action_row_height_px = 24.0_f32;
        let section1_height = group_header_and_margins_px
            + 2.0 * UI_KNOB_CELL_HEIGHT
            + UI_KNOB_GRID_SPACING_Y
            + 2.0
            + action_row_height_px;
        let section2_height =
            group_header_and_margins_px + action_row_height_px + 2.0 + UI_KNOB_CELL_HEIGHT;
        let section3_height = group_header_and_margins_px + UI_KNOB_CELL_HEIGHT;
        let inter_section_gap_px = 6.0_f32;
        let total_left_col_height = section1_height
            + inter_section_gap_px
            + section2_height
            + inter_section_gap_px
            + section3_height;

        assert!(
            total_left_col_height <= available_left_col_height,
            "Left column height ({}) overflows available panel height ({})",
            total_left_col_height,
            available_left_col_height
        );

        // Empirically verify rendered egui layout fits within 820x620 px
        struct DummyGuiContext;
        impl GuiContext for DummyGuiContext {
            fn plugin_api(&self) -> PluginApi {
                PluginApi::Standalone
            }
            fn request_resize(&self) -> bool {
                false
            }
            unsafe fn raw_begin_set_parameter(&self, _param: ParamPtr) {}
            unsafe fn raw_set_parameter_normalized(&self, _param: ParamPtr, _normalized: f32) {}
            unsafe fn raw_end_set_parameter(&self, _param: ParamPtr) {}
            fn get_state(&self) -> PluginState {
                PluginState {
                    version: String::new(),
                    params: std::collections::BTreeMap::new(),
                    fields: std::collections::BTreeMap::new(),
                }
            }
            fn set_state(&self, _state: PluginState) {}
        }

        let ctx = egui::Context::default();
        let params = Arc::new(OmorePunchParams::default());
        let telemetry = Arc::new(MeterTelemetry::default());
        let dummy_gui = DummyGuiContext;
        let setter = ParamSetter::new(&dummy_gui);
        let raw_input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(UI_WINDOW_WIDTH as f32, UI_WINDOW_HEIGHT as f32),
            )),
            ..Default::default()
        };

        let mut final_output = None;
        for _ in 0..2 {
            final_output = Some(ctx.run(raw_input.clone(), |ctx| {
                draw_omore_punch_ui(ctx, &setter, &params, &telemetry);
            }));
        }
        let used = ctx.used_rect();
        let shapes = final_output.expect("egui frame output missing").shapes;
        let max_shape_bottom = shapes
            .iter()
            .map(|clipped| clipped.shape.visual_bounding_rect().bottom())
            .fold(0.0_f32, f32::max);
        let max_inner_bottom = shapes
            .iter()
            .filter_map(|clipped| {
                let rect = clipped.shape.visual_bounding_rect();
                (rect.width() < UI_WINDOW_WIDTH as f32 - 1.0).then_some(rect.bottom())
            })
            .fold(0.0_f32, f32::max);
        let max_inner_right = shapes
            .iter()
            .filter_map(|clipped| {
                let rect = clipped.shape.visual_bounding_rect();
                (rect.width() < UI_WINDOW_WIDTH as f32 - 1.0).then_some(rect.right())
            })
            .fold(0.0_f32, f32::max);
        assert!(
            used.width() <= UI_WINDOW_WIDTH as f32,
            "Rendered egui width ({}) exceeds window width ({})",
            used.width(),
            UI_WINDOW_WIDTH
        );
        assert!(
            used.height() <= UI_WINDOW_HEIGHT as f32,
            "Rendered egui height ({}) exceeds window height ({})",
            used.height(),
            UI_WINDOW_HEIGHT
        );
        assert!(
            max_shape_bottom <= UI_WINDOW_HEIGHT as f32,
            "Painted egui widgets bottom ({}) overflows window height ({})",
            max_shape_bottom,
            UI_WINDOW_HEIGHT
        );
        assert!(
            max_inner_bottom <= UI_WINDOW_HEIGHT as f32 - 6.0,
            "Inner widgets bottom ({}) exceeds padded window height ({})",
            max_inner_bottom,
            UI_WINDOW_HEIGHT as f32 - 6.0
        );
        assert!(
            max_inner_right <= UI_WINDOW_WIDTH as f32 - 6.0,
            "Inner widgets right ({}) exceeds padded window width ({})",
            max_inner_right,
            UI_WINDOW_WIDTH as f32 - 6.0
        );
    }

    #[test]
    fn test_parse_display_version_variants() {
        assert_eq!(parse_display_version("0.2.1-dev"), ("0.2.1", true));
        assert_eq!(parse_display_version("0.2.1"), ("0.2.1", false));
        assert_eq!(parse_display_version("0.3.0-rc.1"), ("0.3.0-rc.1", false));
        assert_eq!(parse_display_version("1.0.0-alpha"), ("1.0.0-alpha", false));
        assert_eq!(parse_display_version("-dev"), ("-dev", false));
        assert_eq!(parse_display_version(""), ("", false));
    }

    #[test]
    fn test_format_dev_badge_label_variants() {
        assert_eq!(format_dev_badge_label("1d2c5b9"), "[DEV 1d2c5b9]");
        assert_eq!(
            format_dev_badge_label("1d2c5b9-dirty"),
            "[DEV 1d2c5b9-dirty]"
        );
        assert_eq!(format_dev_badge_label("  1d2c5b9  "), "[DEV 1d2c5b9]");
        assert_eq!(format_dev_badge_label(""), "[DEV]");
        assert_eq!(format_dev_badge_label("   "), "[DEV]");
        assert_eq!(format_dev_badge_label("bad\u{00e9}"), "[DEV]");
    }

    #[test]
    fn test_host_version_matches_cargo_pkg_version() {
        assert_eq!(OmorePunch::VERSION, env!("CARGO_PKG_VERSION"));
        let (base_version, is_dev) = parse_display_version(OmorePunch::VERSION);
        assert!(!base_version.is_empty());
        assert_eq!(is_dev, env!("CARGO_PKG_VERSION").ends_with("-dev"));
        if is_dev {
            assert!(!base_version.ends_with("-dev"));
        }
    }

    #[test]
    fn test_ui_header_fits_window_with_dev_badge() {
        // Empirical egui render of worst-case header:
        // "[DEV 1d2c5b9-dirty]" on the left + "[CLIP: ACTIVE - CLICK TO RESET]" on the right
        let ctx = egui::Context::default();
        let telemetry = MeterTelemetry::default();
        telemetry.clip_latched.store(true, Ordering::Relaxed);
        let raw_input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(UI_WINDOW_WIDTH as f32, UI_WINDOW_HEIGHT as f32),
            )),
            ..Default::default()
        };

        let mut final_output = None;
        for _ in 0..2 {
            final_output = Some(ctx.run(raw_input.clone(), |ctx| {
                apply_omore_dark_theme(ctx);
                draw_header_bar(ctx, &telemetry, "0.2.1-dev", "1d2c5b9-dirty");
                egui::CentralPanel::default().show(ctx, |_| {});
            }));
        }
        let used = ctx.used_rect();
        assert!(
            used.width() <= UI_WINDOW_WIDTH as f32,
            "Rendered egui width ({}) with [DEV 1d2c5b9-dirty] + latched clip exceeds window width ({})",
            used.width(),
            UI_WINDOW_WIDTH
        );
        let max_inner_right = final_output
            .expect("egui frame output missing")
            .shapes
            .iter()
            .filter_map(|clipped| {
                let rect = clipped.shape.visual_bounding_rect();
                (rect.width() < UI_WINDOW_WIDTH as f32 - 1.0).then_some(rect.right())
            })
            .fold(0.0_f32, f32::max);
        assert!(
            max_inner_right <= UI_WINDOW_WIDTH as f32 - 6.0,
            "Inner widgets right ({}) with [DEV 1d2c5b9-dirty] + latched clip exceeds padded window width ({})",
            max_inner_right,
            UI_WINDOW_WIDTH as f32 - 6.0
        );
    }
}