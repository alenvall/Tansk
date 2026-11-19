//! A 16-bit PCM WAV writer: a few lines, so no crate.

use std::io::{self, Write};

/// Encodes `channels` (planar, equal length) as a 16-bit PCM WAV file.
#[must_use]
pub fn encode(channels: &[Vec<f32>], sample_rate: u32) -> Vec<u8> {
    let ch = channels.len().max(1) as u16;
    let frames = channels.first().map_or(0, Vec::len);
    let data_len = frames as u32 * u32::from(ch) * 2;
    let mut out = Vec::with_capacity(44 + data_len as usize);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_len).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes()); // PCM
    out.extend_from_slice(&ch.to_le_bytes());
    out.extend_from_slice(&sample_rate.to_le_bytes());
    out.extend_from_slice(&(sample_rate * u32::from(ch) * 2).to_le_bytes());
    out.extend_from_slice(&(ch * 2).to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    for i in 0..frames {
        for c in channels {
            let s = c.get(i).copied().unwrap_or(0.0);
            let s = if s.is_finite() { s.clamp(-1.0, 1.0) } else { 0.0 };
            out.extend_from_slice(&((s * 32767.0).round() as i16).to_le_bytes());
        }
    }
    out
}

/// Writes `channels` to `path` as a WAV file.
///
/// # Errors
/// Any I/O error creating or writing the file.
pub fn write(path: &std::path::Path, channels: &[Vec<f32>], sample_rate: u32) -> io::Result<()> {
    let mut f = std::fs::File::create(path)?;
    f.write_all(&encode(channels, sample_rate))
}
