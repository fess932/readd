//! Audio file inspection.

use symphonia::core::{
    formats::{FormatOptions, TrackType, probe::Hint},
    io::MediaSourceStream,
    meta::MetadataOptions,
};

/// Duration in seconds, read from the container header without decoding.
pub fn audio_duration(path: &std::path::Path) -> Option<f64> {
    let file = std::fs::File::open(path).ok()?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());

    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }

    let format = symphonia::default::get_probe()
        .probe(
            &hint,
            mss,
            FormatOptions::default(),
            MetadataOptions::default(),
        )
        .ok()?;

    let track = format.default_track(TrackType::Audio)?;
    let duration = track.time_base?.calc_duration(track.duration?)?;
    Some(duration.as_secs_f64())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Two seconds of 8 kHz mono 8-bit silence.
    fn wav() -> Vec<u8> {
        let (rate, seconds) = (8000u32, 2u32);
        let data_len = rate * seconds;
        let mut bytes = Vec::new();
        bytes.extend(b"RIFF");
        bytes.extend((36 + data_len).to_le_bytes());
        bytes.extend(b"WAVEfmt ");
        bytes.extend(16u32.to_le_bytes()); // fmt chunk size
        bytes.extend(1u16.to_le_bytes()); // PCM
        bytes.extend(1u16.to_le_bytes()); // channels
        bytes.extend(rate.to_le_bytes());
        bytes.extend(rate.to_le_bytes()); // byte rate
        bytes.extend(1u16.to_le_bytes()); // block align
        bytes.extend(8u16.to_le_bytes()); // bits per sample
        bytes.extend(b"data");
        bytes.extend(data_len.to_le_bytes());
        bytes.extend(std::iter::repeat_n(0x80u8, data_len as usize));
        bytes
    }

    #[test]
    fn reads_the_duration_of_a_wav() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.wav");
        std::fs::write(&path, wav()).unwrap();

        let duration = audio_duration(&path).unwrap();
        assert!((duration - 2.0).abs() < 0.01, "got {duration}");
    }

    #[test]
    fn unknown_or_missing_files_have_no_duration() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("a.mp3");
        std::fs::write(&path, b"not audio").unwrap();

        assert_eq!(audio_duration(&path), None);
        assert_eq!(audio_duration(&dir.path().join("missing.mp3")), None);
    }
}
