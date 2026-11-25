//! AI-powered stem separation via Demucs CLI integration with progress, cancellation, and sync preservation.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use thiserror::Error;

use crate::decode::{decode_file, DecodeError, DecodedAudio};

/// RAII guard that removes a temporary directory when dropped.
pub struct TempDirGuard(pub PathBuf);

impl Drop for TempDirGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// RAII guard that removes a temporary file when dropped.
pub struct TempFileGuard(pub PathBuf);

impl Drop for TempFileGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

/// Removes orphaned `stem_demucs_out_*`, `stem_demucs_in_*`, and `stem_rec_*` temp artifacts
/// older than `max_age`.
pub fn cleanup_stale_temp_artifacts(max_age: Duration) {
    let temp_dir = std::env::temp_dir();
    let Ok(entries) = std::fs::read_dir(&temp_dir) else {
        return;
    };
    let now = SystemTime::now();
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        let is_stem_artifact = name_str.starts_with("stem_demucs_out_")
            || name_str.starts_with("stem_demucs_in_")
            || name_str.starts_with("stem_rec_");
        if !is_stem_artifact {
            continue;
        }
        if let Ok(meta) = entry.metadata() {
            let modified = meta.modified().unwrap_or(now);
            if now.duration_since(modified).unwrap_or_default() > max_age {
                let path = entry.path();
                if path.is_dir() {
                    let _ = std::fs::remove_dir_all(path);
                } else {
                    let _ = std::fs::remove_file(path);
                }
            }
        }
    }
}

/// RAII guard that terminates a child process and its descendant tree on Drop if not disarmed.
struct ProcessTreeGuard {
    child: Option<Child>,
}

impl ProcessTreeGuard {
    fn new(child: Child) -> Self {
        Self { child: Some(child) }
    }

    fn child_mut(&mut self) -> Option<&mut Child> {
        self.child.as_mut()
    }

    fn disarm(&mut self) -> Option<Child> {
        self.child.take()
    }

