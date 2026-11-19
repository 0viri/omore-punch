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
const DENORMAL_FLUSH_THRESHOLD: f32 = 1.0e-20;
const MIN_DETECTOR_PEAK: f32 = 1.0e-6;
const MIN_FLOOR_DB: f32 = -60.0;
const MAX_GR_METER_DB: f32 = 24.0;
const AUTO_SLOW_CHARGE_MS: f32 = 180.0;
const GAIN_MATCH_RMS_MS: f32 = 400.0;
const GAIN_MATCH_PRESENCE_MS: f32 = 50.0;
const GAIN_MATCH_SLEW_MS: f32 = 150.0;
const GAIN_MATCH_TOGGLE_SLEW_MS: f32 = 35.0;
const GAIN_MATCH_NOISE_FLOOR_POWER: f32 = 1.0e-6;
const GAIN_MATCH_MIN_DB: f32 = -24.0;
const GAIN_MATCH_MAX_DB: f32 = 24.0;
const BYPASS_RAMP_MS: f32 = 10.0;

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
fn flush_denormal(x: f32) -> f32 {
    if x.is_finite() && x.abs() > DENORMAL_FLUSH_THRESHOLD {
        x
    } else {
        0.0
    }
}

#[inline(always)]
pub fn sanitize_sample(x: f32) -> f32 {
    flush_denormal(x).clamp(-SAFETY_CLAMP_LIMIT, SAFETY_CLAMP_LIMIT)
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
// Release Mode Enumeration
// ----------------------------------------------------------------------------

#[derive(Enum, PartialEq, Eq, Clone, Copy, Debug)]
pub enum ReleaseMode {
    #[id = "manual"]
    #[name = "Manual"]
    Manual,

    #[id = "auto_tight"]
    #[name = "Auto Tight (50/600 ms)"]
    AutoTight,

    #[id = "auto_punch"]
    #[name = "Auto Punch (80/800 ms)"]
    AutoPunch,

    #[id = "auto_glue"]
    #[name = "Auto Glue (100/1200 ms)"]
    AutoGlue,
}

impl ReleaseMode {
    /// Returns `(fast_ms, slow_ms)` for dual-stage auto-release profiles, or `None` for manual.
    #[inline(always)]
    pub fn auto_time_constants_ms(self) -> Option<(f32, f32)> {
        match self {
            Self::Manual => None,
            Self::AutoTight => Some((50.0, 600.0)),
            Self::AutoPunch => Some((80.0, 800.0)),
            Self::AutoGlue => Some((100.0, 1200.0)),
        }
    }
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
    pub release_mode: ReleaseMode,
    pub sc_hpf_hz: f32,
    pub auto_makeup: bool,
    pub makeup_db: f32,
    pub eq_low_cut_hz: f32,
    pub eq_mid_cut_db: f32,
    pub eq_high_shelf_db: f32,
    pub clipper_mode: ClipperMode,
    pub ceiling_db: f32,
    pub mix_pct: f32,
    pub gain_match: bool,
    pub bypass: bool,
}

impl Default for DspConfig {
    fn default() -> Self {
        Self {
            input_db: 0.0,
            threshold_db: 0.0,
            ratio: 4.0,
            attack_ms: 3.0,
            release_ms: 80.0,
            release_mode: ReleaseMode::Manual,
            sc_hpf_hz: 20.0,
            auto_makeup: true,
            makeup_db: 0.0,
            eq_low_cut_hz: 20.0,
            eq_mid_cut_db: 0.0,
            eq_high_shelf_db: 0.0,
            clipper_mode: ClipperMode::SoftClip,
            ceiling_db: 0.0,
            mix_pct: 100.0,
            gain_match: false,
            bypass: false,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct SampleOutput {
    pub out_l: f32,
    pub out_r: f32,
    pub gr_db: f32,
    pub clipped: bool,
    pub gain_match_db: f32,
}

#[derive(Clone, Debug)]
pub struct StereoPunchDsp {
    sample_rate: f32,
    cached_attack_ms: f32,
    cached_release_ms: f32,
    cached_release_mode: ReleaseMode,
    auto_coeffs_initialized: bool,
    attack_coeff: f32,
    release_coeff: f32,
    auto_fast_release_coeff: f32,
    auto_slow_release_coeff: f32,
    auto_slow_charge_coeff: f32,
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
    gm_rms_coeff: f32,
    gm_presence_coeff: f32,
    gm_slew_coeff: f32,
    gm_off_slew_coeff: f32,
    gm_in_ms: f32,
    gm_post_ms: f32,
    gm_presence_ms: f32,
    pub gm_gain_db: f32,
    auto_slow_gr_db: f32,
    pub env_gr_db: f32,
    bypass_wet: f32,
    bypass_step: f32,
}

impl Default for StereoPunchDsp {
    fn default() -> Self {
        let mut dsp = Self {
            sample_rate: 48000.0,
            cached_attack_ms: -1.0,
            cached_release_ms: -1.0,
            cached_release_mode: ReleaseMode::Manual,
            auto_coeffs_initialized: false,
            attack_coeff: 0.0,
            release_coeff: 0.0,
            auto_fast_release_coeff: 0.0,
            auto_slow_release_coeff: 0.0,
            auto_slow_charge_coeff: 0.0,
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
            gm_rms_coeff: 0.0,
            gm_presence_coeff: 0.0,
            gm_slew_coeff: 0.0,
            gm_off_slew_coeff: 0.0,
            gm_in_ms: 0.0,
            gm_post_ms: 0.0,
            gm_presence_ms: 0.0,
            gm_gain_db: 0.0,
            auto_slow_gr_db: 0.0,
            env_gr_db: 0.0,
            bypass_wet: 1.0,
            bypass_step: 0.0,
        };
        dsp.update_ballistics(48000.0, 3.0, 80.0, ReleaseMode::Manual);
        dsp.update_sidechain_hpf(48000.0, 20.0);
        dsp.update_eq(48000.0, 20.0, 0.0, 0.0);
        dsp.update_gain_match_coeffs(48000.0);
        dsp.update_bypass_coeff(48000.0);
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
    fn exp_coeff_from_ms(time_ms: f32, sample_rate: f32, min_sec: f32) -> f32 {
        let time_sec = (time_ms * 0.001).max(min_sec);
        (-1.0 / (time_sec * sample_rate)).exp()
    }

    #[inline(always)]
    fn sanitize_sample_rate(&self, sample_rate: f32) -> f32 {
        if sample_rate.is_finite() {
            sample_rate.max(8000.0)
        } else if self.sample_rate.is_finite() {
            self.sample_rate.max(8000.0)
        } else {
            48000.0
        }
    }

    #[inline(always)]
    fn update_gain_match_coeffs(&mut self, sample_rate: f32) {
        let sr = self.sanitize_sample_rate(sample_rate);
        self.gm_rms_coeff = Self::exp_coeff_from_ms(GAIN_MATCH_RMS_MS, sr, 0.001);
        self.gm_presence_coeff = Self::exp_coeff_from_ms(GAIN_MATCH_PRESENCE_MS, sr, 0.001);
        self.gm_slew_coeff = Self::exp_coeff_from_ms(GAIN_MATCH_SLEW_MS, sr, 0.001);
        self.gm_off_slew_coeff = Self::exp_coeff_from_ms(GAIN_MATCH_TOGGLE_SLEW_MS, sr, 0.001);
    }

    #[inline(always)]
    fn update_bypass_coeff(&mut self, sample_rate: f32) {
        let sr = self.sanitize_sample_rate(sample_rate);
        let ramp_samples = (BYPASS_RAMP_MS * 0.001 * sr).max(1.0);
        self.bypass_step = 1.0 / ramp_samples;
    }

    #[inline(always)]
    fn apply_sample_rate_change(&mut self, sr: f32) {
        self.sample_rate = sr;
        self.cached_attack_ms = -1.0;
        self.cached_release_ms = -1.0;
        self.auto_coeffs_initialized = false;
        self.cached_sc_hpf_hz = -1.0;
        self.cached_eq_low_cut_hz = -1.0;
        self.cached_eq_mid_cut_db = -999.0;
        self.cached_eq_high_shelf_db = -999.0;
        self.auto_slow_gr_db = 0.0;
        self.update_gain_match_coeffs(sr);
        self.update_bypass_coeff(sr);
    }

    #[inline(always)]
    fn reset_transient_processing_state(&mut self) {
        self.env_gr_db = 0.0;
        self.auto_slow_gr_db = 0.0;
        self.sc_prev_in_l = 0.0;
        self.sc_prev_in_r = 0.0;
        self.sc_prev_out_l = 0.0;
        self.sc_prev_out_r = 0.0;
        self.eq_low_cut.reset();
        self.eq_mid_cut.reset();
        self.eq_high_shelf.reset();
    }

    #[inline(always)]
    pub fn sync_bypass_state(&mut self, bypass: bool) {
        self.bypass_wet = if bypass { 0.0 } else { 1.0 };
        if bypass {
            self.reset_transient_processing_state();
        }
    }

    #[inline(always)]
    pub fn reset(&mut self) {
        self.reset_transient_processing_state();
        self.gm_in_ms = 0.0;
        self.gm_post_ms = 0.0;
        self.gm_presence_ms = 0.0;
        self.gm_gain_db = 0.0;
        self.bypass_wet = 1.0;
    }

    #[inline(always)]
    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        let sr = self.sanitize_sample_rate(sample_rate);
        if (sr - self.sample_rate).abs() > 1e-3 {
            let att = if self.cached_attack_ms.is_finite() && self.cached_attack_ms > 0.0 {
                self.cached_attack_ms.clamp(0.1, 100.0)
            } else {
                3.0
            };
            let rel = if self.cached_release_ms.is_finite() && self.cached_release_ms > 0.0 {
                self.cached_release_ms.clamp(10.0, 500.0)
            } else {
                80.0
            };
            let rel_mode = self.cached_release_mode;
            let hpf = if self.cached_sc_hpf_hz.is_finite() && self.cached_sc_hpf_hz > 0.0 {
                self.cached_sc_hpf_hz.clamp(20.0, 250.0)
            } else {
                20.0
            };
            let eq_lc = if self.cached_eq_low_cut_hz.is_finite() && self.cached_eq_low_cut_hz > 0.0 {
                self.cached_eq_low_cut_hz.clamp(20.0, 160.0)
            } else {
                20.0
            };
            let eq_mc = if !self.cached_eq_mid_cut_db.is_finite()
                || self.cached_eq_mid_cut_db < -100.0
            {
                0.0
            } else {
                self.cached_eq_mid_cut_db.clamp(-12.0, 0.0)
            };
            let eq_hs = if !self.cached_eq_high_shelf_db.is_finite()
                || self.cached_eq_high_shelf_db < -100.0
            {
                0.0
            } else {
                self.cached_eq_high_shelf_db.clamp(-6.0, 12.0)
            };
            self.apply_sample_rate_change(sr);
            self.update_ballistics(sr, att, rel, rel_mode);
            self.update_sidechain_hpf(sr, hpf);
            self.update_eq(sr, eq_lc, eq_mc, eq_hs);
        }
    }

    #[inline(always)]
    pub fn update_ballistics(
        &mut self,
        sample_rate: f32,
        attack_ms: f32,
        release_ms: f32,
        release_mode: ReleaseMode,
    ) {
        let sr = self.sanitize_sample_rate(sample_rate);
        let att_ms = if attack_ms.is_finite() {
            attack_ms.clamp(0.1, 100.0)
        } else {
            3.0
        };
        let rel_ms = if release_ms.is_finite() {
            release_ms.clamp(10.0, 500.0)
        } else {
            80.0
        };
        if (sr - self.sample_rate).abs() > 1e-3 {
            self.apply_sample_rate_change(sr);
        }

        let att_changed = (att_ms - self.cached_attack_ms).abs() > 1e-5;
        let rel_changed = (rel_ms - self.cached_release_ms).abs() > 1e-5;
        let mode_changed =
            !self.auto_coeffs_initialized || release_mode != self.cached_release_mode;

        if att_changed {
            self.cached_attack_ms = att_ms;
            self.attack_coeff = Self::exp_coeff_from_ms(att_ms, sr, 0.0001);
        }

        if rel_changed {
            self.cached_release_ms = rel_ms;
            self.release_coeff = Self::exp_coeff_from_ms(rel_ms, sr, 0.001);
        }

        if mode_changed {
            self.cached_release_mode = release_mode;
            if !self.auto_coeffs_initialized {
                self.auto_slow_charge_coeff =
                    Self::exp_coeff_from_ms(AUTO_SLOW_CHARGE_MS, sr, 0.001);
            }
            let (fast_ms, slow_ms) = release_mode.auto_time_constants_ms().unwrap_or((80.0, 800.0));
            self.auto_fast_release_coeff = Self::exp_coeff_from_ms(fast_ms, sr, 0.001);
            self.auto_slow_release_coeff = Self::exp_coeff_from_ms(slow_ms, sr, 0.001);
            self.auto_coeffs_initialized = true;
        }
    }

    #[inline(always)]
    pub fn update_sidechain_hpf(&mut self, sample_rate: f32, sc_hpf_hz: f32) {
        let sr = self.sanitize_sample_rate(sample_rate);
        let clamped_hz = if sc_hpf_hz.is_finite() {
            sc_hpf_hz.clamp(20.0, 250.0)
        } else {
            20.0
        };
        if (sr - self.sample_rate).abs() > 1e-3 {
            self.apply_sample_rate_change(sr);
        }

        if (clamped_hz - self.cached_sc_hpf_hz).abs() > 1e-4 {
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
        let sr = self.sanitize_sample_rate(sample_rate);
        if (sr - self.sample_rate).abs() > 1e-3 {
            self.apply_sample_rate_change(sr);
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

        // Advance linear soft-bypass ramp (10 ms) with finite guard
        if !self.bypass_wet.is_finite() {
            self.bypass_wet = if cfg.bypass { 0.0 } else { 1.0 };
        }
        let target_wet = if cfg.bypass { 0.0 } else { 1.0 };
        if self.bypass_wet < target_wet {
            self.bypass_wet = (self.bypass_wet + self.bypass_step).min(1.0);
        } else if self.bypass_wet > target_wet {
            self.bypass_wet = (self.bypass_wet - self.bypass_step).max(0.0);
        }

        // Settled bypass: bit-transparent sanitized passthrough; keep Gain Match state frozen
        if cfg.bypass && self.bypass_wet <= 0.0 {
            self.reset_transient_processing_state();
            return SampleOutput {
                out_l: dry_l,
                out_r: dry_r,
                gr_db: 0.0,
                clipped: false,
                gain_match_db: self.gm_gain_db,
            };
        }

        // Update envelope, sidechain filter and EQ coefficients only if parameters changed
        self.update_ballistics(
            self.sample_rate,
            cfg.attack_ms,
            cfg.release_ms,
            cfg.release_mode,
        );
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

            let clean_det_l = flush_denormal(raw_det_l);
            let clean_det_r = flush_denormal(raw_det_r);

            self.sc_prev_in_l = flush_denormal(driven_l);
            self.sc_prev_in_r = flush_denormal(driven_r);
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

        // 4. Omore Punch Ballistics (Manual or Dual-Stage Auto-Release)
        let slow_coeff = if target_gr_db > self.auto_slow_gr_db {
            self.auto_slow_charge_coeff
        } else {
            self.auto_slow_release_coeff
        };
        self.auto_slow_gr_db =
            slow_coeff * self.auto_slow_gr_db + (1.0 - slow_coeff) * target_gr_db;
        if !self.auto_slow_gr_db.is_finite() || self.auto_slow_gr_db < 1.0e-20 {
            self.auto_slow_gr_db = 0.0;
        }

        if target_gr_db > self.env_gr_db {
            self.env_gr_db =
                self.attack_coeff * self.env_gr_db + (1.0 - self.attack_coeff) * target_gr_db;
        } else if cfg.release_mode == ReleaseMode::Manual {
            self.env_gr_db =
                self.release_coeff * self.env_gr_db + (1.0 - self.release_coeff) * target_gr_db;
        } else {
            let fast_next = self.auto_fast_release_coeff * self.env_gr_db
                + (1.0 - self.auto_fast_release_coeff) * target_gr_db;
            self.env_gr_db = fast_next.max(self.auto_slow_gr_db);
        }

        if !self.env_gr_db.is_finite() || self.env_gr_db < 1.0e-20 {
            self.env_gr_db = 0.0;
        }
        self.auto_slow_gr_db = self.auto_slow_gr_db.min(self.env_gr_db);

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

        // 8. Post-Processing Gain Match (Feed-Forward Stereo-Linked RMS)
        // While ramping into bypass, freeze RMS accumulators and trim for seamless A/B return
        if !cfg.bypass {
            let in_sq = 0.5 * (dry_l * dry_l + dry_r * dry_r);
            let post_sq = 0.5 * (mixed_l * mixed_l + mixed_r * mixed_r);

            // Fast presence detector freezes (Hold) RMS & trim below -60 dBFS
            let next_presence = self.gm_presence_coeff * self.gm_presence_ms
                + (1.0 - self.gm_presence_coeff) * in_sq;
            self.gm_presence_ms = if next_presence.is_finite() && next_presence > 1.0e-20 {
                next_presence
            } else {
                0.0
            };

            if self.gm_presence_ms >= GAIN_MATCH_NOISE_FLOOR_POWER {
                let next_in_ms =
                    self.gm_rms_coeff * self.gm_in_ms + (1.0 - self.gm_rms_coeff) * in_sq;
                let next_post_ms =
                    self.gm_rms_coeff * self.gm_post_ms + (1.0 - self.gm_rms_coeff) * post_sq;

                self.gm_in_ms = if next_in_ms.is_finite() && next_in_ms > 1.0e-20 {
                    next_in_ms
                } else {
                    0.0
                };
                self.gm_post_ms = if next_post_ms.is_finite() && next_post_ms > 1.0e-20 {
                    next_post_ms
                } else {
                    0.0
                };

                if cfg.gain_match {
                    let ratio_power =
                        self.gm_in_ms.max(1.0e-12) / self.gm_post_ms.max(1.0e-12);
                    let target_db = (10.0 * ratio_power.log10())
                        .clamp(GAIN_MATCH_MIN_DB, GAIN_MATCH_MAX_DB);
                    self.gm_gain_db = self.gm_slew_coeff * self.gm_gain_db
                        + (1.0 - self.gm_slew_coeff) * target_db;
                }
            }

            if !cfg.gain_match {
                self.gm_gain_db *= self.gm_off_slew_coeff;
                if !self.gm_gain_db.is_finite() || self.gm_gain_db.abs() < 1.0e-5 {
                    self.gm_gain_db = 0.0;
                }
            } else if !self.gm_gain_db.is_finite() || self.gm_gain_db.abs() < 1.0e-20 {
                self.gm_gain_db = 0.0;
            } else {
                self.gm_gain_db = self.gm_gain_db.clamp(GAIN_MATCH_MIN_DB, GAIN_MATCH_MAX_DB);
            }
        }

        let (matched_l, matched_r, gain_match_clipped) =
            if self.gm_gain_db.is_finite() && self.gm_gain_db != 0.0 {
                let gm_lin = db_to_lin(self.gm_gain_db);
                let ml = mixed_l * gm_lin;
                let mr = mixed_r * gm_lin;
                let over = !cfg.bypass && cfg.gain_match && (ml.abs() > 1.0 || mr.abs() > 1.0);
                (ml, mr, over)
            } else {
                (mixed_l, mixed_r, false)
            };

        // 9. Soft-Bypass Crossfade + Final Safety Sanitization & Clamp [-4.0, 4.0]
        let (final_l, final_r) = if self.bypass_wet >= 1.0 {
            (matched_l, matched_r)
        } else {
            let dry_weight = 1.0 - self.bypass_wet;
            (
                dry_l * dry_weight + matched_l * self.bypass_wet,
                dry_r * dry_weight + matched_r * self.bypass_wet,
            )
        };
        let out_l = sanitize_sample(final_l);
        let out_r = sanitize_sample(final_r);

        SampleOutput {
            out_l,
            out_r,
            gr_db: if cfg.bypass { 0.0 } else { self.env_gr_db },
            clipped: if cfg.bypass {
                false
            } else {
                clipped || gain_match_clipped
            },
            gain_match_db: self.gm_gain_db,
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
    /// Current applied Gain Match compensation trim in dB
    pub gain_match_db: AtomicU32,
    /// Clipper activity indicator latch
    pub clip_latched: AtomicBool,
}

impl MeterTelemetry {
    pub fn reset_peaks(&self) {
        store_f32_relaxed(&self.max_gr_db, 0.0);
        self.clip_latched.store(false, Ordering::Relaxed);
    }

    /// Returns the instantaneous gain adjustment in dB for DAW host metering
    /// (`0.0` when idle or non-finite, negative down to `-MAX_GR_METER_DB` when compressing).
    #[inline(always)]
    pub fn host_gain_reduction_db(&self) -> f64 {
        let raw = load_f32_relaxed(&self.current_gr_db);
        if !raw.is_finite() || raw <= 0.0 {
            0.0
        } else {
            -(raw.min(MAX_GR_METER_DB) as f64)
        }
    }

    /// Clears the instantaneous gain reduction telemetry back to `0.0`.
    #[inline(always)]
    pub fn clear_gain_reduction(&self) {
        store_f32_relaxed(&self.current_gr_db, 0.0);
    }
}

// ----------------------------------------------------------------------------
// Plugin Parameters Definition
// ----------------------------------------------------------------------------

#[derive(Params)]
pub struct OmorePunchParams {
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

    #[id = "release_mode"]
    pub release_mode: EnumParam<ReleaseMode>,

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

    #[id = "gain_match"]
    pub gain_match: BoolParam,

    #[id = "bypass"]
    pub bypass: BoolParam,
}

/// Builds the `Release` parameter with a configurable default so tests can exercise wide readouts.
fn make_release_ms_param(default_ms: f32) -> FloatParam {
    FloatParam::new(
        "Release",
        default_ms,
        FloatRange::Skewed {
            min: 10.0,
            max: 500.0,
            factor: FloatRange::skew_factor(-1.2),
        },
    )
    .with_smoother(SmoothingStyle::Linear(5.0))
    .with_unit(" ms")
    .with_value_to_string(formatters::v2s_f32_rounded(1))
}

impl Default for OmorePunchParams {
    fn default() -> Self {
        Self {
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

            release_ms: make_release_ms_param(80.0),

            release_mode: EnumParam::new("Release Mode", ReleaseMode::Manual),

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

            gain_match: BoolParam::new("Gain Match", false),

            bypass: BoolParam::new("Bypass", false).make_bypass(),
        }
    }
}

// ----------------------------------------------------------------------------
// Preset & State Sanitization (Backward Compatibility + Range Clamping)
// ----------------------------------------------------------------------------

struct FloatParamSpec {
    id: &'static str,
    default: f32,
    min: f32,
    max: f32,
}

const FLOAT_PARAM_SPECS: &[FloatParamSpec] = &[
    FloatParamSpec {
        id: "input_db",
        default: 0.0,
        min: -12.0,
        max: 12.0,
    },
    FloatParamSpec {
        id: "threshold_db",
        default: 0.0,
        min: -40.0,
        max: 0.0,
    },
    FloatParamSpec {
        id: "ratio",
        default: 4.0,
        min: 1.0,
        max: 20.0,
    },
    FloatParamSpec {
        id: "attack_ms",
        default: 3.0,
        min: 0.1,
        max: 100.0,
    },
    FloatParamSpec {
        id: "release_ms",
        default: 80.0,
        min: 10.0,
        max: 500.0,
    },
    FloatParamSpec {
        id: "sc_hpf_hz",
        default: 20.0,
        min: 20.0,
        max: 250.0,
    },
    FloatParamSpec {
        id: "makeup_db",
        default: 0.0,
        min: -12.0,
        max: 24.0,
    },
    FloatParamSpec {
        id: "eq_low_cut_hz",
        default: 20.0,
        min: 20.0,
        max: 160.0,
    },
    FloatParamSpec {
        id: "eq_mid_cut_db",
        default: 0.0,
        min: -12.0,
        max: 0.0,
    },
    FloatParamSpec {
        id: "eq_high_shelf_db",
        default: 0.0,
        min: -6.0,
        max: 12.0,
    },
    FloatParamSpec {
        id: "ceiling_db",
        default: 0.0,
        min: -6.0,
        max: 0.0,
    },
    FloatParamSpec {
        id: "mix_pct",
        default: 100.0,
        min: 0.0,
        max: 100.0,
    },
];

const BOOL_PARAM_SPECS: &[(&str, bool)] = &[
    ("auto_makeup", true),
    ("gain_match", false),
    ("bypass", false),
];

struct EnumParamSpec {
    id: &'static str,
    default_id: &'static str,
    variant_ids: &'static [&'static str],
}

const ENUM_PARAM_SPECS: &[EnumParamSpec] = &[
    EnumParamSpec {
        id: "release_mode",
        default_id: "manual",
        variant_ids: &["manual", "auto_tight", "auto_punch", "auto_glue"],
    },
    EnumParamSpec {
        id: "clipper_mode",
        default_id: "soft_clip",
        variant_ids: &["soft_clip", "hard_clip", "bypass_clip"],
    },
];

/// Normalizes a deserialized `PluginState` before `nih_plug` applies it so partial,
/// legacy, or malformed Reaper presets fill missing defaults, clamp floats, validate
/// enums, and drop unknown keys without retaining stale parameters.
pub fn sanitize_plugin_state(state: &mut PluginState) {
    use nih_plug::wrapper::state::ParamValue;
    use std::collections::BTreeMap;

    let mut clean_params = BTreeMap::new();

    for spec in FLOAT_PARAM_SPECS {
        let value = match state.params.get(spec.id) {
            Some(ParamValue::F32(v)) if v.is_finite() => v.clamp(spec.min, spec.max),
            Some(ParamValue::I32(v)) => (*v as f32).clamp(spec.min, spec.max),
            _ => spec.default,
        };
        clean_params.insert(spec.id.to_string(), ParamValue::F32(value));
    }

    for &(id, default_val) in BOOL_PARAM_SPECS {
        let value = match state.params.get(id) {
            Some(ParamValue::Bool(b)) => *b,
            _ => default_val,
        };
        clean_params.insert(id.to_string(), ParamValue::Bool(value));
    }

    for spec in ENUM_PARAM_SPECS {
        let variant_id = match state.params.get(spec.id) {
            Some(ParamValue::String(s)) if spec.variant_ids.contains(&s.as_str()) => s.as_str(),
            Some(ParamValue::I32(idx)) if *idx >= 0 && (*idx as usize) < spec.variant_ids.len() => {
                spec.variant_ids[*idx as usize]
            }
            _ => spec.default_id,
        };
        clean_params.insert(spec.id.to_string(), ParamValue::String(variant_id.to_string()));
    }

    state.params = clean_params;
    state.fields.remove("editor-state");
}

// ----------------------------------------------------------------------------
// Main Plugin Struct
// ----------------------------------------------------------------------------

thread_local! {
    static LAST_CREATED_TELEMETRY: std::cell::RefCell<Option<Arc<MeterTelemetry>>> =
        const { std::cell::RefCell::new(None) };
}

fn take_last_created_telemetry() -> Option<Arc<MeterTelemetry>> {
    LAST_CREATED_TELEMETRY.with(|slot| slot.borrow_mut().take())
}

#[cfg(test)]
thread_local! {
    static LAST_INSPECTABLE_TELEMETRY: std::cell::RefCell<Option<Arc<MeterTelemetry>>> =
        const { std::cell::RefCell::new(None) };
}

#[cfg(test)]
fn take_last_inspectable_telemetry() -> Option<Arc<MeterTelemetry>> {
    LAST_INSPECTABLE_TELEMETRY.with(|slot| slot.borrow_mut().take())
}

pub struct OmorePunch {
    params: Arc<OmorePunchParams>,
    editor_state: Arc<EguiState>,
    dsp: StereoPunchDsp,
    telemetry: Arc<MeterTelemetry>,
}

impl Default for OmorePunch {
    fn default() -> Self {
        let telemetry = Arc::new(MeterTelemetry::default());
        LAST_CREATED_TELEMETRY.with(|slot| {
            *slot.borrow_mut() = Some(telemetry.clone());
        });
        #[cfg(test)]
        LAST_INSPECTABLE_TELEMETRY.with(|slot| {
            *slot.borrow_mut() = Some(telemetry.clone());
        });
        Self {
            params: Arc::new(OmorePunchParams::default()),
            editor_state: EguiState::from_size(UI_WINDOW_WIDTH, UI_WINDOW_HEIGHT),
            dsp: StereoPunchDsp::default(),
            telemetry,
        }
    }
}

impl OmorePunch {
    /// Returns the instantaneous gain adjustment in dB (negative when attenuating,
    /// e.g. -4.8) for CLAP `clap.gain-adjustment-metering/0` and VST3 `IGainReductionInfo`.
    #[inline(always)]
    pub fn current_gain_adjustment_db(&self) -> f64 {
        self.telemetry.host_gain_reduction_db()
    }
}

impl Plugin for OmorePunch {
    const NAME: &'static str = "Omore Punch";
    const VENDOR: &'static str = "Omore";
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
        self.dsp.sync_bypass_state(self.params.bypass.value());
        self.telemetry.clear_gain_reduction();
        self.telemetry.reset_peaks();
        true
    }

    fn reset(&mut self) {
        self.dsp.reset();
        self.dsp.sync_bypass_state(self.params.bypass.value());
        self.telemetry.clear_gain_reduction();
    }

    fn filter_state(state: &mut PluginState) {
        sanitize_plugin_state(state);
    }

    fn editor(&mut self, _async_executor: AsyncExecutor<Self>) -> Option<Box<dyn Editor>> {
        let params = self.params.clone();
        let telemetry = self.telemetry.clone();

        create_egui_editor(
            self.editor_state.clone(),
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
        let mut block_active = false;

        let auto_makeup = self.params.auto_makeup.value();
        let release_mode = self.params.release_mode.value();
        let clipper_mode = self.params.clipper_mode.value();
        let gain_match = self.params.gain_match.value();
        let mut last_bypass = self.params.bypass.value();

        for mut channel_samples in buffer.iter_samples() {
            if channel_samples.len() < 2 {
                continue;
            }

            let bypass = self.params.bypass.value();
            last_bypass = bypass;
            if !bypass {
                block_active = true;
            }

            let cfg = DspConfig {
                input_db: self.params.input_db.smoothed.next(),
                threshold_db: self.params.threshold_db.smoothed.next(),
                ratio: self.params.ratio.smoothed.next(),
                attack_ms: self.params.attack_ms.smoothed.next(),
                release_ms: self.params.release_ms.smoothed.next(),
                release_mode,
                sc_hpf_hz: self.params.sc_hpf_hz.smoothed.next(),
                auto_makeup,
                makeup_db: self.params.makeup_db.smoothed.next(),
                eq_low_cut_hz: self.params.eq_low_cut_hz.smoothed.next(),
                eq_mid_cut_db: self.params.eq_mid_cut_db.smoothed.next(),
                eq_high_shelf_db: self.params.eq_high_shelf_db.smoothed.next(),
                clipper_mode,
                ceiling_db: self.params.ceiling_db.smoothed.next(),
                mix_pct: self.params.mix_pct.smoothed.next(),
                gain_match,
                bypass,
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
            if !bypass {
                block_max_gr = block_max_gr.max(out.gr_db);
                if out.clipped {
                    block_clipped = true;
                }
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
        let reported_gr = if block_active && !last_bypass {
            self.dsp.env_gr_db
        } else {
            0.0
        };
        store_f32_relaxed(&self.telemetry.current_gr_db, reported_gr);
        store_f32_relaxed(&self.telemetry.gain_match_db, self.dsp.gm_gain_db);

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

const UI_WINDOW_WIDTH: u32 = 900;
const UI_WINDOW_HEIGHT: u32 = 650;
const UI_LEFT_COL_WIDTH: f32 = 510.0;
const UI_RIGHT_COL_WIDTH: f32 = 352.0;
const UI_COL_GAP: f32 = 14.0;
const UI_KNOB_COLS_MAX: usize = 4;
const UI_KNOB_CELL_WIDTH: f32 = 110.0;
const UI_KNOB_CELL_HEIGHT: f32 = 94.0;
const UI_KNOB_DIAMETER: f32 = 44.0;
const UI_KNOB_GRID_SPACING_X: f32 = 8.0;
const UI_KNOB_GRID_SPACING_Y: f32 = 4.0;
const UI_KNOB_STEP_BTN_WIDTH: f32 = 22.0;
const UI_KNOB_RESET_BTN_WIDTH: f32 = 46.0;
const UI_KNOB_BTN_HEIGHT: f32 = 16.0;
const UI_METER_HEIGHT: f32 = 384.0;

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
    draw_ssl_knob_cell_ex(
        ui,
        setter,
        param,
        label,
        step,
        min_val,
        max_val,
        color_family,
        false,
    );
}

#[allow(clippy::too_many_arguments)]
fn draw_ssl_knob_cell_ex(
    ui: &mut egui::Ui,
    setter: &ParamSetter,
    param: &FloatParam,
    label: &str,
    step: f32,
    min_val: f32,
    max_val: f32,
    color_family: SslKnobColor,
    auto_override: bool,
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

            let label_color = if auto_override {
                egui::Color32::from_rgb(135, 142, 154)
            } else {
                egui::Color32::from_rgb(220, 224, 232)
            };
            ui.label(
                egui::RichText::new(label)
                    .monospace()
                    .strong()
                    .size(11.0)
                    .color(label_color),
            );

            let (knob_rect, resp) = ui.allocate_exact_size(
                egui::vec2(UI_KNOB_DIAMETER, UI_KNOB_DIAMETER),
                egui::Sense::click_and_drag(),
            );
            let hover_msg = if auto_override {
                "Overridden by Auto-Release (drag or step to pre-stage Manual Release, double-click to reset)"
            } else {
                "Drag vertically to adjust (hold Shift for fine control, double-click to reset)"
            };
            let resp = resp.on_hover_text(hover_msg);

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
                    if auto_override {
                        egui::Color32::from_rgb(108, 114, 126)
                    } else {
                        egui::Color32::from_rgb(245, 185, 45)
                    }
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

            // Inner SSL colored cap (dimmed when overridden by Auto-Release)
            let cap_r = outer_r - 8.5;
            let active_or_hovered = resp.hovered() || resp.dragged();
            let base_cap = color_family.cap_color(active_or_hovered);
            let cap_color = if auto_override {
                egui::Color32::from_rgb(
                    base_cap.r() / 3 + 38,
                    base_cap.g() / 3 + 38,
                    base_cap.b() / 3 + 40,
                )
            } else {
                base_cap
            };
            painter.circle_filled(center, cap_r, cap_color);
            painter.circle_stroke(
                center,
                cap_r,
                egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(25, 28, 34)),
            );

            // Angular pointer line
            let val_angle = (-135.0_f32 + norm * 270.0_f32).to_radians();
            let val_dir = egui::vec2(val_angle.sin(), -val_angle.cos());
            let pointer_color = if auto_override {
                egui::Color32::from_rgb(155, 160, 170)
            } else {
                egui::Color32::from_rgb(250, 250, 245)
            };
            painter.line_segment(
                [center + val_dir * 2.5, center + val_dir * (skirt_r - 1.5)],
                egui::Stroke::new(2.2_f32, pointer_color),
            );
            painter.circle_filled(center, 2.0, egui::Color32::from_rgb(25, 28, 34));

            if auto_override {
                let auto_readout = format!("AUTO ({})", formatted_val);
                ui.label(
                    egui::RichText::new(auto_readout)
                        .monospace()
                        .size(10.0)
                        .color(egui::Color32::from_rgb(150, 156, 168)),
                );
            } else {
                ui.label(
                    egui::RichText::new(formatted_val)
                        .monospace()
                        .size(11.0)
                        .color(egui::Color32::from_rgb(245, 185, 45)),
                );
            }

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
    setter: &ParamSetter,
    params: &OmorePunchParams,
    telemetry: &MeterTelemetry,
    raw_version: &str,
    git_tag: &str,
) {
    let is_clipped = telemetry.clip_latched.load(Ordering::Relaxed);
    let is_bypassed = params.bypass.value();
    let (base_version, is_dev) = parse_display_version(raw_version);
    let header_amber = egui::Color32::from_rgb(245, 185, 35);
    let header_text = |text: String| {
        egui::RichText::new(text)
            .monospace()
            .strong()
            .size(12.0)
            .color(header_amber)
    };

    egui::TopBottomPanel::top("omore_punch_header").show(egui_ctx, |ui| {
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;

            if is_dev {
                ui.label(header_text(format!("OMORE PUNCH v{}", base_version)));
                egui::Frame::new()
                    .fill(egui::Color32::from_rgb(28, 78, 120))
                    .stroke(egui::Stroke::new(
                        1.0_f32,
                        egui::Color32::from_rgb(70, 155, 220),
                    ))
                    .corner_radius(3.0)
                    .inner_margin(egui::Margin::symmetric(4, 1))
                    .show(ui, |ui| {
                        ui.label(
                            egui::RichText::new(format_dev_badge_label(git_tag))
                                .monospace()
                                .strong()
                                .size(10.5)
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
                ui.spacing_mut().item_spacing.x = 4.0;
                ui.spacing_mut().button_padding = egui::vec2(5.0, 2.0);

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
                                .size(11.0)
                                .color(egui::Color32::WHITE),
                        )
                        .fill(clip_color),
                    )
                    .clicked()
                {
                    telemetry.clip_latched.store(false, Ordering::Relaxed);
                }

                let (bypass_text, bypass_fill, bypass_text_color, bypass_stroke) = if is_bypassed {
                    (
                        "[BYPASS: ON]",
                        egui::Color32::from_rgb(230, 150, 25),
                        egui::Color32::from_rgb(16, 17, 22),
                        egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(255, 220, 105)),
                    )
                } else {
                    (
                        "[BYPASS: OFF]",
                        egui::Color32::from_rgb(40, 44, 54),
                        egui::Color32::from_rgb(215, 220, 230),
                        egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(72, 78, 94)),
                    )
                };
                if ui
                    .add(
                        egui::Button::new(
                            egui::RichText::new(bypass_text)
                                .monospace()
                                .strong()
                                .size(11.0)
                                .color(bypass_text_color),
                        )
                        .fill(bypass_fill)
                        .stroke(bypass_stroke),
                    )
                    .on_hover_text("Soft-bypass all audio processing for A/B comparison")
                    .clicked()
                {
                    setter.begin_set_parameter(&params.bypass);
                    setter.set_parameter(&params.bypass, !is_bypassed);
                    setter.end_set_parameter(&params.bypass);
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

    let is_bypassed = params.bypass.value();
    let in_l = load_f32_relaxed(&telemetry.in_peak_l);
    let in_r = load_f32_relaxed(&telemetry.in_peak_r);
    let out_l = load_f32_relaxed(&telemetry.out_peak_l);
    let out_r = load_f32_relaxed(&telemetry.out_peak_r);
    let current_gr_db = load_f32_relaxed(&telemetry.current_gr_db);
    let max_gr_db = load_f32_relaxed(&telemetry.max_gr_db);
    let gain_match_db_raw = load_f32_relaxed(&telemetry.gain_match_db);
    let gain_match_db_display =
        if !gain_match_db_raw.is_finite() || gain_match_db_raw.abs() < 0.05 {
            0.0
        } else {
            gain_match_db_raw
        };

    draw_header_bar(
        egui_ctx,
        setter,
        params,
        telemetry,
        env!("CARGO_PKG_VERSION"),
        option_env!("OMORE_GIT_TAG").unwrap_or(""),
    );

    egui::CentralPanel::default().show(egui_ctx, |ui| {
        let panel_height = ui.available_height();
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.horizontal_top(|ui| {
                    ui.spacing_mut().item_spacing.x = UI_COL_GAP;

                    // ================================================================
                    // LEFT COLUMN: COMPRESSOR, CLIPPER, PARALLEL MIX & DRUM BUS EQ
                    // ================================================================
                    ui.allocate_ui_with_layout(
                        egui::vec2(UI_LEFT_COL_WIDTH, panel_height),
                egui::Layout::top_down(egui::Align::Min),
                |left_col| {
                    left_col.set_width(UI_LEFT_COL_WIDTH);
                    if is_bypassed {
                        left_col.set_opacity(0.55);
                    }

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

                        let current_rel_mode = params.release_mode.value();
                        let is_auto_release = current_rel_mode != ReleaseMode::Manual;

                        for (row_idx, chunk) in comp_knobs.chunks(UI_KNOB_COLS_MAX).enumerate() {
                            if row_idx > 0 {
                                ui.add_space(UI_KNOB_GRID_SPACING_Y);
                            }
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = UI_KNOB_GRID_SPACING_X;
                                for &(param, label, step, min_val, max_val, color_family) in chunk {
                                    let auto_override =
                                        is_auto_release && std::ptr::eq(param, &params.release_ms);
                                    draw_ssl_knob_cell_ex(
                                        ui,
                                        setter,
                                        param,
                                        label,
                                        step,
                                        min_val,
                                        max_val,
                                        color_family,
                                        auto_override,
                                    );
                                }
                            });
                        }

                        ui.add_space(2.0);
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new("Release Mode:")
                                    .monospace()
                                    .strong(),
                            );
                            for (mode, text, hover) in [
                                (
                                    ReleaseMode::Manual,
                                    "[Manual]",
                                    "Use manual Release knob setting",
                                ),
                                (
                                    ReleaseMode::AutoTight,
                                    "[Auto Tight]",
                                    "Dual-stage Auto-Release: ~50 ms fast / ~600 ms slow",
                                ),
                                (
                                    ReleaseMode::AutoPunch,
                                    "[Auto Punch]",
                                    "Dual-stage Auto-Release: ~80 ms fast / ~800 ms slow",
                                ),
                                (
                                    ReleaseMode::AutoGlue,
                                    "[Auto Glue]",
                                    "Dual-stage Auto-Release: ~100 ms fast / ~1200 ms slow",
                                ),
                            ] {
                                let active = current_rel_mode == mode;
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
                                    .on_hover_text(hover)
                                    .clicked()
                                {
                                    setter.begin_set_parameter(&params.release_mode);
                                    setter.set_parameter(&params.release_mode, mode);
                                    setter.end_set_parameter(&params.release_mode);
                                }
                            }
                        });

                        ui.add_space(2.0);
                        ui.horizontal(|ui| {
                            if ui
                                .button(
                                    egui::RichText::new("[Punch 3ms / 80ms]")
                                        .monospace()
                                        .strong(),
                                )
                                .on_hover_text(
                                    "Set Omore Sampler signature Drum Bus Attack (3.0 ms), Manual Release (80.0 ms), and Release Mode to Manual",
                                )
                                .clicked()
                            {
                                setter.begin_set_parameter(&params.attack_ms);
                                setter.set_parameter(&params.attack_ms, 3.0);
                                setter.end_set_parameter(&params.attack_ms);

                                setter.begin_set_parameter(&params.release_ms);
                                setter.set_parameter(&params.release_ms, 80.0);
                                setter.end_set_parameter(&params.release_ms);

                                setter.begin_set_parameter(&params.release_mode);
                                setter.set_parameter(&params.release_mode, ReleaseMode::Manual);
                                setter.end_set_parameter(&params.release_mode);
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
                egui::vec2(UI_RIGHT_COL_WIDTH, panel_height),
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

                        ui.add_space(4.0);
                        ui.scope(|ui| {
                            if is_bypassed {
                                ui.set_opacity(0.55);
                            }
                            ui.horizontal(|ui| {
                                let gain_match_on = params.gain_match.value();
                                let gm_label = if gain_match_on {
                                    "[Gain Match: ON]"
                                } else {
                                    "[Gain Match: OFF]"
                                };
                                let gm_fill = if gain_match_on {
                                    egui::Color32::from_rgb(195, 125, 20)
                                } else {
                                    egui::Color32::from_rgb(45, 48, 58)
                                };
                                if ui
                                    .add_sized(
                                        [158.0, 24.0],
                                        egui::Button::new(
                                            egui::RichText::new(gm_label).monospace().strong(),
                                        )
                                        .fill(gm_fill),
                                    )
                                    .on_hover_text(
                                        "Match post-processing output RMS volume to raw input level",
                                    )
                                    .clicked()
                                {
                                    setter.begin_set_parameter(&params.gain_match);
                                    setter.set_parameter(&params.gain_match, !gain_match_on);
                                    setter.end_set_parameter(&params.gain_match);
                                }

                                ui.add_space(6.0);
                                let trim_color = if gain_match_on {
                                    egui::Color32::from_rgb(245, 185, 35)
                                } else {
                                    egui::Color32::from_rgb(145, 150, 162)
                                };
                                ui.label(
                                    egui::RichText::new(format!(
                                        "Trim: {:+.1} dB",
                                        gain_match_db_display
                                    ))
                                    .monospace()
                                    .strong()
                                    .size(13.5)
                                    .color(trim_color),
                                );
                            });
                        });

                        ui.add_space(6.0);

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

mod clap {
    use super::{take_last_created_telemetry, MeterTelemetry, OmorePunch};
    use nih_plug::wrapper::clap::{
        clap_host, clap_plugin, clap_plugin_descriptor, clap_plugin_entry, clap_plugin_factory,
        PluginDescriptor, Wrapper as ClapWrapper, CLAP_PLUGIN_FACTORY_ID, CLAP_VERSION,
    };
    use nih_plug::wrapper::setup_logger;
    use std::ffi::{c_char, c_void, CStr};
    use std::sync::atomic::{AtomicBool, AtomicPtr, Ordering};
    use std::sync::{Arc, OnceLock};

    const CLAP_EXT_GAIN_ADJUSTMENT_METERING: &[u8] = b"clap.gain-adjustment-metering/0\0";
    const CLAP_EXT_STATE: &[u8] = b"clap.state\0";
    const CLAP_EXT_PARAMS: &[u8] = b"clap.params\0";
    const CLAP_PARAM_RESCAN_VALUES: u32 = 1 << 0;

    #[repr(C)]
    pub struct ClapPluginGainAdjustmentMetering {
        pub get: Option<unsafe extern "C" fn(plugin: *const clap_plugin) -> f64>,
    }

    #[repr(C)]
    pub struct ClapPluginState {
        pub save: Option<
            unsafe extern "C" fn(plugin: *const clap_plugin, stream: *const c_void) -> bool,
        >,
        pub load: Option<
            unsafe extern "C" fn(plugin: *const clap_plugin, stream: *const c_void) -> bool,
        >,
    }

    #[repr(C)]
    pub struct ClapHostParams {
        pub rescan: Option<unsafe extern "C" fn(host: *const clap_host, flags: u32)>,
        pub clear: Option<unsafe extern "C" fn(host: *const clap_host, param_id: u32, flags: u32)>,
        pub request_flush: Option<unsafe extern "C" fn(host: *const clap_host)>,
    }

    static CLAP_GAIN_ADJUSTMENT_METERING: ClapPluginGainAdjustmentMetering =
        ClapPluginGainAdjustmentMetering {
            get: Some(clap_gain_adjustment_get),
        };

    static CLAP_STATE: ClapPluginState = ClapPluginState {
        save: Some(clap_state_save),
        load: Some(clap_state_load),
    };

    #[repr(C)]
    struct ClapGainReductionBox {
        // Must remain the first field so `*const clap_plugin` casts cleanly to `*mut Self`.
        plugin: clap_plugin,
        host: *const clap_host,
        inner_destroy: Option<unsafe extern "C" fn(plugin: *const clap_plugin)>,
        inner_deactivate: Option<unsafe extern "C" fn(plugin: *const clap_plugin)>,
        inner_stop_processing: Option<unsafe extern "C" fn(plugin: *const clap_plugin)>,
        inner_reset: Option<unsafe extern "C" fn(plugin: *const clap_plugin)>,
        inner_get_extension: Option<
            unsafe extern "C" fn(plugin: *const clap_plugin, id: *const c_char) -> *const c_void,
        >,
        inner_state: AtomicPtr<ClapPluginState>,
        rescanning_state: AtomicBool,
        telemetry: Arc<MeterTelemetry>,
    }

    fn plugin_descriptor() -> &'static PluginDescriptor {
        static DESCRIPTOR: OnceLock<PluginDescriptor> = OnceLock::new();
        DESCRIPTOR.get_or_init(PluginDescriptor::for_plugin::<OmorePunch>)
    }

    #[cfg(test)]
    pub unsafe fn test_telemetry(plugin: *const clap_plugin) -> Arc<MeterTelemetry> {
        let boxed = &*(plugin as *const ClapGainReductionBox);
        boxed.telemetry.clone()
    }

    unsafe extern "C" fn clap_gain_adjustment_get(plugin: *const clap_plugin) -> f64 {
        if plugin.is_null() {
            return 0.0;
        }
        let boxed = &*(plugin as *const ClapGainReductionBox);
        boxed.telemetry.host_gain_reduction_db()
    }

    unsafe extern "C" fn clap_state_save(
        plugin: *const clap_plugin,
        stream: *const c_void,
    ) -> bool {
        if plugin.is_null() || stream.is_null() {
            return false;
        }
        let boxed = &*(plugin as *const ClapGainReductionBox);
        let inner_state = boxed.inner_state.load(Ordering::Acquire);
        if inner_state.is_null() {
            return false;
        }
        if let Some(save) = (*inner_state).save {
            save(plugin, stream)
        } else {
            false
        }
    }

    unsafe extern "C" fn clap_state_load(
        plugin: *const clap_plugin,
        stream: *const c_void,
    ) -> bool {
        if plugin.is_null() || stream.is_null() {
            return false;
        }
        let boxed = &*(plugin as *const ClapGainReductionBox);
        let inner_state = boxed.inner_state.load(Ordering::Acquire);
        if inner_state.is_null() {
            return false;
        }
        let Some(load) = (*inner_state).load else {
            return false;
        };

        let loaded = load(plugin, stream);
        if loaded {
            // Ensure transient metering resets even if the plugin has not been activated yet.
            boxed.telemetry.clear_gain_reduction();
            boxed.telemetry.reset_peaks();

            if !boxed.rescanning_state.swap(true, Ordering::AcqRel) {
                if !boxed.host.is_null() {
                    if let Some(get_host_ext) = (*boxed.host).get_extension {
                        let host_params = get_host_ext(
                            boxed.host,
                            CLAP_EXT_PARAMS.as_ptr() as *const c_char,
                        ) as *const ClapHostParams;
                        if !host_params.is_null() {
                            if let Some(rescan) = (*host_params).rescan {
                                rescan(boxed.host, CLAP_PARAM_RESCAN_VALUES);
                            }
                        }
                    }
                }
                boxed.rescanning_state.store(false, Ordering::Release);
            }
        }
        loaded
    }

    unsafe extern "C" fn clap_destroy(plugin: *const clap_plugin) {
        if plugin.is_null() {
            return;
        }
        let boxed = Box::from_raw(plugin as *mut ClapGainReductionBox);
        if let Some(destroy) = boxed.inner_destroy {
            destroy(&boxed.plugin);
        }
    }

    unsafe extern "C" fn clap_deactivate(plugin: *const clap_plugin) {
        if plugin.is_null() {
            return;
        }
        let boxed = &*(plugin as *const ClapGainReductionBox);
        boxed.telemetry.clear_gain_reduction();
        if let Some(deactivate) = boxed.inner_deactivate {
            deactivate(plugin);
        }
    }

    unsafe extern "C" fn clap_stop_processing(plugin: *const clap_plugin) {
        if plugin.is_null() {
            return;
        }
        let boxed = &*(plugin as *const ClapGainReductionBox);
        boxed.telemetry.clear_gain_reduction();
        if let Some(stop_processing) = boxed.inner_stop_processing {
            stop_processing(plugin);
        }
    }

    unsafe extern "C" fn clap_reset(plugin: *const clap_plugin) {
        if plugin.is_null() {
            return;
        }
        let boxed = &*(plugin as *const ClapGainReductionBox);
        boxed.telemetry.clear_gain_reduction();
        if let Some(reset) = boxed.inner_reset {
            reset(plugin);
        }
    }

    unsafe extern "C" fn clap_get_extension(
        plugin: *const clap_plugin,
        id: *const c_char,
    ) -> *const c_void {
        if plugin.is_null() || id.is_null() {
            return std::ptr::null();
        }

        let requested = CStr::from_ptr(id);
        let gr_ext_id = CStr::from_bytes_with_nul_unchecked(CLAP_EXT_GAIN_ADJUSTMENT_METERING);
        if requested == gr_ext_id {
            return &CLAP_GAIN_ADJUSTMENT_METERING as *const ClapPluginGainAdjustmentMetering
                as *const c_void;
        }

        let boxed = &*(plugin as *const ClapGainReductionBox);
        let Some(get_extension) = boxed.inner_get_extension else {
            return std::ptr::null();
        };

        let inner_ext = get_extension(plugin, id);
        let state_ext_id = CStr::from_bytes_with_nul_unchecked(CLAP_EXT_STATE);
        if requested == state_ext_id && !inner_ext.is_null() {
            boxed
                .inner_state
                .store(inner_ext as *mut ClapPluginState, Ordering::Release);
            return &CLAP_STATE as *const ClapPluginState as *const c_void;
        }

        inner_ext
    }

    unsafe extern "C" fn clap_get_plugin_count(_factory: *const clap_plugin_factory) -> u32 {
        1
    }

    unsafe extern "C" fn clap_get_plugin_descriptor(
        _factory: *const clap_plugin_factory,
        index: u32,
    ) -> *const clap_plugin_descriptor {
        if index == 0 {
            plugin_descriptor().clap_plugin_descriptor()
        } else {
            std::ptr::null()
        }
    }

    unsafe extern "C" fn clap_create_plugin(
        _factory: *const clap_plugin_factory,
        host: *const clap_host,
        plugin_id: *const c_char,
    ) -> *const clap_plugin {
        if host.is_null() || plugin_id.is_null() {
            return std::ptr::null();
        }

        if CStr::from_ptr(plugin_id) != plugin_descriptor().clap_id() {
            return std::ptr::null();
        }

        // Clear any leftover entry on this thread before instantiating OmorePunch.
        let _ = take_last_created_telemetry();
        let wrapper = ClapWrapper::<OmorePunch>::new(host);
        let telemetry = take_last_created_telemetry().unwrap_or_default();

        let inner_ptr = wrapper.clap_plugin.as_ptr();
        let inner = &*inner_ptr;

        let boxed = Box::new(ClapGainReductionBox {
            plugin: clap_plugin {
                desc: inner.desc,
                plugin_data: inner.plugin_data,
                init: inner.init,
                destroy: Some(clap_destroy),
                activate: inner.activate,
                deactivate: Some(clap_deactivate),
                start_processing: inner.start_processing,
                stop_processing: Some(clap_stop_processing),
                reset: Some(clap_reset),
                process: inner.process,
                get_extension: Some(clap_get_extension),
                on_main_thread: inner.on_main_thread,
            },
            host,
            inner_destroy: inner.destroy,
            inner_deactivate: inner.deactivate,
            inner_stop_processing: inner.stop_processing,
            inner_reset: inner.reset,
            inner_get_extension: inner.get_extension,
            inner_state: AtomicPtr::new(std::ptr::null_mut()),
            rescanning_state: AtomicBool::new(false),
            telemetry,
        });

        // Wrapper ownership transfers into `plugin.plugin_data` and is freed inside
        // `inner_destroy` when `clap_destroy` runs.
        std::mem::forget(wrapper);

        &(*Box::into_raw(boxed)).plugin
    }

    static CLAP_FACTORY: clap_plugin_factory = clap_plugin_factory {
        get_plugin_count: Some(clap_get_plugin_count),
        get_plugin_descriptor: Some(clap_get_plugin_descriptor),
        create_plugin: Some(clap_create_plugin),
    };

    unsafe extern "C" fn clap_init(_plugin_path: *const c_char) -> bool {
        setup_logger();
        true
    }

    unsafe extern "C" fn clap_deinit() {}

    unsafe extern "C" fn clap_get_factory(factory_id: *const c_char) -> *const c_void {
        if factory_id.is_null() {
            return std::ptr::null();
        }
        if CStr::from_ptr(factory_id) == CLAP_PLUGIN_FACTORY_ID {
            &CLAP_FACTORY as *const clap_plugin_factory as *const c_void
        } else {
            std::ptr::null()
        }
    }

    #[no_mangle]
    #[used]
    pub static clap_entry: clap_plugin_entry = clap_plugin_entry {
        clap_version: CLAP_VERSION,
        init: Some(clap_init),
        deinit: Some(clap_deinit),
        get_factory: Some(clap_get_factory),
    };
}

mod vst3 {
    #![allow(non_snake_case)]

    use super::{take_last_created_telemetry, MeterTelemetry, OmorePunch};
    use nih_plug::wrapper::setup_logger;
    use nih_plug::wrapper::vst3::{vst3_sys, PluginInfo, Wrapper as Vst3Wrapper};
    use std::ffi::c_void;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, Mutex, OnceLock};
    use vst3_sys as vst3_com;
    use vst3_sys::base::{
        kInvalidArgument, kNoInterface, kResultOk, tresult, FIDString, IBStream, IPluginBase,
        IPluginFactory, IPluginFactory2, IPluginFactory3, IUnknown, PClassInfo, PClassInfo2,
        PClassInfoW, PFactoryInfo, TBool,
    };
    use vst3_sys::utils::SharedVstPtr;
    use vst3_sys::vst::{
        BusDirection, BusInfo, CtrlNumber, IAudioProcessor, IComponent, IComponentHandler,
        IEditController, IMidiMapping, INoteExpressionController, IProcessContextRequirements,
        IUnitInfo, IoMode, MediaType, NoteExpressionTypeInfo, ParamID, ParameterInfo,
        ProcessData, ProcessSetup, ProgramListInfo, RestartFlags, RoutingInfo, SpeakerArrangement,
        TChar, UnitInfo,
    };
    use vst3_sys::{com_interface, IID, VstPtr, VST3};

    struct ComponentHandlerPtr(VstPtr<dyn IComponentHandler>);
    unsafe impl Send for ComponentHandlerPtr {}
    unsafe impl Sync for ComponentHandlerPtr {}

    #[cfg(test)]
    thread_local! {
        static TEST_LAST_VST3_TELEMETRY: std::cell::RefCell<Option<Arc<MeterTelemetry>>> =
            const { std::cell::RefCell::new(None) };
    }

    #[cfg(test)]
    pub fn take_test_vst3_telemetry() -> Option<Arc<MeterTelemetry>> {
        TEST_LAST_VST3_TELEMETRY.with(|slot| slot.borrow_mut().take())
    }

    #[com_interface("8E3C292C-9592-4F9D-B259-0B1E100E4198")]
    pub trait IGainReductionInfo: IUnknown {
        unsafe fn get_gain_reduction_value_in_db(&self) -> f64;
    }

    #[VST3(implements(
        IComponent,
        IEditController,
        IAudioProcessor,
        IMidiMapping,
        INoteExpressionController,
        IProcessContextRequirements,
        IUnitInfo,
        IGainReductionInfo
    ))]
    struct Vst3GainReductionProxy {
        inner: *mut Vst3Wrapper<OmorePunch>,
        telemetry: Arc<MeterTelemetry>,
        component_handler: Mutex<Option<ComponentHandlerPtr>>,
        restarting_params: AtomicBool,
    }

    unsafe impl Send for Vst3GainReductionProxy {}
    unsafe impl Sync for Vst3GainReductionProxy {}

    impl Vst3GainReductionProxy {
        fn clear_component_handler(&self) {
            if let Ok(mut guard) = self.component_handler.lock() {
                *guard = None;
            }
        }

        fn notify_param_values_changed(&self) {
            if self.restarting_params.swap(true, Ordering::AcqRel) {
                return;
            }
            let handler = self
                .component_handler
                .lock()
                .ok()
                .and_then(|guard| guard.as_ref().map(|entry| entry.0.clone()));
            if let Some(handler) = handler {
                unsafe {
                    handler.restart_component(RestartFlags::kParamValuesChanged as i32);
                }
            }
            self.restarting_params.store(false, Ordering::Release);
        }
    }

    impl Drop for Vst3GainReductionProxy {
        fn drop(&mut self) {
            self.clear_component_handler();
            unsafe {
                if !self.inner.is_null() {
                    (*self.inner).release();
                    self.inner = std::ptr::null_mut();
                }
            }
        }
    }

    impl IPluginBase for Vst3GainReductionProxy {
        unsafe fn initialize(&self, context: *mut c_void) -> tresult {
            (*self.inner).initialize(context)
        }

        unsafe fn terminate(&self) -> tresult {
            self.telemetry.clear_gain_reduction();
            self.clear_component_handler();
            (*self.inner).terminate()
        }
    }

    impl IComponent for Vst3GainReductionProxy {
        unsafe fn get_controller_class_id(&self, tuid: *mut IID) -> tresult {
            (*self.inner).get_controller_class_id(tuid)
        }

        unsafe fn set_io_mode(&self, mode: IoMode) -> tresult {
            (*self.inner).set_io_mode(mode)
        }

        unsafe fn get_bus_count(&self, type_: MediaType, dir: BusDirection) -> i32 {
            (*self.inner).get_bus_count(type_, dir)
        }

        unsafe fn get_bus_info(
            &self,
            type_: MediaType,
            dir: BusDirection,
            index: i32,
            info: *mut BusInfo,
        ) -> tresult {
            (*self.inner).get_bus_info(type_, dir, index, info)
        }

        unsafe fn get_routing_info(
            &self,
            in_info: *mut RoutingInfo,
            out_info: *mut RoutingInfo,
        ) -> tresult {
            (*self.inner).get_routing_info(in_info, out_info)
        }

        unsafe fn activate_bus(
            &self,
            type_: MediaType,
            dir: BusDirection,
            index: i32,
            state: TBool,
        ) -> tresult {
            (*self.inner).activate_bus(type_, dir, index, state)
        }

        unsafe fn set_active(&self, state: TBool) -> tresult {
            if state == 0 {
                self.telemetry.clear_gain_reduction();
            }
            (*self.inner).set_active(state)
        }

        unsafe fn set_state(&self, state: SharedVstPtr<dyn IBStream>) -> tresult {
            let result = <Vst3Wrapper<OmorePunch> as IComponent>::set_state(&*self.inner, state);
            if result == kResultOk {
                // Ensure transient metering resets even when loaded before activation.
                self.telemetry.clear_gain_reduction();
                self.telemetry.reset_peaks();
                self.notify_param_values_changed();
            }
            result
        }

        unsafe fn get_state(&self, state: SharedVstPtr<dyn IBStream>) -> tresult {
            <Vst3Wrapper<OmorePunch> as IComponent>::get_state(&*self.inner, state)
        }
    }

    impl IEditController for Vst3GainReductionProxy {
        unsafe fn set_component_state(&self, state: SharedVstPtr<dyn IBStream>) -> tresult {
            (*self.inner).set_component_state(state)
        }

        unsafe fn set_state(&self, state: SharedVstPtr<dyn IBStream>) -> tresult {
            <Vst3Wrapper<OmorePunch> as IEditController>::set_state(&*self.inner, state)
        }

        unsafe fn get_state(&self, state: SharedVstPtr<dyn IBStream>) -> tresult {
            <Vst3Wrapper<OmorePunch> as IEditController>::get_state(&*self.inner, state)
        }

        unsafe fn get_parameter_count(&self) -> i32 {
            (*self.inner).get_parameter_count()
        }

        unsafe fn get_parameter_info(
            &self,
            param_index: i32,
            info: *mut ParameterInfo,
        ) -> tresult {
            (*self.inner).get_parameter_info(param_index, info)
        }

        unsafe fn get_param_string_by_value(
            &self,
            id: u32,
            value_normalized: f64,
            string: *mut TChar,
        ) -> tresult {
            (*self.inner).get_param_string_by_value(id, value_normalized, string)
        }

        unsafe fn get_param_value_by_string(
            &self,
            id: u32,
            string: *const TChar,
            value_normalized: *mut f64,
        ) -> tresult {
            (*self.inner).get_param_value_by_string(id, string, value_normalized)
        }

        unsafe fn normalized_param_to_plain(&self, id: u32, value_normalized: f64) -> f64 {
            (*self.inner).normalized_param_to_plain(id, value_normalized)
        }

        unsafe fn plain_param_to_normalized(&self, id: u32, plain_value: f64) -> f64 {
            (*self.inner).plain_param_to_normalized(id, plain_value)
        }

        unsafe fn get_param_normalized(&self, id: u32) -> f64 {
            (*self.inner).get_param_normalized(id)
        }

        unsafe fn set_param_normalized(&self, id: u32, value: f64) -> tresult {
            (*self.inner).set_param_normalized(id, value)
        }

        unsafe fn set_component_handler(
            &self,
            handler: SharedVstPtr<dyn IComponentHandler>,
        ) -> tresult {
            let upgraded = handler.upgrade().map(ComponentHandlerPtr);
            if let Ok(mut guard) = self.component_handler.lock() {
                *guard = upgraded;
            }
            (*self.inner).set_component_handler(handler)
        }

        unsafe fn create_view(&self, name: FIDString) -> *mut c_void {
            (*self.inner).create_view(name)
        }
    }

    impl IAudioProcessor for Vst3GainReductionProxy {
        unsafe fn set_bus_arrangements(
            &self,
            inputs: *mut SpeakerArrangement,
            num_ins: i32,
            outputs: *mut SpeakerArrangement,
            num_outs: i32,
        ) -> tresult {
            (*self.inner).set_bus_arrangements(inputs, num_ins, outputs, num_outs)
        }

        unsafe fn get_bus_arrangement(
            &self,
            dir: BusDirection,
            index: i32,
            arr: *mut SpeakerArrangement,
        ) -> tresult {
            (*self.inner).get_bus_arrangement(dir, index, arr)
        }

        unsafe fn can_process_sample_size(&self, symbolic_sample_size: i32) -> tresult {
            (*self.inner).can_process_sample_size(symbolic_sample_size)
        }

        unsafe fn get_latency_samples(&self) -> u32 {
            (*self.inner).get_latency_samples()
        }

        unsafe fn setup_processing(&self, setup: *const ProcessSetup) -> tresult {
            (*self.inner).setup_processing(setup)
        }

        unsafe fn set_processing(&self, state: TBool) -> tresult {
            if state == 0 {
                self.telemetry.clear_gain_reduction();
            }
            (*self.inner).set_processing(state)
        }

        unsafe fn process(&self, data: *mut ProcessData) -> tresult {
            (*self.inner).process(data)
        }

        unsafe fn get_tail_samples(&self) -> u32 {
            (*self.inner).get_tail_samples()
        }
    }

    impl IMidiMapping for Vst3GainReductionProxy {
        unsafe fn get_midi_controller_assignment(
            &self,
            bus_index: i32,
            channel: i16,
            midi_cc_number: CtrlNumber,
            param_id: *mut ParamID,
        ) -> tresult {
            (*self.inner).get_midi_controller_assignment(bus_index, channel, midi_cc_number, param_id)
        }
    }

    impl INoteExpressionController for Vst3GainReductionProxy {
        unsafe fn get_note_expression_count(&self, bus_idx: i32, channel: i16) -> i32 {
            (*self.inner).get_note_expression_count(bus_idx, channel)
        }

        unsafe fn get_note_expression_info(
            &self,
            bus_idx: i32,
            channel: i16,
            note_expression_idx: i32,
            info: *mut NoteExpressionTypeInfo,
        ) -> tresult {
            (*self.inner).get_note_expression_info(bus_idx, channel, note_expression_idx, info)
        }

        unsafe fn get_note_expression_string_by_value(
            &self,
            bus_idx: i32,
            channel: i16,
            id: u32,
            value: f64,
            string: *mut TChar,
        ) -> tresult {
            (*self.inner).get_note_expression_string_by_value(bus_idx, channel, id, value, string)
        }

        unsafe fn get_note_expression_value_by_string(
            &self,
            bus_idx: i32,
            channel: i16,
            id: u32,
            string: *const TChar,
            value: *mut f64,
        ) -> tresult {
            (*self.inner).get_note_expression_value_by_string(bus_idx, channel, id, string, value)
        }
    }

    impl IProcessContextRequirements for Vst3GainReductionProxy {
        unsafe fn get_process_context_requirements(&self) -> u32 {
            (*self.inner).get_process_context_requirements()
        }
    }

    impl IUnitInfo for Vst3GainReductionProxy {
        unsafe fn get_unit_count(&self) -> i32 {
            (*self.inner).get_unit_count()
        }

        unsafe fn get_unit_info(&self, unit_index: i32, info: *mut UnitInfo) -> tresult {
            (*self.inner).get_unit_info(unit_index, info)
        }

        unsafe fn get_program_list_count(&self) -> i32 {
            (*self.inner).get_program_list_count()
        }

        unsafe fn get_program_list_info(
            &self,
            list_index: i32,
            info: *mut ProgramListInfo,
        ) -> tresult {
            (*self.inner).get_program_list_info(list_index, info)
        }

        unsafe fn get_program_name(
            &self,
            list_id: i32,
            program_index: i32,
            name: *mut u16,
        ) -> tresult {
            (*self.inner).get_program_name(list_id, program_index, name)
        }

        unsafe fn get_program_info(
            &self,
            list_id: i32,
            program_index: i32,
            attribute_id: *const u8,
            attribute_value: *mut u16,
        ) -> tresult {
            (*self.inner).get_program_info(list_id, program_index, attribute_id, attribute_value)
        }

        unsafe fn has_program_pitch_names(&self, id: i32, index: i32) -> tresult {
            (*self.inner).has_program_pitch_names(id, index)
        }

        unsafe fn get_program_pitch_name(
            &self,
            id: i32,
            index: i32,
            pitch: i16,
            name: *mut u16,
        ) -> tresult {
            (*self.inner).get_program_pitch_name(id, index, pitch, name)
        }

        unsafe fn get_selected_unit(&self) -> i32 {
            (*self.inner).get_selected_unit()
        }

        unsafe fn select_unit(&self, id: i32) -> tresult {
            (*self.inner).select_unit(id)
        }

        unsafe fn get_unit_by_bus(
            &self,
            type_: i32,
            dir: i32,
            bus_index: i32,
            channel: i32,
            unit_id: *mut i32,
        ) -> tresult {
            (*self.inner).get_unit_by_bus(type_, dir, bus_index, channel, unit_id)
        }

        unsafe fn set_unit_program_data(
            &self,
            list_or_unit: i32,
            program_idx: i32,
            data: SharedVstPtr<dyn IBStream>,
        ) -> tresult {
            (*self.inner).set_unit_program_data(list_or_unit, program_idx, data)
        }
    }

    impl IGainReductionInfo for Vst3GainReductionProxy {
        unsafe fn get_gain_reduction_value_in_db(&self) -> f64 {
            self.telemetry.host_gain_reduction_db()
        }
    }

    fn plugin_info() -> &'static PluginInfo {
        static INFO: OnceLock<PluginInfo> = OnceLock::new();
        INFO.get_or_init(PluginInfo::for_plugin::<OmorePunch>)
    }

    #[VST3(implements(IPluginFactory3))]
    struct Factory {
        info: &'static PluginInfo,
    }

    impl IPluginFactory for Factory {
        unsafe fn get_factory_info(&self, info: *mut PFactoryInfo) -> tresult {
            if info.is_null() {
                return kInvalidArgument;
            }
            *info = self.info.create_factory_info();
            kResultOk
        }

        unsafe fn count_classes(&self) -> i32 {
            1
        }

        unsafe fn get_class_info(&self, index: i32, info: *mut PClassInfo) -> tresult {
            if index != 0 || info.is_null() {
                return kInvalidArgument;
            }
            *info = self.info.create_class_info();
            kResultOk
        }

        unsafe fn create_instance(
            &self,
            cid: *const IID,
            iid: *const IID,
            obj: *mut *mut c_void,
        ) -> tresult {
            if obj.is_null() {
                return kInvalidArgument;
            }
            *obj = std::ptr::null_mut();

            if cid.is_null() || iid.is_null() {
                return kInvalidArgument;
            }
            if (*cid).data != *self.info.cid {
                return kNoInterface;
            }

            // Clear any unused entry on this thread before instantiating OmorePunch.
            let _ = take_last_created_telemetry();
            let inner = Box::into_raw(Vst3Wrapper::<OmorePunch>::new());
            let telemetry = take_last_created_telemetry().unwrap_or_default();
            #[cfg(test)]
            let test_telemetry = telemetry.clone();

            let proxy = Vst3GainReductionProxy::allocate(
                inner,
                telemetry,
                Mutex::new(None),
                AtomicBool::new(false),
            );
            let result = proxy.query_interface(&*iid, obj);
            if result == kResultOk {
                #[cfg(test)]
                TEST_LAST_VST3_TELEMETRY.with(|slot| {
                    *slot.borrow_mut() = Some(test_telemetry);
                });
                proxy.release();
                std::mem::forget(proxy);
                kResultOk
            } else {
                drop(proxy);
                *obj = std::ptr::null_mut();
                result
            }
        }
    }

    impl IPluginFactory2 for Factory {
        unsafe fn get_class_info2(&self, index: i32, info: *mut PClassInfo2) -> tresult {
            if index != 0 || info.is_null() {
                return kInvalidArgument;
            }
            *info = self.info.create_class_info_2();
            kResultOk
        }
    }

    impl IPluginFactory3 for Factory {
        unsafe fn get_class_info_unicode(&self, index: i32, info: *mut PClassInfoW) -> tresult {
            if index != 0 || info.is_null() {
                return kInvalidArgument;
            }
            *info = self.info.create_class_info_unicode();
            kResultOk
        }

        unsafe fn set_host_context(&self, _context: *mut c_void) -> tresult {
            kResultOk
        }
    }

    #[no_mangle]
    pub extern "system" fn GetPluginFactory() -> *mut c_void {
        Box::into_raw(Factory::allocate(plugin_info())) as *mut c_void
    }

    #[cfg(all(target_family = "unix", not(target_os = "macos")))]
    #[no_mangle]
    pub extern "C" fn ModuleEntry(_lib_handle: *mut c_void) -> bool {
        setup_logger();
        true
    }

    #[cfg(all(target_family = "unix", not(target_os = "macos")))]
    #[no_mangle]
    pub extern "C" fn ModuleExit() -> bool {
        true
    }

    #[cfg(target_os = "macos")]
    #[no_mangle]
    pub extern "C" fn bundleEntry(_bundle_ref: *mut c_void) -> bool {
        setup_logger();
        true
    }

    #[cfg(target_os = "macos")]
    #[no_mangle]
    pub extern "C" fn bundleExit() -> bool {
        true
    }

    #[cfg(target_os = "windows")]
    #[no_mangle]
    pub extern "system" fn InitDll() -> bool {
        setup_logger();
        true
    }

    #[cfg(target_os = "windows")]
    #[no_mangle]
    pub extern "system" fn ExitDll() -> bool {
        true
    }
}

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
            release_mode: ReleaseMode::Manual,
            sc_hpf_hz: 20.0,
            auto_makeup: false,
            makeup_db: 0.0,
            eq_low_cut_hz: 20.0,
            eq_mid_cut_db: 0.0,
            eq_high_shelf_db: 0.0,
            clipper_mode: ClipperMode::Bypass,
            ceiling_db: 0.0,
            mix_pct: 100.0,
            gain_match: false,
            bypass: false,
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
            gain_match_db: 0.0,
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
            release_mode: ReleaseMode::Manual,
            sc_hpf_hz: 20.0,
            auto_makeup: false,
            makeup_db: 0.0,
            eq_low_cut_hz: 20.0,
            eq_mid_cut_db: 0.0,
            eq_high_shelf_db: 0.0,
            clipper_mode: ClipperMode::Bypass,
            ceiling_db: 0.0,
            mix_pct: 100.0,
            gain_match: false,
            bypass: false,
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
            release_mode: ReleaseMode::Manual,
            sc_hpf_hz: 20.0,
            auto_makeup: false,
            makeup_db: 0.0,
            eq_low_cut_hz: 20.0,
            eq_mid_cut_db: 0.0,
            eq_high_shelf_db: 0.0,
            clipper_mode: ClipperMode::Bypass,
            ceiling_db: 0.0,
            mix_pct: 100.0,
            gain_match: false,
            bypass: false,
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
                release_mode: ReleaseMode::Manual,
                sc_hpf_hz: 20.0,
                auto_makeup: false,
                makeup_db: 0.0,
                eq_low_cut_hz: 20.0,
                eq_mid_cut_db: 0.0,
                eq_high_shelf_db: 0.0,
                clipper_mode: mode,
                ceiling_db,
                mix_pct: 100.0,
                gain_match: false,
                bypass: false,
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
            release_mode: ReleaseMode::Manual,
            sc_hpf_hz: 90.0,
            auto_makeup: true,
            makeup_db: 6.0,
            eq_low_cut_hz: 20.0,
            eq_mid_cut_db: 0.0,
            eq_high_shelf_db: 0.0,
            clipper_mode: ClipperMode::HardClip,
            ceiling_db: -4.0,
            mix_pct: 0.0,
            gain_match: false,
            bypass: false,
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
        assert_eq!(DspConfig::default().release_mode, ReleaseMode::Manual);
        assert_eq!(
            OmorePunchParams::default().release_mode.value(),
            ReleaseMode::Manual
        );

        // Subnormals, threshold-floor values, and -0.0 must flush to exact +0.0
        for &sub_or_zero in &[
            1.0e-39_f32,
            -1.0e-39_f32,
            DENORMAL_FLUSH_THRESHOLD,
            -DENORMAL_FLUSH_THRESHOLD,
            f32::from_bits(1),
            -f32::from_bits(1),
            -0.0_f32,
            f32::NAN,
            f32::INFINITY,
            f32::NEG_INFINITY,
        ] {
            assert_eq!(
                sanitize_sample(sub_or_zero).to_bits(),
                0.0_f32.to_bits(),
                "sanitize_sample({:?}) must flush to exact +0.0",
                sub_or_zero
            );
        }
        assert_eq!(sanitize_sample(0.25_f32).to_bits(), 0.25_f32.to_bits());
        assert_eq!(sanitize_sample(-0.25_f32).to_bits(), (-0.25_f32).to_bits());
        assert_eq!(sanitize_sample(10.0_f32), SAFETY_CLAMP_LIMIT);
        assert_eq!(sanitize_sample(-10.0_f32), -SAFETY_CLAMP_LIMIT);

        let sr = 48000.0_f32;
        let pathological_inputs = [
            (f32::NAN, f32::NAN),
            (f32::INFINITY, f32::NEG_INFINITY),
            (1.0e30, -1.0e30),
            (0.5, -0.5),
            (1.0e-39, -1.0e-39),
        ];

        for mode in [
            ReleaseMode::Manual,
            ReleaseMode::AutoTight,
            ReleaseMode::AutoPunch,
            ReleaseMode::AutoGlue,
        ] {
            for gain_match in [false, true] {
                let mut dsp = StereoPunchDsp::new(sr);
                let cfg = DspConfig {
                    input_db: 12.0,
                    threshold_db: -24.0,
                    ratio: 8.0,
                    attack_ms: 3.0,
                    release_ms: 80.0,
                    release_mode: mode,
                    sc_hpf_hz: 90.0,
                    auto_makeup: true,
                    makeup_db: 24.0,
                    eq_low_cut_hz: 120.0,
                    eq_mid_cut_db: -9.0,
                    eq_high_shelf_db: 12.0,
                    clipper_mode: ClipperMode::Bypass,
                    ceiling_db: 0.0,
                    mix_pct: 100.0,
                    gain_match,
                    bypass: false,
                };

                for (in_l, in_r) in pathological_inputs {
                    let out = dsp.process_sample(in_l, in_r, &cfg);
                    assert!(
                        out.out_l.is_finite(),
                        "[{:?}] Left output is not finite: {}",
                        mode,
                        out.out_l
                    );
                    assert!(
                        out.out_r.is_finite(),
                        "[{:?}] Right output is not finite: {}",
                        mode,
                        out.out_r
                    );
                    assert!(
                        out.out_l >= -SAFETY_CLAMP_LIMIT && out.out_l <= SAFETY_CLAMP_LIMIT,
                        "[{:?}] Left output {} out of [-4.0, 4.0] safety bounds",
                        mode,
                        out.out_l
                    );
                    assert!(
                        out.out_r >= -SAFETY_CLAMP_LIMIT && out.out_r <= SAFETY_CLAMP_LIMIT,
                        "[{:?}] Right output {} out of [-4.0, 4.0] safety bounds",
                        mode,
                        out.out_r
                    );
                    assert!(
                        (out.out_l == 0.0 || out.out_l.is_normal())
                            && (out.out_r == 0.0 || out.out_r.is_normal()),
                        "[{:?}] Subnormal output leaked: L={} R={}",
                        mode,
                        out.out_l,
                        out.out_r
                    );
                    for (sc_name, sc_val) in [
                        ("sc_prev_in_l", dsp.sc_prev_in_l),
                        ("sc_prev_in_r", dsp.sc_prev_in_r),
                        ("sc_prev_out_l", dsp.sc_prev_out_l),
                        ("sc_prev_out_r", dsp.sc_prev_out_r),
                    ] {
                        assert!(
                            sc_val.is_finite() && (sc_val == 0.0 || sc_val.is_normal()),
                            "[{:?}] Sidechain HPF state {} invalid/subnormal: {}",
                            mode,
                            sc_name,
                            sc_val
                        );
                    }
                    assert!(
                        dsp.env_gr_db.is_finite()
                            && (dsp.env_gr_db == 0.0 || dsp.env_gr_db.is_normal()),
                        "[{:?}] Internal env_gr_db invalid/subnormal: {}",
                        mode,
                        dsp.env_gr_db
                    );
                    assert!(
                        dsp.auto_slow_gr_db.is_finite()
                            && (dsp.auto_slow_gr_db == 0.0 || dsp.auto_slow_gr_db.is_normal()),
                        "[{:?}] Internal auto_slow_gr_db invalid/subnormal: {}",
                        mode,
                        dsp.auto_slow_gr_db
                    );
                    for (gm_name, gm_val) in [
                        ("gm_in_ms", dsp.gm_in_ms),
                        ("gm_post_ms", dsp.gm_post_ms),
                        ("gm_presence_ms", dsp.gm_presence_ms),
                        ("gm_gain_db", dsp.gm_gain_db),
                    ] {
                        assert!(
                            gm_val.is_finite() && (gm_val == 0.0 || gm_val.is_normal()),
                            "[{:?} gm={}] Internal {} invalid/subnormal: {}",
                            mode,
                            gain_match,
                            gm_name,
                            gm_val
                        );
                    }
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
                            "[{:?}] Internal EQ biquad states for {} became non-finite",
                            mode,
                            name
                        );
                    }
                }

                // Long silence tail flushes both envelope stages cleanly to exact 0.0 (no denormals)
                for _ in 0..(60.0 * sr) as usize {
                    let out = dsp.process_sample(0.0, 0.0, &cfg);
                    assert!(
                        dsp.env_gr_db == 0.0 || dsp.env_gr_db.is_normal(),
                        "[{:?}] Subnormal env_gr_db during silence: {}",
                        mode,
                        dsp.env_gr_db
                    );
                    assert!(
                        dsp.auto_slow_gr_db == 0.0 || dsp.auto_slow_gr_db.is_normal(),
                        "[{:?}] Subnormal auto_slow_gr_db during silence: {}",
                        mode,
                        dsp.auto_slow_gr_db
                    );
                    assert!(
                        dsp.gm_presence_ms == 0.0 || dsp.gm_presence_ms.is_normal(),
                        "[{:?}] Subnormal gm_presence_ms during silence: {}",
                        mode,
                        dsp.gm_presence_ms
                    );
                    assert!(out.out_l.is_finite() && out.out_r.is_finite());
                }
                assert_eq!(
                    dsp.env_gr_db, 0.0,
                    "[{:?}] env_gr_db failed to flush to 0.0 on silence",
                    mode
                );
                assert_eq!(
                    dsp.auto_slow_gr_db, 0.0,
                    "[{:?}] auto_slow_gr_db failed to flush to 0.0 on silence",
                    mode
                );
                assert_eq!(
                    dsp.gm_presence_ms, 0.0,
                    "[{:?}] gm_presence_ms failed to flush to 0.0 on silence",
                    mode
                );
            }
        }

        // Fresh DSP receiving pure subnormals must flush both sc_prev_in_* and output (active & bypass)
        for bypass in [false, true] {
            let mut sub_dsp = StereoPunchDsp::new(sr);
            sub_dsp.sync_bypass_state(bypass);
            let sub_cfg = DspConfig {
                input_db: 12.0,
                sc_hpf_hz: 90.0,
                bypass,
                ..DspConfig::default()
            };
            let out = sub_dsp.process_sample(1.0e-39, -1.0e-39, &sub_cfg);
            assert_eq!(out.out_l.to_bits(), 0.0_f32.to_bits());
            assert_eq!(out.out_r.to_bits(), 0.0_f32.to_bits());
            assert_eq!(sub_dsp.sc_prev_in_l.to_bits(), 0.0_f32.to_bits());
            assert_eq!(sub_dsp.sc_prev_in_r.to_bits(), 0.0_f32.to_bits());
            assert_eq!(sub_dsp.sc_prev_out_l.to_bits(), 0.0_f32.to_bits());
            assert_eq!(sub_dsp.sc_prev_out_r.to_bits(), 0.0_f32.to_bits());
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

        // Combined left-column sections (Section 1: 2 knob rows + Release Mode row + quick
        // preset row; Section 2: clipper mode row + 1 knob row; Section 3: 1 EQ knob row;
        // group headers, separators, margins and gaps) fit within CentralPanel height
        // (UI_WINDOW_HEIGHT - header_and_panel_margins_px = 602.0 px).
        let header_and_panel_margins_px = 48.0_f32;
        let available_left_col_height =
            UI_WINDOW_HEIGHT as f32 - header_and_panel_margins_px;
        let group_header_and_margins_px = 36.0_f32;
        let action_row_height_px = 24.0_f32;
        let section1_height = group_header_and_margins_px
            + 2.0 * UI_KNOB_CELL_HEIGHT
            + UI_KNOB_GRID_SPACING_Y
            + 2.0
            + action_row_height_px
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

        // Empirically verify rendered egui layout fits within 820x650 px in both Manual and
        // AutoTight worst-case ("AUTO (500.0 ms)") readout configurations.
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

        let assert_ui_fits = |label: &str, params: Arc<OmorePunchParams>, gain_match_db: f32| {
            let ctx = egui::Context::default();
            let telemetry = Arc::new(MeterTelemetry::default());
            store_f32_relaxed(&telemetry.gain_match_db, gain_match_db);
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
                "[{}] Rendered egui width ({}) exceeds window width ({})",
                label,
                used.width(),
                UI_WINDOW_WIDTH
            );
            assert!(
                used.height() <= UI_WINDOW_HEIGHT as f32,
                "[{}] Rendered egui height ({}) exceeds window height ({})",
                label,
                used.height(),
                UI_WINDOW_HEIGHT
            );
            assert!(
                max_shape_bottom <= UI_WINDOW_HEIGHT as f32,
                "[{}] Painted egui widgets bottom ({}) overflows window height ({})",
                label,
                max_shape_bottom,
                UI_WINDOW_HEIGHT
            );
            assert!(
                max_inner_bottom <= UI_WINDOW_HEIGHT as f32 - 6.0,
                "[{}] Inner widgets bottom ({}) exceeds padded window height ({})",
                label,
                max_inner_bottom,
                UI_WINDOW_HEIGHT as f32 - 6.0
            );
            assert!(
                max_inner_right <= UI_WINDOW_WIDTH as f32 - 6.0,
                "[{}] Inner widgets right ({}) exceeds padded window width ({})",
                label,
                max_inner_right,
                UI_WINDOW_WIDTH as f32 - 6.0
            );
        };

        let manual_params = Arc::new(OmorePunchParams::default());
        assert_ui_fits("manual_default", manual_params, 0.0);

        let auto_params = OmorePunchParams {
            release_mode: EnumParam::new("Release Mode", ReleaseMode::AutoTight),
            release_ms: make_release_ms_param(500.0),
            ..OmorePunchParams::default()
        };
        assert_eq!(auto_params.release_mode.value(), ReleaseMode::AutoTight);
        assert_eq!(auto_params.release_ms.value(), 500.0);
        assert_ui_fits("auto_tight_500ms", Arc::new(auto_params), 0.0);

        let gain_match_params = OmorePunchParams {
            gain_match: BoolParam::new("Gain Match", true),
            ..OmorePunchParams::default()
        };
        assert_ui_fits("gain_match_on_extreme", Arc::new(gain_match_params), -24.0);

        let bypass_params = OmorePunchParams {
            bypass: BoolParam::new("Bypass", true).make_bypass(),
            gain_match: BoolParam::new("Gain Match", true),
            ..OmorePunchParams::default()
        };
        assert_ui_fits("bypass_on_with_gain_match", Arc::new(bypass_params), -12.0);
    }

    #[test]
    fn test_ui_bypass_button_and_controls_remain_editable() {
        use std::sync::Mutex;

        fn find_text_rect(shape: &egui::epaint::Shape, needle: &str) -> Option<egui::Rect> {
            match shape {
                egui::epaint::Shape::Text(text_shape) => {
                    if text_shape.galley.text().contains(needle) {
                        Some(text_shape.visual_bounding_rect())
                    } else {
                        None
                    }
                }
                egui::epaint::Shape::Vec(items) => {
                    items.iter().find_map(|item| find_text_rect(item, needle))
                }
                _ => None,
            }
        }

        struct TrackingGuiContext {
            calls: Mutex<Vec<(ParamPtr, f32)>>,
        }
        impl GuiContext for TrackingGuiContext {
            fn plugin_api(&self) -> PluginApi {
                PluginApi::Standalone
            }
            fn request_resize(&self) -> bool {
                false
            }
            unsafe fn raw_begin_set_parameter(&self, _param: ParamPtr) {}
            unsafe fn raw_set_parameter_normalized(&self, param: ParamPtr, normalized: f32) {
                self.calls.lock().unwrap().push((param, normalized));
            }
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

        let telemetry = Arc::new(MeterTelemetry::default());
        let base_input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(UI_WINDOW_WIDTH as f32, UI_WINDOW_HEIGHT as f32),
            )),
            ..Default::default()
        };

        // Phase A: bypass=false -> clicking [BYPASS: OFF] invokes setter for params.bypass -> true
        let params_off = Arc::new(OmorePunchParams::default());
        let gui_off = TrackingGuiContext {
            calls: Mutex::new(Vec::new()),
        };
        let setter_off = ParamSetter::new(&gui_off);
        let ctx_off = egui::Context::default();
        let render_off = |events: Vec<egui::Event>| {
            let mut input = base_input.clone();
            input.events = events;
            ctx_off.run(input, |ctx| {
                draw_omore_punch_ui(ctx, &setter_off, &params_off, &telemetry);
            })
        };

        let _ = render_off(vec![]);
        let out_off = render_off(vec![]);
        let bypass_pos = out_off
            .shapes
            .iter()
            .find_map(|c| find_text_rect(&c.shape, "[BYPASS: OFF]"))
            .expect("[BYPASS: OFF] button must be painted in header")
            .center();

        let _ = render_off(vec![
            egui::Event::PointerMoved(bypass_pos),
            egui::Event::PointerButton {
                pos: bypass_pos,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
        ]);
        let _ = render_off(vec![egui::Event::PointerButton {
            pos: bypass_pos,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        }]);
        let off_calls = gui_off.calls.lock().unwrap();
        assert!(
            off_calls
                .iter()
                .any(|&(ptr, norm)| ptr == params_off.bypass.as_ptr() && norm >= 0.5),
            "Clicking [BYPASS: OFF] must set bypass=true, got {:?}",
            *off_calls
        );

        // Phase B: bypass=true -> [BYPASS: ON] shown, controls remain editable under opacity 0.55
        let params_on = Arc::new(OmorePunchParams {
            bypass: BoolParam::new("Bypass", true).make_bypass(),
            ..OmorePunchParams::default()
        });
        let gui_on = TrackingGuiContext {
            calls: Mutex::new(Vec::new()),
        };
        let setter_on = ParamSetter::new(&gui_on);
        let ctx_on = egui::Context::default();
        let render_on = |events: Vec<egui::Event>| {
            let mut input = base_input.clone();
            input.events = events;
            ctx_on.run(input, |ctx| {
                draw_omore_punch_ui(ctx, &setter_on, &params_on, &telemetry);
            })
        };

        let _ = render_on(vec![]);
        let out_on = render_on(vec![]);
        assert!(
            out_on
                .shapes
                .iter()
                .any(|c| find_text_rect(&c.shape, "[BYPASS: ON]").is_some()),
            "Header must show [BYPASS: ON] when bypass is enabled"
        );

        // Click [Hard Clip] in left column and [Gain Match: OFF] in right column while bypassed
        let hard_clip_pos = out_on
            .shapes
            .iter()
            .find_map(|c| find_text_rect(&c.shape, "[Hard Clip]"))
            .expect("[Hard Clip] control must remain visible when bypassed")
            .center();
        let _ = render_on(vec![
            egui::Event::PointerMoved(hard_clip_pos),
            egui::Event::PointerButton {
                pos: hard_clip_pos,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
        ]);
        let _ = render_on(vec![egui::Event::PointerButton {
            pos: hard_clip_pos,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        }]);

        let gm_pos = out_on
            .shapes
            .iter()
            .find_map(|c| find_text_rect(&c.shape, "[Gain Match: OFF]"))
            .expect("[Gain Match: OFF] button must remain visible when bypassed")
            .center();
        let _ = render_on(vec![
            egui::Event::PointerMoved(gm_pos),
            egui::Event::PointerButton {
                pos: gm_pos,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
        ]);
        let _ = render_on(vec![egui::Event::PointerButton {
            pos: gm_pos,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        }]);

        let on_calls = gui_on.calls.lock().unwrap();
        assert!(
            on_calls
                .iter()
                .any(|&(ptr, _)| ptr == params_on.clipper_mode.as_ptr()),
            "Left-column controls must remain editable under opacity 0.55 while bypassed"
        );
        assert!(
            on_calls
                .iter()
                .any(|&(ptr, norm)| ptr == params_on.gain_match.as_ptr() && norm >= 0.5),
            "Gain Match button must remain editable under opacity 0.55 while bypassed"
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
        // "[DEV 1d2c5b9-dirty]" on the left + "[BYPASS: OFF]" + "[CLIP: ACTIVE - CLICK TO RESET]" on the right
        struct NoopGuiContext;
        impl GuiContext for NoopGuiContext {
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

        fn find_text_rect_header(
            shape: &egui::epaint::Shape,
            needle: &str,
        ) -> Option<egui::Rect> {
            match shape {
                egui::epaint::Shape::Text(text_shape) => {
                    if text_shape.galley.text().contains(needle) {
                        Some(text_shape.visual_bounding_rect())
                    } else {
                        None
                    }
                }
                egui::epaint::Shape::Vec(items) => items
                    .iter()
                    .find_map(|item| find_text_rect_header(item, needle)),
                _ => None,
            }
        }

        for bypass_state in [false, true] {
            let ctx = egui::Context::default();
            let telemetry = MeterTelemetry::default();
            telemetry.clip_latched.store(true, Ordering::Relaxed);
            let params = OmorePunchParams {
                bypass: BoolParam::new("Bypass", bypass_state).make_bypass(),
                ..OmorePunchParams::default()
            };
            let noop_gui = NoopGuiContext;
            let setter = ParamSetter::new(&noop_gui);
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
                    draw_header_bar(
                        ctx,
                        &setter,
                        &params,
                        &telemetry,
                        "0.2.1-dev",
                        "1d2c5b9-dirty",
                    );
                    egui::CentralPanel::default().show(ctx, |_| {});
                }));
            }
            let used = ctx.used_rect();
            assert!(
                used.width() <= UI_WINDOW_WIDTH as f32,
                "Rendered egui width ({}) with [DEV 1d2c5b9-dirty] + bypass + latched clip exceeds window width ({})",
                used.width(),
                UI_WINDOW_WIDTH
            );
            let shapes = final_output.expect("egui frame output missing").shapes;
            let max_inner_right = shapes
                .iter()
                .filter_map(|clipped| {
                    let rect = clipped.shape.visual_bounding_rect();
                    (rect.width() < UI_WINDOW_WIDTH as f32 - 1.0).then_some(rect.right())
                })
                .fold(0.0_f32, f32::max);
            assert!(
                max_inner_right <= UI_WINDOW_WIDTH as f32 - 6.0,
                "Inner widgets right ({}) with [DEV 1d2c5b9-dirty] + bypass + latched clip exceeds padded window width ({})",
                max_inner_right,
                UI_WINDOW_WIDTH as f32 - 6.0
            );

            // Preserve constitutional labels intact and ensure single-line non-overlapping header
            let subtitle_rect = shapes
                .iter()
                .find_map(|c| find_text_rect_header(&c.shape, "STEREO DRUM BUS COMPRESSOR & CLIPPER"))
                .expect("Constitutional header subtitle missing");
            let clip_rect = shapes
                .iter()
                .find_map(|c| find_text_rect_header(&c.shape, "[CLIP: ACTIVE - CLICK TO RESET]"))
                .expect("Constitutional clip reset button missing");
            let bypass_label = if bypass_state {
                "[BYPASS: ON]"
            } else {
                "[BYPASS: OFF]"
            };
            let bypass_rect = shapes
                .iter()
                .find_map(|c| find_text_rect_header(&c.shape, bypass_label))
                .expect("Header bypass button missing");

            assert!(
                subtitle_rect.right() < bypass_rect.left(),
                "Header left subtitle ({}) overlaps bypass button ({})",
                subtitle_rect.right(),
                bypass_rect.left()
            );
            assert!(
                bypass_rect.right() < clip_rect.left(),
                "Header bypass button ({}) overlaps clip button ({})",
                bypass_rect.right(),
                clip_rect.left()
            );
            assert!(
                subtitle_rect.bottom().max(bypass_rect.bottom()).max(clip_rect.bottom()) <= 36.0,
                "Header widgets wrapped unexpectedly onto multiple lines"
            );
        }
    }

    #[test]
    fn test_editor_size_ignores_legacy_persisted_editor_state() {
        let plugin = OmorePunch::default();
        assert_eq!(
            plugin.editor_state.size(),
            (UI_WINDOW_WIDTH, UI_WINDOW_HEIGHT)
        );

        // Serialized params must not persist fixed window dimensions across plugin upgrades
        assert!(
            !plugin.params.serialize_fields().contains_key("editor-state"),
            "Fixed-size editor_state must not be persisted in DAW params state"
        );

        // Restoring a legacy DAW state (e.g. v0.2.0 820x560 or v0.3.x 820x650) must keep current dimensions
        for legacy_json in [
            r#"{"size":[820,650]}"#,
            r#"{"size":[820,560]}"#,
            r#"{"size":[820,480]}"#,
        ] {
            let mut legacy_fields = std::collections::BTreeMap::new();
            legacy_fields.insert("editor-state".to_string(), legacy_json.to_string());
            plugin.params.deserialize_fields(&legacy_fields);
            assert_eq!(
                plugin.editor_state.size(),
                (UI_WINDOW_WIDTH, UI_WINDOW_HEIGHT),
                "Legacy editor-state '{}' must not shrink window below {}x{}",
                legacy_json,
                UI_WINDOW_WIDTH,
                UI_WINDOW_HEIGHT
            );
        }
    }

    fn measure_recovery_samples_to_90pct(
        sample_rate: f32,
        mode: ReleaseMode,
        burst_ms: f32,
    ) -> (usize, f32) {
        let mut dsp = StereoPunchDsp::new(sample_rate);
        let cfg = DspConfig {
            input_db: 0.0,
            threshold_db: -24.0,
            ratio: 8.0,
            attack_ms: 1.0,
            release_ms: 80.0,
            release_mode: mode,
            sc_hpf_hz: 20.0,
            auto_makeup: false,
            clipper_mode: ClipperMode::Bypass,
            ..DspConfig::default()
        };

        let burst_samples = ((burst_ms * 0.001) * sample_rate).round() as usize;
        let mut peak_gr = 0.0_f32;
        for n in 0..burst_samples {
            let phase = 2.0 * std::f32::consts::PI * 1000.0 * (n as f32) / sample_rate;
            let s = 0.8_f32 * phase.sin();
            let out = dsp.process_sample(s, s, &cfg);
            peak_gr = peak_gr.max(out.gr_db);
        }

        let target_recovered_gr = peak_gr * 0.10;
        let max_release_samples = (6.0 * sample_rate).round() as usize;
        for n in 1..=max_release_samples {
            let out = dsp.process_sample(0.0, 0.0, &cfg);
            if out.gr_db <= target_recovered_gr {
                return (n, peak_gr);
            }
        }

        (max_release_samples, peak_gr)
    }

    #[test]
    fn test_auto_release_transient_vs_sustained_recovery() {
        for mode in [
            ReleaseMode::AutoTight,
            ReleaseMode::AutoPunch,
            ReleaseMode::AutoGlue,
        ] {
            let mut short_ms_per_sr = Vec::new();
            let mut long_ms_per_sr = Vec::new();

            for sr in [44100.0_f32, 48000.0_f32, 96000.0_f32] {
                let (short_samples, short_peak) =
                    measure_recovery_samples_to_90pct(sr, mode, 15.0);
                let (long_samples, long_peak) =
                    measure_recovery_samples_to_90pct(sr, mode, 400.0);

                assert!(
                    short_peak > 8.0 && long_peak > 8.0,
                    "Mode {:?} sr={}: expected > 8 dB GR (short={:.2}, long={:.2})",
                    mode,
                    sr,
                    short_peak,
                    long_peak
                );
                assert!(
                    (long_samples as f32) >= (short_samples as f32) * 2.0,
                    "Mode {:?} sr={}: expected short burst recovery ({} samp) >= 2x faster than sustained ({} samp)",
                    mode,
                    sr,
                    short_samples,
                    long_samples
                );

                short_ms_per_sr.push((short_samples as f32) * 1000.0 / sr);
                long_ms_per_sr.push((long_samples as f32) * 1000.0 / sr);
            }

            let short_spread = short_ms_per_sr
                .iter()
                .copied()
                .fold(0.0_f32, f32::max)
                - short_ms_per_sr
                    .iter()
                    .copied()
                    .fold(f32::INFINITY, f32::min);
            let long_spread = long_ms_per_sr
                .iter()
                .copied()
                .fold(0.0_f32, f32::max)
                - long_ms_per_sr
                    .iter()
                    .copied()
                    .fold(f32::INFINITY, f32::min);
            assert!(
                short_spread < 10.0,
                "Mode {:?}: short recovery time spread across 44.1/48/96k too large ({:.2} ms)",
                mode,
                short_spread
            );
            assert!(
                long_spread < 25.0,
                "Mode {:?}: sustained recovery time spread across 44.1/48/96k too large ({:.2} ms)",
                mode,
                long_spread
            );
        }
    }

    #[test]
    fn test_auto_release_profile_ordering() {
        let sr = 48000.0_f32;
        let (tight_short, _) = measure_recovery_samples_to_90pct(sr, ReleaseMode::AutoTight, 15.0);
        let (punch_short, _) = measure_recovery_samples_to_90pct(sr, ReleaseMode::AutoPunch, 15.0);
        let (glue_short, _) = measure_recovery_samples_to_90pct(sr, ReleaseMode::AutoGlue, 15.0);
        assert!(
            tight_short < punch_short && punch_short < glue_short,
            "Short burst recovery must order AutoTight ({}) < AutoPunch ({}) < AutoGlue ({})",
            tight_short,
            punch_short,
            glue_short
        );

        let (tight_long, _) = measure_recovery_samples_to_90pct(sr, ReleaseMode::AutoTight, 400.0);
        let (punch_long, _) = measure_recovery_samples_to_90pct(sr, ReleaseMode::AutoPunch, 400.0);
        let (glue_long, _) = measure_recovery_samples_to_90pct(sr, ReleaseMode::AutoGlue, 400.0);
        assert!(
            tight_long < punch_long && punch_long < glue_long,
            "Sustained recovery must order AutoTight ({}) < AutoPunch ({}) < AutoGlue ({})",
            tight_long,
            punch_long,
            glue_long
        );
    }

    #[test]
    fn test_auto_release_manual_bit_exact_compatibility() {
        let sr = 48000.0_f32;

        // 1. In Manual mode, manual release_ms controls recovery directly via single exponential stage
        let (manual_fast, _) =
            measure_recovery_samples_to_90pct(sr, ReleaseMode::Manual, 400.0);
        let mut dsp_manual_slow = StereoPunchDsp::new(sr);
        let cfg_manual_slow = DspConfig {
            input_db: 0.0,
            threshold_db: -24.0,
            ratio: 8.0,
            attack_ms: 1.0,
            release_ms: 320.0,
            release_mode: ReleaseMode::Manual,
            sc_hpf_hz: 20.0,
            auto_makeup: false,
            clipper_mode: ClipperMode::Bypass,
            ..DspConfig::default()
        };
        let burst = (0.4 * sr) as usize;
        let mut peak = 0.0_f32;
        for n in 0..burst {
            let phase = 2.0 * std::f32::consts::PI * 1000.0 * (n as f32) / sr;
            let s = 0.8_f32 * phase.sin();
            peak = peak.max(dsp_manual_slow.process_sample(s, s, &cfg_manual_slow).gr_db);
        }
        let mut manual_slow = 0_usize;
        for n in 1..=(6.0 * sr) as usize {
            if dsp_manual_slow
                .process_sample(0.0, 0.0, &cfg_manual_slow)
                .gr_db
                <= peak * 0.10
            {
                manual_slow = n;
                break;
            }
        }
        assert!(
            manual_slow > manual_fast * 3,
            "Manual mode must scale with manual release_ms (80ms={} vs 320ms={})",
            manual_fast,
            manual_slow
        );

        // Verify Manual release follows single-stage exponential decay from steady GR
        let mut dsp_exp = StereoPunchDsp::new(sr);
        let cfg_manual_80 = DspConfig {
            release_ms: 80.0,
            ..cfg_manual_slow
        };
        for _ in 0..2000 {
            dsp_exp.process_sample(0.8, 0.8, &cfg_manual_80);
        }
        let gr_start = dsp_exp.env_gr_db;
        let coeff = (-1.0_f32 / (0.080 * sr)).exp();
        for step in 1..=2000 {
            let out = dsp_exp.process_sample(0.0, 0.0, &cfg_manual_80);
            let expected = gr_start * coeff.powi(step);
            assert!(
                (out.gr_db - expected).abs() < 1.0e-4,
                "Manual release step {} deviated from single-stage exponential: got {} expected {}",
                step,
                out.gr_db,
                expected
            );
        }

        // 2. In every Auto-Release mode, manual release_ms has zero effect on output or GR
        for mode in [
            ReleaseMode::AutoTight,
            ReleaseMode::AutoPunch,
            ReleaseMode::AutoGlue,
        ] {
            let mut dsp_a = StereoPunchDsp::new(sr);
            let mut dsp_b = StereoPunchDsp::new(sr);
            let cfg_a = DspConfig {
                release_ms: 10.0,
                release_mode: mode,
                ..cfg_manual_slow
            };
            let cfg_b = DspConfig {
                release_ms: 500.0,
                release_mode: mode,
                ..cfg_manual_slow
            };

            for n in 0..20000 {
                let s = if n < 4800 {
                    let phase = 2.0 * std::f32::consts::PI * 1000.0 * (n as f32) / sr;
                    0.8 * phase.sin()
                } else {
                    0.0
                };
                let out_a = dsp_a.process_sample(s, -s * 0.7, &cfg_a);
                let out_b = dsp_b.process_sample(s, -s * 0.7, &cfg_b);
                assert_eq!(out_a.out_l.to_bits(), out_b.out_l.to_bits());
                assert_eq!(out_a.out_r.to_bits(), out_b.out_r.to_bits());
                assert_eq!(out_a.gr_db.to_bits(), out_b.gr_db.to_bits());
            }
        }
    }

    #[test]
    fn test_auto_release_mode_switch_continuity_and_stereo_link() {
        let sr = 48000.0_f32;
        let mut dsp = StereoPunchDsp::new(sr);
        let schedule = [
            ReleaseMode::Manual,
            ReleaseMode::AutoTight,
            ReleaseMode::AutoPunch,
            ReleaseMode::AutoGlue,
            ReleaseMode::Manual,
        ];

        let mut cfg = DspConfig {
            input_db: 0.0,
            threshold_db: -24.0,
            ratio: 8.0,
            attack_ms: 1.0,
            release_ms: 80.0,
            release_mode: ReleaseMode::Manual,
            sc_hpf_hz: 20.0,
            auto_makeup: false,
            clipper_mode: ClipperMode::Bypass,
            ..DspConfig::default()
        };

        let mut prev_gr_db = 0.0_f32;
        let mut max_gr_jump_db = 0.0_f32;

        // Exercise transitions both during active sustained compression and during release
        for phase_idx in 0..2 {
            for (seg_idx, &mode) in schedule.iter().enumerate() {
                cfg.release_mode = mode;
                let seg_len = 2400_usize;
                for n in 0..seg_len {
                    let global_n = (phase_idx * schedule.len() + seg_idx) * seg_len + n;
                    let amp = if phase_idx == 0 || (seg_idx % 2 == 0) {
                        0.8_f32
                    } else {
                        0.05_f32
                    };
                    let phase = 2.0 * std::f32::consts::PI * 1000.0 * (global_n as f32) / sr;
                    let s = phase.sin();
                    let in_l = amp * s;
                    let in_r = (amp * 0.25) * s;

                    let out = dsp.process_sample(in_l, in_r, &cfg);
                    assert!(out.out_l.is_finite() && out.out_r.is_finite());
                    assert!(out.gr_db.is_finite());

                    if global_n > 0 {
                        let jump = (out.gr_db - prev_gr_db).abs();
                        max_gr_jump_db = max_gr_jump_db.max(jump);
                    }
                    prev_gr_db = out.gr_db;

                    // Stereo-linked detector must apply identical gain to L and R despite 12 dB offset
                    assert!(
                        (out.out_r - 0.25 * out.out_l).abs() < 1.0e-5,
                        "Stereo link imbalance at mode {:?} sample {}: L={}, R={}",
                        mode,
                        global_n,
                        out.out_l,
                        out.out_r
                    );
                }
            }
        }

        assert!(
            max_gr_jump_db <= 1.0,
            "Sample-to-sample GR jump {} dB exceeded 1.0 dB threshold during mode switches",
            max_gr_jump_db
        );
    }

    #[test]
    fn test_plugin_metadata_and_daw_compat_ids() {
        assert_eq!(OmorePunch::NAME, "Omore Punch");
        assert_eq!(OmorePunch::VENDOR, "Omore");
        assert_eq!(OmorePunch::VERSION, env!("CARGO_PKG_VERSION"));
        assert_eq!(OmorePunch::VST3_CLASS_ID, *b"OmorePunchBusV01");
        assert_eq!(OmorePunch::CLAP_ID, "local.omore-audio.omore-punch");
        assert_eq!(
            OmorePunch::VST3_SUBCATEGORIES,
            &[
                Vst3SubCategory::Fx,
                Vst3SubCategory::Dynamics,
                Vst3SubCategory::Distortion,
                Vst3SubCategory::Stereo,
            ]
        );
    }

    fn measure_gain_match_rms_db(
        sr: f32,
        cfg: &DspConfig,
        freq_hz: f32,
        amp_l: f32,
        amp_r: f32,
        settle_sec: f32,
        measure_sec: f32,
    ) -> (f32, f32, f32, f32, f32) {
        let mut dsp = StereoPunchDsp::new(sr);
        let settle_samples = (settle_sec * sr).round() as usize;
        let measure_samples = (measure_sec * sr).round() as usize;

        let mut in_sq_l = 0.0_f64;
        let mut in_sq_r = 0.0_f64;
        let mut out_sq_l = 0.0_f64;
        let mut out_sq_r = 0.0_f64;

        for n in 0..(settle_samples + measure_samples) {
            let phase = 2.0 * std::f32::consts::PI * freq_hz * (n as f32) / sr;
            let s = phase.sin();
            let in_l = amp_l * s;
            let in_r = amp_r * s;
            let out = dsp.process_sample(in_l, in_r, cfg);
            assert!(out.out_l.is_finite() && out.out_r.is_finite());

            if n >= settle_samples {
                in_sq_l += (in_l as f64) * (in_l as f64);
                in_sq_r += (in_r as f64) * (in_r as f64);
                out_sq_l += (out.out_l as f64) * (out.out_l as f64);
                out_sq_r += (out.out_r as f64) * (out.out_r as f64);
            }
        }

        let inv_n = 1.0 / (measure_samples as f64);
        let in_rms_l = (in_sq_l * inv_n).sqrt() as f32;
        let in_rms_r = (in_sq_r * inv_n).sqrt() as f32;
        let out_rms_l = (out_sq_l * inv_n).sqrt() as f32;
        let out_rms_r = (out_sq_r * inv_n).sqrt() as f32;

        let in_total_rms = (0.5 * (in_sq_l + in_sq_r) * inv_n).sqrt() as f32;
        let out_total_rms = (0.5 * (out_sq_l + out_sq_r) * inv_n).sqrt() as f32;
        let diff_db = 20.0 * (out_total_rms / in_total_rms.max(1.0e-12)).log10();

        (diff_db, in_rms_l, in_rms_r, out_rms_l, out_rms_r)
    }

    #[test]
    fn test_gain_match_post_processing_convergence_and_stereo_balance() {
        let scenarios = [
            (
                "heavy_comp_no_makeup",
                DspConfig {
                    input_db: 0.0,
                    threshold_db: -24.0,
                    ratio: 8.0,
                    auto_makeup: false,
                    makeup_db: 0.0,
                    clipper_mode: ClipperMode::Bypass,
                    gain_match: true,
                    ..DspConfig::default()
                },
                1000.0_f32,
                0.4_f32,
                0.4_f32,
            ),
            (
                "eq_auto_makeup_drive_soft_clip",
                DspConfig {
                    input_db: 3.0,
                    threshold_db: -18.0,
                    ratio: 4.0,
                    auto_makeup: true,
                    makeup_db: 6.0,
                    eq_high_shelf_db: 9.0,
                    clipper_mode: ClipperMode::SoftClip,
                    ceiling_db: -3.0,
                    gain_match: true,
                    ..DspConfig::default()
                },
                8000.0_f32,
                0.3_f32,
                0.3_f32,
            ),
            (
                "hard_clip_low_ceiling_post_clipper",
                DspConfig {
                    input_db: 6.0,
                    threshold_db: 0.0,
                    ratio: 1.0,
                    auto_makeup: false,
                    makeup_db: 0.0,
                    clipper_mode: ClipperMode::HardClip,
                    ceiling_db: -6.0,
                    gain_match: true,
                    ..DspConfig::default()
                },
                500.0_f32,
                0.5_f32,
                0.5_f32,
            ),
            (
                "eq_500hz_cut",
                DspConfig {
                    threshold_db: 0.0,
                    ratio: 1.0,
                    auto_makeup: false,
                    eq_mid_cut_db: -9.0,
                    clipper_mode: ClipperMode::Bypass,
                    gain_match: true,
                    ..DspConfig::default()
                },
                500.0_f32,
                0.35_f32,
                0.35_f32,
            ),
        ];

        for sr in [44100.0_f32, 48000.0_f32, 96000.0_f32] {
            for (name, cfg, freq_hz, amp_l, amp_r) in &scenarios {
                let (diff_db, _, _, _, _) =
                    measure_gain_match_rms_db(sr, cfg, *freq_hz, *amp_l, *amp_r, 2.5, 0.5);
                assert!(
                    diff_db.abs() <= 0.5,
                    "sr={} scenario '{}': RMS mismatch {:.3} dB exceeded 0.5 dB",
                    sr,
                    name,
                    diff_db
                );
            }
        }

        // Stereo balance must remain identical (0.0 dB shift) with asymmetric L/R signal
        let asym_cfg = DspConfig {
            threshold_db: -20.0,
            ratio: 4.0,
            auto_makeup: true,
            makeup_db: 3.0,
            eq_high_shelf_db: 6.0,
            clipper_mode: ClipperMode::Bypass,
            gain_match: true,
            ..DspConfig::default()
        };
        let (diff_db, in_l, in_r, out_l, out_r) =
            measure_gain_match_rms_db(48000.0, &asym_cfg, 1000.0, 0.5, 0.15, 2.5, 0.5);
        assert!(diff_db.abs() <= 0.5);
        let in_lr_db = 20.0 * (in_l / in_r).log10();
        let out_lr_db = 20.0 * (out_l / out_r).log10();
        assert!(
            (in_lr_db - out_lr_db).abs() < 1.0e-3,
            "Gain Match altered L/R stereo balance: in={:.4} dB vs out={:.4} dB",
            in_lr_db,
            out_lr_db
        );

        // Compensation bounds [-24, +24] dB and final safety clamp [-4.0, 4.0]
        let mut dsp_extreme = StereoPunchDsp::new(48000.0);
        let extreme_drop_cfg = DspConfig {
            input_db: -12.0,
            threshold_db: -40.0,
            ratio: 20.0,
            auto_makeup: false,
            makeup_db: -12.0,
            clipper_mode: ClipperMode::HardClip,
            ceiling_db: -6.0,
            gain_match: true,
            ..DspConfig::default()
        };
        let mut max_observed_trim = 0.0_f32;
        for n in 0..192000 {
            let phase = 2.0 * std::f32::consts::PI * 1000.0 * (n as f32) / 48000.0;
            let s = 0.9 * phase.sin();
            let out = dsp_extreme.process_sample(s, s, &extreme_drop_cfg);
            assert!(out.out_l.abs() <= SAFETY_CLAMP_LIMIT);
            assert!(out.out_r.abs() <= SAFETY_CLAMP_LIMIT);
            assert!(
                out.gain_match_db >= GAIN_MATCH_MIN_DB && out.gain_match_db <= GAIN_MATCH_MAX_DB
            );
            max_observed_trim = max_observed_trim.max(out.gain_match_db);
        }
        assert!(
            (max_observed_trim - GAIN_MATCH_MAX_DB).abs() < 0.1,
            "Expected positive trim to saturate cleanly at +24.0 dB, got {:.3} dB",
            max_observed_trim
        );
    }

    #[test]
    fn test_gain_match_smooth_toggle_and_defaults() {
        assert!(!DspConfig::default().gain_match);
        assert!(!OmorePunchParams::default().gain_match.value());

        let sr = 48000.0_f32;
        let mut dsp = StereoPunchDsp::new(sr);
        let mut cfg = DspConfig {
            threshold_db: 0.0,
            ratio: 1.0,
            auto_makeup: false,
            makeup_db: 6.0,
            clipper_mode: ClipperMode::Bypass,
            gain_match: false,
            ..DspConfig::default()
        };

        // Phase 1: Gain Match OFF -> trim stays strictly 0.0
        let mut prev_trim_db = 0.0_f32;
        let mut max_step_db = 0.0_f32;
        for n in 0..24000 {
            let phase = 2.0 * std::f32::consts::PI * 1000.0 * (n as f32) / sr;
            let s = 0.25 * phase.sin();
            let out = dsp.process_sample(s, s, &cfg);
            assert_eq!(out.gain_match_db, 0.0);
        }

        // Phase 2: Toggle ON -> smooth convergence toward -6.0 dB with no abrupt jumps
        cfg.gain_match = true;
        for n in 0..72000 {
            let phase = 2.0 * std::f32::consts::PI * 1000.0 * (n as f32) / sr;
            let s = 0.25 * phase.sin();
            let out = dsp.process_sample(s, s, &cfg);
            max_step_db = max_step_db.max((out.gain_match_db - prev_trim_db).abs());
            prev_trim_db = out.gain_match_db;
        }
        assert!(
            (prev_trim_db + 6.0).abs() < 0.25,
            "Expected convergence near -6.0 dB after toggle ON, got {:.3} dB",
            prev_trim_db
        );

        // Phase 3: Toggle OFF -> smooth return to exact 0.0 dB
        cfg.gain_match = false;
        for n in 0..48000 {
            let phase = 2.0 * std::f32::consts::PI * 1000.0 * (n as f32) / sr;
            let s = 0.25 * phase.sin();
            let out = dsp.process_sample(s, s, &cfg);
            max_step_db = max_step_db.max((out.gain_match_db - prev_trim_db).abs());
            prev_trim_db = out.gain_match_db;
        }
        assert_eq!(
            prev_trim_db, 0.0,
            "Expected exact 0.0 dB once settled after toggle OFF"
        );
        assert!(
            max_step_db < 0.01,
            "Sample-to-sample trim step ({:.5} dB) too steep during ON/OFF transitions",
            max_step_db
        );

        // Verify Gain Match flags out.clipped if final matched output exceeds 0 dBFS (> 1.0)
        // even with ClipperMode::Bypass
        let mut dsp_clip = StereoPunchDsp::new(sr);
        let clip_flag_cfg = DspConfig {
            input_db: 0.0,
            threshold_db: 0.0,
            ratio: 1.0,
            auto_makeup: false,
            makeup_db: -12.0,
            clipper_mode: ClipperMode::Bypass,
            gain_match: true,
            ..DspConfig::default()
        };
        let mut saw_over_zero_dbfs = false;
        let mut saw_clipped_flag = false;
        for n in 0..72000 {
            let phase = 2.0 * std::f32::consts::PI * 500.0 * (n as f32) / sr;
            let s = 1.5 * phase.sin();
            let out = dsp_clip.process_sample(s, s, &clip_flag_cfg);
            if out.out_l.abs() > 1.0 || out.out_r.abs() > 1.0 {
                saw_over_zero_dbfs = true;
                if out.clipped {
                    saw_clipped_flag = true;
                }
            }
        }
        assert!(saw_over_zero_dbfs && saw_clipped_flag);
    }

    #[test]
    fn test_gain_match_silence_hold_and_transient_preservation() {
        let sr = 48000.0_f32;

        // 1. Cold-start silence / sub-threshold noise (-70 dBFS) stays strictly at 0.0 dB
        let mut dsp_cold = StereoPunchDsp::new(sr);
        let cold_cfg = DspConfig {
            threshold_db: 0.0,
            ratio: 1.0,
            auto_makeup: false,
            makeup_db: -12.0,
            clipper_mode: ClipperMode::Bypass,
            gain_match: true,
            ..DspConfig::default()
        };
        let noise_amp_70dbfs = db_to_lin(-70.0);
        for n in 0..96000 {
            let phase = 2.0 * std::f32::consts::PI * 1000.0 * (n as f32) / sr;
            let s = noise_amp_70dbfs * phase.sin();
            let out = dsp_cold.process_sample(s, s, &cold_cfg);
            assert_eq!(
                out.gain_match_db, 0.0,
                "Cold-start -70 dBFS noise must not trigger Gain Match boost (sample {})",
                n
            );
        }

        // 2. Mid-stream silence holds last valid trim without drifting toward +24 dB
        let mut dsp_hold = StereoPunchDsp::new(sr);
        let active_cfg = DspConfig {
            threshold_db: 0.0,
            ratio: 1.0,
            auto_makeup: false,
            makeup_db: 8.0,
            clipper_mode: ClipperMode::Bypass,
            gain_match: true,
            ..DspConfig::default()
        };
        for n in 0..96000 {
            let phase = 2.0 * std::f32::consts::PI * 1000.0 * (n as f32) / sr;
            let s = 0.25 * phase.sin();
            dsp_hold.process_sample(s, s, &active_cfg);
        }
        let converged_trim = dsp_hold.gm_gain_db;
        assert!(
            (converged_trim + 8.0).abs() < 0.25,
            "Expected ~ -8.0 dB trim before break, got {:.3} dB",
            converged_trim
        );

        // Feed 3 seconds of -70 dBFS noise / silence: trim must hold near converged_trim
        for n in 0..144000 {
            let phase = 2.0 * std::f32::consts::PI * 1000.0 * (n as f32) / sr;
            let s = if n < 72000 {
                noise_amp_70dbfs * phase.sin()
            } else {
                0.0
            };
            let out = dsp_hold.process_sample(s, s, &active_cfg);
            assert!(
                (out.gain_match_db - converged_trim).abs() < 0.1,
                "Hold drifted during silence at sample {}: got {:.3} dB vs {:.3} dB",
                n,
                out.gain_match_db,
                converged_trim
            );
        }

        // First active sample on resume uses held trim immediately
        let resume_out = dsp_hold.process_sample(0.25, 0.25, &active_cfg);
        assert!((resume_out.gain_match_db - converged_trim).abs() < 0.1);

        // 3. Percussive drum bursts preserve transient punch without rapid pumping
        let mut dsp_drums = StereoPunchDsp::new(sr);
        let drum_cfg = DspConfig {
            threshold_db: -20.0,
            ratio: 4.0,
            attack_ms: 5.0,
            release_ms: 80.0,
            auto_makeup: false,
            makeup_db: 0.0,
            clipper_mode: ClipperMode::SoftClip,
            ceiling_db: 0.0,
            gain_match: true,
            ..DspConfig::default()
        };
        let period = (0.4 * sr) as usize;
        let hit_len = (0.015 * sr) as usize;
        let mut attack_trim_span = 0.0_f32;
        for beat in 0..12 {
            let mut min_hit_trim = f32::INFINITY;
            let mut max_hit_trim = f32::NEG_INFINITY;
            for idx in 0..period {
                let amp = if idx < hit_len { 0.6 } else { 0.025 };
                let phase =
                    2.0 * std::f32::consts::PI * 180.0 * ((beat * period + idx) as f32) / sr;
                let s = amp * phase.sin();
                let out = dsp_drums.process_sample(s, s, &drum_cfg);
                if beat >= 8 && idx < hit_len {
                    min_hit_trim = min_hit_trim.min(out.gain_match_db);
                    max_hit_trim = max_hit_trim.max(out.gain_match_db);
                }
            }
            if beat >= 8 {
                attack_trim_span = attack_trim_span.max(max_hit_trim - min_hit_trim);
            }
        }
        assert!(
            attack_trim_span < 0.25,
            "Gain Match modulated too rapidly across 15 ms drum hit ({:.3} dB)",
            attack_trim_span
        );
    }

    #[repr(C)]
    struct TestClapAudioBuffer {
        data32: *mut *mut f32,
        data64: *mut *mut f64,
        channel_count: u32,
        latency: u32,
        constant_mask: u64,
    }

    #[repr(C)]
    struct TestClapInputEvents {
        ctx: *mut std::ffi::c_void,
        size: Option<unsafe extern "C" fn(list: *const TestClapInputEvents) -> u32>,
        get: Option<
            unsafe extern "C" fn(
                list: *const TestClapInputEvents,
                index: u32,
            ) -> *const std::ffi::c_void,
        >,
    }

    #[repr(C)]
    struct TestClapOutputEvents {
        ctx: *mut std::ffi::c_void,
        try_push: Option<
            unsafe extern "C" fn(
                list: *const TestClapOutputEvents,
                event: *const std::ffi::c_void,
            ) -> bool,
        >,
    }

    #[repr(C)]
    struct TestClapProcess {
        steady_time: i64,
        frames_count: u32,
        transport: *const std::ffi::c_void,
        audio_inputs: *const TestClapAudioBuffer,
        audio_outputs: *mut TestClapAudioBuffer,
        audio_inputs_count: u32,
        audio_outputs_count: u32,
        in_events: *const TestClapInputEvents,
        out_events: *const TestClapOutputEvents,
    }

    #[repr(C)]
    struct TestClapParamInfo {
        id: u32,
        flags: u32,
        cookie: *mut std::ffi::c_void,
        name: [std::ffi::c_char; 256],
        module: [std::ffi::c_char; 1024],
        min_value: f64,
        max_value: f64,
        default_value: f64,
    }

    #[repr(C)]
    struct TestClapPluginParams {
        count: Option<
            unsafe extern "C" fn(plugin: *const nih_plug::wrapper::clap::clap_plugin) -> u32,
        >,
        get_info: Option<
            unsafe extern "C" fn(
                plugin: *const nih_plug::wrapper::clap::clap_plugin,
                param_index: u32,
                param_info: *mut TestClapParamInfo,
            ) -> bool,
        >,
        get_value: Option<
            unsafe extern "C" fn(
                plugin: *const nih_plug::wrapper::clap::clap_plugin,
                param_id: u32,
                out_value: *mut f64,
            ) -> bool,
        >,
        value_to_text: Option<
            unsafe extern "C" fn(
                plugin: *const nih_plug::wrapper::clap::clap_plugin,
                param_id: u32,
                value: f64,
                out_buffer: *mut std::ffi::c_char,
                out_buffer_capacity: u32,
            ) -> bool,
        >,
        text_to_value: Option<
            unsafe extern "C" fn(
                plugin: *const nih_plug::wrapper::clap::clap_plugin,
                param_id: u32,
                param_value_text: *const std::ffi::c_char,
                out_value: *mut f64,
            ) -> bool,
        >,
        flush: Option<
            unsafe extern "C" fn(
                plugin: *const nih_plug::wrapper::clap::clap_plugin,
                in_: *const TestClapInputEvents,
                out: *const TestClapOutputEvents,
            ),
        >,
    }

    unsafe extern "C" fn test_clap_host_get_extension(
        _host: *const nih_plug::wrapper::clap::clap_host,
        _extension_id: *const std::ffi::c_char,
    ) -> *const std::ffi::c_void {
        std::ptr::null()
    }

    unsafe extern "C" fn test_clap_host_noop(_host: *const nih_plug::wrapper::clap::clap_host) {}

    unsafe extern "C" fn test_clap_in_events_size(_list: *const TestClapInputEvents) -> u32 {
        0
    }

    unsafe extern "C" fn test_clap_in_events_get(
        _list: *const TestClapInputEvents,
        _index: u32,
    ) -> *const std::ffi::c_void {
        std::ptr::null()
    }

    unsafe extern "C" fn test_clap_out_events_push(
        _list: *const TestClapOutputEvents,
        _event: *const std::ffi::c_void,
    ) -> bool {
        true
    }

    fn process_clap_stereo_block(
        plugin: *const nih_plug::wrapper::clap::clap_plugin,
        sample_rate: f64,
        steady_time: i64,
        left: &[f32],
        right: &[f32],
    ) {
        assert_eq!(left.len(), right.len());
        let frames = left.len();
        let mut in_l = left.to_vec();
        let mut in_r = right.to_vec();
        let mut out_l = vec![0.0_f32; frames];
        let mut out_r = vec![0.0_f32; frames];

        let mut in_ptrs = [in_l.as_mut_ptr(), in_r.as_mut_ptr()];
        let mut out_ptrs = [out_l.as_mut_ptr(), out_r.as_mut_ptr()];

        let in_buf = TestClapAudioBuffer {
            data32: in_ptrs.as_mut_ptr(),
            data64: std::ptr::null_mut(),
            channel_count: 2,
            latency: 0,
            constant_mask: 0,
        };
        let mut out_buf = TestClapAudioBuffer {
            data32: out_ptrs.as_mut_ptr(),
            data64: std::ptr::null_mut(),
            channel_count: 2,
            latency: 0,
            constant_mask: 0,
        };

        let in_events = TestClapInputEvents {
            ctx: std::ptr::null_mut(),
            size: Some(test_clap_in_events_size),
            get: Some(test_clap_in_events_get),
        };
        let out_events = TestClapOutputEvents {
            ctx: std::ptr::null_mut(),
            try_push: Some(test_clap_out_events_push),
        };

        let proc_data = TestClapProcess {
            steady_time,
            frames_count: frames as u32,
            transport: std::ptr::null(),
            audio_inputs: &in_buf,
            audio_outputs: &mut out_buf,
            audio_inputs_count: 1,
            audio_outputs_count: 1,
            in_events: &in_events,
            out_events: &out_events,
        };

        unsafe {
            let process_fn = (*plugin).process.expect("CLAP process callback missing");
            let status = process_fn(
                plugin,
                &proc_data as *const TestClapProcess as *const _,
            );
            assert!(
                status >= 1,
                "Expected valid CLAP process status at {} Hz, got {}",
                sample_rate,
                status
            );
        }
    }

    #[test]
    fn test_clap_gain_adjustment_metering_abi_and_closed_ui() {
        use nih_plug::wrapper::clap::{
            clap_host, clap_plugin_factory, CLAP_PLUGIN_FACTORY_ID, CLAP_VERSION,
        };
        use std::ffi::CStr;

        let host_name = b"ReaperTestHost\0";
        let host_vendor = b"CockosTest\0";
        let host_url = b"https://example.invalid\0";
        let host_ver = b"7.0\0";

        let host = clap_host {
            clap_version: CLAP_VERSION,
            host_data: std::ptr::null_mut(),
            name: host_name.as_ptr() as *const std::ffi::c_char,
            vendor: host_vendor.as_ptr() as *const std::ffi::c_char,
            url: host_url.as_ptr() as *const std::ffi::c_char,
            version: host_ver.as_ptr() as *const std::ffi::c_char,
            get_extension: Some(test_clap_host_get_extension),
            request_restart: Some(test_clap_host_noop),
            request_process: Some(test_clap_host_noop),
            request_callback: Some(test_clap_host_noop),
        };

        unsafe {
            let init_fn = super::clap::clap_entry.init.expect("clap init missing");
            assert!(init_fn(std::ptr::null()));

            let get_factory_fn = super::clap::clap_entry
                .get_factory
                .expect("clap get_factory missing");
            let factory = get_factory_fn(CLAP_PLUGIN_FACTORY_ID.as_ptr())
                as *const clap_plugin_factory;
            assert!(!factory.is_null(), "CLAP factory must not be null");

            let count_fn = (*factory).get_plugin_count.unwrap();
            assert_eq!(count_fn(factory), 1);

            let desc_fn = (*factory).get_plugin_descriptor.unwrap();
            let desc = desc_fn(factory, 0);
            assert!(!desc.is_null());
            assert_eq!(
                CStr::from_ptr((*desc).id).to_str().unwrap(),
                OmorePunch::CLAP_ID
            );

            let create_fn = (*factory).create_plugin.unwrap();
            let plugin_a = create_fn(factory, &host, (*desc).id);
            let plugin_b = create_fn(factory, &host, (*desc).id);
            assert!(!plugin_a.is_null() && !plugin_b.is_null());

            assert!(((*plugin_a).init.unwrap())(plugin_a));
            assert!(((*plugin_b).init.unwrap())(plugin_b));

            let get_ext_a = (*plugin_a).get_extension.unwrap();
            let get_ext_b = (*plugin_b).get_extension.unwrap();
            let gr_ext_id = b"clap.gain-adjustment-metering/0\0";
            let ext_a = get_ext_a(
                plugin_a,
                gr_ext_id.as_ptr() as *const std::ffi::c_char,
            ) as *const super::clap::ClapPluginGainAdjustmentMetering;
            let ext_b = get_ext_b(
                plugin_b,
                gr_ext_id.as_ptr() as *const std::ffi::c_char,
            ) as *const super::clap::ClapPluginGainAdjustmentMetering;
            assert!(!ext_a.is_null() && !ext_b.is_null());
            let get_gr_a = (*ext_a).get.expect("clap GR get callback missing");
            let get_gr_b = (*ext_b).get.expect("clap GR get callback missing");

            // Never call gui extension: UI stays closed throughout the entire test.
            assert!(((*plugin_a).activate.unwrap())(plugin_a, 48000.0, 64, 512));
            assert!(((*plugin_b).activate.unwrap())(plugin_b, 48000.0, 64, 512));
            assert!(((*plugin_a).start_processing.unwrap())(plugin_a));
            assert!(((*plugin_b).start_processing.unwrap())(plugin_b));

            // Initial idle state must read exact +0.0 dB.
            let idle_a = get_gr_a(plugin_a);
            let idle_b = get_gr_b(plugin_b);
            assert_eq!(idle_a.to_bits(), 0.0_f64.to_bits());
            assert_eq!(idle_b.to_bits(), 0.0_f64.to_bits());

            // Below default 0 dB threshold must keep GR at exact +0.0 dB.
            let frames = 512;
            let below_thresh: Vec<f32> = (0..frames)
                .map(|i| {
                    let phase = 2.0 * std::f32::consts::PI * 1000.0 * (i as f32) / 48000.0;
                    0.5 * phase.sin()
                })
                .collect();
            process_clap_stereo_block(plugin_a, 48000.0, 0, &below_thresh, &below_thresh);
            assert_eq!(get_gr_a(plugin_a).to_bits(), 0.0_f64.to_bits());

            // Drive >0 dBFS signal ONLY into instance A; instance B receives silence.
            let mut hot = vec![0.0_f32; frames];
            for (i, s) in hot.iter_mut().enumerate() {
                let phase = 2.0 * std::f32::consts::PI * 1000.0 * (i as f32) / 48000.0;
                *s = 2.5 * phase.sin();
            }
            let silence = vec![0.0_f32; frames];

            for block in 0..8 {
                process_clap_stereo_block(plugin_a, 48000.0, (block * frames) as i64, &hot, &hot);
                process_clap_stereo_block(
                    plugin_b,
                    48000.0,
                    (block * frames) as i64,
                    &silence,
                    &silence,
                );
            }

            let active_a = get_gr_a(plugin_a);
            let active_b = get_gr_b(plugin_b);
            assert!(
                (-MAX_GR_METER_DB as f64..=-0.5).contains(&active_a),
                "Expected active CLAP GR in [-24.0, -0.5] dB with UI closed, got {}",
                active_a
            );
            assert_eq!(
                active_b.to_bits(),
                0.0_f64.to_bits(),
                "Silent second CLAP instance must remain isolated at 0.0 dB"
            );

            // Lifecycle resets must return GR to 0.0 dB.
            ((*plugin_a).stop_processing.unwrap())(plugin_a);
            assert_eq!(get_gr_a(plugin_a).to_bits(), 0.0_f64.to_bits());

            assert!(((*plugin_a).start_processing.unwrap())(plugin_a));
            process_clap_stereo_block(plugin_a, 48000.0, 0, &hot, &hot);
            assert!(get_gr_a(plugin_a) < -0.5);
            ((*plugin_a).deactivate.unwrap())(plugin_a);
            assert_eq!(get_gr_a(plugin_a).to_bits(), 0.0_f64.to_bits());

            assert!(((*plugin_a).activate.unwrap())(plugin_a, 48000.0, 64, 512));
            assert!(((*plugin_a).start_processing.unwrap())(plugin_a));
            process_clap_stereo_block(plugin_a, 48000.0, 0, &hot, &hot);
            assert!(get_gr_a(plugin_a) < -0.5);
            ((*plugin_a).reset.unwrap())(plugin_a);
            assert_eq!(get_gr_a(plugin_a).to_bits(), 0.0_f64.to_bits());

            ((*plugin_a).deactivate.unwrap())(plugin_a);
            ((*plugin_b).stop_processing.unwrap())(plugin_b);
            ((*plugin_b).deactivate.unwrap())(plugin_b);
            ((*plugin_a).destroy.unwrap())(plugin_a);
            ((*plugin_b).destroy.unwrap())(plugin_b);

            if let Some(deinit_fn) = super::clap::clap_entry.deinit {
                deinit_fn();
            }
        }
    }

    fn process_vst3_stereo_block(
        proc_if: &nih_plug::wrapper::vst3::vst3_sys::VstPtr<
            dyn nih_plug::wrapper::vst3::vst3_sys::vst::IAudioProcessor,
        >,
        left: &[f32],
        right: &[f32],
    ) {
        let _ = process_vst3_stereo_block_out(proc_if, left, right);
    }

    fn process_vst3_stereo_block_out(
        proc_if: &nih_plug::wrapper::vst3::vst3_sys::VstPtr<
            dyn nih_plug::wrapper::vst3::vst3_sys::vst::IAudioProcessor,
        >,
        left: &[f32],
        right: &[f32],
    ) -> (Vec<f32>, Vec<f32>) {
        use nih_plug::wrapper::vst3::vst3_sys::base::kResultOk;
        use nih_plug::wrapper::vst3::vst3_sys::vst::{
            AudioBusBuffers, IAudioProcessor, ProcessData, ProcessModes, SymbolicSampleSizes,
        };
        use std::ffi::c_void;

        assert_eq!(left.len(), right.len());
        let frames = left.len();
        let mut in_l = left.to_vec();
        let mut in_r = right.to_vec();
        let mut out_l = vec![0.0_f32; frames];
        let mut out_r = vec![0.0_f32; frames];

        let mut in_ptrs = [
            in_l.as_mut_ptr() as *mut c_void,
            in_r.as_mut_ptr() as *mut c_void,
        ];
        let mut out_ptrs = [
            out_l.as_mut_ptr() as *mut c_void,
            out_r.as_mut_ptr() as *mut c_void,
        ];

        let mut in_bus = AudioBusBuffers {
            num_channels: 2,
            silence_flags: 0,
            buffers: in_ptrs.as_mut_ptr(),
        };
        let mut out_bus = AudioBusBuffers {
            num_channels: 2,
            silence_flags: 0,
            buffers: out_ptrs.as_mut_ptr(),
        };

        unsafe {
            let mut data: ProcessData = std::mem::zeroed();
            data.process_mode = ProcessModes::kRealtime as i32;
            data.symbolic_sample_size = SymbolicSampleSizes::kSample32 as i32;
            data.num_samples = frames as i32;
            data.num_inputs = 1;
            data.num_outputs = 1;
            data.inputs = &mut in_bus;
            data.outputs = &mut out_bus;

            assert_eq!(proc_if.process(&mut data), kResultOk);
        }

        (out_l, out_r)
    }

    #[test]
    fn test_vst3_gain_reduction_info_abi_and_closed_ui() {
        use super::vst3::IGainReductionInfo;
        use nih_plug::prelude::Vst3Plugin;
        use nih_plug::wrapper::vst3::vst3_sys::base::{
            kResultOk, IPluginBase, IPluginFactory, IPluginFactory2, IPluginFactory3, PClassInfo2,
        };
        use nih_plug::wrapper::vst3::vst3_sys::vst::{
            BusDirections, IAudioProcessor, IComponent, IEditController, MediaTypes, ProcessModes,
            ProcessSetup, SymbolicSampleSizes,
        };
        use nih_plug::wrapper::vst3::vst3_sys::{ComInterface, VstPtr, IID};

        unsafe {
            let factory_raw = super::vst3::GetPluginFactory();
            assert!(!factory_raw.is_null());
            let factory = VstPtr::<dyn IPluginFactory>::owned(factory_raw as *mut *mut _)
                .expect("Failed to wrap VST3 factory");

            assert_eq!(factory.count_classes(), 1);
            let factory2 = factory
                .cast::<dyn IPluginFactory2>()
                .expect("IPluginFactory2 missing");
            let _factory3 = factory
                .cast::<dyn IPluginFactory3>()
                .expect("IPluginFactory3 missing");

            let mut info2: PClassInfo2 = std::mem::zeroed();
            assert_eq!(factory2.get_class_info2(0, &mut info2), kResultOk);
            assert_eq!(info2.cid.data, OmorePunch::PLATFORM_VST3_CLASS_ID);

            let cid = IID {
                data: OmorePunch::PLATFORM_VST3_CLASS_ID,
            };

            let mut raw_a = std::ptr::null_mut();
            let mut raw_b = std::ptr::null_mut();
            assert_eq!(
                factory.create_instance(&cid, &<dyn IComponent>::IID, &mut raw_a),
                kResultOk
            );
            assert_eq!(
                factory.create_instance(&cid, &<dyn IComponent>::IID, &mut raw_b),
                kResultOk
            );

            let comp_a = VstPtr::<dyn IComponent>::owned(raw_a as *mut *mut _).unwrap();
            let comp_b = VstPtr::<dyn IComponent>::owned(raw_b as *mut *mut _).unwrap();

            // IGainReductionInfo must be queryable from both IComponent and IEditController.
            let gr_a = comp_a
                .cast::<dyn super::vst3::IGainReductionInfo>()
                .expect("IGainReductionInfo missing on IComponent");
            let edit_a = comp_a
                .cast::<dyn IEditController>()
                .expect("IEditController missing");
            let _gr_a_via_edit = edit_a
                .cast::<dyn super::vst3::IGainReductionInfo>()
                .expect("IGainReductionInfo missing on IEditController");

            let gr_b = comp_b
                .cast::<dyn super::vst3::IGainReductionInfo>()
                .expect("IGainReductionInfo missing on second instance");

            let proc_a = comp_a.cast::<dyn IAudioProcessor>().unwrap();
            let proc_b = comp_b.cast::<dyn IAudioProcessor>().unwrap();

            assert_eq!(comp_a.initialize(std::ptr::null_mut()), kResultOk);
            assert_eq!(comp_b.initialize(std::ptr::null_mut()), kResultOk);

            for comp in [&comp_a, &comp_b] {
                assert_eq!(
                    comp.activate_bus(MediaTypes::kAudio as i32, BusDirections::kInput as i32, 0, 1),
                    kResultOk
                );
                assert_eq!(
                    comp.activate_bus(
                        MediaTypes::kAudio as i32,
                        BusDirections::kOutput as i32,
                        0,
                        1
                    ),
                    kResultOk
                );
            }

            let mut arr_in_a: u64 = 3;
            let mut arr_out_a: u64 = 3;
            let mut arr_in_b: u64 = 3;
            let mut arr_out_b: u64 = 3;
            assert_eq!(
                proc_a.set_bus_arrangements(&mut arr_in_a, 1, &mut arr_out_a, 1),
                kResultOk
            );
            assert_eq!(
                proc_b.set_bus_arrangements(&mut arr_in_b, 1, &mut arr_out_b, 1),
                kResultOk
            );

            let setup = ProcessSetup {
                process_mode: ProcessModes::kRealtime as i32,
                symbolic_sample_size: SymbolicSampleSizes::kSample32 as i32,
                max_samples_per_block: 512,
                sample_rate: 48000.0,
            };
            assert_eq!(proc_a.setup_processing(&setup), kResultOk);
            assert_eq!(proc_b.setup_processing(&setup), kResultOk);
            assert_eq!(comp_a.set_active(1), kResultOk);
            assert_eq!(comp_b.set_active(1), kResultOk);
            assert_eq!(proc_a.set_processing(1), kResultOk);
            assert_eq!(proc_b.set_processing(1), kResultOk);

            // Idle initial state must be exact +0.0 dB without opening create_view().
            assert_eq!(gr_a.get_gain_reduction_value_in_db().to_bits(), 0.0_f64.to_bits());
            assert_eq!(gr_b.get_gain_reduction_value_in_db().to_bits(), 0.0_f64.to_bits());

            // Below default 0 dB threshold must stay at exact +0.0 dB.
            let frames = 512;
            let below_thresh: Vec<f32> = (0..frames)
                .map(|i| {
                    let phase = 2.0 * std::f32::consts::PI * 1000.0 * (i as f32) / 48000.0;
                    0.5 * phase.sin()
                })
                .collect();
            process_vst3_stereo_block(&proc_a, &below_thresh, &below_thresh);
            assert_eq!(gr_a.get_gain_reduction_value_in_db().to_bits(), 0.0_f64.to_bits());

            let mut hot = vec![0.0_f32; frames];
            for (i, s) in hot.iter_mut().enumerate() {
                let phase = 2.0 * std::f32::consts::PI * 1000.0 * (i as f32) / 48000.0;
                *s = 2.5 * phase.sin();
            }
            let silence = vec![0.0_f32; frames];

            for _ in 0..8 {
                process_vst3_stereo_block(&proc_a, &hot, &hot);
                process_vst3_stereo_block(&proc_b, &silence, &silence);
            }

            let active_a = gr_a.get_gain_reduction_value_in_db();
            let active_b = gr_b.get_gain_reduction_value_in_db();
            assert!(
                (-MAX_GR_METER_DB as f64..=-0.5).contains(&active_a),
                "Expected VST3 IGainReductionInfo in [-24.0, -0.5] dB with UI closed, got {}",
                active_a
            );
            assert_eq!(
                active_b.to_bits(),
                0.0_f64.to_bits(),
                "Silent second VST3 instance must remain at 0.0 dB"
            );

            // Lifecycle clears: set_processing(0), set_active(0), terminate()
            assert_eq!(proc_a.set_processing(0), kResultOk);
            assert_eq!(gr_a.get_gain_reduction_value_in_db().to_bits(), 0.0_f64.to_bits());

            assert_eq!(proc_a.set_processing(1), kResultOk);
            process_vst3_stereo_block(&proc_a, &hot, &hot);
            assert!(gr_a.get_gain_reduction_value_in_db() < -0.5);
            assert_eq!(comp_a.set_active(0), kResultOk);
            assert_eq!(gr_a.get_gain_reduction_value_in_db().to_bits(), 0.0_f64.to_bits());

            assert_eq!(comp_a.set_active(1), kResultOk);
            process_vst3_stereo_block(&proc_a, &hot, &hot);
            assert!(gr_a.get_gain_reduction_value_in_db() < -0.5);
            assert_eq!(comp_a.terminate(), kResultOk);
            assert_eq!(gr_a.get_gain_reduction_value_in_db().to_bits(), 0.0_f64.to_bits());
            assert_eq!(comp_b.terminate(), kResultOk);
        }
    }

    #[test]
    fn test_no_exposed_gr_parameter_in_daw_formats() {
        use nih_plug::prelude::{Params, Vst3Plugin};
        use nih_plug::wrapper::clap::{
            clap_host, clap_plugin_factory, CLAP_PLUGIN_FACTORY_ID, CLAP_VERSION,
        };
        use nih_plug::wrapper::vst3::vst3_sys::base::{kResultOk, IPluginFactory};
        use nih_plug::wrapper::vst3::vst3_sys::vst::{IComponent, IEditController, ParameterInfo};
        use nih_plug::wrapper::vst3::vst3_sys::{ComInterface, VstPtr, IID};
        use std::ffi::CStr;

        // 1. Check nih-plug parameter map directly
        let params = OmorePunchParams::default();
        let param_map = params.param_map();
        for (id, ptr, _) in &param_map {
            let name = unsafe { ptr.name() };
            assert_ne!(id, "gr_meter_db", "gr_meter_db must not be exposed");
            assert!(
                !name.to_ascii_lowercase().contains("gain reduction"),
                "Unexpected GR parameter in param_map: {} ({})",
                name,
                id
            );
        }

        // 2. Check CLAP clap.params extension
        let host_name = b"ParamCheckHost\0";
        let host = clap_host {
            clap_version: CLAP_VERSION,
            host_data: std::ptr::null_mut(),
            name: host_name.as_ptr() as *const std::ffi::c_char,
            vendor: std::ptr::null(),
            url: std::ptr::null(),
            version: std::ptr::null(),
            get_extension: Some(test_clap_host_get_extension),
            request_restart: Some(test_clap_host_noop),
            request_process: Some(test_clap_host_noop),
            request_callback: Some(test_clap_host_noop),
        };
        unsafe {
            assert!((super::clap::clap_entry.init.unwrap())(std::ptr::null()));
            let factory = (super::clap::clap_entry.get_factory.unwrap())(
                CLAP_PLUGIN_FACTORY_ID.as_ptr(),
            ) as *const clap_plugin_factory;
            let desc = ((*factory).get_plugin_descriptor.unwrap())(factory, 0);
            let plugin = ((*factory).create_plugin.unwrap())(factory, &host, (*desc).id);
            assert!(((*plugin).init.unwrap())(plugin));

            let params_ext_id = b"clap.params\0";
            let clap_params = ((*plugin).get_extension.unwrap())(
                plugin,
                params_ext_id.as_ptr() as *const std::ffi::c_char,
            ) as *const TestClapPluginParams;
            assert!(!clap_params.is_null());

            let count = ((*clap_params).count.unwrap())(plugin);
            assert_eq!(count as usize, param_map.len());
            let get_info = (*clap_params).get_info.unwrap();
            for idx in 0..count {
                let mut info: TestClapParamInfo = std::mem::zeroed();
                assert!(get_info(plugin, idx, &mut info));
                let name = CStr::from_ptr(info.name.as_ptr())
                    .to_string_lossy()
                    .into_owned();
                assert!(
                    !name.to_ascii_lowercase().contains("gain reduction"),
                    "Unexpected CLAP GR parameter exposed: {}",
                    name
                );
            }

            ((*plugin).destroy.unwrap())(plugin);
        }

        // 3. Check VST3 IEditController parameter list
        unsafe {
            let factory =
                VstPtr::<dyn IPluginFactory>::owned(super::vst3::GetPluginFactory() as *mut *mut _)
                    .unwrap();
            let cid = IID {
                data: OmorePunch::PLATFORM_VST3_CLASS_ID,
            };
            let mut raw = std::ptr::null_mut();
            assert_eq!(
                factory.create_instance(&cid, &<dyn IComponent>::IID, &mut raw),
                kResultOk
            );
            let comp = VstPtr::<dyn IComponent>::owned(raw as *mut *mut _).unwrap();
            let edit = comp.cast::<dyn IEditController>().unwrap();
            let count = edit.get_parameter_count();
            assert_eq!(count as usize, param_map.len());
            for idx in 0..count {
                let mut info: ParameterInfo = std::mem::zeroed();
                assert_eq!(edit.get_parameter_info(idx, &mut info), kResultOk);
                let end = info.title.iter().position(|&c| c == 0).unwrap_or(128);
                let utf16: Vec<u16> = info.title[..end].iter().map(|&c| c as u16).collect();
                let title = String::from_utf16_lossy(&utf16);
                assert!(
                    !title.to_ascii_lowercase().contains("gain reduction"),
                    "Unexpected VST3 GR parameter exposed: {}",
                    title
                );
            }
        }
    }

    #[test]
    fn test_host_gr_telemetry_parity_clamp_and_independence() {
        // 1. Parity, normalization, clamping, and peak-reset isolation on MeterTelemetry
        let telemetry = MeterTelemetry::default();
        assert_eq!(telemetry.host_gain_reduction_db().to_bits(), 0.0_f64.to_bits());

        for bad in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, -0.0_f32, -2.5_f32, 0.0_f32] {
            telemetry.current_gr_db.store(bad.to_bits(), Ordering::Relaxed);
            assert_eq!(
                telemetry.host_gain_reduction_db().to_bits(),
                0.0_f64.to_bits(),
                "Expected exact +0.0 for raw telemetry value {:?}",
                bad
            );
        }

        telemetry.current_gr_db.store(7.25_f32.to_bits(), Ordering::Relaxed);
        telemetry.max_gr_db.store(12.5_f32.to_bits(), Ordering::Relaxed);
        telemetry.clip_latched.store(true, Ordering::Relaxed);
        assert!((telemetry.host_gain_reduction_db() + 7.25).abs() < 1e-6);

        // User clicking UI Reset Peak must NOT clear live host current GR.
        telemetry.reset_peaks();
        assert!((telemetry.host_gain_reduction_db() + 7.25).abs() < 1e-6);
        assert_eq!(
            f32::from_bits(telemetry.max_gr_db.load(Ordering::Relaxed)),
            0.0
        );
        assert!(!telemetry.clip_latched.load(Ordering::Relaxed));

        // Explicit lifecycle clear resets current GR to exact +0.0.
        telemetry.clear_gain_reduction();
        assert_eq!(telemetry.host_gain_reduction_db().to_bits(), 0.0_f64.to_bits());

        // Extreme reduction > MAX_GR_METER_DB clamps strictly to -24.0 dB.
        telemetry.current_gr_db.store(38.0_f32.to_bits(), Ordering::Relaxed);
        assert_eq!(telemetry.host_gain_reduction_db(), -(MAX_GR_METER_DB as f64));

        // 2. Parity with internal compressor envelope and independence from mix, makeup, clipper, EQ, gain match
        let sr = 48000.0_f32;
        let base_cfg = DspConfig {
            threshold_db: -24.0,
            ratio: 8.0,
            attack_ms: 1.0,
            release_ms: 80.0,
            sc_hpf_hz: 20.0,
            auto_makeup: false,
            makeup_db: 0.0,
            clipper_mode: ClipperMode::Bypass,
            ceiling_db: 0.0,
            mix_pct: 100.0,
            gain_match: false,
            ..DspConfig::default()
        };
        let variant_cfg = DspConfig {
            auto_makeup: true,
            makeup_db: 9.5,
            eq_low_cut_hz: 80.0,
            eq_mid_cut_db: -4.0,
            eq_high_shelf_db: 4.5,
            clipper_mode: ClipperMode::HardClip,
            ceiling_db: -6.0,
            mix_pct: 25.0,
            gain_match: true,
            ..base_cfg
        };
        let zero_mix_cfg = DspConfig {
            mix_pct: 0.0,
            ..base_cfg
        };

        let mut dsp_base = StereoPunchDsp::new(sr);
        let mut dsp_var = StereoPunchDsp::new(sr);
        let mut dsp_dry = StereoPunchDsp::new(sr);
        let tel_base = MeterTelemetry::default();

        for n in 0..4096 {
            let phase = 2.0 * std::f32::consts::PI * 500.0 * (n as f32) / sr;
            let s = 0.7 * phase.sin();
            dsp_base.process_sample(s, s, &base_cfg);
            dsp_var.process_sample(s, s, &variant_cfg);
            dsp_dry.process_sample(s, s, &zero_mix_cfg);

            tel_base
                .current_gr_db
                .store(dsp_base.env_gr_db.to_bits(), Ordering::Relaxed);
            let host_gr = tel_base.host_gain_reduction_db();
            let ui_gr = -(dsp_base.env_gr_db.clamp(0.0, MAX_GR_METER_DB) as f64);

            assert!(
                (host_gr - ui_gr).abs() < 0.01,
                "Host/UI GR diverged at sample {}: host={} ui={}",
                n,
                host_gr,
                ui_gr
            );
            assert!(
                (dsp_base.env_gr_db - dsp_var.env_gr_db).abs() < 1e-5
                    && (dsp_base.env_gr_db - dsp_dry.env_gr_db).abs() < 1e-5,
                "Post-compressor stages must not alter detector GR: base={} var={} dry={}",
                dsp_base.env_gr_db,
                dsp_var.env_gr_db,
                dsp_dry.env_gr_db
            );
        }

        // 3. Consistency across supported sample rates & block sizes + below-threshold silence return to 0.0
        for &rate in &[44100.0_f64, 48000.0, 96000.0, 192000.0] {
            for &block_size in &[32_usize, 128, 512, 2048] {
                let mut plugin = OmorePunch::default();
                let layout = AudioIOLayout::default();
                let mut ctx = NoopInitContext;
                let cfg = BufferConfig {
                    sample_rate: rate as f32,
                    min_buffer_size: Some(32),
                    max_buffer_size: block_size as u32,
                    process_mode: ProcessMode::Realtime,
                };
                assert!(plugin.initialize(&layout, &cfg, &mut ctx));

                let mut dsp = StereoPunchDsp::new(rate as f32);
                let total = ((0.06 * rate) as usize).max(block_size);
                let mut processed = 0;
                while processed < total {
                    let count = (total - processed).min(block_size);
                    for i in 0..count {
                        let phase = 2.0 * std::f32::consts::PI * 1000.0 * ((processed + i) as f32)
                            / (rate as f32);
                        let s = 0.85 * phase.sin();
                        dsp.process_sample(s, s, &base_cfg);
                    }
                    plugin
                        .telemetry
                        .current_gr_db
                        .store(dsp.env_gr_db.to_bits(), Ordering::Relaxed);
                    processed += count;
                }

                let active_gr = plugin.current_gain_adjustment_db();
                assert!(
                    (-MAX_GR_METER_DB as f64..=-1.0).contains(&active_gr),
                    "Unexpected GR at {} Hz / {} frames: {}",
                    rate,
                    block_size,
                    active_gr
                );

                // Plugin::reset() must clear host GR to 0.0 dB immediately.
                plugin.reset();
                assert_eq!(
                    plugin.current_gain_adjustment_db().to_bits(),
                    0.0_f64.to_bits()
                );

                // Extreme input produces <= -24.0 clamped safely at -24.0.
                let mut extreme = StereoPunchDsp::new(rate as f32);
                let crush_cfg = DspConfig {
                    threshold_db: -40.0,
                    ratio: 20.0,
                    attack_ms: 0.1,
                    ..base_cfg
                };
                for _ in 0..512 {
                    extreme.process_sample(4.0, 4.0, &crush_cfg);
                }
                plugin
                    .telemetry
                    .current_gr_db
                    .store(extreme.env_gr_db.to_bits(), Ordering::Relaxed);
                assert_eq!(plugin.current_gain_adjustment_db(), -(MAX_GR_METER_DB as f64));

                // Re-initializing plugin clears telemetry cleanly.
                assert!(plugin.initialize(&layout, &cfg, &mut ctx));
                assert_eq!(
                    plugin.current_gain_adjustment_db().to_bits(),
                    0.0_f64.to_bits()
                );
            }
        }
    }

    #[test]
    fn test_bypass_transparency_and_safety_clamp() {
        assert!(!DspConfig::default().bypass);

        for sr in [44100.0_f32, 48000.0_f32, 96000.0_f32] {
            let mut dsp = StereoPunchDsp::new(sr);
            let mut cfg = DspConfig {
                input_db: 6.0,
                threshold_db: -24.0,
                ratio: 8.0,
                attack_ms: 1.0,
                release_ms: 50.0,
                auto_makeup: true,
                makeup_db: 4.0,
                eq_low_cut_hz: 120.0,
                eq_mid_cut_db: -9.0,
                eq_high_shelf_db: 9.0,
                clipper_mode: ClipperMode::HardClip,
                ceiling_db: -6.0,
                mix_pct: 100.0,
                gain_match: false,
                bypass: false,
                ..DspConfig::default()
            };

            // Warm up compressor and filters
            for n in 0..2048 {
                let phase = 2.0 * std::f32::consts::PI * 300.0 * (n as f32) / sr;
                dsp.process_sample(0.6 * phase.sin(), -0.4 * phase.cos(), &cfg);
            }

            // Enable bypass and let ramp settle within <= 20 ms
            cfg.bypass = true;
            let settle_samples = ((0.020 * sr).ceil() as usize).max(1);
            for n in 0..settle_samples {
                let phase = 2.0 * std::f32::consts::PI * 300.0 * (n as f32) / sr;
                dsp.process_sample(0.6 * phase.sin(), -0.4 * phase.cos(), &cfg);
            }

            // After ramp settles, bypass must be bit-exact within [-4.0, 4.0] (< -120 dBFS)
            for n in 0..1024 {
                let phase = 2.0 * std::f32::consts::PI * 440.0 * (n as f32) / sr;
                let in_l = 0.65 * phase.sin();
                let in_r = -0.35 * phase.cos();
                let out = dsp.process_sample(in_l, in_r, &cfg);
                assert_eq!(
                    out.out_l.to_bits(),
                    in_l.to_bits(),
                    "sr={}: bypassed Left output must be bit-identical at sample {}",
                    sr,
                    n
                );
                assert_eq!(
                    out.out_r.to_bits(),
                    in_r.to_bits(),
                    "sr={}: bypassed Right output must be bit-identical at sample {}",
                    sr,
                    n
                );
                assert_eq!(out.gr_db, 0.0);
                assert!(!out.clipped);
            }

            // Pathological inputs must still be sanitized & clamped to [-4.0, 4.0] in bypass
            for &(bad_l, bad_r) in &[
                (f32::NAN, f32::INFINITY),
                (f32::NEG_INFINITY, 1.0e20),
                (-9.5, 12.0),
            ] {
                let out = dsp.process_sample(bad_l, bad_r, &cfg);
                assert!(out.out_l.is_finite() && out.out_l.abs() <= SAFETY_CLAMP_LIMIT);
                assert!(out.out_r.is_finite() && out.out_r.abs() <= SAFETY_CLAMP_LIMIT);
            }
            let nan_out = dsp.process_sample(f32::NAN, f32::NEG_INFINITY, &cfg);
            assert_eq!(nan_out.out_l, 0.0);
            assert_eq!(nan_out.out_r, 0.0);

            // Initialized already bypassed: bit-identical from the very first sample
            let mut pre_bypassed = StereoPunchDsp::new(sr);
            pre_bypassed.sync_bypass_state(true);
            let first = pre_bypassed.process_sample(0.3125, -0.625, &cfg);
            assert_eq!(first.out_l.to_bits(), 0.3125_f32.to_bits());
            assert_eq!(first.out_r.to_bits(), (-0.625_f32).to_bits());
        }
    }

    #[test]
    fn test_bypass_smooth_transition_no_clicks() {
        let sr = 48000.0_f32;
        let freq_hz = 250.0_f32;
        let mut dsp = StereoPunchDsp::new(sr);
        let mut cfg = DspConfig {
            input_db: 6.0,
            threshold_db: -20.0,
            ratio: 8.0,
            attack_ms: 1.0,
            release_ms: 80.0,
            auto_makeup: false,
            makeup_db: 0.0,
            eq_low_cut_hz: 120.0,
            clipper_mode: ClipperMode::HardClip,
            ceiling_db: -6.0,
            mix_pct: 100.0,
            bypass: false,
            ..DspConfig::default()
        };

        let mut prev_l = 0.0_f32;
        let mut prev_r = 0.0_f32;
        for n in 0..4800 {
            let s = 0.5 * (2.0 * std::f32::consts::PI * freq_hz * (n as f32) / sr).sin();
            let out = dsp.process_sample(s, -s, &cfg);
            prev_l = out.out_l;
            prev_r = out.out_r;
        }

        // Toggle ON -> OFF -> ON and verify smooth sample-to-sample steps (< 0.05) and < 20 ms settle
        let ramp_window = (0.020 * sr) as usize; // 20 ms
        let mut max_step_to_bypass = 0.0_f32;
        cfg.bypass = true;
        for n in 0..ramp_window {
            let s = 0.5 * (2.0 * std::f32::consts::PI * freq_hz * ((4800 + n) as f32) / sr).sin();
            let out = dsp.process_sample(s, -s, &cfg);
            max_step_to_bypass = max_step_to_bypass
                .max((out.out_l - prev_l).abs())
                .max((out.out_r - prev_r).abs());
            prev_l = out.out_l;
            prev_r = out.out_r;
        }
        assert!(
            max_step_to_bypass < 0.05,
            "Bypass activation produced click-like step: {}",
            max_step_to_bypass
        );

        let check_s =
            0.5 * (2.0 * std::f32::consts::PI * freq_hz * ((4800 + ramp_window) as f32) / sr).sin();
        let settled = dsp.process_sample(check_s, -check_s, &cfg);
        assert!(
            (settled.out_l - check_s).abs() < 1.0e-6,
            "Bypass did not reach full transparency within 20 ms: diff={}",
            (settled.out_l - check_s).abs()
        );
        prev_l = settled.out_l;
        prev_r = settled.out_r;

        let mut max_step_to_active = 0.0_f32;
        cfg.bypass = false;
        for n in 0..ramp_window {
            let idx = 4800 + ramp_window + 1 + n;
            let s = 0.5 * (2.0 * std::f32::consts::PI * freq_hz * (idx as f32) / sr).sin();
            let out = dsp.process_sample(s, -s, &cfg);
            max_step_to_active = max_step_to_active
                .max((out.out_l - prev_l).abs())
                .max((out.out_r - prev_r).abs());
            prev_l = out.out_l;
            prev_r = out.out_r;
        }
        assert!(
            max_step_to_active < 0.05,
            "Bypass deactivation produced click-like step: {}",
            max_step_to_active
        );

        // Rapid toggles every 100 samples (~2 ms) remain bounded and smooth
        let mut max_rapid_step = 0.0_f32;
        for n in 0..2000 {
            if n % 100 == 0 {
                cfg.bypass = !cfg.bypass;
            }
            let idx = 4800 + 2 * ramp_window + 1 + n;
            let s = 0.5 * (2.0 * std::f32::consts::PI * freq_hz * (idx as f32) / sr).sin();
            let out = dsp.process_sample(s, -s, &cfg);
            assert!(out.out_l.is_finite() && out.out_l.abs() <= SAFETY_CLAMP_LIMIT);
            assert!(out.out_r.is_finite() && out.out_r.abs() <= SAFETY_CLAMP_LIMIT);
            max_rapid_step = max_rapid_step
                .max((out.out_l - prev_l).abs())
                .max((out.out_r - prev_r).abs());
            prev_l = out.out_l;
            prev_r = out.out_r;
        }
        assert!(
            max_rapid_step < 0.05,
            "Rapid bypass toggles produced discontinuous step: {}",
            max_rapid_step
        );
    }

    #[test]
    fn test_bypass_parameter_daw_integration_and_compat() {
        use nih_plug::prelude::{ParamFlags, Params, Vst3Plugin};
        use nih_plug::wrapper::clap::{
            clap_host, clap_plugin_factory, CLAP_PLUGIN_FACTORY_ID, CLAP_VERSION,
        };
        use nih_plug::wrapper::vst3::vst3_sys::base::{kResultOk, IPluginBase, IPluginFactory};
        use nih_plug::wrapper::vst3::vst3_sys::vst::{
            IAudioProcessor, IComponent, IEditController, ParameterFlags, ParameterInfo,
            ProcessModes, ProcessSetup, SymbolicSampleSizes,
        };
        use nih_plug::wrapper::vst3::vst3_sys::{ComInterface, VstPtr, IID};
        use std::ffi::{c_void, CStr};

        const TEST_CLAP_PARAM_IS_BYPASS: u32 = 1 << 4;
        const TEST_CLAP_PARAM_IS_AUTOMATABLE: u32 = 1 << 5;

        #[repr(C)]
        struct TestClapOStream {
            ctx: *mut c_void,
            write: Option<unsafe extern "C" fn(*const TestClapOStream, *const c_void, u64) -> i64>,
        }
        #[repr(C)]
        struct TestClapIStream {
            ctx: *mut c_void,
            read: Option<unsafe extern "C" fn(*const TestClapIStream, *mut c_void, u64) -> i64>,
        }
        #[repr(C)]
        struct TestClapPluginState {
            save: Option<
                unsafe extern "C" fn(
                    plugin: *const nih_plug::wrapper::clap::clap_plugin,
                    stream: *const TestClapOStream,
                ) -> bool,
            >,
            load: Option<
                unsafe extern "C" fn(
                    plugin: *const nih_plug::wrapper::clap::clap_plugin,
                    stream: *const TestClapIStream,
                ) -> bool,
            >,
        }
        struct VecReadCtx<'a> {
            data: &'a [u8],
            pos: usize,
        }
        unsafe extern "C" fn test_clap_ostream_write(
            stream: *const TestClapOStream,
            buffer: *const c_void,
            size: u64,
        ) -> i64 {
            let buf = &mut *((*stream).ctx as *mut Vec<u8>);
            let slice = std::slice::from_raw_parts(buffer as *const u8, size as usize);
            buf.extend_from_slice(slice);
            size as i64
        }
        unsafe extern "C" fn test_clap_istream_read(
            stream: *const TestClapIStream,
            buffer: *mut c_void,
            size: u64,
        ) -> i64 {
            let ctx = &mut *((*stream).ctx as *mut VecReadCtx);
            let remaining = ctx.data.len().saturating_sub(ctx.pos);
            let to_read = remaining.min(size as usize);
            if to_read > 0 {
                std::ptr::copy_nonoverlapping(
                    ctx.data.as_ptr().add(ctx.pos),
                    buffer as *mut u8,
                    to_read,
                );
                ctx.pos += to_read;
            }
            to_read as i64
        }

        // 1. Default false, unique ParamFlags::BYPASS on "bypass"
        let params = OmorePunchParams::default();
        assert!(!params.bypass.value(), "Bypass must default to OFF (false)");

        let param_map = params.param_map();
        let bypass_entries: Vec<_> = param_map
            .iter()
            .filter(|(_, ptr, _)| unsafe { ptr.flags() }.contains(ParamFlags::BYPASS))
            .collect();
        assert_eq!(
            bypass_entries.len(),
            1,
            "Exactly one parameter must carry ParamFlags::BYPASS"
        );
        assert_eq!(bypass_entries[0].0, "bypass");

        // 2. CLAP exposition + state persistence + legacy backward compatibility (no "bypass" key)
        let host_name = b"BypassClapCheck\0";
        let host = clap_host {
            clap_version: CLAP_VERSION,
            host_data: std::ptr::null_mut(),
            name: host_name.as_ptr() as *const std::ffi::c_char,
            vendor: std::ptr::null(),
            url: std::ptr::null(),
            version: std::ptr::null(),
            get_extension: Some(test_clap_host_get_extension),
            request_restart: Some(test_clap_host_noop),
            request_process: Some(test_clap_host_noop),
            request_callback: Some(test_clap_host_noop),
        };
        unsafe {
            assert!((super::clap::clap_entry.init.unwrap())(std::ptr::null()));
            let factory = (super::clap::clap_entry.get_factory.unwrap())(
                CLAP_PLUGIN_FACTORY_ID.as_ptr(),
            ) as *const clap_plugin_factory;
            let desc = ((*factory).get_plugin_descriptor.unwrap())(factory, 0);
            let plugin = ((*factory).create_plugin.unwrap())(factory, &host, (*desc).id);
            assert!(((*plugin).init.unwrap())(plugin));

            let params_ext_id = b"clap.params\0";
            let clap_params = ((*plugin).get_extension.unwrap())(
                plugin,
                params_ext_id.as_ptr() as *const std::ffi::c_char,
            ) as *const TestClapPluginParams;
            assert!(!clap_params.is_null());

            let count = ((*clap_params).count.unwrap())(plugin);
            let get_info = (*clap_params).get_info.unwrap();
            let mut clap_bypass_count = 0_usize;
            let mut clap_bypass_id = 0_u32;
            for idx in 0..count {
                let mut info: TestClapParamInfo = std::mem::zeroed();
                assert!(get_info(plugin, idx, &mut info));
                if (info.flags & TEST_CLAP_PARAM_IS_BYPASS) != 0 {
                    clap_bypass_count += 1;
                    clap_bypass_id = info.id;
                    let name = CStr::from_ptr(info.name.as_ptr())
                        .to_string_lossy()
                        .into_owned();
                    assert_eq!(name, "Bypass");
                    assert_ne!(
                        info.flags & TEST_CLAP_PARAM_IS_AUTOMATABLE,
                        0,
                        "CLAP bypass parameter must remain automatable"
                    );
                    assert_eq!(info.default_value, 0.0);
                }
            }
            assert_eq!(clap_bypass_count, 1);

            // Save default state -> remove "bypass" entry -> load legacy state -> verify OFF
            let state_ext_id = b"clap.state\0";
            let clap_state = ((*plugin).get_extension.unwrap())(
                plugin,
                state_ext_id.as_ptr() as *const std::ffi::c_char,
            ) as *const TestClapPluginState;
            assert!(!clap_state.is_null());

            let mut saved_bytes = Vec::<u8>::new();
            let ostream = TestClapOStream {
                ctx: (&mut saved_bytes as *mut Vec<u8>).cast(),
                write: Some(test_clap_ostream_write),
            };
            assert!(((*clap_state).save.unwrap())(plugin, &ostream));
            assert!(saved_bytes.len() > 8);
            let state_len = u64::from_le_bytes(saved_bytes[..8].try_into().unwrap()) as usize;
            assert_eq!(state_len + 8, saved_bytes.len());
            let state_str = std::str::from_utf8(&saved_bytes[8..]).unwrap().to_string();
            assert!(
                state_str.contains("\"bypass\":{\"bool\":false}"),
                "Saved DAW state must serialize bypass=false parameter: {}",
                state_str
            );

            let encode_clap_state = |json: &str| -> Vec<u8> {
                let mut out = Vec::with_capacity(8 + json.len());
                out.extend_from_slice(&(json.len() as u64).to_le_bytes());
                out.extend_from_slice(json.as_bytes());
                out
            };

            // Strip `"bypass":{"bool":false}` to simulate pre-bypass session
            let legacy_str = state_str
                .replace("\"bypass\":{\"bool\":false},", "")
                .replace(",\"bypass\":{\"bool\":false}", "");
            assert!(!legacy_str.contains("\"bypass\""));
            let legacy_bytes = encode_clap_state(&legacy_str);
            let mut legacy_ctx = VecReadCtx {
                data: &legacy_bytes,
                pos: 0,
            };
            let legacy_istream = TestClapIStream {
                ctx: (&mut legacy_ctx as *mut VecReadCtx).cast(),
                read: Some(test_clap_istream_read),
            };
            assert!(((*clap_state).load.unwrap())(plugin, &legacy_istream));
            let mut val = -1.0_f64;
            assert!(((*clap_params).get_value.unwrap())(
                plugin,
                clap_bypass_id,
                &mut val
            ));
            assert_eq!(val, 0.0, "Legacy state without bypass key must default to OFF");

            // Load a session with `"bypass":{"bool":true}` and confirm restored value == 1.0
            let on_str = state_str.replace(
                "\"bypass\":{\"bool\":false}",
                "\"bypass\":{\"bool\":true}",
            );
            let on_bytes = encode_clap_state(&on_str);
            let mut on_ctx = VecReadCtx {
                data: &on_bytes,
                pos: 0,
            };
            let on_istream = TestClapIStream {
                ctx: (&mut on_ctx as *mut VecReadCtx).cast(),
                read: Some(test_clap_istream_read),
            };
            assert!(((*clap_state).load.unwrap())(plugin, &on_istream));
            let mut val_on = -1.0_f64;
            assert!(((*clap_params).get_value.unwrap())(
                plugin,
                clap_bypass_id,
                &mut val_on
            ));
            assert_eq!(val_on, 1.0, "Restored bypass=true state must read 1.0");

            ((*plugin).destroy.unwrap())(plugin);
        }

        // 3. VST3 exposition: ParameterFlags::kIsBypass | kCanAutomate + pre-bypassed init
        unsafe {
            let factory =
                VstPtr::<dyn IPluginFactory>::owned(super::vst3::GetPluginFactory() as *mut *mut _)
                    .unwrap();
            let cid = IID {
                data: OmorePunch::PLATFORM_VST3_CLASS_ID,
            };
            let mut raw = std::ptr::null_mut();
            assert_eq!(
                factory.create_instance(&cid, &<dyn IComponent>::IID, &mut raw),
                kResultOk
            );
            let comp = VstPtr::<dyn IComponent>::owned(raw as *mut *mut _).unwrap();
            let edit = comp.cast::<dyn IEditController>().unwrap();
            let proc_if = comp.cast::<dyn IAudioProcessor>().unwrap();

            let count = edit.get_parameter_count();
            let mut vst3_bypass_id = None;
            let mut threshold_id = None;
            for idx in 0..count {
                let mut info: ParameterInfo = std::mem::zeroed();
                assert_eq!(edit.get_parameter_info(idx, &mut info), kResultOk);
                let end = info.title.iter().position(|&c| c == 0).unwrap_or(128);
                let utf16: Vec<u16> = info.title[..end].iter().map(|&c| c as u16).collect();
                let title = String::from_utf16_lossy(&utf16);
                if (info.flags & (ParameterFlags::kIsBypass as i32)) != 0 {
                    assert!(vst3_bypass_id.is_none(), "Duplicate VST3 kIsBypass param");
                    assert_eq!(title, "Bypass");
                    assert_ne!(info.flags & (ParameterFlags::kCanAutomate as i32), 0);
                    assert_eq!(info.default_normalized_value, 0.0);
                    vst3_bypass_id = Some(info.id);
                }
                if title == "Threshold" {
                    threshold_id = Some(info.id);
                }
            }
            let bypass_id = vst3_bypass_id.expect("VST3 kIsBypass parameter missing");
            let thresh_id = threshold_id.expect("Threshold parameter missing");

            // Initialize session already bypassed (loaded with bypass = true) and verify
            // bit-exact passthrough from the very first sample even with heavy threshold
            assert_eq!(edit.set_param_normalized(thresh_id, 0.0), kResultOk);
            assert_eq!(edit.set_param_normalized(bypass_id, 1.0), kResultOk);

            let mut arr_in: u64 = 3;
            let mut arr_out: u64 = 3;
            assert_eq!(
                proc_if.set_bus_arrangements(&mut arr_in, 1, &mut arr_out, 1),
                kResultOk
            );
            let setup = ProcessSetup {
                process_mode: ProcessModes::kRealtime as i32,
                symbolic_sample_size: SymbolicSampleSizes::kSample32 as i32,
                max_samples_per_block: 128,
                sample_rate: 48000.0,
            };
            assert_eq!(proc_if.setup_processing(&setup), kResultOk);
            assert_eq!(comp.set_active(1), kResultOk);
            assert_eq!(proc_if.set_processing(1), kResultOk);

            let in_l: Vec<f32> = (0..128).map(|i| 0.7 * ((i as f32) * 0.1).sin()).collect();
            let in_r: Vec<f32> = (0..128).map(|i| -0.5 * ((i as f32) * 0.13).cos()).collect();
            let (out_l, out_r) = process_vst3_stereo_block_out(&proc_if, &in_l, &in_r);
            for i in 0..128 {
                assert_eq!(
                    out_l[i].to_bits(),
                    in_l[i].to_bits(),
                    "Pre-bypassed session altered first block Left sample {}",
                    i
                );
                assert_eq!(
                    out_r[i].to_bits(),
                    in_r[i].to_bits(),
                    "Pre-bypassed session altered first block Right sample {}",
                    i
                );
            }
            assert_eq!(comp.terminate(), kResultOk);
        }
    }

    #[test]
    fn test_bypass_freezes_gain_match_and_clears_gr_and_clip() {
        use super::vst3::IGainReductionInfo;
        use nih_plug::prelude::Vst3Plugin;
        use nih_plug::wrapper::vst3::vst3_sys::base::{kResultOk, IPluginBase, IPluginFactory};
        use nih_plug::wrapper::vst3::vst3_sys::vst::{
            IAudioProcessor, IComponent, IEditController, ParameterInfo, ProcessModes,
            ProcessSetup, SymbolicSampleSizes,
        };
        use nih_plug::wrapper::vst3::vst3_sys::{ComInterface, VstPtr, IID};

        // 1. DSP-level check: Gain Match trim is strictly frozen across bypass, GR=0, clipped=false
        let sr = 48000.0_f32;
        let mut dsp = StereoPunchDsp::new(sr);
        let mut cfg = DspConfig {
            input_db: 0.0,
            threshold_db: -20.0,
            ratio: 8.0,
            attack_ms: 1.0,
            release_ms: 80.0,
            auto_makeup: false,
            makeup_db: 6.0,
            clipper_mode: ClipperMode::SoftClip,
            ceiling_db: -3.0,
            mix_pct: 100.0,
            gain_match: true,
            bypass: false,
            ..DspConfig::default()
        };

        for n in 0..96000 {
            let phase = 2.0 * std::f32::consts::PI * 1000.0 * (n as f32) / sr;
            let s = 0.6 * phase.sin();
            dsp.process_sample(s, s, &cfg);
        }
        let frozen_trim = dsp.gm_gain_db;
        assert!(
            frozen_trim.abs() > 1.0,
            "Expected meaningful Gain Match trim before bypass, got {}",
            frozen_trim
        );

        // Engage bypass and run both hot clipped signals and silence for 1 second
        cfg.bypass = true;
        for n in 0..48000 {
            let phase = 2.0 * std::f32::consts::PI * 1000.0 * (n as f32) / sr;
            let s = if n < 24000 { 2.5 * phase.sin() } else { 0.0 };
            let out = dsp.process_sample(s, s, &cfg);
            assert_eq!(
                out.gr_db, 0.0,
                "Instantaneous GR must read 0.0 dB while bypassed (sample {})",
                n
            );
            assert!(
                !out.clipped,
                "Clipper must not signal internal clipping while bypassed (sample {})",
                n
            );
            assert_eq!(
                out.gain_match_db.to_bits(),
                frozen_trim.to_bits(),
                "Gain Match trim changed during bypass at sample {}",
                n
            );
        }

        // Disengage bypass: Gain Match resumes from frozen_trim without level jump or reset
        cfg.bypass = false;
        let first_return = dsp.process_sample(0.6, 0.6, &cfg);
        assert!(
            (first_return.gain_match_db - frozen_trim).abs() < 0.05,
            "Gain Match jumped on bypass exit: before={} after={}",
            frozen_trim,
            first_return.gain_match_db
        );

        // 2. Full OmorePunch::process + MeterTelemetry + IGainReductionInfo check via VST3
        unsafe {
            let _ = take_last_inspectable_telemetry();
            let factory =
                VstPtr::<dyn IPluginFactory>::owned(super::vst3::GetPluginFactory() as *mut *mut _)
                    .unwrap();
            let cid = IID {
                data: OmorePunch::PLATFORM_VST3_CLASS_ID,
            };
            let mut raw = std::ptr::null_mut();
            assert_eq!(
                factory.create_instance(&cid, &<dyn IComponent>::IID, &mut raw),
                kResultOk
            );
            let telemetry = take_last_inspectable_telemetry()
                .expect("Expected MeterTelemetry captured from new VST3 instance");
            let comp = VstPtr::<dyn IComponent>::owned(raw as *mut *mut _).unwrap();
            let edit = comp.cast::<dyn IEditController>().unwrap();
            let proc_if = comp.cast::<dyn IAudioProcessor>().unwrap();
            let gr_if = comp.cast::<dyn IGainReductionInfo>().unwrap();

            let mut bypass_id = None;
            let mut threshold_id = None;
            for idx in 0..edit.get_parameter_count() {
                let mut info: ParameterInfo = std::mem::zeroed();
                assert_eq!(edit.get_parameter_info(idx, &mut info), kResultOk);
                let end = info.title.iter().position(|&c| c == 0).unwrap_or(128);
                let utf16: Vec<u16> = info.title[..end].iter().map(|&c| c as u16).collect();
                let title = String::from_utf16_lossy(&utf16);
                if title == "Bypass" {
                    bypass_id = Some(info.id);
                } else if title == "Threshold" {
                    threshold_id = Some(info.id);
                }
            }
            let bypass_id = bypass_id.unwrap();
            let threshold_id = threshold_id.unwrap();

            assert_eq!(edit.set_param_normalized(threshold_id, 0.25), kResultOk);
            let mut arr_in: u64 = 3;
            let mut arr_out: u64 = 3;
            assert_eq!(
                proc_if.set_bus_arrangements(&mut arr_in, 1, &mut arr_out, 1),
                kResultOk
            );
            let setup = ProcessSetup {
                process_mode: ProcessModes::kRealtime as i32,
                symbolic_sample_size: SymbolicSampleSizes::kSample32 as i32,
                max_samples_per_block: 512,
                sample_rate: 48000.0,
            };
            assert_eq!(proc_if.setup_processing(&setup), kResultOk);
            assert_eq!(comp.set_active(1), kResultOk);
            assert_eq!(proc_if.set_processing(1), kResultOk);

            let hot: Vec<f32> = (0..512)
                .map(|i| 2.5 * (2.0 * std::f32::consts::PI * 1000.0 * (i as f32) / 48000.0).sin())
                .collect();
            for _ in 0..6 {
                process_vst3_stereo_block(&proc_if, &hot, &hot);
            }

            let held_max_before = load_f32_relaxed(&telemetry.max_gr_db);
            assert!(held_max_before > 1.0, "Expected recorded Max GR before bypass");
            assert!(
                telemetry.clip_latched.load(Ordering::Relaxed),
                "Expected pre-existing clip latch before bypass"
            );
            assert!(gr_if.get_gain_reduction_value_in_db() < -0.5);

            // Engage bypass via VST3 host: current GR drops to 0.0 dB, Max GR & Clip stay frozen
            assert_eq!(proc_if.set_processing(0), kResultOk);
            assert_eq!(edit.set_param_normalized(bypass_id, 1.0), kResultOk);
            assert_eq!(proc_if.set_processing(1), kResultOk);

            for _ in 0..4 {
                process_vst3_stereo_block(&proc_if, &hot, &hot);
            }
            assert_eq!(
                load_f32_relaxed(&telemetry.current_gr_db).to_bits(),
                0.0_f32.to_bits(),
                "UI current_gr_db must drop to 0.0 while bypassed"
            );
            assert_eq!(
                gr_if.get_gain_reduction_value_in_db().to_bits(),
                0.0_f64.to_bits(),
                "Host GR meter must drop to 0.0 dB while bypassed"
            );
            assert_eq!(
                load_f32_relaxed(&telemetry.max_gr_db).to_bits(),
                held_max_before.to_bits(),
                "Max GR hold must remain frozen while bypassed"
            );
            assert!(
                telemetry.clip_latched.load(Ordering::Relaxed),
                "Pre-existing clip latch must stay visible until manually acknowledged"
            );

            // Reset clip latch while bypassed, feed > 0 dBFS signal (< safety clamp): must NOT re-latch
            telemetry.clip_latched.store(false, Ordering::Relaxed);
            let over_ceiling: Vec<f32> = (0..512)
                .map(|i| 2.0 * (2.0 * std::f32::consts::PI * 1000.0 * (i as f32) / 48000.0).sin())
                .collect();
            for _ in 0..4 {
                process_vst3_stereo_block(&proc_if, &over_ceiling, &over_ceiling);
            }
            assert!(
                !telemetry.clip_latched.load(Ordering::Relaxed),
                "Bypassed clipper must not trigger clip indicator"
            );

            assert_eq!(comp.terminate(), kResultOk);
        }
    }

    struct NoopInitContext;
    impl InitContext<OmorePunch> for NoopInitContext {
        fn plugin_api(&self) -> PluginApi {
            PluginApi::Clap
        }

        fn execute(&self, _task: <OmorePunch as Plugin>::BackgroundTask) {}

        fn set_latency_samples(&self, _samples: u32) {}

        fn set_current_voice_capacity(&self, _capacity: u32) {}
    }

    const PRESET_A_JSON: &str = concat!(
        r#"{"version":"0.3.1-dev","params":{"#,
        r#""attack_ms":{"f32":12.5},"#,
        r#""auto_makeup":{"bool":false},"#,
        r#""bypass":{"bool":true},"#,
        r#""ceiling_db":{"f32":-1.5},"#,
        r#""clipper_mode":{"string":"hard_clip"},"#,
        r#""eq_high_shelf_db":{"f32":4.5},"#,
        r#""eq_low_cut_hz":{"f32":75.0},"#,
        r#""eq_mid_cut_db":{"f32":-3.5},"#,
        r#""gain_match":{"bool":true},"#,
        r#""input_db":{"f32":3.25},"#,
        r#""makeup_db":{"f32":6.0},"#,
        r#""mix_pct":{"f32":65.0},"#,
        r#""ratio":{"f32":8.0},"#,
        r#""release_mode":{"string":"auto_punch"},"#,
        r#""release_ms":{"f32":145.0},"#,
        r#""sc_hpf_hz":{"f32":200.0},"#,
        r#""threshold_db":{"f32":-18.5}"#,
        r#"},"fields":{}}"#
    );

    const PRESET_B_JSON: &str = concat!(
        r#"{"version":"0.3.1-dev","params":{"#,
        r#""attack_ms":{"f32":0.5},"#,
        r#""auto_makeup":{"bool":true},"#,
        r#""bypass":{"bool":false},"#,
        r#""ceiling_db":{"f32":-4.0},"#,
        r#""clipper_mode":{"string":"bypass_clip"},"#,
        r#""eq_high_shelf_db":{"f32":-2.5},"#,
        r#""eq_low_cut_hz":{"f32":35.0},"#,
        r#""eq_mid_cut_db":{"f32":-8.0},"#,
        r#""gain_match":{"bool":false},"#,
        r#""input_db":{"f32":-4.0},"#,
        r#""makeup_db":{"f32":2.0},"#,
        r#""mix_pct":{"f32":35.0},"#,
        r#""ratio":{"f32":2.5},"#,
        r#""release_mode":{"string":"auto_glue"},"#,
        r#""release_ms":{"f32":320.0},"#,
        r#""sc_hpf_hz":{"f32":60.0},"#,
        r#""threshold_db":{"f32":-8.0}"#,
        r#"},"fields":{}}"#
    );

    const LEGACY_V02_PRESET_JSON: &str = concat!(
        r#"{"version":"0.2.0","params":{"#,
        r#""input_db":{"f32":2.0},"#,
        r#""threshold_db":{"f32":-16.0},"#,
        r#""ratio":{"f32":6.0},"#,
        r#""attack_ms":{"f32":5.0},"#,
        r#""release_ms":{"f32":110.0},"#,
        r#""sc_hpf_hz":{"f32":120.0},"#,
        r#""auto_makeup":{"bool":false},"#,
        r#""makeup_db":{"f32":3.0},"#,
        r#""clipper_mode":{"string":"hard_clip"},"#,
        r#""ceiling_db":{"f32":-2.0},"#,
        r#""mix_pct":{"f32":70.0}"#,
        r#"},"fields":{"editor-state":"{\"size\":[820,480]}"}}"#
    );

    fn encode_clap_state_chunk(json: &[u8]) -> Vec<u8> {
        let mut out = Vec::with_capacity(8 + json.len());
        out.extend_from_slice(&(json.len() as u64).to_le_bytes());
        out.extend_from_slice(json);
        out
    }

    #[repr(C)]
    struct TestClapOStream {
        ctx: *mut std::ffi::c_void,
        write: Option<
            unsafe extern "C" fn(
                stream: *const TestClapOStream,
                buffer: *const std::ffi::c_void,
                size: u64,
            ) -> i64,
        >,
    }

    #[repr(C)]
    struct TestClapIStream {
        ctx: *mut std::ffi::c_void,
        read: Option<
            unsafe extern "C" fn(
                stream: *const TestClapIStream,
                buffer: *mut std::ffi::c_void,
                size: u64,
            ) -> i64,
        >,
    }

    struct ClapWriteBuffer {
        bytes: Vec<u8>,
    }

    struct ClapReadCursor<'a> {
        bytes: &'a [u8],
        pos: usize,
    }

    unsafe extern "C" fn test_clap_ostream_write(
        stream: *const TestClapOStream,
        buffer: *const std::ffi::c_void,
        size: u64,
    ) -> i64 {
        if stream.is_null() || buffer.is_null() {
            return -1;
        }
        let sink = &mut *((*stream).ctx as *mut ClapWriteBuffer);
        let slice = std::slice::from_raw_parts(buffer as *const u8, size as usize);
        sink.bytes.extend_from_slice(slice);
        size as i64
    }

    unsafe extern "C" fn test_clap_istream_read(
        stream: *const TestClapIStream,
        buffer: *mut std::ffi::c_void,
        size: u64,
    ) -> i64 {
        if stream.is_null() || buffer.is_null() {
            return -1;
        }
        let cursor = &mut *((*stream).ctx as *mut ClapReadCursor<'_>);
        let remaining = cursor.bytes.len().saturating_sub(cursor.pos);
        let count = remaining.min(size as usize);
        if count > 0 {
            std::ptr::copy_nonoverlapping(
                cursor.bytes.as_ptr().add(cursor.pos),
                buffer as *mut u8,
                count,
            );
            cursor.pos += count;
        }
        count as i64
    }

    struct ClapHostRescanTracker {
        rescan_calls: AtomicU32,
        last_rescan_flags: AtomicU32,
    }

    unsafe extern "C" fn test_clap_host_params_rescan(
        host: *const nih_plug::wrapper::clap::clap_host,
        flags: u32,
    ) {
        if host.is_null() || (*host).host_data.is_null() {
            return;
        }
        let tracker = &*((*host).host_data as *const ClapHostRescanTracker);
        tracker.rescan_calls.fetch_add(1, Ordering::SeqCst);
        tracker.last_rescan_flags.store(flags, Ordering::SeqCst);
    }

    unsafe extern "C" fn test_clap_host_params_clear(
        _host: *const nih_plug::wrapper::clap::clap_host,
        _param_id: u32,
        _flags: u32,
    ) {
    }

    static TEST_CLAP_HOST_PARAMS_EXT: super::clap::ClapHostParams = super::clap::ClapHostParams {
        rescan: Some(test_clap_host_params_rescan),
        clear: Some(test_clap_host_params_clear),
        request_flush: Some(test_clap_host_noop),
    };

    unsafe extern "C" fn test_clap_host_get_extension_with_params(
        _host: *const nih_plug::wrapper::clap::clap_host,
        extension_id: *const std::ffi::c_char,
    ) -> *const std::ffi::c_void {
        if extension_id.is_null() {
            return std::ptr::null();
        }
        if std::ffi::CStr::from_ptr(extension_id).to_bytes_with_nul() == b"clap.params\0" {
            &TEST_CLAP_HOST_PARAMS_EXT as *const super::clap::ClapHostParams
                as *const std::ffi::c_void
        } else {
            std::ptr::null()
        }
    }

    mod vst3_test_mocks {
        use nih_plug::wrapper::vst3::vst3_sys;
        use std::ffi::c_void;
        use std::sync::atomic::{AtomicI32, AtomicU32, Ordering};
        use std::sync::Mutex;
        use vst3_sys as vst3_com;
        use vst3_sys::base::{
            kIBSeekCur, kIBSeekEnd, kIBSeekSet, kInvalidArgument, kResultOk, tresult, IBStream,
            IUnknown,
        };
        use vst3_sys::utils::SharedVstPtr;
        use vst3_sys::vst::{IComponentHandler, ParamID, ParamValue};
        use vst3_sys::{ComInterface, VstPtr, VST3};

        #[VST3(implements(IBStream))]
        pub struct MockVst3Stream {
            buf: Mutex<(Vec<u8>, usize)>,
        }

        impl MockVst3Stream {
            pub fn new_empty() -> VstPtr<dyn IBStream> {
                Self::from_bytes(Vec::new())
            }

            pub fn from_bytes(bytes: Vec<u8>) -> VstPtr<dyn IBStream> {
                let raw_box = Self::allocate(Mutex::new((bytes, 0)));
                let mut obj = std::ptr::null_mut();
                unsafe {
                    assert_eq!(raw_box.query_interface(&<dyn IBStream>::IID, &mut obj), kResultOk);
                    raw_box.release();
                    std::mem::forget(raw_box);
                    VstPtr::<dyn IBStream>::owned(obj as *mut *mut _).unwrap()
                }
            }

            pub fn rewind(stream: &VstPtr<dyn IBStream>) {
                unsafe {
                    assert_eq!(stream.seek(0, kIBSeekSet, std::ptr::null_mut()), kResultOk);
                }
            }
        }

        impl IBStream for MockVst3Stream {
            unsafe fn read(
                &self,
                buffer: *mut c_void,
                num_bytes: i32,
                num_bytes_read: *mut i32,
            ) -> tresult {
                if buffer.is_null() || num_bytes < 0 {
                    return kInvalidArgument;
                }
                let mut guard = self.buf.lock().unwrap();
                let (ref data, ref mut pos) = *guard;
                let available = data.len().saturating_sub(*pos);
                let count = available.min(num_bytes as usize);
                if count > 0 {
                    std::ptr::copy_nonoverlapping(
                        data.as_ptr().add(*pos),
                        buffer as *mut u8,
                        count,
                    );
                    *pos += count;
                }
                if !num_bytes_read.is_null() {
                    *num_bytes_read = count as i32;
                }
                kResultOk
            }

            unsafe fn write(
                &self,
                buffer: *const c_void,
                num_bytes: i32,
                num_bytes_written: *mut i32,
            ) -> tresult {
                if buffer.is_null() || num_bytes < 0 {
                    return kInvalidArgument;
                }
                let mut guard = self.buf.lock().unwrap();
                let (ref mut data, ref mut pos) = *guard;
                let slice = std::slice::from_raw_parts(buffer as *const u8, num_bytes as usize);
                let end = *pos + slice.len();
                if end > data.len() {
                    data.resize(end, 0);
                }
                data[*pos..end].copy_from_slice(slice);
                *pos = end;
                if !num_bytes_written.is_null() {
                    *num_bytes_written = slice.len() as i32;
                }
                kResultOk
            }

            unsafe fn seek(&self, pos: i64, mode: i32, result: *mut i64) -> tresult {
                let mut guard = self.buf.lock().unwrap();
                let (ref data, ref mut cursor) = *guard;
                let base: i64 = match mode {
                    x if x == kIBSeekSet => 0,
                    x if x == kIBSeekCur => *cursor as i64,
                    x if x == kIBSeekEnd => data.len() as i64,
                    _ => return kInvalidArgument,
                };
                let next = base + pos;
                if next < 0 {
                    return kInvalidArgument;
                }
                *cursor = (next as usize).min(data.len());
                if !result.is_null() {
                    *result = *cursor as i64;
                }
                kResultOk
            }

            unsafe fn tell(&self, pos: *mut i64) -> tresult {
                if pos.is_null() {
                    return kInvalidArgument;
                }
                let guard = self.buf.lock().unwrap();
                *pos = guard.1 as i64;
                kResultOk
            }
        }

        pub fn stream_to_bytes(stream: &VstPtr<dyn IBStream>) -> Vec<u8> {
            unsafe {
                let mut end_pos = 0_i64;
                assert_eq!(stream.seek(0, kIBSeekEnd, &mut end_pos), kResultOk);
                assert_eq!(stream.seek(0, kIBSeekSet, std::ptr::null_mut()), kResultOk);
                let mut out = vec![0_u8; end_pos as usize];
                let mut read_len = 0_i32;
                assert_eq!(
                    stream.read(out.as_mut_ptr() as *mut c_void, end_pos as i32, &mut read_len),
                    kResultOk
                );
                out.truncate(read_len as usize);
                out
            }
        }

        pub unsafe fn to_shared_stream(
            stream: &VstPtr<dyn IBStream>,
        ) -> SharedVstPtr<dyn IBStream> {
            std::mem::transmute(stream.as_ptr())
        }

        #[VST3(implements(IComponentHandler))]
        pub struct MockComponentHandler {
            pub restart_calls: std::sync::Arc<AtomicU32>,
            pub last_restart_flags: std::sync::Arc<AtomicI32>,
        }

        impl MockComponentHandler {
            pub fn new_shared(
                restart_calls: std::sync::Arc<AtomicU32>,
                last_restart_flags: std::sync::Arc<AtomicI32>,
            ) -> (VstPtr<dyn IComponentHandler>, SharedVstPtr<dyn IComponentHandler>) {
                let raw_box = Self::allocate(restart_calls, last_restart_flags);
                let mut obj = std::ptr::null_mut();
                unsafe {
                    assert_eq!(
                        raw_box.query_interface(&<dyn IComponentHandler>::IID, &mut obj),
                        kResultOk
                    );
                    raw_box.release();
                    std::mem::forget(raw_box);
                    let ptr = VstPtr::<dyn IComponentHandler>::owned(obj as *mut *mut _).unwrap();
                    let shared = std::mem::transmute(ptr.as_ptr());
                    (ptr, shared)
                }
            }
        }

        impl IComponentHandler for MockComponentHandler {
            unsafe fn begin_edit(&self, _id: ParamID) -> tresult {
                kResultOk
            }
            unsafe fn perform_edit(&self, _id: ParamID, _value_normalized: ParamValue) -> tresult {
                kResultOk
            }
            unsafe fn end_edit(&self, _id: ParamID) -> tresult {
                kResultOk
            }
            unsafe fn restart_component(&self, flags: i32) -> tresult {
                self.restart_calls.fetch_add(1, Ordering::SeqCst);
                self.last_restart_flags.store(flags, Ordering::SeqCst);
                kResultOk
            }
        }
    }

    #[test]
    fn test_filter_state_fills_missing_legacy_params() {
        use nih_plug::wrapper::state::ParamValue;
        use std::collections::BTreeMap;

        let mut legacy_params = BTreeMap::new();
        legacy_params.insert("threshold_db".to_string(), ParamValue::F32(-14.0));
        legacy_params.insert("ratio".to_string(), ParamValue::F32(6.0));
        legacy_params.insert("auto_makeup".to_string(), ParamValue::Bool(false));
        legacy_params.insert(
            "clipper_mode".to_string(),
            ParamValue::String("hard_clip".to_string()),
        );

        let mut fields = BTreeMap::new();
        fields.insert(
            "editor-state".to_string(),
            r#"{"size":[820,480]}"#.to_string(),
        );

        let mut state = PluginState {
            version: "0.2.0".to_string(),
            params: legacy_params,
            fields,
        };
        OmorePunch::filter_state(&mut state);

        assert_eq!(state.params.len(), 17);
        assert!(!state.fields.contains_key("editor-state"));

        // Preserved legacy fields
        assert!(matches!(state.params.get("threshold_db"), Some(ParamValue::F32(v)) if (*v - -14.0).abs() < 1e-6));
        assert!(matches!(state.params.get("ratio"), Some(ParamValue::F32(v)) if (*v - 6.0).abs() < 1e-6));
        assert!(matches!(state.params.get("auto_makeup"), Some(ParamValue::Bool(false))));
        assert!(matches!(state.params.get("clipper_mode"), Some(ParamValue::String(s)) if s == "hard_clip"));

        // Missing parameters filled with documented defaults
        assert!(matches!(state.params.get("input_db"), Some(ParamValue::F32(v)) if *v == 0.0));
        assert!(matches!(state.params.get("attack_ms"), Some(ParamValue::F32(v)) if *v == 3.0));
        assert!(matches!(state.params.get("release_ms"), Some(ParamValue::F32(v)) if *v == 80.0));
        assert!(matches!(state.params.get("release_mode"), Some(ParamValue::String(s)) if s == "manual"));
        assert!(matches!(state.params.get("sc_hpf_hz"), Some(ParamValue::F32(v)) if *v == 20.0));
        assert!(matches!(state.params.get("makeup_db"), Some(ParamValue::F32(v)) if *v == 0.0));
        assert!(matches!(state.params.get("eq_low_cut_hz"), Some(ParamValue::F32(v)) if *v == 20.0));
        assert!(matches!(state.params.get("eq_mid_cut_db"), Some(ParamValue::F32(v)) if *v == 0.0));
        assert!(matches!(state.params.get("eq_high_shelf_db"), Some(ParamValue::F32(v)) if *v == 0.0));
        assert!(matches!(state.params.get("ceiling_db"), Some(ParamValue::F32(v)) if *v == 0.0));
        assert!(matches!(state.params.get("mix_pct"), Some(ParamValue::F32(v)) if *v == 100.0));
        assert!(matches!(state.params.get("gain_match"), Some(ParamValue::Bool(false))));
        assert!(matches!(state.params.get("bypass"), Some(ParamValue::Bool(false))));
    }

    #[test]
    fn test_filter_state_clamps_out_of_range_and_nan() {
        use nih_plug::wrapper::state::ParamValue;
        use std::collections::BTreeMap;

        let mut params = BTreeMap::new();
        params.insert("input_db".to_string(), ParamValue::F32(999.0));
        params.insert("threshold_db".to_string(), ParamValue::F32(-120.0));
        params.insert("ratio".to_string(), ParamValue::F32(f32::NAN));
        params.insert("attack_ms".to_string(), ParamValue::F32(0.0));
        params.insert("release_ms".to_string(), ParamValue::F32(f32::INFINITY));
        params.insert("sc_hpf_hz".to_string(), ParamValue::F32(1000.0));
        params.insert("makeup_db".to_string(), ParamValue::F32(-50.0));
        params.insert("eq_low_cut_hz".to_string(), ParamValue::F32(-10.0));
        params.insert("eq_mid_cut_db".to_string(), ParamValue::F32(5.0));
        params.insert("eq_high_shelf_db".to_string(), ParamValue::F32(f32::NEG_INFINITY));
        params.insert("ceiling_db".to_string(), ParamValue::F32(3.0));
        params.insert("mix_pct".to_string(), ParamValue::F32(150.0));

        let mut state = PluginState {
            version: "0.3.1-dev".to_string(),
            params,
            fields: BTreeMap::new(),
        };
        sanitize_plugin_state(&mut state);

        assert!(matches!(state.params.get("input_db"), Some(ParamValue::F32(v)) if *v == 12.0));
        assert!(matches!(state.params.get("threshold_db"), Some(ParamValue::F32(v)) if *v == -40.0));
        assert!(matches!(state.params.get("ratio"), Some(ParamValue::F32(v)) if *v == 4.0));
        assert!(matches!(state.params.get("attack_ms"), Some(ParamValue::F32(v)) if *v == 0.1));
        assert!(matches!(state.params.get("release_ms"), Some(ParamValue::F32(v)) if *v == 80.0));
        assert!(matches!(state.params.get("sc_hpf_hz"), Some(ParamValue::F32(v)) if *v == 250.0));
        assert!(matches!(state.params.get("makeup_db"), Some(ParamValue::F32(v)) if *v == -12.0));
        assert!(matches!(state.params.get("eq_low_cut_hz"), Some(ParamValue::F32(v)) if *v == 20.0));
        assert!(matches!(state.params.get("eq_mid_cut_db"), Some(ParamValue::F32(v)) if *v == 0.0));
        assert!(matches!(state.params.get("eq_high_shelf_db"), Some(ParamValue::F32(v)) if *v == 0.0));
        assert!(matches!(state.params.get("ceiling_db"), Some(ParamValue::F32(v)) if *v == 0.0));
        assert!(matches!(state.params.get("mix_pct"), Some(ParamValue::F32(v)) if *v == 100.0));
    }

    #[test]
    fn test_filter_state_normalizes_enums_and_strips_unknowns() {
        use nih_plug::wrapper::state::ParamValue;
        use std::collections::BTreeMap;

        let mut params = BTreeMap::new();
        params.insert("release_mode".to_string(), ParamValue::I32(2));
        params.insert(
            "clipper_mode".to_string(),
            ParamValue::String("invalid_clip_mode".to_string()),
        );
        params.insert("auto_makeup".to_string(), ParamValue::I32(99));
        params.insert("gain_match".to_string(), ParamValue::F32(1.0));
        params.insert("gr_meter_db".to_string(), ParamValue::F32(-5.0));
        params.insert("unknown_future_param".to_string(), ParamValue::Bool(true));

        let mut state = PluginState {
            version: "0.3.0".to_string(),
            params,
            fields: BTreeMap::new(),
        };
        sanitize_plugin_state(&mut state);

        assert!(!state.params.contains_key("gr_meter_db"));
        assert!(!state.params.contains_key("unknown_future_param"));
        assert!(matches!(
            state.params.get("release_mode"),
            Some(ParamValue::String(s)) if s == "auto_punch"
        ));
        assert!(matches!(
            state.params.get("clipper_mode"),
            Some(ParamValue::String(s)) if s == "soft_clip"
        ));
        assert!(matches!(state.params.get("auto_makeup"), Some(ParamValue::Bool(true))));
        assert!(matches!(state.params.get("gain_match"), Some(ParamValue::Bool(false))));

        // Out-of-bounds enum index also falls back safely to default.
        let mut bad_idx = PluginState {
            version: String::new(),
            params: BTreeMap::from([("release_mode".to_string(), ParamValue::I32(42))]),
            fields: BTreeMap::new(),
        };
        sanitize_plugin_state(&mut bad_idx);
        assert!(matches!(
            bad_idx.params.get("release_mode"),
            Some(ParamValue::String(s)) if s == "manual"
        ));
    }

    #[test]
    fn test_clap_wrapper_state_roundtrip_and_host_sync() {
        use nih_plug::prelude::Params;
        use nih_plug::wrapper::clap::{
            clap_host, clap_plugin_factory, CLAP_PLUGIN_FACTORY_ID, CLAP_VERSION,
        };

        let tracker = ClapHostRescanTracker {
            rescan_calls: AtomicU32::new(0),
            last_rescan_flags: AtomicU32::new(0),
        };
        let host_name = b"ReaperPresetHost\0";
        let host = clap_host {
            clap_version: CLAP_VERSION,
            host_data: &tracker as *const ClapHostRescanTracker as *mut std::ffi::c_void,
            name: host_name.as_ptr() as *const std::ffi::c_char,
            vendor: std::ptr::null(),
            url: std::ptr::null(),
            version: std::ptr::null(),
            get_extension: Some(test_clap_host_get_extension_with_params),
            request_restart: Some(test_clap_host_noop),
            request_process: Some(test_clap_host_noop),
            request_callback: Some(test_clap_host_noop),
        };

        let default_params = OmorePunchParams::default();
        let default_map = default_params.param_map();

        unsafe {
            assert!((super::clap::clap_entry.init.unwrap())(std::ptr::null()));
            let factory = (super::clap::clap_entry.get_factory.unwrap())(
                CLAP_PLUGIN_FACTORY_ID.as_ptr(),
            ) as *const clap_plugin_factory;
            let desc = ((*factory).get_plugin_descriptor.unwrap())(factory, 0);

            let plugin_a = ((*factory).create_plugin.unwrap())(factory, &host, (*desc).id);
            let plugin_b = ((*factory).create_plugin.unwrap())(factory, &host, (*desc).id);
            assert!(((*plugin_a).init.unwrap())(plugin_a));
            assert!(((*plugin_b).init.unwrap())(plugin_b));

            let params_ext_a = ((*plugin_a).get_extension.unwrap())(
                plugin_a,
                b"clap.params\0".as_ptr() as *const std::ffi::c_char,
            ) as *const TestClapPluginParams;
            let params_ext_b = ((*plugin_b).get_extension.unwrap())(
                plugin_b,
                b"clap.params\0".as_ptr() as *const std::ffi::c_char,
            ) as *const TestClapPluginParams;
            let state_ext_a = ((*plugin_a).get_extension.unwrap())(
                plugin_a,
                b"clap.state\0".as_ptr() as *const std::ffi::c_char,
            ) as *const super::clap::ClapPluginState;
            let state_ext_b = ((*plugin_b).get_extension.unwrap())(
                plugin_b,
                b"clap.state\0".as_ptr() as *const std::ffi::c_char,
            ) as *const super::clap::ClapPluginState;
            assert!(!params_ext_a.is_null() && !params_ext_b.is_null());
            assert!(!state_ext_a.is_null() && !state_ext_b.is_null());

            // 1. Verify CLAP default_value matches OmorePunchParams::default() for all 16 params.
            let count = ((*params_ext_b).count.unwrap())(plugin_b);
            assert_eq!(count as usize, default_map.len());
            let mut param_ids = Vec::with_capacity(count as usize);
            for (idx, (_, ptr, _)) in default_map.iter().enumerate() {
                let mut info: TestClapParamInfo = std::mem::zeroed();
                assert!(((*params_ext_b).get_info.unwrap())(plugin_b, idx as u32, &mut info));
                let expected_default =
                    ptr.default_normalized_value() as f64 * ptr.step_count().unwrap_or(1) as f64;
                assert!(
                    (info.default_value - expected_default).abs() < 1e-6,
                    "CLAP default_value mismatch at index {}: got {}, expected {}",
                    idx,
                    info.default_value,
                    expected_default
                );
                param_ids.push(info.id);
            }

            // 2. Load Preset A into plugin_a and save via clap_plugin_state::save.
            let preset_a_chunk = encode_clap_state_chunk(PRESET_A_JSON.as_bytes());
            let mut cursor_a = ClapReadCursor {
                bytes: &preset_a_chunk,
                pos: 0,
            };
            let istream_a = TestClapIStream {
                ctx: &mut cursor_a as *mut ClapReadCursor<'_> as *mut std::ffi::c_void,
                read: Some(test_clap_istream_read),
            };
            assert!(((*state_ext_a).load.unwrap())(
                plugin_a,
                &istream_a as *const TestClapIStream as *const std::ffi::c_void
            ));

            let mut saved_sink = ClapWriteBuffer { bytes: Vec::new() };
            let ostream_a = TestClapOStream {
                ctx: &mut saved_sink as *mut ClapWriteBuffer as *mut std::ffi::c_void,
                write: Some(test_clap_ostream_write),
            };
            assert!(((*state_ext_a).save.unwrap())(
                plugin_a,
                &ostream_a as *const TestClapOStream as *const std::ffi::c_void
            ));

            // 3. Load Preset B into inactive plugin_b first, seed stale telemetry latches,
            //    then recall Preset A chunk and verify full parameter fidelity, host rescan,
            //    and automatic latch reset (FR-008).
            let preset_b_chunk = encode_clap_state_chunk(PRESET_B_JSON.as_bytes());
            let mut cursor_b1 = ClapReadCursor {
                bytes: &preset_b_chunk,
                pos: 0,
            };
            let istream_b1 = TestClapIStream {
                ctx: &mut cursor_b1 as *mut ClapReadCursor<'_> as *mut std::ffi::c_void,
                read: Some(test_clap_istream_read),
            };
            assert!(((*state_ext_b).load.unwrap())(
                plugin_b,
                &istream_b1 as *const TestClapIStream as *const std::ffi::c_void
            ));

            let tel_b = super::clap::test_telemetry(plugin_b);
            store_f32_relaxed(&tel_b.current_gr_db, 5.5);
            store_f32_relaxed(&tel_b.max_gr_db, 14.2);
            tel_b.clip_latched.store(true, Ordering::Relaxed);

            let prev_rescans = tracker.rescan_calls.load(Ordering::SeqCst);
            let mut cursor_b2 = ClapReadCursor {
                bytes: &saved_sink.bytes,
                pos: 0,
            };
            let istream_b2 = TestClapIStream {
                ctx: &mut cursor_b2 as *mut ClapReadCursor<'_> as *mut std::ffi::c_void,
                read: Some(test_clap_istream_read),
            };
            assert!(((*state_ext_b).load.unwrap())(
                plugin_b,
                &istream_b2 as *const TestClapIStream as *const std::ffi::c_void
            ));

            assert_eq!(tracker.rescan_calls.load(Ordering::SeqCst), prev_rescans + 1);
            assert_eq!(tracker.last_rescan_flags.load(Ordering::SeqCst), 1);

            assert_eq!(load_f32_relaxed(&tel_b.current_gr_db), 0.0);
            assert_eq!(load_f32_relaxed(&tel_b.max_gr_db), 0.0);
            assert!(!tel_b.clip_latched.load(Ordering::Relaxed));

            for &param_id in &param_ids {
                let mut val_a = 0.0_f64;
                let mut val_b = 0.0_f64;
                assert!(((*params_ext_a).get_value.unwrap())(plugin_a, param_id, &mut val_a));
                assert!(((*params_ext_b).get_value.unwrap())(plugin_b, param_id, &mut val_b));
                assert!(
                    (val_a - val_b).abs() < 1e-6,
                    "CLAP param {} mismatch after recall: A={} B={}",
                    param_id,
                    val_a,
                    val_b
                );
            }

            // 4. Malformed / truncated stream must return false cleanly without altering state.
            let bad_chunk = encode_clap_state_chunk(b"{not-valid-json");
            let mut bad_cursor = ClapReadCursor {
                bytes: &bad_chunk,
                pos: 0,
            };
            let bad_istream = TestClapIStream {
                ctx: &mut bad_cursor as *mut ClapReadCursor<'_> as *mut std::ffi::c_void,
                read: Some(test_clap_istream_read),
            };
            assert!(!((*state_ext_b).load.unwrap())(
                plugin_b,
                &bad_istream as *const TestClapIStream as *const std::ffi::c_void
            ));
            let empty_bytes: [u8; 0] = [];
            let mut empty_cursor = ClapReadCursor {
                bytes: &empty_bytes,
                pos: 0,
            };
            let empty_istream = TestClapIStream {
                ctx: &mut empty_cursor as *mut ClapReadCursor<'_> as *mut std::ffi::c_void,
                read: Some(test_clap_istream_read),
            };
            assert!(!((*state_ext_b).load.unwrap())(
                plugin_b,
                &empty_istream as *const TestClapIStream as *const std::ffi::c_void
            ));

            ((*plugin_a).destroy.unwrap())(plugin_a);
            ((*plugin_b).destroy.unwrap())(plugin_b);
        }
    }

    #[test]
    fn test_vst3_proxy_state_roundtrip_and_host_sync() {
        use nih_plug::prelude::{Params, Vst3Plugin};
        use nih_plug::wrapper::vst3::vst3_sys::base::{
            kResultFalse, kResultOk, IPluginBase, IPluginFactory,
        };
        use nih_plug::wrapper::vst3::vst3_sys::vst::{
            IComponent, IEditController, ParameterInfo, RestartFlags,
        };
        use nih_plug::wrapper::vst3::vst3_sys::{ComInterface, VstPtr, IID};
        use vst3_test_mocks::{
            stream_to_bytes, to_shared_stream, MockComponentHandler, MockVst3Stream,
        };

        let default_params = OmorePunchParams::default();
        let default_map = default_params.param_map();

        unsafe {
            let factory =
                VstPtr::<dyn IPluginFactory>::owned(super::vst3::GetPluginFactory() as *mut *mut _)
                    .unwrap();
            let cid = IID {
                data: OmorePunch::PLATFORM_VST3_CLASS_ID,
            };

            let mut raw_a = std::ptr::null_mut();
            let mut raw_b = std::ptr::null_mut();
            let _ = super::vst3::take_test_vst3_telemetry();
            assert_eq!(
                factory.create_instance(&cid, &<dyn IComponent>::IID, &mut raw_a),
                kResultOk
            );
            let _tel_a = super::vst3::take_test_vst3_telemetry().expect("tel_a missing");
            assert_eq!(
                factory.create_instance(&cid, &<dyn IComponent>::IID, &mut raw_b),
                kResultOk
            );
            let tel_b = super::vst3::take_test_vst3_telemetry().expect("tel_b missing");

            let comp_a = VstPtr::<dyn IComponent>::owned(raw_a as *mut *mut _).unwrap();
            let comp_b = VstPtr::<dyn IComponent>::owned(raw_b as *mut *mut _).unwrap();
            let edit_a = comp_a.cast::<dyn IEditController>().unwrap();
            let edit_b = comp_b.cast::<dyn IEditController>().unwrap();

            assert_eq!(comp_a.initialize(std::ptr::null_mut()), kResultOk);
            assert_eq!(comp_b.initialize(std::ptr::null_mut()), kResultOk);

            let restart_calls = Arc::new(std::sync::atomic::AtomicU32::new(0));
            let last_restart_flags = Arc::new(std::sync::atomic::AtomicI32::new(0));
            let (_handler_keepalive, handler_shared) =
                MockComponentHandler::new_shared(restart_calls.clone(), last_restart_flags.clone());
            assert_eq!(edit_b.set_component_handler(handler_shared), kResultOk);

            // 1. Verify VST3 ParameterInfo default_normalized_value matches OmorePunchParams::default().
            let count = edit_b.get_parameter_count();
            assert_eq!(count as usize, default_map.len());
            let mut param_infos = Vec::with_capacity(count as usize);
            for (idx, (_, ptr, _)) in default_map.iter().enumerate() {
                let mut info: ParameterInfo = std::mem::zeroed();
                assert_eq!(edit_b.get_parameter_info(idx as i32, &mut info), kResultOk);
                let expected_norm = ptr.default_normalized_value() as f64;
                assert!(
                    (info.default_normalized_value - expected_norm).abs() < 1e-6,
                    "VST3 default_normalized_value mismatch at index {}: got {}, expected {}",
                    idx,
                    info.default_normalized_value,
                    expected_norm
                );
                param_infos.push(info);
            }

            // 2. Load Preset A into comp_a and serialize via IComponent::get_state.
            let stream_a_in = MockVst3Stream::from_bytes(PRESET_A_JSON.as_bytes().to_vec());
            assert_eq!(comp_a.set_state(to_shared_stream(&stream_a_in)), kResultOk);

            let stream_a_out = MockVst3Stream::new_empty();
            assert_eq!(comp_a.get_state(to_shared_stream(&stream_a_out)), kResultOk);
            MockVst3Stream::rewind(&stream_a_out);

            // 3. Load Preset B into never-activated comp_b, seed stale latches, then recall A.
            let stream_b_in = MockVst3Stream::from_bytes(PRESET_B_JSON.as_bytes().to_vec());
            assert_eq!(comp_b.set_state(to_shared_stream(&stream_b_in)), kResultOk);

            store_f32_relaxed(&tel_b.current_gr_db, 4.25);
            store_f32_relaxed(&tel_b.max_gr_db, 16.0);
            tel_b.clip_latched.store(true, Ordering::Relaxed);

            // Reusing stream_a_out verifies exact IComponent::get_state -> set_state round-trip.
            let prev_restarts = restart_calls.load(Ordering::SeqCst);
            let saved_bytes = stream_to_bytes(&stream_a_out);
            let stream_recall = MockVst3Stream::from_bytes(saved_bytes);
            assert_eq!(comp_b.set_state(to_shared_stream(&stream_recall)), kResultOk);

            // Verify IComponentHandler::restart_component(kParamValuesChanged) was invoked.
            assert_eq!(restart_calls.load(Ordering::SeqCst), prev_restarts + 1);
            assert_eq!(
                last_restart_flags.load(Ordering::SeqCst),
                RestartFlags::kParamValuesChanged as i32
            );
            assert_eq!(load_f32_relaxed(&tel_b.current_gr_db), 0.0);
            assert_eq!(load_f32_relaxed(&tel_b.max_gr_db), 0.0);
            assert!(!tel_b.clip_latched.load(Ordering::Relaxed));

            for info in &param_infos {
                let norm_a = edit_a.get_param_normalized(info.id);
                let norm_b = edit_b.get_param_normalized(info.id);
                assert!(
                    (norm_a - norm_b).abs() < 1e-6,
                    "VST3 param id {} mismatch after recall: A={} B={}",
                    info.id,
                    norm_a,
                    norm_b
                );
            }

            // 4. Verify Reaper-style reset-to-default via set_param_normalized restores exact defaults.
            for info in &param_infos {
                assert_eq!(
                    edit_b.set_param_normalized(info.id, info.default_normalized_value),
                    kResultOk
                );
                let cur = edit_b.get_param_normalized(info.id);
                assert!(
                    (cur - info.default_normalized_value).abs() < 1e-6,
                    "VST3 reset to default mismatch for param id {}: got {}, expected {}",
                    info.id,
                    cur,
                    info.default_normalized_value
                );
            }

            // 5. Malformed and empty streams must fail gracefully with kResultFalse.
            let bad_stream = MockVst3Stream::from_bytes(b"{corrupt-preset".to_vec());
            assert_eq!(comp_b.set_state(to_shared_stream(&bad_stream)), kResultFalse);
            let empty_stream = MockVst3Stream::new_empty();
            assert_eq!(comp_b.set_state(to_shared_stream(&empty_stream)), kResultFalse);

            // Verify handler flag constants match VST3 RestartFlags::kParamValuesChanged (1 << 2).
            assert_eq!(RestartFlags::kParamValuesChanged as i32, 1 << 2);

            assert_eq!(comp_a.terminate(), kResultOk);
            assert_eq!(comp_b.terminate(), kResultOk);
        }
    }

    #[test]
    fn test_live_preset_switching_stability() {
        use nih_plug::prelude::Vst3Plugin;
        use nih_plug::wrapper::vst3::vst3_sys::base::{kResultOk, IPluginBase, IPluginFactory};
        use nih_plug::wrapper::vst3::vst3_sys::vst::{
            BusDirections, IAudioProcessor, IComponent, IEditController, MediaTypes, ProcessModes,
            ProcessSetup, SymbolicSampleSizes,
        };
        use nih_plug::wrapper::vst3::vst3_sys::{ComInterface, VstPtr, IID};
        use vst3_test_mocks::{to_shared_stream, MockVst3Stream};

        let frames = 256_usize;
        let drum_like_l: Vec<f32> = (0..frames)
            .map(|i| {
                let t = i as f32 / 48000.0;
                1.4 * (2.0 * std::f32::consts::PI * 95.0 * t).sin() * (-18.0 * t).exp()
                    + 0.4 * (2.0 * std::f32::consts::PI * 1800.0 * t).sin()
            })
            .collect();
        let drum_like_r: Vec<f32> = drum_like_l.iter().map(|x| x * 0.95).collect();

        let default_json = concat!(
            r#"{"version":"0.3.1-dev","params":{"#,
            r#""attack_ms":{"f32":3.0},"auto_makeup":{"bool":true},"bypass":{"bool":false},"#,
            r#""ceiling_db":{"f32":0.0},"clipper_mode":{"string":"soft_clip"},"#,
            r#""eq_high_shelf_db":{"f32":0.0},"eq_low_cut_hz":{"f32":20.0},"#,
            r#""eq_mid_cut_db":{"f32":0.0},"gain_match":{"bool":false},"#,
            r#""input_db":{"f32":0.0},"makeup_db":{"f32":0.0},"#,
            r#""mix_pct":{"f32":100.0},"ratio":{"f32":4.0},"#,
            r#""release_mode":{"string":"manual"},"release_ms":{"f32":80.0},"#,
            r#""sc_hpf_hz":{"f32":20.0},"threshold_db":{"f32":0.0}"#,
            r#"},"fields":{}}"#
        );

        unsafe {
            let factory =
                VstPtr::<dyn IPluginFactory>::owned(super::vst3::GetPluginFactory() as *mut *mut _)
                    .unwrap();
            let cid = IID {
                data: OmorePunch::PLATFORM_VST3_CLASS_ID,
            };
            let mut raw = std::ptr::null_mut();
            let _ = super::vst3::take_test_vst3_telemetry();
            assert_eq!(
                factory.create_instance(&cid, &<dyn IComponent>::IID, &mut raw),
                kResultOk
            );
            let tel = super::vst3::take_test_vst3_telemetry().expect("telemetry missing");
            let comp = VstPtr::<dyn IComponent>::owned(raw as *mut *mut _).unwrap();
            let edit = comp.cast::<dyn IEditController>().unwrap();
            let proc_if = comp.cast::<dyn IAudioProcessor>().unwrap();

            assert_eq!(comp.initialize(std::ptr::null_mut()), kResultOk);
            assert_eq!(
                comp.activate_bus(MediaTypes::kAudio as i32, BusDirections::kInput as i32, 0, 1),
                kResultOk
            );
            assert_eq!(
                comp.activate_bus(MediaTypes::kAudio as i32, BusDirections::kOutput as i32, 0, 1),
                kResultOk
            );
            let mut arr_in: u64 = 3;
            let mut arr_out: u64 = 3;
            assert_eq!(
                proc_if.set_bus_arrangements(&mut arr_in, 1, &mut arr_out, 1),
                kResultOk
            );
            let setup = ProcessSetup {
                process_mode: ProcessModes::kRealtime as i32,
                symbolic_sample_size: SymbolicSampleSizes::kSample32 as i32,
                max_samples_per_block: frames as i32,
                sample_rate: 48000.0,
            };
            assert_eq!(proc_if.setup_processing(&setup), kResultOk);
            assert_eq!(comp.set_active(1), kResultOk);
            assert_eq!(proc_if.set_processing(1), kResultOk);

            let presets = [PRESET_A_JSON, PRESET_B_JSON, default_json];
            for step in 0..10 {
                let json = presets[step % presets.len()];
                let stream = MockVst3Stream::from_bytes(json.as_bytes().to_vec());
                assert_eq!(comp.set_state(to_shared_stream(&stream)), kResultOk);
                assert_eq!(load_f32_relaxed(&tel.max_gr_db), 0.0);
                assert!(!tel.clip_latched.load(Ordering::Relaxed));

                process_vst3_stereo_block(&proc_if, &drum_like_l, &drum_like_r);
            }

            // Finish on default preset and verify all parameters match documented defaults.
            let stream_def = MockVst3Stream::from_bytes(default_json.as_bytes().to_vec());
            assert_eq!(comp.set_state(to_shared_stream(&stream_def)), kResultOk);
            for idx in 0..edit.get_parameter_count() {
                let mut info = std::mem::zeroed();
                assert_eq!(edit.get_parameter_info(idx, &mut info), kResultOk);
                let val = edit.get_param_normalized(info.id);
                assert!(
                    (val - info.default_normalized_value).abs() < 1e-6,
                    "Default preset param {} drifted: {} != {}",
                    idx,
                    val,
                    info.default_normalized_value
                );
            }

            assert_eq!(proc_if.set_processing(0), kResultOk);
            assert_eq!(comp.set_active(0), kResultOk);
            assert_eq!(comp.terminate(), kResultOk);
        }

        // Also verify direct DSP switching across 10 cycles stays finite and within safety bounds.
        let mut dsp = StereoPunchDsp::new(48000.0);
        let cfg_a = DspConfig {
            input_db: 3.25,
            threshold_db: -18.5,
            ratio: 8.0,
            attack_ms: 12.5,
            release_ms: 145.0,
            release_mode: ReleaseMode::AutoPunch,
            sc_hpf_hz: 200.0,
            auto_makeup: false,
            makeup_db: 6.0,
            eq_low_cut_hz: 75.0,
            eq_mid_cut_db: -3.5,
            eq_high_shelf_db: 4.5,
            clipper_mode: ClipperMode::HardClip,
            ceiling_db: -1.5,
            mix_pct: 65.0,
            gain_match: true,
            bypass: false,
        };
        let cfg_b = DspConfig {
            threshold_db: -8.0,
            ratio: 2.5,
            release_mode: ReleaseMode::AutoGlue,
            clipper_mode: ClipperMode::Bypass,
            ..DspConfig::default()
        };
        let cfg_default = DspConfig::default();
        let cfgs = [cfg_a, cfg_b, cfg_default];
        for step in 0..10 {
            dsp.reset();
            let cfg = &cfgs[step % cfgs.len()];
            for i in 0..frames {
                let out = dsp.process_sample(drum_like_l[i], drum_like_r[i], cfg);
                assert!(out.out_l.is_finite() && out.out_r.is_finite());
                assert!(out.out_l.abs() <= SAFETY_CLAMP_LIMIT);
                assert!(out.out_r.abs() <= SAFETY_CLAMP_LIMIT);
            }
        }
    }

    #[test]
    fn test_preset_json_portability_and_backward_compat() {
        use nih_plug::prelude::Vst3Plugin;
        use nih_plug::wrapper::vst3::vst3_sys::base::{kResultOk, IPluginBase, IPluginFactory};
        use nih_plug::wrapper::vst3::vst3_sys::vst::{IComponent, IEditController};
        use nih_plug::wrapper::vst3::vst3_sys::{ComInterface, VstPtr, IID};
        use vst3_test_mocks::{stream_to_bytes, to_shared_stream, MockVst3Stream};

        unsafe {
            let factory =
                VstPtr::<dyn IPluginFactory>::owned(super::vst3::GetPluginFactory() as *mut *mut _)
                    .unwrap();
            let cid = IID {
                data: OmorePunch::PLATFORM_VST3_CLASS_ID,
            };
            let mut raw = std::ptr::null_mut();
            assert_eq!(
                factory.create_instance(&cid, &<dyn IComponent>::IID, &mut raw),
                kResultOk
            );
            let comp = VstPtr::<dyn IComponent>::owned(raw as *mut *mut _).unwrap();
            let edit = comp.cast::<dyn IEditController>().unwrap();
            assert_eq!(comp.initialize(std::ptr::null_mut()), kResultOk);

            // First load Preset A (non-default EQ/AutoRelease/GainMatch), then load legacy v0.2.0
            // state missing those fields + containing legacy editor-state.
            let stream_a = MockVst3Stream::from_bytes(PRESET_A_JSON.as_bytes().to_vec());
            assert_eq!(comp.set_state(to_shared_stream(&stream_a)), kResultOk);

            let stream_legacy =
                MockVst3Stream::from_bytes(LEGACY_V02_PRESET_JSON.as_bytes().to_vec());
            assert_eq!(comp.set_state(to_shared_stream(&stream_legacy)), kResultOk);

            let out_stream = MockVst3Stream::new_empty();
            assert_eq!(comp.get_state(to_shared_stream(&out_stream)), kResultOk);
            let json_bytes = stream_to_bytes(&out_stream);
            let json_str = std::str::from_utf8(&json_bytes).expect("State must be UTF-8 JSON");

            // Must not persist OS/editor window state and must contain sanitized defaults for new params.
            assert!(!json_str.contains("editor-state"));
            assert!(json_str.contains("\"eq_low_cut_hz\":{\"f32\":20.0}"));
            assert!(json_str.contains("\"eq_mid_cut_db\":{\"f32\":0.0}"));
            assert!(json_str.contains("\"eq_high_shelf_db\":{\"f32\":0.0}"));
            assert!(json_str.contains("\"release_mode\":{\"string\":\"manual\"}"));
            assert!(json_str.contains("\"gain_match\":{\"bool\":false}"));
            assert!(json_str.contains("\"bypass\":{\"bool\":false}"));
            // Preserved legacy threshold (-16 dB) and ratio (6.0).
            assert!(json_str.contains("\"threshold_db\":{\"f32\":-16.0}"));
            assert!(json_str.contains("\"ratio\":{\"f32\":6.0}"));

            // Exporting and importing into a second fresh instance produces identical JSON.
            let mut raw_b = std::ptr::null_mut();
            assert_eq!(
                factory.create_instance(&cid, &<dyn IComponent>::IID, &mut raw_b),
                kResultOk
            );
            let comp_b = VstPtr::<dyn IComponent>::owned(raw_b as *mut *mut _).unwrap();
            let edit_b = comp_b.cast::<dyn IEditController>().unwrap();
            assert_eq!(comp_b.initialize(std::ptr::null_mut()), kResultOk);

            let import_stream = MockVst3Stream::from_bytes(json_bytes.clone());
            assert_eq!(
                comp_b.set_state(to_shared_stream(&import_stream)),
                kResultOk
            );
            let out_stream_b = MockVst3Stream::new_empty();
            assert_eq!(comp_b.get_state(to_shared_stream(&out_stream_b)), kResultOk);
            assert_eq!(stream_to_bytes(&out_stream_b), json_bytes);

            for idx in 0..edit.get_parameter_count() {
                let mut info = std::mem::zeroed();
                assert_eq!(edit.get_parameter_info(idx, &mut info), kResultOk);
                assert!(
                    (edit.get_param_normalized(info.id) - edit_b.get_param_normalized(info.id))
                        .abs()
                        < 1e-6
                );
            }

            assert_eq!(comp.terminate(), kResultOk);
            assert_eq!(comp_b.terminate(), kResultOk);
        }
    }

    #[test]
    fn test_sample_rate_change_updates_all_coeffs_and_guards_non_finite() {
        let cfg = DspConfig {
            threshold_db: -24.0,
            ratio: 8.0,
            attack_ms: 4.5,
            release_ms: 120.0,
            release_mode: ReleaseMode::AutoPunch,
            sc_hpf_hz: 90.0,
            eq_low_cut_hz: 80.0,
            eq_mid_cut_db: -3.5,
            eq_high_shelf_db: 2.5,
            gain_match: true,
            ..DspConfig::default()
        };

        let mut reference = StereoPunchDsp::new(96000.0);
        reference.process_sample(0.5, -0.5, &cfg);

        // 1. Sample rate change triggered through every public entry point must match fresh 96 kHz
        for entry in ["set_sample_rate", "update_ballistics", "update_sidechain_hpf", "update_eq"] {
            let mut dsp = StereoPunchDsp::default();
            // Warm up auto_slow_gr_db at 48 kHz first to verify reset on sample-rate change
            for _ in 0..256 {
                dsp.process_sample(0.8, 0.8, &cfg);
            }
            assert!(
                dsp.auto_slow_gr_db > 0.1,
                "[{entry}] expected non-zero auto_slow_gr_db before SR change"
            );

            match entry {
                "set_sample_rate" => dsp.set_sample_rate(96000.0),
                "update_ballistics" => {
                    dsp.update_ballistics(96000.0, cfg.attack_ms, cfg.release_ms, cfg.release_mode);
                }
                "update_sidechain_hpf" => {
                    dsp.update_sidechain_hpf(96000.0, cfg.sc_hpf_hz);
                }
                "update_eq" => {
                    dsp.update_eq(
                        96000.0,
                        cfg.eq_low_cut_hz,
                        cfg.eq_mid_cut_db,
                        cfg.eq_high_shelf_db,
                    );
                }
                _ => unreachable!(),
            }

            assert_eq!(
                dsp.auto_slow_gr_db.to_bits(),
                0.0_f32.to_bits(),
                "[{entry}] auto_slow_gr_db must reset to 0.0 on sample rate change"
            );
            assert_eq!(
                dsp.gm_rms_coeff.to_bits(),
                reference.gm_rms_coeff.to_bits(),
                "[{entry}] gm_rms_coeff stale after SR change"
            );
            assert_eq!(
                dsp.gm_presence_coeff.to_bits(),
                reference.gm_presence_coeff.to_bits(),
                "[{entry}] gm_presence_coeff stale after SR change"
            );
            assert_eq!(
                dsp.gm_slew_coeff.to_bits(),
                reference.gm_slew_coeff.to_bits(),
                "[{entry}] gm_slew_coeff stale after SR change"
            );
            assert_eq!(
                dsp.gm_off_slew_coeff.to_bits(),
                reference.gm_off_slew_coeff.to_bits(),
                "[{entry}] gm_off_slew_coeff stale after SR change"
            );
            assert_eq!(
                dsp.bypass_step.to_bits(),
                reference.bypass_step.to_bits(),
                "[{entry}] bypass_step stale after SR change"
            );

            // Subsequent set_sample_rate(96000.0) + process_sample must produce identical coefficients
            dsp.set_sample_rate(96000.0);
            dsp.process_sample(0.5, -0.5, &cfg);
            assert_eq!(dsp.attack_coeff.to_bits(), reference.attack_coeff.to_bits());
            assert_eq!(dsp.release_coeff.to_bits(), reference.release_coeff.to_bits());
            assert_eq!(
                dsp.auto_fast_release_coeff.to_bits(),
                reference.auto_fast_release_coeff.to_bits()
            );
            assert_eq!(
                dsp.auto_slow_release_coeff.to_bits(),
                reference.auto_slow_release_coeff.to_bits()
            );
            assert_eq!(
                dsp.auto_slow_charge_coeff.to_bits(),
                reference.auto_slow_charge_coeff.to_bits()
            );
            assert_eq!(dsp.sc_hpf_coeff.to_bits(), reference.sc_hpf_coeff.to_bits());
            assert_eq!(
                dsp.eq_low_cut.coeffs.b0.to_bits(),
                reference.eq_low_cut.coeffs.b0.to_bits()
            );
            assert_eq!(
                dsp.eq_mid_cut.coeffs.b0.to_bits(),
                reference.eq_mid_cut.coeffs.b0.to_bits()
            );
            assert_eq!(
                dsp.eq_high_shelf.coeffs.b0.to_bits(),
                reference.eq_high_shelf.coeffs.b0.to_bits()
            );
        }

        // 2. Non-finite sample_rate / sc_hpf_hz / attack_ms / release_ms must never corrupt state
        let mut guard_dsp = StereoPunchDsp::new(96000.0);
        for bad_sr in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            guard_dsp.set_sample_rate(bad_sr);
            guard_dsp.update_ballistics(bad_sr, f32::NAN, f32::INFINITY, ReleaseMode::AutoTight);
            guard_dsp.update_sidechain_hpf(bad_sr, f32::NAN);
            guard_dsp.update_eq(bad_sr, f32::NAN, f32::INFINITY, f32::NEG_INFINITY);
        }
        guard_dsp.update_sidechain_hpf(44100.0, f32::NAN);
        guard_dsp.update_ballistics(44100.0, f32::NAN, f32::NEG_INFINITY, ReleaseMode::AutoGlue);

        for (name, val) in [
            ("sample_rate", guard_dsp.sample_rate),
            ("attack_coeff", guard_dsp.attack_coeff),
            ("release_coeff", guard_dsp.release_coeff),
            ("auto_fast_release_coeff", guard_dsp.auto_fast_release_coeff),
            ("auto_slow_release_coeff", guard_dsp.auto_slow_release_coeff),
            ("auto_slow_charge_coeff", guard_dsp.auto_slow_charge_coeff),
            ("sc_hpf_coeff", guard_dsp.sc_hpf_coeff),
            ("gm_rms_coeff", guard_dsp.gm_rms_coeff),
            ("gm_presence_coeff", guard_dsp.gm_presence_coeff),
            ("gm_slew_coeff", guard_dsp.gm_slew_coeff),
            ("gm_off_slew_coeff", guard_dsp.gm_off_slew_coeff),
            ("bypass_step", guard_dsp.bypass_step),
            ("eq_low_cut.b0", guard_dsp.eq_low_cut.coeffs.b0),
            ("eq_mid_cut.b0", guard_dsp.eq_mid_cut.coeffs.b0),
            ("eq_high_shelf.b0", guard_dsp.eq_high_shelf.coeffs.b0),
        ] {
            assert!(val.is_finite(), "Non-finite coefficient {name}: {val}");
        }
        let out = guard_dsp.process_sample(0.4, -0.4, &cfg);
        assert!(out.out_l.is_finite() && out.out_r.is_finite());
    }
}