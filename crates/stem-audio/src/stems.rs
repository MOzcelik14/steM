//! AI-powered stem separation via Demucs CLI integration with progress, cancellation, and sync preservation.

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;

use crate::decode::{decode_file, DecodeError, DecodedAudio};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DemucsModel {
    Htdemucs,
    HtdemucsFt,
    MdxExtra,
    Htdemucs6s,
}

impl DemucsModel {
    pub fn as_cli_name(self) -> &'static str {
        match self {
            Self::Htdemucs => "htdemucs",
            Self::HtdemucsFt => "htdemucs_ft",
            Self::MdxExtra => "mdx_extra",
            Self::Htdemucs6s => "htdemucs_6s",
        }
    }

    pub fn display_name(self) -> &'static str {
        match self {
            Self::Htdemucs => "htdemucs (Default · 4 Stems)",
            Self::HtdemucsFt => "htdemucs_ft (Fine-Tuned · 4 Stems)",
            Self::MdxExtra => "mdx_extra (High Clarity · 4 Stems)",
            Self::Htdemucs6s => "htdemucs_6s (6 Stems)",
        }
    }

    pub fn expected_stems(self) -> &'static [StemKind] {
        match self {
            Self::Htdemucs6s => &[
                StemKind::Vocals,
                StemKind::Drums,
                StemKind::Bass,
                StemKind::Guitar,
                StemKind::Piano,
                StemKind::Other,
            ],
            _ => &[
                StemKind::Vocals,
                StemKind::Drums,
                StemKind::Bass,
                StemKind::Other,
            ],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StemKind {
    Vocals,
    Drums,
    Bass,
    Other,
    Guitar,
    Piano,
}

impl StemKind {
    pub fn filename_stem(self) -> &'static str {
        match self {
            Self::Vocals => "vocals",
            Self::Drums => "drums",
            Self::Bass => "bass",
            Self::Other => "other",
            Self::Guitar => "guitar",
            Self::Piano => "piano",
        }
    }

    pub fn track_title(self) -> &'static str {
        match self {
            Self::Vocals => "Vocals",
            Self::Drums => "Drums",
            Self::Bass => "Bass",
            Self::Other => "Other",
            Self::Guitar => "Guitar",
            Self::Piano => "Piano",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SeparationQuality {
    Fast,
    Balanced,
    HighQuality,
}

#[derive(Debug, Clone)]
pub struct StemSeparationRequest {
    pub input_path: PathBuf,
    pub output_dir: PathBuf,
    pub model: DemucsModel,
    pub quality: SeparationQuality,
    pub prefer_gpu: bool,
    pub target_sample_rate: u32,
}

#[derive(Debug, Error)]
pub enum StemSeparationError {
    #[error("Demucs executable not found on PATH (install via `pip install demucs`)")]
    ToolNotFound,
    #[error("Stem separation cancelled by user")]
    Cancelled,
    #[error("Demucs failed: {0}")]
    ProcessFailed(String),
    #[error("Failed to decode separated stem: {0}")]
    Decode(#[from] DecodeError),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

pub struct DemucsRunner;

impl DemucsRunner {
    /// Checks whether `demucs` or `python3 -m demucs` is available on the host.
    pub fn is_available() -> bool {
        Command::new("demucs")
            .arg("--help")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|s| s.success())
            || Command::new("python3")
                .args(["-m", "demucs", "--help"])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .is_ok_and(|s| s.success())
    }

    /// Builds CLI arguments for invoking Demucs.
    pub fn build_command_args(req: &StemSeparationRequest) -> Vec<String> {
        let mut args = vec![
            "-n".to_string(),
            req.model.as_cli_name().to_string(),
            "-o".to_string(),
            req.output_dir.to_string_lossy().to_string(),
        ];
        match req.quality {
            SeparationQuality::Fast => {
                args.push("--shifts".to_string());
                args.push("0".to_string());
            }
            SeparationQuality::Balanced => {
                args.push("--shifts".to_string());
                args.push("1".to_string());
            }
            SeparationQuality::HighQuality => {
                args.push("--shifts".to_string());
                args.push("2".to_string());
                args.push("--overlap".to_string());
                args.push("0.5".to_string());
            }
        }
        if !req.prefer_gpu {
            args.push("-d".to_string());
            args.push("cpu".to_string());
        }
        args.push(req.input_path.to_string_lossy().to_string());
        args
    }

    /// Resolves expected output stem WAV paths produced by Demucs.
    pub fn resolve_stem_paths(
        output_dir: &Path,
        model: DemucsModel,
        input_path: &Path,
    ) -> Vec<(StemKind, PathBuf)> {
        let input_stem = input_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("audio");
        let base_dir = output_dir.join(model.as_cli_name()).join(input_stem);
        model
            .expected_stems()
            .iter()
            .map(|&kind| (kind, base_dir.join(format!("{}.wav", kind.filename_stem()))))
            .collect()
    }

    /// Executes Demucs in a subprocess, streaming progress and supporting cancellation.
    pub fn run_separation<F>(
        req: &StemSeparationRequest,
        mut on_progress: F,
        cancel_token: Option<Arc<AtomicBool>>,
    ) -> Result<Vec<(StemKind, DecodedAudio)>, StemSeparationError>
    where
        F: FnMut(f32),
    {
        if !req.input_path.exists() {
            return Err(StemSeparationError::ProcessFailed(format!(
                "Input file does not exist: {}",
                req.input_path.display()
            )));
        }
        std::fs::create_dir_all(&req.output_dir)?;

        let cli_args = Self::build_command_args(req);
        let mut child = Command::new("demucs")
            .args(&cli_args)
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .or_else(|_| {
                Command::new("python3")
                    .arg("-m")
                    .arg("demucs")
                    .args(&cli_args)
                    .stdout(Stdio::null())
                    .stderr(Stdio::piped())
                    .spawn()
            })
            .map_err(|_| StemSeparationError::ToolNotFound)?;

        if let Some(stderr) = child.stderr.take() {
            let reader = BufReader::new(stderr);
            for line in reader.lines().map_while(Result::ok) {
                if cancel_token
                    .as_ref()
                    .is_some_and(|t| t.load(Ordering::Relaxed))
                {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(StemSeparationError::Cancelled);
                }
                if let Some(pct) = Self::parse_progress_percent(&line) {
                    on_progress((pct / 100.0).clamp(0.0, 0.95));
                }
            }
        }

        loop {
            if cancel_token
                .as_ref()
                .is_some_and(|t| t.load(Ordering::Relaxed))
            {
                let _ = child.kill();
                let _ = child.wait();
                return Err(StemSeparationError::Cancelled);
            }
            match child.try_wait()? {
                Some(status) => {
                    if !status.success() {
                        return Err(StemSeparationError::ProcessFailed(format!(
                            "Demucs exited with status {status}"
                        )));
                    }
                    break;
                }
                None => std::thread::sleep(Duration::from_millis(80)),
            }
        }

        let stem_paths = Self::resolve_stem_paths(&req.output_dir, req.model, &req.input_path);
        let mut results = Vec::new();
        for (kind, path) in stem_paths {
            if !path.is_file() {
                continue;
            }
            let mut decoded = decode_file(&path)?;
            if req.target_sample_rate > 0 && decoded.source.sample_rate != req.target_sample_rate {
                if let Ok(resampled) = stem_dsp::resample_audio(
                    &decoded.channels,
                    decoded.source.sample_rate,
                    req.target_sample_rate,
                ) {
                    decoded.source.sample_rate = req.target_sample_rate;
                    decoded.source.duration_frames =
                        resampled.first().map(|c| c.len() as u64).unwrap_or(0);
                    decoded.channels = resampled;
                }
            }
            results.push((kind, decoded));
        }

        if results.is_empty() {
            return Err(StemSeparationError::ProcessFailed(
                "Demucs completed but no stem WAV files were found".to_string(),
            ));
        }

        on_progress(1.0);
        Ok(results)
    }

    /// Runs stem separation across multiple files sequentially, reporting `(file_idx, total, progress)`
    /// and collecting per-file results so one failure does not abort the entire batch unless cancelled.
    pub fn run_batch_separation<F>(
        requests: &[StemSeparationRequest],
        mut on_progress: F,
        cancel_token: Option<Arc<AtomicBool>>,
    ) -> Vec<Result<Vec<(StemKind, DecodedAudio)>, StemSeparationError>>
    where
        F: FnMut(usize, usize, f32),
    {
        let total = requests.len();
        let mut results = Vec::with_capacity(total);
        for (idx, req) in requests.iter().enumerate() {
            if cancel_token
                .as_ref()
                .is_some_and(|t| t.load(Ordering::Relaxed))
            {
                results.push(Err(StemSeparationError::Cancelled));
                continue;
            }
            let res =
                Self::run_separation(req, |p| on_progress(idx, total, p), cancel_token.clone());
            results.push(res);
        }
        results
    }

    fn parse_progress_percent(line: &str) -> Option<f32> {
        let pct_pos = line.find('%')?;
        let prefix = line[..pct_pos].trim_end();
        let digits: String = prefix
            .chars()
            .rev()
            .take_while(|c| c.is_ascii_digit() || *c == '.')
            .collect::<String>()
            .chars()
            .rev()
            .collect();
        digits.parse::<f32>().ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_demucs_args_and_stem_path_resolution() {
        let req = StemSeparationRequest {
            input_path: PathBuf::from("/tmp/MyTrack.wav"),
            output_dir: PathBuf::from("/tmp/demucs_out"),
            model: DemucsModel::HtdemucsFt,
            quality: SeparationQuality::HighQuality,
            prefer_gpu: false,
            target_sample_rate: 48000,
        };
        let args = DemucsRunner::build_command_args(&req);
        assert!(args.contains(&"htdemucs_ft".to_string()));
        assert!(args.contains(&"cpu".to_string()));
        assert!(args.contains(&"0.5".to_string()));

        let paths = DemucsRunner::resolve_stem_paths(&req.output_dir, req.model, &req.input_path);
        assert_eq!(paths.len(), 4);
        assert_eq!(
            paths[0].1,
            PathBuf::from("/tmp/demucs_out/htdemucs_ft/MyTrack/vocals.wav")
        );
        assert_eq!(
            DemucsRunner::parse_progress_percent(" 42%|████|"),
            Some(42.0)
        );
    }

    #[test]
    fn test_batch_separation_handles_missing_and_cancelled() {
        let cancel = Arc::new(AtomicBool::new(true));
        let reqs = vec![StemSeparationRequest {
            input_path: PathBuf::from("/tmp/nonexistent_stem_batch_1.wav"),
            output_dir: PathBuf::from("/tmp/demucs_out"),
            model: DemucsModel::Htdemucs,
            quality: SeparationQuality::Fast,
            prefer_gpu: false,
            target_sample_rate: 48000,
        }];
        let res = DemucsRunner::run_batch_separation(&reqs, |_, _, _| {}, Some(cancel));
        assert_eq!(res.len(), 1);
        assert!(matches!(res[0], Err(StemSeparationError::Cancelled)));
    }
}
