//! Alert chimes.
//!
//! The sounds are synthesised once at startup (nothing to ship, nothing to decode) and played
//! through the Windows multimedia API, so audio works even while the popup webview is hidden —
//! WebView2's autoplay policy would otherwise block sound from a window nobody has clicked.

use std::f32::consts::TAU;
use std::sync::OnceLock;

use crate::db::Priority;

const SAMPLE_RATE: u32 = 44_100;

struct Chimes {
    low: Vec<u8>,
    normal: Vec<u8>,
    high: Vec<u8>,
}

static CHIMES: OnceLock<Chimes> = OnceLock::new();

// Note frequencies (Hz).
const A5: f32 = 880.0;
const CS6: f32 = 1108.73;
const E6: f32 = 1318.51;
const A6: f32 = 1760.0;

fn chimes() -> &'static Chimes {
    CHIMES.get_or_init(|| Chimes {
        // One soft note.
        low: synth(&[(E6, 0.0)], 1.1, 0.20),
        // Two-note "ding-dong" rising a fifth.
        normal: synth(&[(A5, 0.0), (E6, 0.17)], 1.5, 0.22),
        // A rising arpeggio, played twice.
        high: synth(
            &[
                (A5, 0.0), (CS6, 0.13), (E6, 0.26), (A6, 0.39),
                (A5, 0.95), (CS6, 1.08), (E6, 1.21), (A6, 1.34),
            ],
            2.5,
            0.20,
        ),
    })
}

/// Build the chimes ahead of the first alert so it never waits on synthesis.
pub fn warm_up() {
    std::thread::spawn(|| {
        chimes();
    });
}

pub fn play(priority: Priority) {
    let c = chimes();
    let wav = match priority {
        Priority::Low => &c.low,
        Priority::Normal => &c.normal,
        Priority::High => &c.high,
    };
    play_wav(wav);
}

#[cfg(windows)]
fn play_wav(wav: &'static [u8]) {
    use windows::core::PCWSTR;
    use windows::Win32::Media::Audio::{PlaySoundW, SND_ASYNC, SND_MEMORY, SND_NODEFAULT};

    // SND_MEMORY | SND_ASYNC keeps reading the buffer after this call returns; the chimes live
    // in a static, so the pointer stays valid for the whole process.
    unsafe {
        let _ = PlaySoundW(PCWSTR(wav.as_ptr().cast()), None, SND_MEMORY | SND_ASYNC | SND_NODEFAULT);
    }
}

#[cfg(not(windows))]
fn play_wav(_wav: &'static [u8]) {}

/// Additive "chime": a fundamental plus two quieter, faster-decaying overtones per note.
fn synth(notes: &[(f32, f32)], seconds: f32, gain: f32) -> Vec<u8> {
    let sr = SAMPLE_RATE as f32;
    let len = (seconds * sr) as usize;
    let mut mix = vec![0.0f32; len];

    for &(freq, start) in notes {
        let first = (start * sr) as usize;
        for (i, out) in mix.iter_mut().enumerate().skip(first) {
            let t = (i - first) as f32 / sr;
            let attack = (t / 0.004).min(1.0);
            let body = (TAU * freq * t).sin() * (-t * 4.5).exp()
                + 0.40 * (TAU * freq * 2.0 * t).sin() * (-t * 7.0).exp()
                + 0.12 * (TAU * freq * 3.0 * t).sin() * (-t * 11.0).exp();
            *out += attack * body * gain;
        }
    }

    // Short fade-out so the buffer never ends on a click.
    let fade = (0.02 * sr) as usize;
    for i in 0..fade.min(len) {
        mix[len - 1 - i] *= i as f32 / fade as f32;
    }

    wav_bytes(&mix)
}

/// 16-bit mono PCM WAV.
fn wav_bytes(samples: &[f32]) -> Vec<u8> {
    let data_len = (samples.len() * 2) as u32;
    let mut b = Vec::with_capacity(44 + data_len as usize);
    b.extend_from_slice(b"RIFF");
    b.extend_from_slice(&(36 + data_len).to_le_bytes());
    b.extend_from_slice(b"WAVEfmt ");
    b.extend_from_slice(&16u32.to_le_bytes()); // fmt chunk size
    b.extend_from_slice(&1u16.to_le_bytes()); // PCM
    b.extend_from_slice(&1u16.to_le_bytes()); // mono
    b.extend_from_slice(&SAMPLE_RATE.to_le_bytes());
    b.extend_from_slice(&(SAMPLE_RATE * 2).to_le_bytes()); // byte rate
    b.extend_from_slice(&2u16.to_le_bytes()); // block align
    b.extend_from_slice(&16u16.to_le_bytes()); // bits per sample
    b.extend_from_slice(b"data");
    b.extend_from_slice(&data_len.to_le_bytes());
    for s in samples {
        b.extend_from_slice(&((s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16).to_le_bytes());
    }
    b
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_chime_is_a_well_formed_wav() {
        let c = chimes();
        for wav in [&c.low, &c.normal, &c.high] {
            assert_eq!(&wav[0..4], b"RIFF");
            assert_eq!(&wav[8..12], b"WAVE");
            let data_len = u32::from_le_bytes(wav[40..44].try_into().unwrap()) as usize;
            assert_eq!(wav.len(), 44 + data_len);
            assert!(data_len > 0);
        }
    }

    #[test]
    fn chimes_never_clip() {
        let c = chimes();
        for wav in [&c.low, &c.normal, &c.high] {
            let peak = wav[44..]
                .chunks_exact(2)
                .map(|p| i16::from_le_bytes([p[0], p[1]]).unsigned_abs())
                .max()
                .unwrap();
            assert!(peak < i16::MAX as u16, "chime is clipping");
            assert!(peak > 2_000, "chime is nearly silent");
        }
    }
}