    fn kill_and_wait(&mut self) {
        if let Some(mut child) = self.child.take() {
            let pid = child.id();
            #[cfg(unix)]
            {
                let _ = Command::new("kill")
                    .args(["-s", "KILL", "--", &format!("-{pid}")])
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .status();
            }
            #[cfg(windows)]
            {
                let _ = Command::new("taskkill")
                    .args(["/PID", &pid.to_string(), "/T", "/F"])
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .status();
            }
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

impl Drop for ProcessTreeGuard {
    fn drop(&mut self) {
        self.kill_and_wait();
    }
}

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
        args.push("--".to_string());
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
        on_progress: F,
        cancel_token: Option<Arc<AtomicBool>>,
    ) -> Result<Vec<(StemKind, DecodedAudio)>, StemSeparationError>
    where
        F: FnMut(f32),
    {
        Self::run_separation_with_command(req, None, on_progress, cancel_token)
    }

    /// Executes stem separation using an explicit executable path (or default `demucs` / `python3 -m demucs` when `None`).
    pub fn run_separation_with_command<F>(
        req: &StemSeparationRequest,
        program_override: Option<&Path>,
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
        let spawn_cmd = |program: &Path, extra_prefix: &[&str]| -> std::io::Result<Child> {
            let mut cmd = Command::new(program);
            cmd.args(extra_prefix)
                .args(&cli_args)
                .stdout(Stdio::null())
                .stderr(Stdio::piped());
            #[cfg(unix)]
            {
                use std::os::unix::process::CommandExt;
                cmd.process_group(0);
            }
            cmd.spawn()
        };

        let child = if let Some(prog) = program_override {
            spawn_cmd(prog, &[]).map_err(|_| StemSeparationError::ToolNotFound)?
        } else {
            spawn_cmd(Path::new("demucs"), &[])
                .or_else(|_| spawn_cmd(Path::new("python3"), &["-m", "demucs"]))
                .map_err(|_| StemSeparationError::ToolNotFound)?
        };

        let mut guard = ProcessTreeGuard::new(child);

        let (stderr_tx, stderr_rx) = crossbeam_channel::bounded::<String>(128);
        let reader_handle = guard
            .child_mut()
            .and_then(|c| c.stderr.take())
            .map(|mut stderr| {
                std::thread::spawn(move || {
                    let mut buf = [0u8; 512];
                    let mut line_buf = Vec::with_capacity(256);
                    loop {
                        match stderr.read(&mut buf) {
                            Ok(0) | Err(_) => {
                                if !line_buf.is_empty() {
                                    let s = String::from_utf8_lossy(&line_buf).trim().to_string();
                                    if !s.is_empty() {
                                        let _ = stderr_tx.send(s);
                                    }
                                }
                                break;
                            }
                            Ok(n) => {
                                for &b in &buf[..n] {
                                    if b == b'\n' || b == b'\r' {
                                        if !line_buf.is_empty() {
                                            let s = String::from_utf8_lossy(&line_buf)
                                                .trim()
                                                .to_string();
                                            if !s.is_empty() {
                                                let _ = stderr_tx.send(s);
                                            }
                                            line_buf.clear();
                                        }
                                    } else if line_buf.len() < 4096 {
                                        line_buf.push(b);
                                    }
                                }
                            }
                        }
                    }
                })
            });

        loop {
            if cancel_token
                .as_ref()
                .is_some_and(|t| t.load(Ordering::Relaxed))
            {
                guard.kill_and_wait();
                if let Some(h) = reader_handle {
                    let _ = h.join();
                }
                return Err(StemSeparationError::Cancelled);
            }

            match stderr_rx.recv_timeout(Duration::from_millis(100)) {
                Ok(line) => {
                    if let Some(pct) = Self::parse_progress_percent(&line) {
                        on_progress((pct / 100.0).clamp(0.0, 0.95));
                    }
                }
                Err(crossbeam_channel::RecvTimeoutError::Timeout) => {}
                Err(crossbeam_channel::RecvTimeoutError::Disconnected) => {}
            }

            let exited = match guard.child_mut() {
                Some(c) => c.try_wait()?,
                None => break,
            };
            if let Some(status) = exited {
                let _ = guard.disarm();
                while let Ok(line) = stderr_rx.try_recv() {
                    if let Some(pct) = Self::parse_progress_percent(&line) {
                        on_progress((pct / 100.0).clamp(0.0, 0.95));
                    }
                }
                if let Some(h) = reader_handle {
                    let _ = h.join();
                }
                if !status.success() {
                    return Err(StemSeparationError::ProcessFailed(format!(
                        "Demucs exited with status {status}"
                    )));
                }
                break;
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
        let sep_pos = args
            .iter()
            .position(|a| a == "--")
            .expect("must contain --");
        assert_eq!(
            args.get(sep_pos + 1).map(String::as_str),
            Some("/tmp/MyTrack.wav")
        );

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
        assert_eq!(
            DemucsRunner::parse_progress_percent("\r 12%|███        | 1/8"),
            Some(12.0)
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

    #[cfg(unix)]
    fn write_mock_script(dir: &Path, name: &str, body: &str) -> PathBuf {
        use std::os::unix::fs::PermissionsExt;
        let path = dir.join(name);
        std::fs::write(&path, format!("#!/usr/bin/env sh\n{body}\n")).unwrap();
        let mut perms = std::fs::metadata(&path).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&path, perms).unwrap();
        path
    }

    #[cfg(unix)]
    #[test]
    fn test_mock_demucs_silent_sleep_cancels_quickly() {
        let tmp = std::env::temp_dir().join(format!("stem_mock_silent_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&tmp).unwrap();
        let _dir_guard = TempDirGuard(tmp.clone());

        let input = tmp.join("in.wav");
        std::fs::write(&input, b"dummy").unwrap();
        let out_dir = tmp.join("out");
        let script = write_mock_script(&tmp, "mock_demucs.sh", "sleep 30");

        let req = StemSeparationRequest {
            input_path: input,
            output_dir: out_dir,
            model: DemucsModel::Htdemucs,
            quality: SeparationQuality::Fast,
            prefer_gpu: false,
            target_sample_rate: 48000,
        };
        let cancel = Arc::new(AtomicBool::new(false));
        let cancel_trigger = cancel.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(150));
            cancel_trigger.store(true, Ordering::Relaxed);
        });

        let start = std::time::Instant::now();
        let res =
            DemucsRunner::run_separation_with_command(&req, Some(&script), |_| {}, Some(cancel));
        let elapsed = start.elapsed();
        assert!(matches!(res, Err(StemSeparationError::Cancelled)));
        assert!(
            elapsed < Duration::from_secs(2),
            "cancellation took too long: {elapsed:?}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_mock_demucs_carriage_return_progress_and_tree_cleanup() {
        let tmp = std::env::temp_dir().join(format!("stem_mock_cr_{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&tmp).unwrap();
        let _dir_guard = TempDirGuard(tmp.clone());

        let input = tmp.join("in.wav");
        std::fs::write(&input, b"dummy").unwrap();
        let out_dir = tmp.join("stem_demucs_out_test");
        let pid_file = tmp.join("grandchild.pid");
        let script_body = format!(
            "sleep 30 &\necho $! > \"{}\"\nprintf '\\r 12%%|███|\\r 55%%|██████|\\r' >&2\nsleep 30",
            pid_file.display()
        );
        let script = write_mock_script(&tmp, "mock_demucs_cr.sh", &script_body);

        let req = StemSeparationRequest {
            input_path: input,
            output_dir: out_dir.clone(),
            model: DemucsModel::Htdemucs,
            quality: SeparationQuality::Fast,
            prefer_gpu: false,
            target_sample_rate: 48000,
        };

        let cancel = Arc::new(AtomicBool::new(false));
        let cancel_trigger = cancel.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(250));
            cancel_trigger.store(true, Ordering::Relaxed);
        });

        let mut reported = Vec::new();
        let res = {
            let _out_guard = TempDirGuard(out_dir.clone());
            DemucsRunner::run_separation_with_command(
                &req,
                Some(&script),
                |p| reported.push(p),
                Some(cancel),
            )
        };
        assert!(matches!(res, Err(StemSeparationError::Cancelled)));
        assert!(!out_dir.exists(), "TempDirGuard must remove output dir");
        assert!(
            reported.iter().any(|&p| (p - 0.12).abs() < 0.01)
                && reported.iter().any(|&p| (p - 0.55).abs() < 0.01),
            "expected CR progress 12% and 55%, got {reported:?}"
        );

        // Verify grandchild process was terminated by process group kill
        if let Ok(pid_str) = std::fs::read_to_string(&pid_file) {
            let pid_str = pid_str.trim();
            if !pid_str.is_empty() {
                std::thread::sleep(Duration::from_millis(50));
                let alive = Command::new("kill")
                    .args(["-0", pid_str])
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .status()
                    .is_ok_and(|s| s.success());
                assert!(!alive, "grandchild PID {pid_str} must be killed");
            }
        }
    }
}
