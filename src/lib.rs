// ============================================================================
// OMORE PUNCH - STEREO DRUM BUS PARALLEL COMPRESSOR & CLIPPER
// Monolithic Architecture (src/lib.rs) - 100% Pure ASCII
// Compatible with Reaper on Linux (Debian) and Windows 11 (VST3 & CLAP)
// ============================================================================

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
pub struct DspConfig {
    pub input_db: f32,
    pub threshold_db: f32,
    pub ratio: f32,
    pub attack_ms: f32,
    pub release_ms: f32,
    pub sc_hpf_hz: f32,
    pub auto_makeup: bool,
    pub makeup_db: f32,
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
            env_gr_db: 0.0,
        };
        dsp.update_ballistics(48000.0, 3.0, 80.0);
        dsp.update_sidechain_hpf(48000.0, 20.0);
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
    }

    #[inline(always)]
    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        let sr = sample_rate.max(8000.0);
        if (sr - self.sample_rate).abs() > 1e-3 {
            self.sample_rate = sr;
            let att = self.cached_attack_ms.max(0.1);
            let rel = self.cached_release_ms.max(10.0);
            let hpf = self.cached_sc_hpf_hz.clamp(20.0, 250.0);
            self.cached_attack_ms = -1.0;
            self.cached_release_ms = -1.0;
            self.cached_sc_hpf_hz = -1.0;
            self.update_ballistics(sr, att, rel);
            self.update_sidechain_hpf(sr, hpf);
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

        if sr_changed || hpf_changed {
            self.sample_rate = sr;
            self.cached_sc_hpf_hz = clamped_hz;
            let rc = 1.0 / (2.0 * std::f32::consts::PI * clamped_hz);
            let dt = 1.0 / sr;
            self.sc_hpf_coeff = rc / (rc + dt);
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

        // Update envelope and sidechain filter coefficients only if parameters changed
        self.update_ballistics(self.sample_rate, cfg.attack_ms, cfg.release_ms);
        self.update_sidechain_hpf(self.sample_rate, cfg.sc_hpf_hz);

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

        // 6. Final Clipper Stage (Soft Knee Saturation + Hard Ceiling)
        let ceiling_lin = db_to_lin(cfg.ceiling_db).max(1.0e-4);
        let (wet_l, wet_r, clipped) = match cfg.clipper_mode {
            ClipperMode::SoftClip => {
                let (cl_l, over_l) = Self::soft_clip_channel(comp_l, ceiling_lin);
                let (cl_r, over_r) = Self::soft_clip_channel(comp_r, ceiling_lin);
                (cl_l, cl_r, over_l || over_r)
            }
            ClipperMode::HardClip => {
                let over = comp_l.abs() > ceiling_lin || comp_r.abs() > ceiling_lin;
                (
                    comp_l.clamp(-ceiling_lin, ceiling_lin),
                    comp_r.clamp(-ceiling_lin, ceiling_lin),
                    over,
                )
            }
            ClipperMode::Bypass => (comp_l, comp_r, false),
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
            editor_state: EguiState::from_size(820, 560),

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
fn draw_param_row(
    ui: &mut egui::Ui,
    setter: &ParamSetter,
    param: &FloatParam,
    label: &str,
    step: f32,
    min_val: f32,
    max_val: f32,
) {
    let current_val = param.value();
    let default_val = param.default_plain_value();
    let formatted_val = param.to_string();

    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new(format!("{:<14}", label))
                .monospace()
                .strong()
                .color(egui::Color32::from_rgb(220, 224, 232)),
        );

        if ui
            .add_sized([28.0, 22.0], egui::Button::new("-"))
            .on_hover_text("Decrease value")
            .clicked()
        {
            let next_val = (current_val - step).clamp(min_val, max_val);
            setter.begin_set_parameter(param);
            setter.set_parameter(param, next_val);
            setter.end_set_parameter(param);
        }

        let mut slider_val = current_val;
        let slider_resp = ui.add_sized(
            [210.0, 22.0],
            egui::Slider::new(&mut slider_val, min_val..=max_val).show_value(false),
        );

        if slider_resp.drag_started() {
            setter.begin_set_parameter(param);
        }
        if slider_resp.changed() {
            setter.set_parameter(param, slider_val.clamp(min_val, max_val));
        }
        if slider_resp.drag_stopped() {
            setter.end_set_parameter(param);
        }

        if ui
            .add_sized([28.0, 22.0], egui::Button::new("+"))
            .on_hover_text("Increase value")
            .clicked()
        {
            let next_val = (current_val + step).clamp(min_val, max_val);
            setter.begin_set_parameter(param);
            setter.set_parameter(param, next_val);
            setter.end_set_parameter(param);
        }

        ui.label(
            egui::RichText::new(format!("{:>10}", formatted_val))
                .monospace()
                .color(egui::Color32::from_rgb(245, 185, 45)),
        );

        if ui
            .add_sized([52.0, 22.0], egui::Button::new("Reset"))
            .on_hover_text("Reset to default value")
            .clicked()
        {
            setter.begin_set_parameter(param);
            setter.set_parameter(param, default_val);
            setter.end_set_parameter(param);
        }
    });
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
    let is_clipped = telemetry.clip_latched.load(Ordering::Relaxed);

    // Keep Reaper's readable GR parameter synchronized while UI is open
    let target_gr_param = (-current_gr_db).clamp(-MAX_GR_METER_DB, 0.0);
    if (params.gr_meter_db.value() - target_gr_param).abs() > 0.05 {
        setter.set_parameter(&params.gr_meter_db, target_gr_param);
    }

    egui::TopBottomPanel::top("omore_punch_header").show(egui_ctx, |ui| {
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new(format!(
                    "OMORE PUNCH v{}  |  STEREO DRUM BUS COMPRESSOR & CLIPPER",
                    env!("CARGO_PKG_VERSION")
                ))
                .monospace()
                .strong()
                .size(15.0)
                .color(egui::Color32::from_rgb(245, 185, 35)),
            );

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
                    .add(egui::Button::new(
                        egui::RichText::new(clip_text)
                            .monospace()
                            .strong()
                            .color(egui::Color32::WHITE),
                    ).fill(clip_color))
                    .clicked()
                {
                    telemetry.clip_latched.store(false, Ordering::Relaxed);
                }
            });
        });
        ui.add_space(4.0);
    });

    egui::CentralPanel::default().show(egui_ctx, |ui| {
        ui.columns(2, |cols| {
            // ================================================================
            // LEFT COLUMN: COMPRESSOR, CLIPPER & PARALLEL MIX CONTROLS
            // ================================================================
            cols[0].group(|ui| {
                ui.label(
                    egui::RichText::new("1. COMPRESSOR & ENVELOPE BALLISTICS")
                        .monospace()
                        .strong()
                        .color(egui::Color32::from_rgb(245, 185, 35)),
                );
                ui.separator();

                draw_param_row(
                    ui,
                    setter,
                    &params.input_db,
                    "Input Drive",
                    0.5,
                    -12.0,
                    12.0,
                );
                draw_param_row(
                    ui,
                    setter,
                    &params.threshold_db,
                    "Threshold",
                    0.5,
                    -40.0,
                    0.0,
                );
                draw_param_row(ui, setter, &params.ratio, "Ratio", 0.25, 1.0, 20.0);
                draw_param_row(ui, setter, &params.attack_ms, "Attack", 0.5, 0.1, 100.0);
                draw_param_row(
                    ui,
                    setter,
                    &params.release_ms,
                    "Release",
                    5.0,
                    10.0,
                    500.0,
                );
                draw_param_row(
                    ui,
                    setter,
                    &params.sc_hpf_hz,
                    "SC HPF",
                    5.0,
                    20.0,
                    250.0,
                );

                ui.add_space(4.0);
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
                            egui::RichText::new("[Kick Safe 90Hz]")
                                .monospace()
                                .strong(),
                        )
                        .on_hover_text(
                            "Set Sidechain HPF to 90.0 Hz so Kick sub-bass (40-80 Hz) does not pump the Drum Bus",
                        )
                        .clicked()
                    {
                        setter.begin_set_parameter(&params.sc_hpf_hz);
                        setter.set_parameter(&params.sc_hpf_hz, 90.0);
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

                ui.add_space(4.0);
                draw_param_row(
                    ui,
                    setter,
                    &params.makeup_db,
                    "Makeup Gain",
                    0.5,
                    -12.0,
                    24.0,
                );
            });

            cols[0].add_space(8.0);

            cols[0].group(|ui| {
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
                                egui::Button::new(egui::RichText::new(text).monospace().strong())
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

                ui.add_space(4.0);
                draw_param_row(
                    ui,
                    setter,
                    &params.ceiling_db,
                    "Clip Ceiling",
                    0.25,
                    -6.0,
                    0.0,
                );
                draw_param_row(
                    ui,
                    setter,
                    &params.mix_pct,
                    "Mix Dry/Wet",
                    5.0,
                    0.0,
                    100.0,
                );
            });

            // ================================================================
            // RIGHT COLUMN: GAIN REDUCTION & STEREO IN/OUT METERS
            // ================================================================
            cols[1].group(|ui| {
                ui.label(
                    egui::RichText::new("3. GAIN REDUCTION & STEREO BUS METERING")
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
                                egui::RichText::new(format!("GR: -{:04.1} dB", current_gr_db))
                                    .monospace()
                                    .strong()
                                    .size(21.0)
                                    .color(egui::Color32::from_rgb(250, 195, 40)),
                            );
                            ui.label(
                                egui::RichText::new(format!("Max GR: -{:04.1} dB", max_gr_db))
                                    .monospace()
                                    .strong()
                                    .size(16.0)
                                    .color(egui::Color32::from_rgb(255, 125, 65)),
                            );
                        });
                    });

                    ui.vertical(|ui| {
                        if ui
                            .add_sized(
                                [120.0, 32.0],
                                egui::Button::new(
                                    egui::RichText::new("[Reset Peak]").monospace().strong(),
                                ),
                            )
                            .on_hover_text("Reset Max Gain Reduction Hold and Clipper Latch")
                            .clicked()
                        {
                            telemetry.reset_peaks();
                        }
                    });
                });

                ui.add_space(8.0);

                // Vertical Meters Row: In L/R, Out L/R, and Reaper-Style Top-Down GR Bar
                let meter_height = 295.0;
                ui.horizontal(|ui| {
                    ui.add_space(12.0);
                    draw_vertical_level_meter(ui, "In L/R", in_l, in_r, meter_height);
                    ui.add_space(24.0);
                    draw_reaper_style_gr_meter(ui, current_gr_db, max_gr_db, meter_height);
                    ui.add_space(24.0);
                    draw_vertical_level_meter(ui, "Out L/R", out_l, out_r, meter_height);
                });
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
        }
    }
}