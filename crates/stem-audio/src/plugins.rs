//! External audio plugin discovery, metadata parsing, lifecycle, and hosting (VST3 / LV2).

use std::path::{Path, PathBuf};
use stem_core::{PluginFormat, PluginParamValue, PluginSlotConfig};
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginDescriptor {
    pub id: String,
    pub name: String,
    pub vendor: String,
    pub format: PluginFormat,
    pub bundle_path: PathBuf,
    pub binary_path: Option<PathBuf>,
    pub category: String,
}

impl PluginDescriptor {
    /// Converts this discovered plugin descriptor into an insertable `PluginSlotConfig`.
    pub fn to_slot_config(&self) -> PluginSlotConfig {
        PluginSlotConfig {
            id: Uuid::new_v4(),
            descriptor_id: self.id.clone(),
            name: self.name.clone(),
            vendor: self.vendor.clone(),
            format: self.format,
            bundle_path: self.bundle_path.clone(),
            bypassed: false,
            parameters: vec![PluginParamValue {
                id: "mix".to_string(),
                name: "Mix".to_string(),
                value: 1.0,
                default_value: 1.0,
            }],
            state_blob: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PluginStatus {
    Available,
    Loaded,
    Missing,
    Incompatible(String),
}

#[derive(Debug, Error)]
pub enum PluginLoadError {
    #[error("Plugin bundle not found: {0}")]
    NotFound(PathBuf),
    #[error("Incompatible plugin binary for current platform: {0}")]
    IncompatibleBinary(String),
    #[error("I/O error loading plugin: {0}")]
    Io(#[from] std::io::Error),
}

/// Discovers installed VST3 and LV2 plugin bundles on the host system.
pub struct PluginScanner;

impl PluginScanner {
    /// Returns standard OS installation paths for the given plugin format.
    pub fn default_scan_paths(format: PluginFormat) -> Vec<PathBuf> {
        let mut paths = Vec::new();
        match format {
            PluginFormat::Vst3 => {
                #[cfg(target_os = "linux")]
                {
                    if let Some(home) = std::env::var_os("HOME") {
                        paths.push(PathBuf::from(home).join(".vst3"));
                    }
                    paths.push(PathBuf::from("/usr/lib/vst3"));
                    paths.push(PathBuf::from("/usr/local/lib/vst3"));
                }
                #[cfg(target_os = "windows")]
                {
                    if let Some(common) = std::env::var_os("COMMONPROGRAMFILES") {
                        paths.push(PathBuf::from(common).join("VST3"));
                    }
                    paths.push(PathBuf::from(r"C:\Program Files\Common Files\VST3"));
                }
                #[cfg(target_os = "macos")]
                {
                    if let Some(home) = std::env::var_os("HOME") {
                        paths.push(PathBuf::from(home).join("Library/Audio/Plug-Ins/VST3"));
                    }
                    paths.push(PathBuf::from("/Library/Audio/Plug-Ins/VST3"));
                }
            }
            PluginFormat::Lv2 => {
                #[cfg(target_os = "linux")]
                {
                    if let Some(home) = std::env::var_os("HOME") {
                        paths.push(PathBuf::from(home).join(".lv2"));
                    }
                    paths.push(PathBuf::from("/usr/lib/lv2"));
                    paths.push(PathBuf::from("/usr/local/lib/lv2"));
                }
                #[cfg(target_os = "windows")]
                {
                    if let Some(common) = std::env::var_os("COMMONPROGRAMFILES") {
                        paths.push(PathBuf::from(common).join("LV2"));
                    }
                }
                #[cfg(target_os = "macos")]
                {
                    if let Some(home) = std::env::var_os("HOME") {
                        paths.push(PathBuf::from(home).join("Library/Audio/Plug-Ins/LV2"));
                    }
                    paths.push(PathBuf::from("/Library/Audio/Plug-Ins/LV2"));
                }
            }
        }
        paths
    }

    /// Scans all default OS VST3 and LV2 directories.
    pub fn scan_default_paths() -> Vec<PluginDescriptor> {
        let mut all_paths = Self::default_scan_paths(PluginFormat::Vst3);
        all_paths.extend(Self::default_scan_paths(PluginFormat::Lv2));
        Self::scan_paths(&all_paths)
    }

    /// Scans a list of directories for `.vst3` and `.lv2` bundles.
    pub fn scan_paths(paths: &[PathBuf]) -> Vec<PluginDescriptor> {
        let mut found = Vec::new();
        for dir in paths {
            if !dir.is_dir() {
                continue;
            }
            Self::scan_dir_recursive(dir, 0, &mut found);
        }
        found.sort_by_key(|a| a.name.to_lowercase());
        found.dedup_by(|a, b| a.bundle_path == b.bundle_path && a.id == b.id);
        found
    }

    fn scan_dir_recursive(dir: &Path, depth: usize, out: &mut Vec<PluginDescriptor>) {
        if depth > 3 {
            return;
        }
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let ext = path
                .extension()
                .and_then(|s| s.to_str())
                .map(|s| s.to_ascii_lowercase());
            match ext.as_deref() {
                Some("vst3") => {
                    out.extend(Self::inspect_vst3_bundle(&path));
                }
                Some("lv2") if path.is_dir() => {
                    out.extend(Self::inspect_lv2_bundle(&path));
                }
                _ if path.is_dir() => {
                    Self::scan_dir_recursive(&path, depth + 1, out);
                }
                _ => {}
            }
        }
    }

    fn inspect_vst3_bundle(bundle_path: &Path) -> Vec<PluginDescriptor> {
        let fallback_name = bundle_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("VST3 Plugin")
            .to_string();

        let binary_path = Self::find_vst3_binary(bundle_path);
        let moduleinfo = bundle_path.join("Contents/Resources/moduleinfo.json");
        if let Ok(json_str) = std::fs::read_to_string(&moduleinfo) {
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&json_str) {
                let vendor = val
                    .get("Factory Info")
                    .and_then(|f| f.get("Vendor"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("Unknown")
                    .to_string();
                if let Some(classes) = val.get("Classes").and_then(|c| c.as_array()) {
                    let mut list = Vec::new();
                    for cls in classes {
                        let cat = cls
                            .get("Category")
                            .and_then(|c| c.as_str())
                            .unwrap_or("Audio Module Class");
                        if !cat.contains("Audio") {
                            continue;
                        }
                        let name = cls
                            .get("Name")
                            .and_then(|n| n.as_str())
                            .unwrap_or(&fallback_name)
                            .to_string();
                        let cid = cls
                            .get("CID")
                            .and_then(|c| c.as_str())
                            .unwrap_or(&name)
                            .to_string();
                        list.push(PluginDescriptor {
                            id: cid,
                            name,
                            vendor: vendor.clone(),
                            format: PluginFormat::Vst3,
                            bundle_path: bundle_path.to_path_buf(),
                            binary_path: binary_path.clone(),
                            category: cat.to_string(),
                        });
                    }
                    if !list.is_empty() {
                        return list;
                    }
                }
            }
        }

        vec![PluginDescriptor {
            id: format!("vst3:{fallback_name}"),
            name: fallback_name,
            vendor: "VST3".to_string(),
            format: PluginFormat::Vst3,
            bundle_path: bundle_path.to_path_buf(),
            binary_path,
            category: "Audio Effect".to_string(),
        }]
    }

    fn find_vst3_binary(bundle_path: &Path) -> Option<PathBuf> {
        if bundle_path.is_file() {
            return Some(bundle_path.to_path_buf());
        }
        #[cfg(target_os = "linux")]
        let arch_dir = bundle_path.join("Contents/x86_64-linux");
        #[cfg(target_os = "windows")]
        let arch_dir = bundle_path.join("Contents/x86_64-win");
        #[cfg(target_os = "macos")]
        let arch_dir = bundle_path.join("Contents/MacOS");
        #[cfg(not(any(target_os = "linux", target_os = "windows", target_os = "macos")))]
        let arch_dir = bundle_path.join("Contents");

        if let Ok(entries) = std::fs::read_dir(arch_dir) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.is_file() {
                    return Some(p);
                }
            }
        }
        None
    }

    fn inspect_lv2_bundle(bundle_path: &Path) -> Vec<PluginDescriptor> {
        let fallback_name = bundle_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("LV2 Plugin")
            .to_string();

        let manifest = bundle_path.join("manifest.ttl");
        let mut name = fallback_name.clone();
        let mut binary_path = None;

        if let Ok(ttl) = std::fs::read_to_string(&manifest) {
            if let Some(parsed_name) = Self::extract_ttl_literal(&ttl, "doap:name")
                .or_else(|| Self::extract_ttl_literal(&ttl, "rdfs:label"))
            {
                name = parsed_name;
            }
            if let Some(bin_rel) = Self::extract_ttl_uri(&ttl, "lv2:binary") {
                let rel_path = Path::new(&bin_rel);
                let safe_rel = !rel_path.is_absolute()
                    && rel_path.components().all(|c| {
                        matches!(
                            c,
                            std::path::Component::Normal(_) | std::path::Component::CurDir
                        )
                    });
                if safe_rel {
                    let candidate = bundle_path.join(rel_path);
                    if let (Ok(canon_bundle), Ok(canon_cand)) =
                        (bundle_path.canonicalize(), candidate.canonicalize())
                    {
                        if canon_cand.starts_with(&canon_bundle) && canon_cand.is_file() {
                            binary_path = Some(canon_cand);
                        }
                    }
                }
            }
        }

        if binary_path.is_none() {
            if let Ok(entries) = std::fs::read_dir(bundle_path) {
                for entry in entries.flatten() {
                    let p = entry.path();
                    let ext = p.extension().and_then(|s| s.to_str()).unwrap_or("");
                    if matches!(ext, "so" | "dll" | "dylib") && p.is_file() {
                        binary_path = Some(p);
                        break;
                    }
                }
            }
        }

        vec![PluginDescriptor {
            id: format!("lv2:{fallback_name}"),
            name,
            vendor: "LV2".to_string(),
            format: PluginFormat::Lv2,
            bundle_path: bundle_path.to_path_buf(),
            binary_path,
            category: "LV2 Effect".to_string(),
        }]
    }

    fn extract_ttl_literal(ttl: &str, key: &str) -> Option<String> {
        let idx = ttl.find(key)?;
        let rest = &ttl[idx + key.len()..];
        let q1 = rest.find('"')?;
        let after_q1 = &rest[q1 + 1..];
        let q2 = after_q1.find('"')?;
        Some(after_q1[..q2].trim().to_string())
    }

    fn extract_ttl_uri(ttl: &str, key: &str) -> Option<String> {
        let idx = ttl.find(key)?;
        let rest = &ttl[idx + key.len()..];
        let a1 = rest.find('<')?;
        let after_a1 = &rest[a1 + 1..];
        let a2 = after_a1.find('>')?;
        Some(after_a1[..a2].trim().to_string())
    }
}

/// Loaded runtime instance for an external plugin slot.
#[derive(Debug, Clone)]
pub struct PluginHostInstance {
    pub slot_id: Uuid,
    pub descriptor_id: String,
    pub name: String,
    pub format: PluginFormat,
    pub bypassed: bool,
    pub parameters: Vec<PluginParamValue>,
    pub state_blob: Option<Vec<u8>>,
}

impl PluginHostInstance {
    /// Validates whether a persisted plugin slot is available and binary-compatible on this host.
    pub fn validate_slot_status(slot: &PluginSlotConfig) -> PluginStatus {
        if !slot.bundle_path.exists() {
            return PluginStatus::Missing;
        }
        let has_binary = match slot.format {
            PluginFormat::Vst3 => PluginScanner::find_vst3_binary(&slot.bundle_path).is_some(),
            PluginFormat::Lv2 => {
                std::fs::read_dir(&slot.bundle_path)
                    .ok()
                    .into_iter()
                    .flatten()
                    .flatten()
                    .any(|e| {
                        matches!(
                            e.path().extension().and_then(|s| s.to_str()),
                            Some("so" | "dll" | "dylib")
                        )
                    })
                    || slot.bundle_path.join("manifest.ttl").is_file()
            }
        };
        if !has_binary {
            return PluginStatus::Incompatible(
                "No compatible native binary for this OS/architecture".to_string(),
            );
        }
        PluginStatus::Loaded
    }

    /// Loads a plugin slot after verifying bundle and platform binary compatibility.
    pub fn load(slot: &PluginSlotConfig) -> Result<Self, PluginLoadError> {
        match Self::validate_slot_status(slot) {
            PluginStatus::Missing => Err(PluginLoadError::NotFound(slot.bundle_path.clone())),
            PluginStatus::Incompatible(reason) => Err(PluginLoadError::IncompatibleBinary(reason)),
            PluginStatus::Loaded | PluginStatus::Available => Ok(Self {
                slot_id: slot.id,
                descriptor_id: slot.descriptor_id.clone(),
                name: slot.name.clone(),
                format: slot.format,
                bypassed: slot.bypassed,
                parameters: slot.parameters.clone(),
                state_blob: slot.state_blob.clone(),
            }),
        }
    }

    /// Updates a parameter value cleanly.
    pub fn set_parameter(&mut self, index: usize, value: f32) {
        if let Some(p) = self.parameters.get_mut(index) {
            p.value = value.clamp(0.0, 1.0);
        }
    }

    /// Passthrough stub for a stereo audio frame (zero-allocation).
    ///
    /// Note: External VST3/LV2 native binaries are not loaded or executed yet;
    /// audio passes through unmodified.
    #[inline]
    pub fn process_stereo(&mut self, left: f32, right: f32) -> (f32, f32) {
        if self.bypassed {
            return (left, right);
        }
        (left, right)
    }

    /// Exports opaque state bytes for project persistence.
    pub fn export_state(&self) -> Option<Vec<u8>> {
        self.state_blob.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scan_vst3_and_lv2_bundles_and_passthrough_stub() {
        let dir = std::env::temp_dir().join(format!("stem_plugin_test_{}", Uuid::new_v4()));
        let vst3_bundle = dir.join("TestComp.vst3");
        let vst3_res = vst3_bundle.join("Contents/Resources");
        #[cfg(target_os = "linux")]
        let vst3_bin_dir = vst3_bundle.join("Contents/x86_64-linux");
        #[cfg(target_os = "windows")]
        let vst3_bin_dir = vst3_bundle.join("Contents/x86_64-win");
        #[cfg(not(any(target_os = "linux", target_os = "windows")))]
        let vst3_bin_dir = vst3_bundle.join("Contents/MacOS");

        std::fs::create_dir_all(&vst3_res).unwrap();
        std::fs::create_dir_all(&vst3_bin_dir).unwrap();
        std::fs::write(vst3_bin_dir.join("TestComp.so"), b"stub").unwrap();
        std::fs::write(
            vst3_res.join("moduleinfo.json"),
            r#"{"Factory Info":{"Vendor":"steM Labs"},"Classes":[{"Name":"TestComp","Category":"Audio Module Class","CID":"1234"}]}"#,
        )
        .unwrap();

        let lv2_bundle = dir.join("TestEQ.lv2");
        std::fs::create_dir_all(&lv2_bundle).unwrap();
        std::fs::write(
            lv2_bundle.join("manifest.ttl"),
            r#"<http://example.org/eq> a lv2:Plugin ; doap:name "Test EQ" ; lv2:binary <eq.so> ."#,
        )
        .unwrap();
        std::fs::write(lv2_bundle.join("eq.so"), b"stub").unwrap();

        let found = PluginScanner::scan_paths(std::slice::from_ref(&dir));
        assert_eq!(found.len(), 2);
        assert!(found
            .iter()
            .any(|p| p.name == "TestComp" && p.vendor == "steM Labs"));
        assert!(found
            .iter()
            .any(|p| p.name == "Test EQ" && p.format == PluginFormat::Lv2));

        let slot = found[0].to_slot_config();
        let mut inst = PluginHostInstance::load(&slot).expect("Should load valid bundle");
        assert_eq!(inst.process_stereo(0.5, -0.5), (0.5, -0.5));

        let mut missing_slot = slot.clone();
        missing_slot.bundle_path = dir.join("NonExistent.vst3");
        assert_eq!(
            PluginHostInstance::validate_slot_status(&missing_slot),
            PluginStatus::Missing
        );
        assert!(PluginHostInstance::load(&missing_slot).is_err());

        let _ = std::fs::remove_dir_all(&dir);
    }
}
