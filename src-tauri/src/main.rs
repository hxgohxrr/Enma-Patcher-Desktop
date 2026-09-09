#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

use std::collections::{HashMap, HashSet};
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

use regex::Regex;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};

use clap::Parser;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProgressEvent {
    step: String,
    detail: String,
    done: usize,
    total: usize,
}

fn emit(app: &AppHandle, step: &str, detail: &str, done: usize, total: usize) {
    let _ = app.emit(
        "patch-progress",
        ProgressEvent {
            step: step.to_string(),
            detail: detail.to_string(),
            done,
            total,
        },
    );
}

fn app_work_dir(app: &AppHandle, name: &str) -> Result<PathBuf, String> {
    let base = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Could not resolve data directory: {e}"))?;
    let dir = base.join(name);
    fs::create_dir_all(&dir).map_err(|e| format!("Could not create {}: {e}", dir.display()))?;
    Ok(dir)
}

fn default_output_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let docs = dirs_documents().unwrap_or_else(|| {
        app.path()
            .document_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
    });
    let dir = docs.join("EnmaPatcherDesktop").join("output");
    fs::create_dir_all(&dir).map_err(|e| format!("Could not create {}: {e}", dir.display()))?;
    Ok(dir)
}

fn platform_key(platform: &str) -> &'static str {
    if platform == "ios" {
        "ios"
    } else {
        "android"
    }
}

fn resolve_output_dir(app: &AppHandle, platform: &str) -> Result<PathBuf, String> {
    let key = platform_key(platform);
    if let Ok(data) = app.path().app_data_dir() {
        for name in [
            format!("output_dir_{key}.txt"),
            "output_dir.txt".to_string(),
        ] {
            if let Ok(txt) = fs::read_to_string(data.join(name)) {
                let custom = PathBuf::from(txt.trim());
                if custom.is_absolute() {
                    fs::create_dir_all(&custom).map_err(|e| {
                        format!("Cannot use output folder {}: {e}", custom.display())
                    })?;
                    return Ok(custom);
                }
            }
        }
    }
    default_output_dir(app)
}

#[tauri::command]
fn get_output_dir(app: AppHandle, platform: String) -> Result<String, String> {
    Ok(resolve_output_dir(&app, &platform)?
        .to_string_lossy()
        .to_string())
}

#[tauri::command]
fn set_output_dir(
    app: AppHandle,
    platform: String,
    path: Option<String>,
) -> Result<String, String> {
    let key = platform_key(&platform);
    let data = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Could not resolve data directory: {e}"))?;
    let marker = data.join(format!("output_dir_{key}.txt"));
    match path {
        None => {
            let _ = fs::remove_file(&marker);
        }
        Some(p) => {
            let custom = PathBuf::from(p.trim());
            if !custom.is_absolute() {
                return Err("Output folder must be an absolute path".to_string());
            }
            fs::create_dir_all(&custom)
                .map_err(|e| format!("Cannot use output folder {}: {e}", custom.display()))?;
            fs::write(&marker, custom.to_string_lossy().as_bytes()).map_err(|e| format!("{e}"))?;
        }
    }
    Ok(resolve_output_dir(&app, key)?.to_string_lossy().to_string())
}

fn dirs_documents() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        std::env::var("USERPROFILE")
            .ok()
            .map(|h| PathBuf::from(h).join("Documents"))
    }
    #[cfg(not(target_os = "windows"))]
    {
        std::env::var("HOME")
            .ok()
            .map(|h| PathBuf::from(h).join("Documents"))
    }
}

fn unique_work_dir(app: &AppHandle, prefix: &str) -> Result<PathBuf, String> {
    let base = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Could not resolve data directory: {e}"))?;
    let dir = base.join(format!(
        "{}_{}",
        prefix,
        chrono::Local::now().format("%Y%m%d_%H%M%S")
    ));
    fs::create_dir_all(&dir).map_err(|e| format!("Could not create {}: {e}", dir.display()))?;
    Ok(dir)
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ApksInfo {
    file_name: String,
    total_size: u64,
    base_apk: Option<String>,
    splits: Vec<SplitInfo>,
    single_apk: bool,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct SplitInfo {
    name: String,
    size: u64,
    kind: String,
}

fn classify_split(name: &str) -> String {
    let n = name.to_lowercase();
    if n.starts_with("base") {
        "base".to_string()
    } else if n.contains("asset") && (n.contains("pack") || n.contains("install_time")) {
        "asset_pack".to_string()
    } else if n.contains("install_time") {
        "asset_pack".to_string()
    } else if n.contains("config.") || n.starts_with("split_config") {
        "config".to_string()
    } else if n.contains("density") || n.contains("dpi") {
        "config".to_string()
    } else if n.contains("abi") || n.contains("arm") || n.contains("x86") {
        "config".to_string()
    } else if n.contains("language") || n.contains("lang") {
        "config".to_string()
    } else {
        "split".to_string()
    }
}

#[tauri::command]
fn inspect_apks(path: String) -> Result<ApksInfo, String> {
    let p = Path::new(&path);
    let meta = fs::metadata(p).map_err(|e| format!("Cannot read file: {e}"))?;
    let mut archive = zip::ZipArchive::new(File::open(p).map_err(|e| format!("Cannot open: {e}"))?)
        .map_err(|_| "File is not a valid ZIP (.apks / .apk)".to_string())?;

    let mut apks: Vec<(String, u64)> = Vec::new();
    for i in 0..archive.len() {
        let f = archive
            .by_index(i)
            .map_err(|e| format!("Corrupt ZIP: {e}"))?;
        let name = f.name().replace('\\', "/");
        if !f.is_dir() && name.to_lowercase().ends_with(".apk") {
            apks.push((
                name.rsplit('/').next().unwrap_or(&name).to_string(),
                f.size(),
            ));
        }
    }

    if apks.is_empty() {
        return Ok(ApksInfo {
            file_name: file_name_of(p),
            total_size: meta.len(),
            base_apk: Some(file_name_of(p)),
            splits: vec![],
            single_apk: true,
        });
    }

    apks.sort_by(|a, b| {
        let ka = if a.0.to_lowercase().starts_with("base") {
            0
        } else {
            1
        };
        let kb = if b.0.to_lowercase().starts_with("base") {
            0
        } else {
            1
        };
        ka.cmp(&kb).then(a.0.cmp(&b.0))
    });

    let base = apks
        .iter()
        .find(|(n, _)| n.to_lowercase().starts_with("base"))
        .map(|(n, _)| n.clone())
        .or_else(|| apks.first().map(|(n, _)| n.clone()));

    let splits = apks
        .iter()
        .filter(|(n, _)| Some(n) != base.as_ref())
        .map(|(n, s)| SplitInfo {
            kind: classify_split(n),
            name: n.clone(),
            size: *s,
        })
        .collect();

    Ok(ApksInfo {
        file_name: file_name_of(p),
        total_size: meta.len(),
        base_apk: base,
        splits,
        single_apk: false,
    })
}

fn file_name_of(p: &Path) -> String {
    p.file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| p.display().to_string())
}

fn guess_package_from_manifest(manifest_bytes: &[u8]) -> Option<String> {
    let mut s = String::with_capacity(manifest_bytes.len() / 2);
    let mut chunks = manifest_bytes.chunks_exact(2);
    for c in &mut chunks {
        let v = u16::from_le_bytes([c[0], c[1]]);
        if v == 0 {
            s.push('\0');
        } else if (0x20..=0x7E).contains(&v) || v > 0x7F {
            if let Some(ch) = char::from_u32(v as u32) {
                s.push(ch);
            }
        }
    }
    if let Some(pkg) = ["jp.co.level5.yws1", "jp.co.level5.yws2"]
        .iter()
        .find(|k| s.contains(**k))
    {
        return Some(pkg.to_string());
    }
    let re = Regex::new(r"[a-z][a-z0-9_]*(\.[a-z][a-z0-9_]+){2,}").ok()?;
    let mut best: Option<String> = None;
    for m in re.find_iter(&s) {
        let cand = m.as_str();
        if cand.contains("android")
            || cand.contains("google")
            || cand.contains("vending")
            || cand.len() > 64
        {
            continue;
        }
        best = Some(cand.to_string());
        if cand.matches('.').count() >= 3 {
            break;
        }
    }
    best
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SingleApkAnalysis {
    file_name: String,
    size: u64,
    package_guess: Option<String>,
    entry_count: usize,
    has_split_requirement: bool,
    dex_count: usize,
    verdict: String,
    detail: String,
}

#[tauri::command]
fn inspect_single_apk(path: String) -> Result<SingleApkAnalysis, String> {
    let p = Path::new(&path);
    let meta = fs::metadata(p).map_err(|e| format!("Cannot read file: {e}"))?;
    let mut archive = zip::ZipArchive::new(File::open(p).map_err(|e| format!("{e}"))?)
        .map_err(|_| "Not a valid APK".to_string())?;

    let mut entry_count = 0;
    let mut dex_count = 0;
    let mut manifest_bytes: Option<Vec<u8>> = None;
    for i in 0..archive.len() {
        let mut f = archive
            .by_index(i)
            .map_err(|e| format!("Corrupt ZIP: {e}"))?;
        if f.is_dir() {
            continue;
        }
        entry_count += 1;
        let name = f.name().to_string();
        if name == "AndroidManifest.xml" {
            let mut b = Vec::new();
            f.read_to_end(&mut b).map_err(|e| format!("{e}"))?;
            manifest_bytes = Some(b);
        } else if name == "classes.dex" || (name.starts_with("classes") && name.ends_with(".dex")) {
            dex_count += 1;
        }
    }

    let package_guess = manifest_bytes
        .as_deref()
        .and_then(guess_package_from_manifest);
    let has_split_requirement = manifest_bytes
        .as_deref()
        .map(has_split_marker)
        .unwrap_or(false);

    Ok(SingleApkAnalysis {
        file_name: file_name_of(p),
        size: meta.len(),
        package_guess,
        entry_count,
        has_split_requirement,
        dex_count,
        verdict: "unknown".to_string(),
        detail: "Use check_apk_patched with the mod file list to decide.".to_string(),
    })
}

fn has_split_marker(manifest: &[u8]) -> bool {
    let targets = [
        "com.android.vending.splits.required",
        "base__abi",
        "base__density",
    ];
    for t in targets {
        let utf8 = t.as_bytes();
        if manifest.windows(utf8.len()).any(|w| w == utf8) {
            return true;
        }
        let utf16: Vec<u8> = t.encode_utf16().flat_map(|u| u.to_le_bytes()).collect();
        if manifest.windows(utf16.len()).any(|w| w == utf16.as_slice()) {
            return true;
        }
    }
    false
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PatchedCheck {
    sampled: usize,
    matched: usize,
    coverage: f64,
    verdict: String,
    detail: String,
}

#[tauri::command]
fn apk_abis(path: String) -> Result<Vec<String>, String> {
    let mut archive = zip::ZipArchive::new(
        File::open(Path::new(&path)).map_err(|e| format!("Cannot open APK: {e}"))?,
    )
    .map_err(|_| "Not a valid APK".to_string())?;
    let mut abis = std::collections::BTreeSet::new();
    for i in 0..archive.len() {
        if let Ok(f) = archive.by_index(i) {
            let n = f.name().replace('\\', "/");
            if let Some(rest) = n.strip_prefix("lib/") {
                if let Some(abi) = rest.split('/').next() {
                    if !abi.is_empty() {
                        abis.insert(abi.to_string());
                    }
                }
            }
        }
    }
    Ok(abis.into_iter().collect())
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DeviceCompat {
    devices: Vec<String>,
    apk_abis: Vec<String>,
    device_abis: Vec<String>,
    compatible: Option<bool>,
    note: String,
}

fn adb_shell(adb: &str, device: &str, args: &[&str]) -> Result<String, String> {
    let mut cmd = Command::new(adb);
    cmd.args(["-s", device, "shell"]);
    cmd.args(args);
    let out = cmd.output().map_err(|e| format!("{e}"))?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

#[tauri::command]
fn check_device_compat(app: AppHandle, apk_path: String) -> Result<DeviceCompat, String> {
    let apk_abis = apk_abis(apk_path)?;
    let adb = match find_adb(&app) {
        Some(a) => a,
        None => {
            return Ok(DeviceCompat {
                devices: vec![],
                apk_abis,
                device_abis: vec![],
                compatible: None,
                note: "adb not available".to_string(),
            })
        }
    };
    let devs = adb_devices(app)?;
    if devs.devices.is_empty() {
        return Ok(DeviceCompat {
            devices: vec![],
            apk_abis,
            device_abis: vec![],
            compatible: None,
            note: "no devices connected".to_string(),
        });
    }
    let target = devs.devices[0].clone();
    let abilist = adb_shell(&adb, &target, &["getprop", "ro.product.cpu.abilist"])
        .or_else(|_| adb_shell(&adb, &target, &["getprop", "ro.product.cpu.abi"]))
        .unwrap_or_default();
    let device_abis: Vec<String> = abilist
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    if apk_abis.is_empty() {
        return Ok(DeviceCompat {
            devices: devs.devices,
            apk_abis,
            device_abis,
            compatible: Some(true),
            note: "no native libraries, architecture independent".to_string(),
        });
    }
    let overlap = device_abis.iter().any(|a| apk_abis.contains(a));
    let note = if overlap {
        format!("device {target} supports {}", device_abis.join(","))
    } else {
        format!(
            "ABI mismatch: apk has [{}] but device {target} supports [{}]",
            apk_abis.join(", "),
            device_abis.join(", ")
        )
    };
    Ok(DeviceCompat {
        devices: devs.devices,
        apk_abis,
        device_abis,
        compatible: Some(overlap),
        note,
    })
}

#[tauri::command]
fn check_apk_patched(apk_path: String, markers: Vec<String>) -> Result<PatchedCheck, String> {
    let mut archive = zip::ZipArchive::new(
        File::open(Path::new(&apk_path)).map_err(|e| format!("Cannot open APK: {e}"))?,
    )
    .map_err(|_| "Not a valid APK".to_string())?;

    let mut entries: HashSet<String> = HashSet::new();
    for i in 0..archive.len() {
        if let Ok(f) = archive.by_index(i) {
            if !f.is_dir() {
                entries.insert(f.name().replace('\\', "/"));
            }
        }
    }

    let sample: Vec<&String> = markers.iter().take(400).collect();
    let mut matched = 0;
    for m in &sample {
        let norm = m.replace('\\', "/");
        let norm = norm.trim_start_matches("./");
        if entries.contains(norm)
            || entries.contains(&format!("assets/{norm}"))
            || entries.iter().any(|e| e.ends_with(norm))
        {
            matched += 1;
        }
    }

    let coverage = if sample.is_empty() {
        0.0
    } else {
        matched as f64 / sample.len() as f64
    };
    let (verdict, detail) = if sample.is_empty() {
        (
            "unknown".to_string(),
            "No mod file list: cannot decide. Pass the repository file list.".to_string(),
        )
    } else if coverage >= 0.5 {
        (
            "patched".to_string(),
            format!(
                "APK already contains {matched} of {} patch files. You can continue.",
                sample.len()
            ),
        )
    } else if coverage <= 0.05 {
        (
            "clean".to_string(),
            "This APK does not look patched. Buy the game and export the full package (.apks with base + splits) to patch from a clean base."
                .to_string(),
        )
    } else {
        (
            "unknown".to_string(),
            format!(
                "Partial match ({matched} of {}). It may be another patch version.",
                sample.len()
            ),
        )
    };

    Ok(PatchedCheck {
        sampled: sample.len(),
        matched,
        coverage,
        verdict: verdict.to_string(),
        detail: detail.to_string(),
    })
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DrmbInfo {
    file_name: String,
    total_size: u64,
    base_files: usize,
    split_files: usize,
    smali_files: usize,
    sample: Vec<String>,
}

#[tauri::command]
fn inspect_drmb(path: String) -> Result<DrmbInfo, String> {
    let p = Path::new(&path);
    let meta = fs::metadata(p).map_err(|e| format!("Cannot read file: {e}"))?;
    let mut archive = zip::ZipArchive::new(File::open(p).map_err(|e| format!("{e}"))?)
        .map_err(|_| "Not a valid .drmb ZIP".to_string())?;

    let mut base_files = 0;
    let mut split_files = 0;
    let mut smali_files = 0;
    let mut sample = Vec::new();
    for i in 0..archive.len() {
        let f = archive
            .by_index(i)
            .map_err(|e| format!("Corrupt ZIP: {e}"))?;
        if f.is_dir() {
            continue;
        }
        let name = f.name().replace('\\', "/");
        if name.starts_with("base/") {
            base_files += 1;
            if name.starts_with("base/smali/") || name.ends_with(".smali") {
                smali_files += 1;
            }
        } else if name.starts_with("split/") {
            split_files += 1;
        }
        if sample.len() < 12 {
            sample.push(name);
        }
    }

    if base_files == 0 && split_files == 0 {
        return Err(".drmb has no base/ or split/ folders. Invalid file.".to_string());
    }

    Ok(DrmbInfo {
        file_name: file_name_of(p),
        total_size: meta.len(),
        base_files,
        split_files,
        smali_files,
        sample,
    })
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct IpaInfo {
    file_name: String,
    total_size: u64,
    app_dir: String,
    bundle_id: Option<String>,
    bundle_name: Option<String>,
    version: Option<String>,
    total_entries: usize,
    data_entries: usize,
}

#[tauri::command]
fn inspect_ipa(path: String) -> Result<IpaInfo, String> {
    let p = Path::new(&path);
    let meta = fs::metadata(p).map_err(|e| format!("Cannot read file: {e}"))?;
    let mut archive = zip::ZipArchive::new(File::open(p).map_err(|e| format!("{e}"))?)
        .map_err(|_| "Not a valid .ipa ZIP".to_string())?;

    let mut app_dir: Option<String> = None;
    let mut total_entries = 0;
    let mut data_entries = 0;
    let mut plist_bytes: Option<Vec<u8>> = None;

    for i in 0..archive.len() {
        let mut f = archive
            .by_index(i)
            .map_err(|e| format!("Corrupt ZIP: {e}"))?;
        if f.is_dir() {
            continue;
        }
        total_entries += 1;
        let name = f.name().replace('\\', "/");
        if app_dir.is_none() && name.starts_with("Payload/") {
            let rest = &name["Payload/".len()..];
            if let Some(first) = rest.split('/').next() {
                if first.ends_with(".app") {
                    app_dir = Some(format!("Payload/{first}"));
                }
            }
        }
        if name.contains(".app/data/") {
            data_entries += 1;
        }
        if name.ends_with(".app/Info.plist") && plist_bytes.is_none() {
            let mut b = Vec::new();
            f.read_to_end(&mut b).map_err(|e| format!("{e}"))?;
            plist_bytes = Some(b);
        }
    }

    let app_dir = app_dir.ok_or(".ipa contains no Payload/*.app. It must be a decrypted .ipa.")?;

    let (bundle_id, bundle_name, version) = plist_bytes
        .as_deref()
        .and_then(|b| {
            plist::Value::from_reader(std::io::Cursor::new(b))
                .ok()
                .and_then(|v| v.into_dictionary())
        })
        .map(|d| {
            let s = |k: &str| d.get(k).and_then(|v| v.as_string()).map(|x| x.to_string());
            (
                s("CFBundleIdentifier"),
                s("CFBundleDisplayName").or_else(|| s("CFBundleName")),
                s("CFBundleShortVersionString"),
            )
        })
        .unwrap_or((None, None, None));

    Ok(IpaInfo {
        file_name: file_name_of(p),
        total_size: meta.len(),
        app_dir,
        bundle_id,
        bundle_name,
        version,
        total_entries,
        data_entries,
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ModSpec {
    kind: String,
    #[serde(default)]
    repo: String,
    #[serde(default)]
    branch: String,
    #[serde(default)]
    path: String,
    #[serde(default = "default_true")]
    enabled: bool,
}

fn default_true() -> bool {
    true
}

fn sign_mode_default() -> String {
    "auto".to_string()
}

fn github_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .user_agent("EnmaPatcherDesktop/0.1")
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .map_err(|e| format!("Could not create HTTP client: {e}"))
}

#[tauri::command]
async fn list_github_files(
    owner: String,
    repo: String,
    branch: String,
) -> Result<Vec<String>, String> {
    let client = github_client()?;
    let url = format!(
        "https://api.github.com/repos/{}/{}/git/trees/{}?recursive=1",
        owner.trim(),
        repo.trim(),
        branch.trim()
    );
    let resp = client
        .get(&url)
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .map_err(|e| format!("Network error listing repository: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!(
            "GitHub returned {} listing {}/{}",
            resp.status(),
            owner,
            repo
        ));
    }
    let json: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("Unexpected GitHub response: {e}"))?;
    if json
        .get("truncated")
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
    {
        return Err("Repository tree too large (truncated).".to_string());
    }
    let mut out = Vec::new();
    if let Some(tree) = json.get("tree").and_then(|v| v.as_array()) {
        for node in tree {
            if node.get("type").and_then(|v| v.as_str()) != Some("blob") {
                continue;
            }
            if let Some(path) = node.get("path").and_then(|v| v.as_str()) {
                if !path.is_empty() {
                    out.push(path.to_string());
                }
            }
        }
    }
    out.sort();
    Ok(out)
}

#[tauri::command]
async fn fetch_remote_config(
    owner: String,
    repo: String,
    branch: String,
) -> Result<EnmaCfg, String> {
    let client = github_client()?;
    let url = format!(
        "https://raw.githubusercontent.com/{}/{}/{}/enmapatcher.cfg.json",
        owner.trim(),
        repo.trim(),
        branch.trim()
    );
    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("Network error: {e}"))?;
    if !resp.status().is_success() {
        return Ok(EnmaCfg::default());
    }
    let text = resp.text().await.map_err(|e| format!("{e}"))?;
    Ok(EnmaCfg::from_json(&text))
}

fn string_or_vec<'de, D>(d: D) -> Result<Vec<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::Deserialize;
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum OneOrMany {
        One(String),
        Many(Vec<String>),
    }
    Ok(match OneOrMany::deserialize(d)? {
        OneOrMany::One(s) => vec![s],
        OneOrMany::Many(v) => v,
    })
}

fn glob_match(pattern: &str, path: &str) -> bool {
    let mut pat = pattern.trim().replace('\\', "/");
    while pat.starts_with("./") {
        pat = pat[2..].to_string();
    }
    pat = pat.trim_start_matches('/').to_string();
    if pat.is_empty() {
        return false;
    }
    if pat.ends_with('/') {
        return path == pat.trim_end_matches('/') || path.starts_with(&pat);
    }
    let full = |p: &str, s: &str| {
        let mut re = String::from("^");
        let chars: Vec<char> = p.chars().collect();
        let mut i = 0;
        while i < chars.len() {
            if chars[i] == '*' {
                if i + 1 < chars.len() && chars[i + 1] == '*' {
                    re.push_str(".*");
                    i += 2;
                    if i < chars.len() && chars[i] == '/' {
                        i += 1;
                    }
                } else {
                    re.push_str("[^/]*");
                    i += 1;
                }
            } else if chars[i] == '?' {
                re.push_str("[^/]");
                i += 1;
            } else {
                re.push_str(&regex::escape(&chars[i].to_string()));
                i += 1;
            }
        }
        re.push('$');
        regex::Regex::new(&re)
            .map(|r| r.is_match(s))
            .unwrap_or(false)
    };
    if full(&pat, path) {
        return true;
    }
    if !pat.contains('/') {
        if let Some(base) = path.rsplit('/').next() {
            return full(&pat, base);
        }
    }
    false
}

fn keep_path(path: &str, include: &[String], exclude: &[String]) -> bool {
    if exclude.iter().any(|p| glob_match(p, path)) {
        return false;
    }
    for p in exclude {
        let mut c = p.trim().replace('\\', "/");
        while c.starts_with("./") {
            c = c[2..].to_string();
        }
        c = c.trim_start_matches('/').trim_end_matches('/').to_string();
        if c.is_empty() || c.contains('*') || c.contains('?') || c.contains('[') {
            continue;
        }
        let want = c.rsplit('/').next().unwrap_or("").to_lowercase();
        let actual = path.rsplit('/').next().unwrap_or("").to_lowercase();
        if !want.is_empty() && want == actual {
            return false;
        }
    }
    if include.is_empty() {
        return true;
    }
    include.iter().any(|p| glob_match(p, path))
}

fn expand_image_excludes(exc: &mut Vec<String>) {
    let image_names = ["image", "images", "img"];
    let mut wanted: Vec<String> = Vec::new();
    for p in exc.iter() {
        let mut c = p.trim().replace('\\', "/");
        while c.starts_with("./") {
            c = c[2..].to_string();
        }
        let leaf = c
            .trim_start_matches('/')
            .trim_end_matches('/')
            .rsplit('/')
            .next()
            .unwrap_or("")
            .to_lowercase();
        if image_names.contains(&leaf.as_str()) {
            for a in ["image/", "images/", "img/"] {
                if !exc.contains(&a.to_string()) && !wanted.contains(&a.to_string()) {
                    wanted.push(a.to_string());
                }
            }
        }
    }
    exc.extend(wanted);
}

fn merge_string_list(into: &mut Vec<String>, from: Vec<String>) {
    for p in from {
        if !into.contains(&p) {
            into.push(p);
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Platforms {
    #[serde(default = "default_true", alias = "Android")]
    android: bool,
    #[serde(default = "default_true", alias = "iOS")]
    ios: bool,
}

impl Default for Platforms {
    fn default() -> Self {
        Platforms {
            android: true,
            ios: true,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct EnmaCfg {
    #[serde(default)]
    app_name: Option<String>,
    #[serde(default)]
    current_label: Option<String>,
    #[serde(default)]
    version: Option<String>,
    #[serde(default, alias = "Include", deserialize_with = "string_or_vec")]
    include: Vec<String>,
    #[serde(default, alias = "Exclude", deserialize_with = "string_or_vec")]
    exclude: Vec<String>,
    #[serde(default, alias = "IncludeAndroid", deserialize_with = "string_or_vec")]
    include_android: Vec<String>,
    #[serde(default, alias = "ExcludeAndroid", deserialize_with = "string_or_vec")]
    exclude_android: Vec<String>,
    #[serde(default, alias = "IncludeIos", deserialize_with = "string_or_vec")]
    include_ios: Vec<String>,
    #[serde(default, alias = "ExcludeIos", deserialize_with = "string_or_vec")]
    exclude_ios: Vec<String>,
    #[serde(default)]
    platforms: Platforms,
    #[serde(default)]
    compatible_mods: Vec<String>,
    #[serde(default, alias = "uncompatible_mods")]
    incompatible_mods: Vec<String>,
    #[serde(default)]
    recommended_version: Option<String>,
    #[serde(default)]
    tested_versions: Vec<String>,
    #[serde(default, alias = "uncompatible_versions")]
    incompatible_versions: Vec<String>,
    #[serde(default, alias = "ai_content")]
    ai_content: bool,
    #[serde(default, alias = "License")]
    license: Option<String>,
    #[serde(default, alias = "renameassets", alias = "rename_assets")]
    rename_assets: Option<bool>,
}

fn platform_enabled(cfg: &EnmaCfg, platform: &str) -> bool {
    if platform == "ios" {
        cfg.platforms.ios
    } else {
        cfg.platforms.android
    }
}

fn effective_filters(cfg: &EnmaCfg, platform: &str) -> (Vec<String>, Vec<String>) {
    let mut inc = cfg.include.clone();
    let mut exc = cfg.exclude.clone();
    if platform == "ios" {
        merge_string_list(&mut inc, cfg.include_ios.clone());
        merge_string_list(&mut exc, cfg.exclude_ios.clone());
        let wants_android = inc.iter().any(|p| p.contains("assets/android"));
        if !wants_android && !exc.iter().any(|p| p.contains("assets/android")) {
            exc.push("assets/android/**".to_string());
        }
    } else {
        merge_string_list(&mut inc, cfg.include_android.clone());
        merge_string_list(&mut exc, cfg.exclude_android.clone());
    }
    expand_image_excludes(&mut exc);
    (inc, exc)
}

impl EnmaCfg {
    fn from_json(text: &str) -> Self {
        serde_json::from_str(text).unwrap_or_default()
    }
}

struct ModFetch {
    config: EnmaCfg,
    files: usize,
    skipped_platform: bool,
}

struct ModReport {
    label: String,
    config: EnmaCfg,
}

struct ModsDownload {
    config: EnmaCfg,
    files: usize,
    skipped: Vec<String>,
    reports: Vec<ModReport>,
}

async fn download_github_zip(
    app: &AppHandle,
    client: &reqwest::Client,
    owner: &str,
    repo: &str,
    branch: &str,
    dest: &Path,
    scratch: &Path,
    label: &str,
    platform: &str,
) -> Result<ModFetch, String> {
    use futures_util::StreamExt;
    use std::io::Write as _;
    let url = format!("https://codeload.github.com/{owner}/{repo}/zip/refs/heads/{branch}");
    emit(
        app,
        "download",
        &format!("{label}: downloading repo ZIP..."),
        0,
        1,
    );
    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("ZIP request failed: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("ZIP download failed: {}", resp.status()));
    }
    let total = resp.content_length().unwrap_or(0);
    let zip_path = scratch.join(".mod_repo.zip");
    let mut file = File::create(&zip_path).map_err(|e| format!("{e}"))?;
    let mut stream = resp.bytes_stream();
    let mut done = 0u64;
    let mut last_pct = 0u64;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| format!("ZIP download interrupted: {e}"))?;
        file.write_all(&chunk).map_err(|e| format!("{e}"))?;
        done += chunk.len() as u64;
        if total > 0 {
            let pct = done * 100 / total;
            if pct >= last_pct + 10 {
                last_pct = pct;
                emit(app, "download", &format!("{label}: ZIP {pct}%"), 0, 1);
            }
        }
    }
    drop(file);
    let mut archive = zip::ZipArchive::new(File::open(&zip_path).map_err(|e| format!("{e}"))?)
        .map_err(|_| "downloaded ZIP is invalid".to_string())?;
    let mut prefix: Option<String> = None;
    for i in 0..archive.len() {
        if let Ok(f) = archive.by_index(i) {
            let n = f.name().to_string();
            if n.ends_with('/') && n.matches('/').count() == 1 && prefix.is_none() {
                prefix = Some(n);
                break;
            }
        }
    }
    let mut cfg = EnmaCfg::default();
    let mut rels: Vec<String> = Vec::new();
    for i in 0..archive.len() {
        let mut f = archive.by_index(i).map_err(|e| format!("{e}"))?;
        if f.is_dir() || f.enclosed_name().is_none() {
            continue;
        }
        let mut name = f.name().replace('\\', "/");
        if let Some(p) = &prefix {
            if name.starts_with(p) {
                name = name[p.len()..].to_string();
            }
        }
        if name.is_empty() {
            continue;
        }
        if name == "enmapatcher.cfg.json" {
            let mut text = String::new();
            f.read_to_string(&mut text).map_err(|e| format!("{e}"))?;
            cfg = EnmaCfg::from_json(&text);
            continue;
        }
        let dest_file = dest.join(&name);
        if let Some(parent) = dest_file.parent() {
            fs::create_dir_all(parent).map_err(|e| format!("{e}"))?;
        }
        let mut out = File::create(&dest_file).map_err(|e| format!("{e}"))?;
        std::io::copy(&mut f, &mut out).map_err(|e| format!("{e}"))?;
        rels.push(name.clone());
        if rels.len() % 200 == 0 {
            emit(
                app,
                "download",
                &format!("{label}: extracting {} files...", rels.len()),
                0,
                1,
            );
        }
    }
    let _ = fs::remove_file(&zip_path);
    if !platform_enabled(&cfg, platform) {
        for rel in &rels {
            let _ = fs::remove_file(dest.join(rel));
        }
        return Ok(ModFetch {
            config: cfg,
            files: 0,
            skipped_platform: true,
        });
    }
    let (inc, exc) = effective_filters(&cfg, platform);
    let mut kept = 0usize;
    for rel in &rels {
        if keep_path(rel, &inc, &exc) {
            kept += 1;
        } else {
            let _ = fs::remove_file(dest.join(rel));
        }
    }
    Ok(ModFetch {
        config: cfg,
        files: kept,
        skipped_platform: false,
    })
}

async fn download_github_raw(
    app: &AppHandle,
    client: &reqwest::Client,
    owner: &str,
    repo: &str,
    branch: &str,
    dest: &Path,
    platform: &str,
    idx: usize,
    total_mods: usize,
    label: &str,
) -> Result<ModFetch, String> {
    let cfg_url =
        format!("https://raw.githubusercontent.com/{owner}/{repo}/{branch}/enmapatcher.cfg.json");
    let mut remote_cfg = EnmaCfg::default();
    if let Ok(resp) = client.get(&cfg_url).send().await {
        if resp.status().is_success() {
            if let Ok(text) = resp.text().await {
                remote_cfg = EnmaCfg::from_json(&text);
            }
        }
    }
    if !platform_enabled(&remote_cfg, platform) {
        return Ok(ModFetch {
            config: remote_cfg,
            files: 0,
            skipped_platform: true,
        });
    }
    let (inc, exc) = effective_filters(&remote_cfg, platform);
    let files = list_github_files(owner.to_string(), repo.to_string(), branch.to_string()).await?;
    let targets: Vec<String> = files
        .into_iter()
        .filter(|p| p != "enmapatcher.cfg.json" && keep_path(p, &inc, &exc))
        .collect();
    let mut count = 0usize;
    for (fi, path) in targets.iter().enumerate() {
        let url = format!("https://raw.githubusercontent.com/{owner}/{repo}/{branch}/{path}");
        match client.get(&url).send().await {
            Ok(resp) if resp.status().is_success() => match resp.bytes().await {
                Ok(bytes) => {
                    let dest_file = dest.join(path);
                    if let Some(parent) = dest_file.parent() {
                        let _ = fs::create_dir_all(parent);
                    }
                    if fs::write(&dest_file, &bytes).is_ok() {
                        count += 1;
                    }
                }
                Err(_) => continue,
            },
            _ => continue,
        }
        if fi % 25 == 0 {
            emit(
                app,
                "download",
                &format!("{label}: {fi}/{} files", targets.len()),
                idx,
                total_mods,
            );
        }
    }
    Ok(ModFetch {
        config: remote_cfg,
        files: count,
        skipped_platform: false,
    })
}

fn merge_mod_config(merged: &mut EnmaCfg, cfg: EnmaCfg) {
    if merged
        .app_name
        .as_ref()
        .map(|s| s.is_empty())
        .unwrap_or(true)
    {
        merged.app_name = cfg.app_name;
    }
    if merged.current_label.is_none() {
        merged.current_label = cfg.current_label;
    }
    if merged.version.is_none() {
        merged.version = cfg.version;
    }
    merge_string_list(&mut merged.include, cfg.include.clone());
    merge_string_list(&mut merged.exclude, cfg.exclude.clone());
    merge_string_list(&mut merged.include_android, cfg.include_android.clone());
    merge_string_list(&mut merged.exclude_android, cfg.exclude_android.clone());
    merge_string_list(&mut merged.include_ios, cfg.include_ios.clone());
    merge_string_list(&mut merged.exclude_ios, cfg.exclude_ios.clone());
    if merged.recommended_version.is_none() {
        merged.recommended_version = cfg.recommended_version.clone();
    }
    merge_string_list(&mut merged.tested_versions, cfg.tested_versions.clone());
    merge_string_list(
        &mut merged.incompatible_versions,
        cfg.incompatible_versions.clone(),
    );
    merge_string_list(&mut merged.compatible_mods, cfg.compatible_mods.clone());
    merge_string_list(&mut merged.incompatible_mods, cfg.incompatible_mods.clone());
    if cfg.rename_assets == Some(true) {
        merged.rename_assets = Some(true);
    } else if cfg.rename_assets == Some(false) && merged.rename_assets.is_none() {
        merged.rename_assets = Some(false);
    }
}

async fn download_mods_to_dir(
    app: &AppHandle,
    mods: &[ModSpec],
    dest: &Path,
    platform: &str,
) -> Result<ModsDownload, String> {
    let client = github_client()?;
    let mut merged = EnmaCfg::default();
    let mut total_files = 0usize;
    let mut skipped: Vec<String> = Vec::new();
    let mut reports: Vec<ModReport> = Vec::new();
    let enabled: Vec<&ModSpec> = mods.iter().filter(|m| m.enabled).collect();
    if enabled.is_empty() {
        return Ok(ModsDownload {
            config: merged,
            files: 0,
            skipped,
            reports,
        });
    }

    for (idx, m) in enabled.iter().enumerate() {
        let label = if m.kind == "github" {
            format!("{}/{}@{}", m.repo, m.branch, idx + 1)
        } else {
            file_name_of(Path::new(&m.path))
        };
        emit(
            app,
            "download",
            &format!(
                "Downloading mod {} of {}: {}",
                idx + 1,
                enabled.len(),
                label
            ),
            idx,
            enabled.len(),
        );

        if m.kind == "github" {
            let mut parts = m.repo.split('/');
            let owner = parts.next().unwrap_or("").trim();
            let repo = parts.next().unwrap_or("").trim();
            let branch = if m.branch.trim().is_empty() {
                "main"
            } else {
                m.branch.trim()
            };
            if owner.is_empty() || repo.is_empty() {
                return Err(format!("Invalid repository: {}", m.repo));
            }
            let scratch = dest
                .parent()
                .map(|p| p.to_path_buf())
                .unwrap_or_else(std::env::temp_dir);
            let mut fetch: Option<ModFetch> = None;
            match download_github_zip(
                app, &client, owner, repo, branch, dest, &scratch, &label, platform,
            )
            .await
            {
                Ok(f) if f.files > 0 || f.skipped_platform => {
                    fetch = Some(f);
                }
                Ok(_) => {}
                Err(e) => {
                    emit(
                        app,
                        "download",
                        &format!("ZIP unavailable, downloading file by file ({e})"),
                        idx,
                        enabled.len(),
                    );
                }
            }
            if fetch.is_none() {
                match download_github_raw(
                    app,
                    &client,
                    owner,
                    repo,
                    branch,
                    dest,
                    platform,
                    idx,
                    enabled.len(),
                    &label,
                )
                .await
                {
                    Ok(f) => {
                        fetch = Some(f);
                    }
                    Err(e) => {
                        return Err(e);
                    }
                }
            }
            if let Some(f) = fetch {
                if f.skipped_platform {
                    skipped.push(label.clone());
                } else {
                    reports.push(ModReport {
                        label: label.clone(),
                        config: f.config.clone(),
                    });
                    merge_mod_config(&mut merged, f.config);
                    total_files += f.files;
                }
            }
        } else if m.kind == "zip" {
            let fetch = extract_local_mod_zip(Path::new(&m.path), dest, platform)?;
            if fetch.skipped_platform {
                skipped.push(label.clone());
            } else {
                reports.push(ModReport {
                    label: label.clone(),
                    config: fetch.config.clone(),
                });
                merge_mod_config(&mut merged, fetch.config);
                total_files += fetch.files;
            }
        } else {
            return Err(format!("Unsupported mod type: {}", m.kind));
        }
        emit(
            app,
            "download",
            &format!("Mod ready: {label}"),
            idx + 1,
            enabled.len(),
        );
    }

    Ok(ModsDownload {
        config: merged,
        files: total_files,
        skipped,
        reports,
    })
}

fn mod_has_smali_path(path: &str) -> bool {
    let lower = path.replace('\\', "/").to_lowercase();
    lower.ends_with(".smali") || lower.split('/').any(|seg| seg == "smali")
}

fn extract_local_mod_zip(zip_path: &Path, dest: &Path, platform: &str) -> Result<ModFetch, String> {
    let mut archive =
        zip::ZipArchive::new(File::open(zip_path).map_err(|e| format!("Cannot open ZIP: {e}"))?)
            .map_err(|_| "Not a valid mod ZIP".to_string())?;

    let mut prefix: Option<String> = None;
    for i in 0..archive.len() {
        if let Ok(f) = archive.by_index(i) {
            let n = f.name().to_string();
            if n.ends_with('/') && n.matches('/').count() == 1 && prefix.is_none() {
                prefix = Some(n);
                break;
            }
        }
    }

    let mut cfg = EnmaCfg::default();
    let mut rels: Vec<String> = Vec::new();
    for i in 0..archive.len() {
        let mut f = archive
            .by_index(i)
            .map_err(|e| format!("Corrupt ZIP: {e}"))?;
        if f.is_dir() {
            continue;
        }
        let mut name = f.name().replace('\\', "/");
        if let Some(p) = &prefix {
            if name.starts_with(p) {
                name = name[p.len()..].to_string();
            }
        }
        if name.is_empty() {
            continue;
        }
        if name == "enmapatcher.cfg.json" {
            let mut t = String::new();
            f.read_to_string(&mut t).map_err(|e| format!("{e}"))?;
            cfg = EnmaCfg::from_json(&t);
            continue;
        }
        let dest_file = dest.join(&name);
        if let Some(parent) = dest_file.parent() {
            fs::create_dir_all(parent).map_err(|e| format!("{e}"))?;
        }
        let mut out = File::create(&dest_file).map_err(|e| format!("{e}"))?;
        std::io::copy(&mut f, &mut out).map_err(|e| format!("{e}"))?;
        rels.push(name.clone());
    }
    let mut kept = 0usize;
    if !platform_enabled(&cfg, platform) {
        for rel in &rels {
            let _ = fs::remove_file(dest.join(rel));
        }
        return Ok(ModFetch {
            config: cfg,
            files: 0,
            skipped_platform: true,
        });
    }
    let (inc, exc) = effective_filters(&cfg, platform);
    for rel in &rels {
        if keep_path(rel, &inc, &exc) {
            kept += 1;
        } else {
            let _ = fs::remove_file(dest.join(rel));
        }
    }
    Ok(ModFetch {
        config: cfg,
        files: kept,
        skipped_platform: false,
    })
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ModInfo {
    config: EnmaCfg,
    file_count: usize,
    stars: Option<u32>,
    source_url: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ModSmaliInfo {
    has_smali: bool,
    smali_files: usize,
}

#[tauri::command]
async fn inspect_mod_smali(spec: ModSpec) -> Result<ModSmaliInfo, String> {
    if spec.kind == "github" {
        let mut parts = spec.repo.split('/');
        let owner = parts.next().unwrap_or("").trim();
        let repo = parts.next().unwrap_or("").trim();
        let branch = if spec.branch.trim().is_empty() {
            "main"
        } else {
            spec.branch.trim()
        };
        if owner.is_empty() || repo.is_empty() {
            return Err(format!("Invalid repository: {}", spec.repo));
        }
        let files =
            list_github_files(owner.to_string(), repo.to_string(), branch.to_string()).await?;
        let hits = files.iter().filter(|p| mod_has_smali_path(p)).count();
        return Ok(ModSmaliInfo {
            has_smali: hits > 0,
            smali_files: hits,
        });
    }
    if spec.kind == "zip" {
        let mut archive = zip::ZipArchive::new(
            File::open(Path::new(&spec.path)).map_err(|e| format!("Cannot open ZIP: {e}"))?,
        )
        .map_err(|_| "Not a valid mod ZIP".to_string())?;
        let mut hits = 0usize;
        for i in 0..archive.len() {
            if let Ok(f) = archive.by_index(i) {
                if !f.is_dir() && mod_has_smali_path(f.name()) {
                    hits += 1;
                }
            }
        }
        return Ok(ModSmaliInfo {
            has_smali: hits > 0,
            smali_files: hits,
        });
    }
    Err(format!("Unsupported mod type: {}", spec.kind))
}

#[tauri::command]
async fn mod_info(spec: ModSpec) -> Result<ModInfo, String> {
    if spec.kind == "github" {
        let mut parts = spec.repo.split('/');
        let owner = parts.next().unwrap_or("").trim();
        let repo = parts.next().unwrap_or("").trim();
        let branch = if spec.branch.trim().is_empty() {
            "main"
        } else {
            spec.branch.trim()
        };
        if owner.is_empty() || repo.is_empty() {
            return Err(format!("Invalid repository: {}", spec.repo));
        }
        let config =
            fetch_remote_config(owner.to_string(), repo.to_string(), branch.to_string()).await?;
        let files =
            list_github_files(owner.to_string(), repo.to_string(), branch.to_string()).await?;
        let file_count = files
            .iter()
            .filter(|p| *p != "enmapatcher.cfg.json")
            .count();
        let (stars, source_url) = match github_client() {
            Ok(client) => {
                let url = format!("https://api.github.com/repos/{owner}/{repo}");
                match client
                    .get(&url)
                    .header("Accept", "application/vnd.github+json")
                    .send()
                    .await
                {
                    Ok(resp) if resp.status().is_success() => {
                        let v: serde_json::Value = resp.json().await.unwrap_or_default();
                        (
                            v.get("stargazers_count")
                                .and_then(|n| n.as_u64())
                                .map(|n| n as u32),
                            v.get("html_url")
                                .and_then(|s| s.as_str())
                                .map(|s| s.to_string())
                                .or_else(|| Some(format!("https://github.com/{owner}/{repo}"))),
                        )
                    }
                    _ => (None, Some(format!("https://github.com/{owner}/{repo}"))),
                }
            }
            Err(_) => (None, Some(format!("https://github.com/{owner}/{repo}"))),
        };
        Ok(ModInfo {
            config,
            file_count,
            stars,
            source_url,
        })
    } else if spec.kind == "zip" {
        let path = PathBuf::from(&spec.path);
        let mut archive =
            zip::ZipArchive::new(File::open(&path).map_err(|e| format!("Cannot open ZIP: {e}"))?)
                .map_err(|_| "Not a valid mod ZIP".to_string())?;
        let mut prefix: Option<String> = None;
        for i in 0..archive.len() {
            if let Ok(f) = archive.by_index(i) {
                let n = f.name().replace('\\', "/");
                if n.ends_with('/') && n.matches('/').count() == 1 && prefix.is_none() {
                    prefix = Some(n);
                    break;
                }
            }
        }
        let mut config = EnmaCfg::default();
        let mut file_count = 0usize;
        for i in 0..archive.len() {
            let mut f = archive
                .by_index(i)
                .map_err(|e| format!("Corrupt ZIP: {e}"))?;
            if f.is_dir() {
                continue;
            }
            let mut name = f.name().replace('\\', "/");
            if let Some(p) = &prefix {
                if name.starts_with(p) {
                    name = name[p.len()..].to_string();
                }
            }
            if name.is_empty() {
                continue;
            }
            if name == "enmapatcher.cfg.json" {
                let mut text = String::new();
                f.read_to_string(&mut text).map_err(|e| format!("{e}"))?;
                config = EnmaCfg::from_json(&text);
                continue;
            }
            file_count += 1;
        }
        Ok(ModInfo {
            config,
            file_count,
            stars: None,
            source_url: None,
        })
    } else {
        Err(format!("Unsupported mod type: {}", spec.kind))
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AndroidPatchRequest {
    apks_path: String,
    #[serde(default)]
    drmb_path: Option<String>,
    mods: Vec<ModSpec>,
    #[serde(default)]
    output_name: Option<String>,
    #[serde(default)]
    force_single_apk: bool,
    #[serde(default)]
    work_dir: Option<String>,
    #[serde(default = "sign_mode_default")]
    sign_mode: String,
    #[serde(default = "default_true")]
    sign_v1: bool,
    #[serde(default = "default_true")]
    sign_v2: bool,
    #[serde(default = "default_true")]
    sign_v3: bool,
    #[serde(default)]
    android_version: Option<String>,
    #[serde(default)]
    app_name: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AndroidPatchResult {
    output_path: String,
    total_overrides: usize,
    smali_files: usize,
    used_apktool: bool,
    signed: bool,
    sign_schemes: Vec<String>,
    warning: Option<String>,
    app_name: Option<String>,
}

fn extract_apks_bundle(apks_path: &Path, work: &Path) -> Result<(PathBuf, Vec<PathBuf>), String> {
    let mut archive =
        zip::ZipArchive::new(File::open(apks_path).map_err(|e| format!("Cannot open: {e}"))?)
            .map_err(|_| "File is not a valid package".to_string())?;

    let mut found: Vec<String> = Vec::new();
    for i in 0..archive.len() {
        if let Ok(f) = archive.by_index(i) {
            let n = f.name().replace('\\', "/");
            if !f.is_dir() && n.to_lowercase().ends_with(".apk") {
                found.push(n);
            }
        }
    }

    let out_dir = work.join("bundle");
    fs::create_dir_all(&out_dir).map_err(|e| format!("{e}"))?;

    if found.is_empty() {
        let dest = out_dir.join("base.apk");
        fs::copy(apks_path, &dest).map_err(|e| format!("{e}"))?;
        return Ok((dest, vec![]));
    }

    let mut base: Option<PathBuf> = None;
    let mut splits = Vec::new();
    for name in &found {
        let simple = name.rsplit('/').next().unwrap_or(name);
        let dest = out_dir.join(simple);
        let mut f = archive.by_name(name).map_err(|e| format!("{e}"))?;
        let mut out = File::create(&dest).map_err(|e| format!("{e}"))?;
        std::io::copy(&mut f, &mut out).map_err(|e| format!("{e}"))?;
        drop(f);
        if simple.to_lowercase().starts_with("base") && base.is_none() {
            base = Some(dest);
        } else {
            splits.push(dest);
        }
    }
    let base = base.unwrap_or_else(|| {
        let first = splits.remove(0);
        first
    });
    splits.sort_by_key(|p| {
        let n = p
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_lowercase();
        if n.contains("install_time") || (n.contains("asset") && n.contains("pack")) {
            1
        } else {
            0
        }
    });
    Ok((base, splits))
}

struct DrmbLoaded {
    base_dir: PathBuf,
    split_names: HashSet<String>,
    drmb_zip: PathBuf,
}

fn load_drmb(drmb_path: &Path, work: &Path) -> Result<DrmbLoaded, String> {
    let mut archive =
        zip::ZipArchive::new(File::open(drmb_path).map_err(|e| format!("Cannot open .drmb: {e}"))?)
            .map_err(|_| "Invalid .drmb".to_string())?;

    let base_dir = work.join("drmb_base");
    fs::create_dir_all(&base_dir).map_err(|e| format!("{e}"))?;
    let mut split_names = HashSet::new();

    for i in 0..archive.len() {
        let mut f = archive
            .by_index(i)
            .map_err(|e| format!("Corrupt ZIP: {e}"))?;
        if f.is_dir() {
            continue;
        }
        let name = f.name().replace('\\', "/");
        if let Some(rel) = name.strip_prefix("base/") {
            if rel.is_empty() {
                continue;
            }
            let dest = base_dir.join(rel);
            if let Some(parent) = dest.parent() {
                fs::create_dir_all(parent).map_err(|e| format!("{e}"))?;
            }
            let mut out = File::create(&dest).map_err(|e| format!("{e}"))?;
            std::io::copy(&mut f, &mut out).map_err(|e| format!("{e}"))?;
        } else if let Some(rel) = name.strip_prefix("split/") {
            if !rel.is_empty() {
                split_names.insert(rel.to_string());
            }
        }
    }

    Ok(DrmbLoaded {
        base_dir,
        split_names,
        drmb_zip: drmb_path.to_path_buf(),
    })
}

fn index_dir_files(dir: &Path) -> HashMap<String, PathBuf> {
    let mut map = HashMap::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(cur) = stack.pop() {
        let entries: Vec<PathBuf> = fs::read_dir(&cur)
            .map(|r| r.filter_map(|e| e.ok()).map(|e| e.path()).collect())
            .unwrap_or_default();
        for p in entries {
            if p.is_dir() {
                stack.push(p);
            } else if let Ok(rel) = p.strip_prefix(dir) {
                map.insert(rel.to_string_lossy().replace('\\', "/"), p);
            }
        }
    }
    map
}

fn strip_split_requirements(manifest: &mut [u8]) {
    let targets = [
        "base__abi,base__density",
        "base__abi,base__density,base__language",
        "base__density,base__abi",
        "base__abi",
        "base__density",
        "base__language",
        "com.android.vending.splits.required",
    ];
    for t in targets {
        for enc in [utf8_bytes(t), utf16le_bytes(t)] {
            null_out_occurrences(manifest, &enc, 2);
        }
    }
}

fn utf8_bytes(s: &str) -> Vec<u8> {
    s.as_bytes().to_vec()
}
fn utf16le_bytes(s: &str) -> Vec<u8> {
    s.encode_utf16().flat_map(|u| u.to_le_bytes()).collect()
}

fn null_out_occurrences(buf: &mut [u8], needle: &[u8], prefix: usize) {
    if needle.is_empty() || buf.len() < needle.len() + prefix {
        return;
    }
    let mut i = prefix;
    while i + needle.len() <= buf.len() {
        if &buf[i..i + needle.len()] == needle {
            for k in 1..=prefix {
                buf[i - k] = 0;
            }
            for j in 0..needle.len() {
                buf[i + j] = 0;
            }
            i += needle.len();
        } else {
            i += 1;
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn build_apk_fast(
    app: &AppHandle,
    base_apk: &Path,
    splits: &[PathBuf],
    overrides: &HashMap<String, PathBuf>,
    split_names: &HashSet<String>,
    drmb_zip: Option<&Path>,
    _work: &Path,
    output_unsigned: &Path,
) -> Result<usize, String> {
    use zip::write::SimpleFileOptions;
    use zip::CompressionMethod;

    let mut applied = 0usize;
    let out_file = File::create(output_unsigned).map_err(|e| format!("{e}"))?;
    let mut writer = zip::ZipWriter::new(out_file);
    let mut included: HashSet<String> = HashSet::new();

    let mut base = zip::ZipArchive::new(File::open(base_apk).map_err(|e| format!("{e}"))?)
        .map_err(|_| "Cannot read base.apk".to_string())?;

    let has_splits = !splits.is_empty() || drmb_zip.is_some();

    for i in 0..base.len() {
        let mut entry = base.by_index(i).map_err(|e| format!("{e}"))?;
        if entry.is_dir() {
            continue;
        }
        let name = entry.name().replace('\\', "/");
        if overrides.contains_key(&name) && split_names.contains(&name) {}
        if !overrides.contains_key(&name) && split_names.contains(&name) {
            continue;
        }
        included.insert(name.clone());

        if name == "AndroidManifest.xml" && has_splits {
            let mut raw = Vec::new();
            if let Some(ov) = overrides.get(&name) {
                raw = fs::read(ov).map_err(|e| format!("{e}"))?;
                applied += 1;
            } else {
                entry.read_to_end(&mut raw).map_err(|e| format!("{e}"))?;
            }
            strip_split_requirements(&mut raw);
            writer
                .start_file(
                    &name,
                    SimpleFileOptions::default().compression_method(CompressionMethod::Deflated),
                )
                .map_err(|e| format!("{e}"))?;
            writer.write_all(&raw).map_err(|e| format!("{e}"))?;
            continue;
        }

        if let Some(ov) = overrides.get(&name) {
            let bytes = fs::read(ov).map_err(|e| format!("{e}"))?;
            writer
                .start_file(
                    &name,
                    SimpleFileOptions::default().compression_method(method_for(&name)),
                )
                .map_err(|e| format!("{e}"))?;
            writer.write_all(&bytes).map_err(|e| format!("{e}"))?;
            applied += 1;
        } else {
            copy_entry_verbatim(&mut writer, entry, &name)?;
        }
    }
    drop(base);

    let mut new_files: Vec<&String> = overrides
        .keys()
        .filter(|k| !included.contains(*k))
        .collect();
    new_files.sort();
    for path in new_files {
        let bytes = fs::read(&overrides[path]).map_err(|e| format!("{e}"))?;
        writer
            .start_file(
                path,
                SimpleFileOptions::default().compression_method(method_for(path)),
            )
            .map_err(|e| format!("{e}"))?;
        writer.write_all(&bytes).map_err(|e| format!("{e}"))?;
        included.insert(path.clone());
        applied += 1;
    }

    if let Some(dz) = drmb_zip {
        let mut dzip = zip::ZipArchive::new(File::open(dz).map_err(|e| format!("{e}"))?)
            .map_err(|_| "Cannot re-read .drmb".to_string())?;
        let mut names: Vec<String> = Vec::new();
        for i in 0..dzip.len() {
            if let Ok(f) = dzip.by_index(i) {
                if !f.is_dir() {
                    let n = f.name().replace('\\', "/");
                    if n.starts_with("split/") {
                        names.push(n);
                    }
                }
            }
        }
        names.sort();
        for raw in names {
            let name = raw["split/".len()..].to_string();
            if name.is_empty() || name == "AndroidManifest.xml" || name.starts_with("META-INF/") {
                continue;
            }
            if included.contains(&name) {
                continue;
            }
            if let Some(ov) = overrides.get(&name) {
                let bytes = fs::read(ov).map_err(|e| format!("{e}"))?;
                writer
                    .start_file(
                        &name,
                        SimpleFileOptions::default().compression_method(method_for(&name)),
                    )
                    .map_err(|e| format!("{e}"))?;
                writer.write_all(&bytes).map_err(|e| format!("{e}"))?;
                applied += 1;
            } else {
                let src = dzip.by_name(&raw).map_err(|e| format!("{e}"))?;
                copy_entry_verbatim(&mut writer, src, &name)?;
            }
            included.insert(name);
        }
        drop(dzip);
    }

    for split_apk in splits {
        let mut sapk = zip::ZipArchive::new(File::open(split_apk).map_err(|e| format!("{e}"))?)
            .map_err(|_| format!("Cannot read {}", split_apk.display()))?;
        let mut names: Vec<String> = Vec::new();
        for i in 0..sapk.len() {
            if let Ok(f) = sapk.by_index(i) {
                if !f.is_dir() {
                    names.push(f.name().replace('\\', "/"));
                }
            }
        }
        for name in names {
            if name == "AndroidManifest.xml" || name.starts_with("META-INF/") {
                continue;
            }
            if included.contains(&name) {
                continue;
            }
            if let Some(ov) = overrides.get(&name) {
                let bytes = fs::read(ov).map_err(|e| format!("{e}"))?;
                writer
                    .start_file(
                        &name,
                        SimpleFileOptions::default().compression_method(method_for(&name)),
                    )
                    .map_err(|e| format!("{e}"))?;
                writer.write_all(&bytes).map_err(|e| format!("{e}"))?;
                applied += 1;
            } else {
                let src = sapk.by_name(&name).map_err(|e| format!("{e}"))?;
                copy_entry_verbatim(&mut writer, src, &name)?;
            }
            included.insert(name);
        }
    }

    writer
        .finish()
        .map_err(|e| format!("Error writing APK: {e}"))?;
    emit(
        app,
        "apply",
        &format!("Merged entries: {}", included.len()),
        1,
        1,
    );
    Ok(applied)
}

fn method_for(name: &str) -> zip::CompressionMethod {
    let n = name.to_lowercase();
    if n.ends_with(".so") || n.ends_with(".arsc") || n.ends_with(".png") && n.contains("res/") {
        zip::CompressionMethod::Stored
    } else if n.ends_with(".dex") || n.ends_with(".arsc") {
        zip::CompressionMethod::Stored
    } else {
        zip::CompressionMethod::Deflated
    }
}

fn copy_entry_verbatim<W: std::io::Write + std::io::Seek>(
    writer: &mut zip::ZipWriter<W>,
    mut entry: zip::read::ZipFile<'_>,
    dest_name: &str,
) -> Result<(), String> {
    match entry.compression() {
        zip::CompressionMethod::Stored | zip::CompressionMethod::Deflated => writer
            .raw_copy_file_rename(entry, dest_name)
            .map_err(|e| format!("{e}")),
        _ => {
            writer
                .start_file(
                    dest_name,
                    zip::write::SimpleFileOptions::default()
                        .compression_method(zip::CompressionMethod::Deflated),
                )
                .map_err(|e| format!("{e}"))?;
            std::io::copy(&mut entry, writer).map_err(|e| format!("{e}"))?;
            Ok(())
        }
    }
}

fn tools_dir(app: &AppHandle) -> Result<PathBuf, String> {
    app_work_dir(app, "tools")
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ToolStatus {
    name: String,
    present: bool,
    path: Option<String>,
    hint: String,
    can_install: bool,
}

#[tauri::command]
fn tool_status(app: AppHandle) -> Result<Vec<ToolStatus>, String> {
    let tools = tools_dir(&app)?;
    let apktool = tools.join("apktool.jar");
    let signer = tools.join("uber-apk-signer.jar");
    let zsign = tools.join(if cfg!(target_os = "windows") {
        "zsign.exe"
    } else {
        "zsign"
    });
    let adb = find_adb(&app);

    Ok(vec![
        ToolStatus {
            name: "apktool".to_string(),
            present: apktool.exists(),
            path: apktool.to_str().map(|s| s.to_string()),
            hint: "Only needed if the patch includes smali/ (license bypass). Downloads itself.".to_string(),
            can_install: true,
        },
        ToolStatus {
            name: "uber-apk-signer".to_string(),
            present: signer.exists(),
            path: signer.to_str().map(|s| s.to_string()),
            hint: "v1+v2 signing of the resulting APK. If missing, jarsigner (v1) is used.".to_string(),
            can_install: true,
        },
        ToolStatus {
            name: "zsign".to_string(),
            present: zsign.exists(),
            path: zsign.to_str().map(|s| s.to_string()),
            hint: "IPA signing for iOS sideloading. Downloads itself (official v1.1.2). Optional if you export for Sideloadly/Scarlet.".to_string(),
            can_install: true,
        },
        ToolStatus {
            name: "adb".to_string(),
            present: adb.is_some(),
            path: adb,
            hint: "Direct install on Android. Put platform-tools under Tools or on the PATH.".to_string(),
            can_install: true,
        },
        ToolStatus {
            name: "java".to_string(),
            present: find_java_bundled(&app).or_else(find_java).is_some(),
            path: find_java_bundled(&app).or_else(find_java),
            hint: "Needed for apktool and signing. Already detected on this machine.".to_string(),
            can_install: true,
        },
    ])
}

fn find_java() -> Option<String> {
    if Command::new("java").arg("-version").output().is_ok() {
        return Some("java".to_string());
    }
    None
}

fn find_java_bundled(app: &AppHandle) -> Option<String> {
    if let Ok(tools) = tools_dir(app) {
        for rel in [
            #[cfg(target_os = "windows")]
            "java/bin/java.exe",
            #[cfg(not(target_os = "windows"))]
            "java/bin/java",
        ] {
            let cand = tools.join(rel);
            if cand.exists() {
                return cand.to_str().map(|s| s.to_string());
            }
        }
    }
    None
}

fn java_cmd(app: &AppHandle) -> Option<String> {
    find_java_bundled(app).or_else(find_java)
}

fn find_adb(app: &AppHandle) -> Option<String> {
    if let Ok(tools) = tools_dir(app) {
        let cand = tools
            .join("platform-tools")
            .join(if cfg!(target_os = "windows") {
                "adb.exe"
            } else {
                "adb"
            });
        if cand.exists() {
            return cand.to_str().map(|s| s.to_string());
        }
    }
    if Command::new("adb").arg("version").output().is_ok() {
        return Some("adb".to_string());
    }
    None
}

const APKTOOL_URL: &str =
    "https://github.com/iBotPeaches/Apktool/releases/download/v2.11.0/apktool_2.11.0.jar";
const PLATFORM_TOOLS_URL: &str =
    "https://dl.google.com/android/repository/platform-tools-latest-windows.zip";
const TEMURIN_JRE_URL: &str =
    "https://api.adoptium.net/v3/binary/latest/21/ga/windows/x64/jre/hotspot/normal/eclipse";
const ZSIGN_RELEASES_URL: &str = "https://github.com/zhlynn/zsign/releases";
const ZSIGN_URL: &str =
    "https://github.com/zhlynn/zsign/releases/download/v1.1.2/zsign-windows-x64.zip";
const ZSIGN_SHA256: &str = "96b5bf7029a52c67cdb78b68b3666d8ff1e31e5c435535964be2a482afc09e5c";

async fn download_zsign(app: &AppHandle, dest: &Path) -> Result<(), String> {
    if !cfg!(target_os = "windows") {
        return Err(format!(
            "zsign auto-install is only wired for Windows. Grab it from {ZSIGN_RELEASES_URL}."
        ));
    }
    let client = github_client()?;
    emit(app, "tool", "Downloading zsign v1.1.2...", 0, 1);
    let resp = client
        .get(ZSIGN_URL)
        .send()
        .await
        .map_err(|e| format!("Error downloading zsign: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("zsign download failed: {}", resp.status()));
    }
    let bytes = resp.bytes().await.map_err(|e| format!("{e}"))?;
    if sha256_hex(&bytes) != ZSIGN_SHA256 {
        return Err("zsign download failed checksum verification.".to_string());
    }
    emit(app, "tool", "Extracting zsign...", 1, 1);
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes))
        .map_err(|_| "zsign download is not a valid ZIP".to_string())?;
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("{e}"))?;
    }
    let mut placed = false;
    for i in 0..archive.len() {
        let mut f = archive
            .by_index(i)
            .map_err(|e| format!("Corrupt ZIP: {e}"))?;
        if f.is_dir() {
            continue;
        }
        let name = f.name().replace('\\', "/");
        if let Some(file) = name.rsplit('/').next() {
            if file.eq_ignore_ascii_case("zsign.exe") {
                let mut out = File::create(dest).map_err(|e| format!("{e}"))?;
                std::io::copy(&mut f, &mut out).map_err(|e| format!("{e}"))?;
                placed = true;
                break;
            }
        }
    }
    if !placed {
        return Err("zsign.exe not found inside the downloaded ZIP.".to_string());
    }
    emit(app, "tool", "zsign ready", 1, 1);
    Ok(())
}

async fn download_zip_to_dir(
    app: &AppHandle,
    label: &str,
    url: &str,
    dest_dir: &Path,
) -> Result<(), String> {
    let client = github_client()?;
    emit(app, "tool", &format!("Downloading {label}..."), 0, 1);
    let resp = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("Error downloading {label}: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("{label} download failed: {}", resp.status()));
    }
    let total = resp.content_length().unwrap_or(0);
    let bytes = resp.bytes().await.map_err(|e| format!("{e}"))?;
    emit(app, "tool", &format!("Extracting {label}..."), 1, 1);
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes))
        .map_err(|_| format!("{label} download is not a valid ZIP"))?;
    fs::create_dir_all(dest_dir).map_err(|e| format!("{e}"))?;
    let mut count = 0usize;
    for i in 0..archive.len() {
        let mut f = archive
            .by_index(i)
            .map_err(|e| format!("Corrupt ZIP: {e}"))?;
        let rel = match f.enclosed_name() {
            Some(p) => p.to_string_lossy().replace('\\', "/"),
            None => continue,
        };
        let stripped = rel.split_once('/').map(|(_, rest)| rest).unwrap_or(&rel);
        if stripped.is_empty() {
            continue;
        }
        let dest = dest_dir.join(stripped);
        if f.is_dir() {
            fs::create_dir_all(&dest).map_err(|e| format!("{e}"))?;
            continue;
        }
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent).map_err(|e| format!("{e}"))?;
        }
        let mut out = File::create(&dest).map_err(|e| format!("{e}"))?;
        std::io::copy(&mut f, &mut out).map_err(|e| format!("{e}"))?;
        count += 1;
        if total > 0 && count % 25 == 0 {
            emit(
                app,
                "tool",
                &format!("Extracting {label}... {count} files"),
                1,
                1,
            );
        }
    }
    emit(app, "tool", &format!("{label} ready ({count} files)"), 1, 1);
    Ok(())
}

async fn download_jar(app: &AppHandle, label: &str, url: &str, dest: &Path) -> Result<(), String> {
    let client = github_client()?;
    emit(app, "tool", &format!("Downloading {label}..."), 0, 1);
    let resp = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("Error downloading {label}: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("{label} download failed: {}", resp.status()));
    }
    let bytes = resp.bytes().await.map_err(|e| format!("{e}"))?;
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("{e}"))?;
    }
    fs::write(dest, &bytes).map_err(|e| format!("{e}"))?;
    emit(app, "tool", &format!("{label} ready"), 1, 1);
    Ok(())
}

async fn ensure_apktool(app: &AppHandle) -> Result<PathBuf, String> {
    let dest = tools_dir(app)?.join("apktool.jar");
    if dest.exists() {
        return Ok(dest);
    }
    download_jar(app, "apktool", APKTOOL_URL, &dest).await?;
    Ok(dest)
}

#[tauri::command]
async fn download_tool(app: AppHandle, name: String) -> Result<String, String> {
    let tools = tools_dir(&app)?;
    match name.as_str() {
        "apktool" => {
            let dest = tools.join("apktool.jar");
            download_jar(&app, "apktool", APKTOOL_URL, &dest).await?;
            Ok(dest.to_string_lossy().to_string())
        }
        "uber-apk-signer" => {
            let dest = tools.join("uber-apk-signer.jar");
            download_jar(&app, "uber-apk-signer", UBER_URL, &dest).await?;
            Ok(dest.to_string_lossy().to_string())
        }
        "adb" => {
            let dest = tools.join("platform-tools");
            download_zip_to_dir(&app, "platform-tools", PLATFORM_TOOLS_URL, &dest).await?;
            Ok(dest.to_string_lossy().to_string())
        }
        "java" => {
            let dest = tools.join("java");
            download_zip_to_dir(&app, "Temurin JRE 21", TEMURIN_JRE_URL, &dest).await?;
            Ok(dest.to_string_lossy().to_string())
        }
        "zsign" => {
            let dest = tools.join(if cfg!(target_os = "windows") {
                "zsign.exe"
            } else {
                "zsign"
            });
            download_zsign(&app, &dest).await?;
            Ok(dest.to_string_lossy().to_string())
        }
        _ => Err(format!("Unknown tool: {name}")),
    }
}

#[tauri::command]
fn import_tool_file(app: AppHandle, name: String, src_path: String) -> Result<String, String> {
    let file_name = match name.as_str() {
        "zsign" => {
            if cfg!(target_os = "windows") {
                "zsign.exe"
            } else {
                "zsign"
            }
        }
        _ => return Err(format!("Import is not supported for: {name}")),
    };
    let src = PathBuf::from(&src_path);
    if !src.exists() {
        return Err("Selected file does not exist.".to_string());
    }
    let dest = tools_dir(&app)?.join(file_name);
    fs::copy(&src, &dest).map_err(|e| format!("Could not place file: {e}"))?;
    Ok(dest.to_string_lossy().to_string())
}

const UBER_URL: &str = "https://github.com/patrickfav/uber-apk-signer/releases/download/v1.3.0/uber-apk-signer-1.3.0.jar";

async fn ensure_uber_signer(app: &AppHandle) -> Result<PathBuf, String> {
    let tools = tools_dir(app)?;
    let dest = tools.join("uber-apk-signer.jar");
    if dest.exists() {
        return Ok(dest);
    }
    emit(
        app,
        "sign",
        "Downloading uber-apk-signer (v2+v3 signing)...",
        0,
        1,
    );
    let client = github_client()?;
    let resp = client
        .get(UBER_URL)
        .send()
        .await
        .map_err(|e| format!("Signer download failed: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("Signer download failed: {}", resp.status()));
    }
    let bytes = resp.bytes().await.map_err(|e| format!("{e}"))?;
    fs::write(&dest, &bytes).map_err(|e| format!("{e}"))?;
    emit(app, "sign", "uber-apk-signer ready.", 1, 1);
    Ok(dest)
}

fn detect_android_major(app: &AppHandle) -> Option<u32> {
    let adb = find_adb(app)?;
    let devs = adb_devices(app.clone()).ok()?;
    let target = devs.devices.first()?;
    let release = adb_shell(&adb, target, &["getprop", "ro.build.version.release"]).ok()?;
    release.split('.').next()?.parse::<u32>().ok()
}

fn resolve_schemes(
    mode: &str,
    v1: bool,
    v2: bool,
    v3: bool,
    android_version: Option<String>,
    app: &AppHandle,
) -> Result<(bool, bool, bool), String> {
    if mode == "custom" {
        if !v1 && !v2 && !v3 {
            return Err("Select at least one signature scheme (v1, v2 or v3).".to_string());
        }
        return Ok((v1, v2, v3));
    }
    let major = android_version
        .as_deref()
        .and_then(|s| s.split('.').next())
        .and_then(|s| s.parse::<u32>().ok())
        .or_else(|| detect_android_major(app));
    match major {
        Some(m) if m < 9 => Ok((true, true, false)),
        Some(9) | Some(10) => Ok((true, true, true)),
        Some(m) if m >= 11 => Ok((false, true, true)),
        _ => Ok((true, true, false)),
    }
}

struct SignOutcome {
    schemes: Vec<String>,
    note: Option<String>,
}

fn ensure_debug_keystore(tools: &Path) -> Result<PathBuf, String> {
    let ks = tools.join("debug.keystore");
    if !ks.exists() {
        let out = Command::new("keytool")
            .args([
                "-genkeypair",
                "-keystore",
                ks.to_str().unwrap_or_default(),
                "-alias",
                "enma",
                "-keyalg",
                "RSA",
                "-keysize",
                "2048",
                "-validity",
                "10950",
                "-storepass",
                "enmapatcher",
                "-keypass",
                "enmapatcher",
                "-dname",
                "CN=EnmaPatcher",
            ])
            .output()
            .map_err(|e| format!("keytool not available: {e}"))?;
        if !out.status.success() {
            return Err(format!(
                "Could not generate key: {}",
                String::from_utf8_lossy(&out.stderr)
            ));
        }
    }
    Ok(ks)
}

fn verify_schemes(java: &str, uber: &Path, signed: &Path) -> Result<Vec<String>, String> {
    let vout = Command::new(java)
        .args([
            "-cp".to_string(),
            uber.to_string_lossy().to_string(),
            "com.android.apksigner.ApkSignerTool".to_string(),
            "verify".to_string(),
            "-v".to_string(),
            signed.to_string_lossy().to_string(),
        ])
        .output()
        .map_err(|e| format!("Could not run ApkSignerTool verify: {e}"))?;
    let log = format!(
        "{}{}",
        String::from_utf8_lossy(&vout.stdout),
        String::from_utf8_lossy(&vout.stderr)
    );
    let mut schemes = Vec::new();
    for n in ["v1", "v2", "v3"] {
        let marker = format!("{n} scheme");
        if log
            .lines()
            .any(|l| l.contains(&marker) && l.trim_end().ends_with("true"))
        {
            schemes.push(n.to_string());
        }
    }
    if schemes.is_empty() {
        return Err("ApkSignerTool verify found no valid signature.".to_string());
    }
    Ok(schemes)
}

fn java_base_args(near: Option<&Path>) -> Vec<String> {
    let mut args = vec!["-Xmx4g".to_string()];
    if let Some(base) = near {
        let dir = base.join(".java-tmp");
        if fs::create_dir_all(&dir).is_ok() {
            args.push(format!("-Djava.io.tmpdir={}", dir.to_string_lossy()));
        }
    }
    args
}

fn disk_full_hint(text: &str) -> Option<String> {
    let low = text.to_lowercase();
    let marks = [
        "no space left",
        "not enough space",
        "espacio en disco insuficiente",
        "espace disque insuffisant",
        "os error 112",
        "enospc",
    ];
    if marks.iter().any(|m| low.contains(m)) {
        Some(
            "The disk is full. Free space on the drive holding the output folder and the system temp dir (or move the output folder to a drive with room), then retry."
                .to_string(),
        )
    } else {
        None
    }
}

fn with_hint(context: &str, log: &str) -> String {
    match disk_full_hint(log) {
        Some(hint) => format!("{context} {hint} Details: {log}"),
        None => format!("{context} {log}"),
    }
}

fn pick_error_lines(log: &str, n: usize) -> String {
    let lines: Vec<&str> = log
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect();
    let keyed: Vec<&&str> = lines
        .iter()
        .filter(|l| {
            let low = l.to_lowercase();
            low.contains("exception")
                || low.contains("caused by")
                || low.contains("error")
                || low.contains("failed")
        })
        .collect();
    let picked: Vec<String> = if keyed.is_empty() {
        lines
            .iter()
            .rev()
            .take(n)
            .rev()
            .map(|s| s.to_string())
            .collect()
    } else {
        keyed.into_iter().take(4).map(|s| s.to_string()).collect()
    };
    picked.join(" ")
}

fn apksigner_sign(
    java: &str,
    uber: &Path,
    tools: &Path,
    unsigned: &Path,
    signed: &Path,
    v1: bool,
    v2: bool,
    v3: bool,
) -> Result<Vec<String>, String> {
    let ks = ensure_debug_keystore(tools)?;
    let flag = |b: bool| {
        if b {
            "true".to_string()
        } else {
            "false".to_string()
        }
    };
    let mut args = java_base_args(unsigned.parent());
    args.extend([
        "-cp".to_string(),
        uber.to_string_lossy().to_string(),
        "com.android.apksigner.ApkSignerTool".to_string(),
        "sign".to_string(),
        "--ks".to_string(),
        ks.to_string_lossy().to_string(),
        "--ks-pass".to_string(),
        "pass:enmapatcher".to_string(),
        "--key-pass".to_string(),
        "pass:enmapatcher".to_string(),
        "--ks-key-alias".to_string(),
        "enma".to_string(),
        "--v1-signing-enabled".to_string(),
        flag(v1),
        "--v2-signing-enabled".to_string(),
        flag(v2),
        "--v3-signing-enabled".to_string(),
        flag(v3),
        "--v4-signing-enabled".to_string(),
        "false".to_string(),
        "--out".to_string(),
        signed.to_string_lossy().to_string(),
        unsigned.to_string_lossy().to_string(),
    ]);
    let out = Command::new(java)
        .args(&args)
        .output()
        .map_err(|e| format!("Could not run ApkSignerTool: {e}"))?;
    if !out.status.success() {
        let log = format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        return Err(with_hint(
            "ApkSignerTool sign failed:",
            &pick_error_lines(&log, 4),
        ));
    }
    verify_schemes(java, uber, signed)
}

async fn sign_apk(
    app: &AppHandle,
    unsigned: &Path,
    signed: &Path,
    v1: bool,
    v2: bool,
    v3: bool,
    _via_uber: bool,
) -> Result<SignOutcome, String> {
    let tools = tools_dir(app)?;
    let mut note: Option<String> = None;

    let java = java_cmd(app)
        .ok_or("No Java available to sign the APK. Install it from the Tools tab.".to_string())?;

    let uber_path: Option<PathBuf> = match ensure_uber_signer(app).await {
        Ok(p) => Some(p),
        Err(e) => {
            note = Some(e);
            None
        }
    };

    let mut uber_ok = false;
    let uber_ks = ensure_debug_keystore(&tools).ok();
    if uber_path.is_some() {
        if let Some(uber) = uber_path.as_ref() {
            emit(app, "sign", "Signing with uber-apk-signer...", 0, 1);
            let mut args: Vec<String> = java_base_args(unsigned.parent());
            args.extend([
                "-jar".to_string(),
                uber.to_str().unwrap_or_default().to_string(),
                "--debug".to_string(),
                "-a".to_string(),
                unsigned.to_str().unwrap_or_default().to_string(),
                "--out".to_string(),
                signed
                    .parent()
                    .unwrap_or(Path::new("."))
                    .to_str()
                    .unwrap_or_default()
                    .to_string(),
                "--allowResign".to_string(),
            ]);
            if let Some(ks) = uber_ks.as_ref() {
                args.extend([
                    "--ks".to_string(),
                    ks.to_string_lossy().to_string(),
                    "--ksAlias".to_string(),
                    "enma".to_string(),
                    "--ksPass".to_string(),
                    "enmapatcher".to_string(),
                    "--ksKeyPass".to_string(),
                    "enmapatcher".to_string(),
                ]);
            }
            #[cfg(target_os = "windows")]
            {
                match ensure_zipalign(app, uber) {
                    Ok(za) => args.extend([
                        "--zipAlignPath".to_string(),
                        za.to_string_lossy().to_string(),
                    ]),
                    Err(e) => {
                        note = Some(match note.take() {
                            Some(prev) => format!("{prev} {e}"),
                            None => e,
                        });
                    }
                }
            }
            match Command::new(&java).args(&args).output() {
                Ok(out) if out.status.success() => {
                    uber_ok = true;
                }
                Ok(out) => {
                    let log = format!(
                        "{}{}",
                        String::from_utf8_lossy(&out.stdout),
                        String::from_utf8_lossy(&out.stderr)
                    );
                    let tail = pick_error_lines(&log, 3);
                    note = Some(with_hint("uber-apk-signer failed.", &tail));
                }
                Err(e) => {
                    note = Some(format!("Could not run signer: {e}."));
                }
            }
            if uber_ok {
                if let Some(parent) = signed.parent() {
                    let stem = unsigned
                        .file_stem()
                        .map(|s| s.to_string_lossy().to_string())
                        .unwrap_or_default();
                    let mut renamed = false;
                    if let Ok(entries) = fs::read_dir(parent) {
                        for entry in entries.flatten() {
                            let p = entry.path();
                            let n = p
                                .file_name()
                                .unwrap_or_default()
                                .to_string_lossy()
                                .to_string();
                            if n.contains(&stem) && n.ends_with(".idsig") {
                                let _ = fs::remove_file(&p);
                            } else if !renamed
                                && n.contains(&stem)
                                && n != unsigned
                                    .file_name()
                                    .unwrap_or_default()
                                    .to_string_lossy()
                                    .as_ref()
                                && p.extension().map(|e| e == "apk").unwrap_or(false)
                            {
                                if fs::rename(&p, signed).is_ok() {
                                    renamed = true;
                                }
                            }
                        }
                    }
                    if renamed {
                        match verify_schemes(&java, uber, signed) {
                            Ok(schemes) => {
                                let covers =
                                    [v1.then_some("v1"), v2.then_some("v2"), v3.then_some("v3")]
                                        .into_iter()
                                        .flatten()
                                        .all(|s| schemes.iter().any(|x| x == s));
                                if covers {
                                    emit(
                                        app,
                                        "sign",
                                        &format!("APK signed ({}).", schemes.join("+")),
                                        1,
                                        1,
                                    );
                                    return Ok(SignOutcome { schemes, note });
                                }
                                emit(
                                    app,
                                    "sign",
                                    &format!(
                                        "Adding requested schemes to {}...",
                                        schemes.join("+")
                                    ),
                                    0,
                                    1,
                                );
                                let stage = signed.with_extension("uber.apk");
                                let _ = fs::remove_file(&stage);
                                if fs::rename(signed, &stage).is_ok() {
                                    match apksigner_sign(
                                        &java, uber, &tools, &stage, signed, v1, v2, v3,
                                    ) {
                                        Ok(schemes2) => {
                                            let _ = fs::remove_file(&stage);
                                            emit(
                                                app,
                                                "sign",
                                                &format!("APK signed ({}).", schemes2.join("+")),
                                                1,
                                                1,
                                            );
                                            return Ok(SignOutcome {
                                                schemes: schemes2,
                                                note,
                                            });
                                        }
                                        Err(e2) => {
                                            let _ = fs::rename(&stage, signed);
                                            note = Some(format!(
                                                "Could not add requested schemes ({e2}); keeping {} build.{}",
                                                schemes.join("+"),
                                                note.map(|n| format!(" {n}")).unwrap_or_default()
                                            ));
                                            emit(
                                                app,
                                                "sign",
                                                &format!("APK signed ({}).", schemes.join("+")),
                                                1,
                                                1,
                                            );
                                            return Ok(SignOutcome { schemes, note });
                                        }
                                    }
                                }
                            }
                            Err(e) => {
                                let msg = format!("Signed output failed verification ({e}).");
                                note = Some(match note.take() {
                                    Some(prev) => format!("{prev} {msg}"),
                                    None => msg,
                                });
                            }
                        }
                    } else {
                        let msg = "uber-apk-signer produced no output.".to_string();
                        note = Some(match note.take() {
                            Some(prev) => format!("{prev} {msg}"),
                            None => msg,
                        });
                    }
                }
            }
        }
    }

    Err(note.unwrap_or_else(|| {
        "Signing failed: uber-apk-signer did not run. Connect to download it from Tools, then retry.".to_string()
    }))
}

fn defuse_split_manifest(decoded: &Path) -> Result<(), String> {
    let manifest_path = decoded.join("AndroidManifest.xml");
    let text = fs::read_to_string(&manifest_path).map_err(|e| format!("{e}"))?;
    let mut out = text.clone();
    for pat in [
        r#"\s+android:requiredSplitTypes="[^"]*""#,
        r#"\s+android:splitTypes="[^"]*""#,
        r#"\s+android:isSplitRequired="[^"]*""#,
        r#"<meta-data android:name="com\.android\.vending\.splits\.required"[^>]*/>\s*"#,
        r#"<meta-data android:name="com\.android\.vending\.splits"[^>]*/>\s*"#,
    ] {
        let re = regex::Regex::new(pat).map_err(|e| format!("{e}"))?;
        out = re.replace_all(&out, "").to_string();
    }
    if out != text {
        fs::write(&manifest_path, out).map_err(|e| format!("{e}"))?;
    }
    let res_xml = decoded.join("res").join("xml");
    if res_xml.is_dir() {
        for entry in fs::read_dir(&res_xml).map_err(|e| format!("{e}"))? {
            let p = entry.map_err(|e| format!("{e}"))?.path();
            if let Some(n) = p.file_name().and_then(|s| s.to_str()) {
                if n.starts_with("splits") && n.ends_with(".xml") {
                    fs::remove_file(&p).map_err(|e| format!("{e}"))?;
                }
            }
        }
    }
    let public_path = decoded.join("res").join("values").join("public.xml");
    if public_path.exists() {
        let pub_text = fs::read_to_string(&public_path).map_err(|e| format!("{e}"))?;
        let re = regex::Regex::new(r#"\s*<public type="xml" name="splits[^"]*"[^>]*/>"#)
            .map_err(|e| format!("{e}"))?;
        let pub_out = re.replace_all(&pub_text, "").to_string();
        if pub_out != pub_text {
            fs::write(&public_path, pub_out).map_err(|e| format!("{e}"))?;
        }
    }
    Ok(())
}
fn xml_attr_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn set_android_label(decoded: &Path, app_name: &str) -> Result<bool, String> {
    let name = app_name.trim();
    if name.is_empty() {
        return Ok(false);
    }
    let manifest_path = decoded.join("AndroidManifest.xml");
    let text = fs::read_to_string(&manifest_path).map_err(|e| format!("{e}"))?;
    let literal = format!("android:label=\"{}\"", xml_attr_escape(name));
    let mut out = text.clone();
    for r in [
        r#"android:label="@string/app_name""#,
        r#"android:label="@string/app_long_name""#,
    ] {
        out = out.replace(r, &literal);
    }
    if out != text {
        fs::write(&manifest_path, out).map_err(|e| format!("{e}"))?;
        return Ok(true);
    }
    Ok(false)
}

async fn build_apk_with_apktool(
    app: &AppHandle,
    base_apk: &Path,
    overrides: &HashMap<String, PathBuf>,
    smali: &HashMap<String, PathBuf>,
    app_name: Option<&str>,
    work: &Path,
    output_unsigned: &Path,
) -> Result<usize, String> {
    let apktool = ensure_apktool(app).await?;
    let java = java_cmd(app)
        .ok_or("Java is required to run apktool. Install it from the Tools tab.".to_string())?;
    let decoded = work.join("decoded");
    emit(app, "smali", "Decoding base.apk with apktool...", 0, 1);
    let out = Command::new(&java)
        .args([
            "-jar",
            apktool.to_str().unwrap_or_default(),
            "d",
            "-f",
            "-o",
            decoded.to_str().unwrap_or_default(),
            base_apk.to_str().unwrap_or_default(),
        ])
        .output()
        .map_err(|e| format!("Could not run apktool: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "apktool decode failed: {}",
            String::from_utf8_lossy(&out.stderr)
        ));
    }

    let mut applied = 0;
    let mut smali_dirs: Vec<PathBuf> = Vec::new();
    for entry in fs::read_dir(&decoded).map_err(|e| format!("{e}"))? {
        let p = entry.map_err(|e| format!("{e}"))?.path();
        if p.is_dir() {
            if let Some(n) = p.file_name().and_then(|s| s.to_str()) {
                if n == "smali" || n.starts_with("smali_classes") {
                    smali_dirs.push(p);
                }
            }
        }
    }
    smali_dirs.sort();
    let mut ordered_smali: Vec<&String> = smali.keys().collect();
    ordered_smali.sort();
    for rel in ordered_smali {
        let stripped = rel.strip_prefix("smali/").unwrap_or(rel);
        let mut targets: Vec<PathBuf> = smali_dirs
            .iter()
            .filter(|d| d.join(stripped).exists())
            .cloned()
            .collect();
        if targets.is_empty() {
            targets.push(decoded.join("smali"));
        }
        for target in &targets {
            let dest = target.join(stripped);
            if let Some(parent) = dest.parent() {
                fs::create_dir_all(parent).map_err(|e| format!("{e}"))?;
            }
            fs::copy(&smali[rel], &dest).map_err(|e| format!("{e}"))?;
            applied += 1;
        }
    }
    let mut ordered: Vec<&String> = overrides.keys().collect();
    ordered.sort();
    for rel in ordered {
        let dest = decoded.join(rel);
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent).map_err(|e| format!("{e}"))?;
        }
        fs::copy(&overrides[rel], &dest).map_err(|e| format!("{e}"))?;
        applied += 1;
    }
    defuse_split_manifest(&decoded)?;
    if let Some(name) = app_name.map(str::trim).filter(|s| !s.is_empty()) {
        if set_android_label(&decoded, name)? {
            emit(app, "smali", &format!("App name set to \"{name}\"."), 0, 1);
        }
    }

    emit(app, "smali", "Rebuilding APK...", 0, 1);
    let rebuilt_dir = work.join("rebuilt");
    fs::create_dir_all(&rebuilt_dir).map_err(|e| format!("{e}"))?;
    let out = Command::new(&java)
        .args([
            "-jar",
            apktool.to_str().unwrap_or_default(),
            "b",
            decoded.to_str().unwrap_or_default(),
            "-o",
            output_unsigned.to_str().unwrap_or_default(),
        ])
        .output()
        .map_err(|e| format!("Could not run apktool: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "apktool build failed: {}",
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    Ok(applied)
}

#[allow(clippy::too_many_arguments)]
fn merge_apktool_with_splits(
    app: &AppHandle,
    rebuilt_apk: &Path,
    splits: &[PathBuf],
    overrides: &HashMap<String, PathBuf>,
    split_names: &HashSet<String>,
    drmb_zip: Option<&Path>,
    output_unsigned: &Path,
) -> Result<usize, String> {
    use zip::write::SimpleFileOptions;
    use zip::CompressionMethod;

    let mut applied = 0usize;
    let mut included: HashSet<String> = HashSet::new();
    let out_file = File::create(output_unsigned).map_err(|e| format!("{e}"))?;
    let mut writer = zip::ZipWriter::new(out_file);

    let mut base = zip::ZipArchive::new(File::open(rebuilt_apk).map_err(|e| format!("{e}"))?)
        .map_err(|_| "Cannot read rebuilt APK".to_string())?;
    let has_splits = !splits.is_empty() || drmb_zip.is_some();
    let mut processed = 0usize;
    for i in 0..base.len() {
        let mut entry = base.by_index(i).map_err(|e| format!("{e}"))?;
        if entry.is_dir() {
            continue;
        }
        let name = entry.name().replace('\\', "/");
        if !overrides.contains_key(&name) && split_names.contains(&name) {
            continue;
        }
        included.insert(name.clone());
        processed += 1;
        if name == "AndroidManifest.xml" && has_splits {
            let mut raw = Vec::new();
            if let Some(ov) = overrides.get(&name) {
                raw = fs::read(ov).map_err(|e| format!("{e}"))?;
                applied += 1;
            } else {
                entry.read_to_end(&mut raw).map_err(|e| format!("{e}"))?;
            }
            strip_split_requirements(&mut raw);
            writer
                .start_file(
                    &name,
                    SimpleFileOptions::default().compression_method(CompressionMethod::Deflated),
                )
                .map_err(|e| format!("{e}"))?;
            writer.write_all(&raw).map_err(|e| format!("{e}"))?;
        } else if let Some(ov) = overrides.get(&name) {
            let bytes = fs::read(ov).map_err(|e| format!("{e}"))?;
            writer
                .start_file(
                    &name,
                    SimpleFileOptions::default().compression_method(method_for(&name)),
                )
                .map_err(|e| format!("{e}"))?;
            writer.write_all(&bytes).map_err(|e| format!("{e}"))?;
            applied += 1;
        } else {
            copy_entry_verbatim(&mut writer, entry, &name)?;
        }
        if processed % 2000 == 0 {
            emit(
                app,
                "merge",
                &format!("Merging base... {processed} entries"),
                1,
                1,
            );
        }
    }
    drop(base);

    let mut new_files: Vec<&String> = overrides
        .keys()
        .filter(|k| !included.contains(*k))
        .collect();
    new_files.sort();
    for path in new_files {
        let bytes = fs::read(&overrides[path]).map_err(|e| format!("{e}"))?;
        writer
            .start_file(
                path,
                SimpleFileOptions::default().compression_method(method_for(path)),
            )
            .map_err(|e| format!("{e}"))?;
        writer.write_all(&bytes).map_err(|e| format!("{e}"))?;
        included.insert(path.clone());
        applied += 1;
    }

    if let Some(dz) = drmb_zip {
        let mut dzip = zip::ZipArchive::new(File::open(dz).map_err(|e| format!("{e}"))?)
            .map_err(|_| "Cannot re-read .drmb".to_string())?;
        let mut names: Vec<String> = Vec::new();
        for i in 0..dzip.len() {
            if let Ok(f) = dzip.by_index(i) {
                if !f.is_dir() {
                    let n = f.name().replace('\\', "/");
                    if n.starts_with("split/") {
                        names.push(n);
                    }
                }
            }
        }
        names.sort();
        processed = 0;
        for raw in names {
            let name = raw["split/".len()..].to_string();
            if name.is_empty() || name == "AndroidManifest.xml" || name.starts_with("META-INF/") {
                continue;
            }
            if included.contains(&name) {
                continue;
            }
            if let Some(ov) = overrides.get(&name) {
                let bytes = fs::read(ov).map_err(|e| format!("{e}"))?;
                writer
                    .start_file(
                        &name,
                        SimpleFileOptions::default().compression_method(method_for(&name)),
                    )
                    .map_err(|e| format!("{e}"))?;
                writer.write_all(&bytes).map_err(|e| format!("{e}"))?;
                applied += 1;
            } else {
                let src = dzip.by_name(&raw).map_err(|e| format!("{e}"))?;
                copy_entry_verbatim(&mut writer, src, &name)?;
            }
            included.insert(name);
            processed += 1;
            if processed % 2000 == 0 {
                emit(
                    app,
                    "merge",
                    &format!("Merging .drmb... {processed} entries"),
                    1,
                    1,
                );
            }
        }
        drop(dzip);
    }

    for split_apk in splits {
        let mut sapk = zip::ZipArchive::new(File::open(split_apk).map_err(|e| format!("{e}"))?)
            .map_err(|_| format!("Cannot read {}", split_apk.display()))?;
        let mut names: Vec<String> = Vec::new();
        for i in 0..sapk.len() {
            if let Ok(f) = sapk.by_index(i) {
                if !f.is_dir() {
                    names.push(f.name().replace('\\', "/"));
                }
            }
        }
        processed = 0;
        for name in names {
            if name == "AndroidManifest.xml" || name.starts_with("META-INF/") {
                continue;
            }
            if included.contains(&name) {
                continue;
            }
            if let Some(ov) = overrides.get(&name) {
                let bytes = fs::read(ov).map_err(|e| format!("{e}"))?;
                writer
                    .start_file(
                        &name,
                        SimpleFileOptions::default().compression_method(method_for(&name)),
                    )
                    .map_err(|e| format!("{e}"))?;
                writer.write_all(&bytes).map_err(|e| format!("{e}"))?;
                applied += 1;
            } else {
                let src = sapk.by_name(&name).map_err(|e| format!("{e}"))?;
                copy_entry_verbatim(&mut writer, src, &name)?;
            }
            included.insert(name);
            processed += 1;
            if processed % 2000 == 0 {
                emit(
                    app,
                    "merge",
                    &format!("Merging split... {processed} entries"),
                    1,
                    1,
                );
            }
        }
    }

    writer
        .finish()
        .map_err(|e| format!("Error writing APK: {e}"))?;
    emit(
        app,
        "merge",
        &format!("Merged entries: {}", included.len()),
        1,
        1,
    );
    Ok(applied)
}

#[tauri::command]
async fn patch_android(
    app: AppHandle,
    req: AndroidPatchRequest,
) -> Result<AndroidPatchResult, String> {
    emit(&app, "start", "Starting Android patch...", 0, 7);

    let apks_path = PathBuf::from(&req.apks_path);
    if !apks_path.exists() {
        return Err(".apks / .apk file not found.".to_string());
    }

    let work = match &req.work_dir {
        Some(d) if !d.trim().is_empty() => {
            let p = PathBuf::from(d);
            fs::create_dir_all(&p).map_err(|e| format!("Could not create work dir: {e}"))?;
            p
        }
        _ => unique_work_dir(&app, "android")?,
    };

    emit(&app, "bundle", "Extracting base + splits...", 1, 7);
    let (base_apk, splits) = extract_apks_bundle(&apks_path, &work)?;
    cli_phase("bundle extracted");

    let drmb_required = !splits.is_empty();
    let drmb_path = req
        .drmb_path
        .clone()
        .filter(|s| !s.trim().is_empty())
        .map(PathBuf::from);
    if drmb_required && drmb_path.is_none() {
        return Err("DRMB_REQUIRED:An .apks needs your own .drmb (bypass + asset pack). Select it to continue.".to_string());
    }

    let mut warning: Option<String> = None;
    if splits.is_empty() {
        let analysis = inspect_single_apk(req.apks_path.clone())?;
        if analysis.has_split_requirement {
            warning = Some(
                "APK declares required splits: may fail without the full package (.apks)."
                    .to_string(),
            );
        }
        if drmb_path.is_none() && !req.force_single_apk {
            return Err("SINGLE_APK_CHECK:You passed a single .apk. Use 'Check' to verify whether it is already patched; if it is clean, buy the game and export the full .apks (base + splits) before patching.".to_string());
        }
    }

    emit(&app, "drmb", "Loading .drmb...", 2, 7);
    let drmb = match &drmb_path {
        Some(p) => Some(load_drmb(p, &work)?),
        None => None,
    };
    cli_phase("drmb loaded");

    emit(&app, "download", "Downloading mods...", 3, 7);
    let mods_dir = work.join("mods");
    fs::create_dir_all(&mods_dir).map_err(|e| format!("{e}"))?;
    let dl = download_mods_to_dir(&app, &req.mods, &mods_dir, "android").await?;
    cli_phase("mods ready");
    let cfg = dl.config;
    let mod_files = dl.files;
    if !dl.skipped.is_empty() {
        let msg = format!(
            "Skipped {} mod(s) not enabled for Android: {}",
            dl.skipped.len(),
            dl.skipped.join(", ")
        );
        warning = Some(match warning.take() {
            Some(prev) => format!("{prev} {msg}"),
            None => msg,
        });
    }

    emit(&app, "merge", "Merging patch...", 4, 7);
    let mut overrides: HashMap<String, PathBuf> = HashMap::new();
    if let Some(d) = &drmb {
        for (rel, abs) in index_dir_files(&d.base_dir) {
            overrides.insert(rel, abs);
        }
    }
    for (rel, abs) in index_dir_files(&mods_dir) {
        if rel == "enmapatcher.cfg.json" {
            continue;
        }
        overrides.insert(rel, abs);
    }
    let smali_keys: Vec<String> = overrides
        .keys()
        .filter(|k| k.starts_with("smali/"))
        .cloned()
        .collect();
    let mut smali_map: HashMap<String, PathBuf> = HashMap::new();
    for k in &smali_keys {
        if let Some(v) = overrides.remove(k) {
            smali_map.insert(k.clone(), v);
        }
    }

    let split_names: HashSet<String> = drmb
        .as_ref()
        .map(|d| d.split_names.clone())
        .unwrap_or_default();

    let display_name: Option<String> = req
        .app_name
        .clone()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .or_else(|| {
            cfg.app_name
                .clone()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
        });

    let unsigned = work.join("patched_unsigned.apk");
    let (applied, used_apktool, apktool_warning) = if !smali_map.is_empty() {
        emit(
            &app,
            "smali",
            &format!("Smali patch detected ({} files).", smali_keys.len()),
            5,
            7,
        );
        let rebuilt = work.join("rebuilt.apk");
        let n = match build_apk_with_apktool(
            &app,
            &base_apk,
            &overrides,
            &smali_map,
            display_name.as_deref(),
            &work,
            &rebuilt,
        )
        .await
        {
            Ok(n) => n,
            Err(e) => {
                if e.starts_with("APKTOOL_MISSING") {
                    return Err(e);
                }
                return Err(e);
            }
        };
        cli_phase("rebuild done");
        emit(
            &app,
            "apply",
            "Merging splits into the rebuilt APK...",
            5,
            7,
        );
        cli_phase("merge start");
        let m = merge_apktool_with_splits(
            &app,
            &rebuilt,
            &splits,
            &overrides,
            &split_names,
            drmb.as_ref().map(|d| d.drmb_zip.as_path()),
            &unsigned,
        )?;
        cli_phase("merge done");
        (n + m, true, None)
    } else {
        emit(
            &app,
            "apply",
            "Applying ZIP replacements and merging splits...",
            5,
            7,
        );
        let n = build_apk_fast(
            &app,
            &base_apk,
            &splits,
            &overrides,
            &split_names,
            drmb.as_ref().map(|d| d.drmb_zip.as_path()),
            &work,
            &unsigned,
        )?;
        (n, false, None)
    };
    if let Some(w) = apktool_warning {
        warning = Some(match warning.take() {
            Some(prev) => format!("{prev} {w}"),
            None => w,
        });
    }

    emit(&app, "sign", "Signing APK...", 6, 7);
    cli_phase("sign start");
    let out_dir = resolve_output_dir(&app, "android")?;
    let stem = apks_path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "juego".to_string());
    let custom = req.output_name.clone().filter(|s| !s.trim().is_empty());
    let out_name = custom.unwrap_or_else(|| format!("{stem}_patched.apk"));
    let signed = out_dir.join(out_name);
    let (sv1, sv2, sv3) = resolve_schemes(
        &req.sign_mode,
        req.sign_v1,
        req.sign_v2,
        req.sign_v3,
        req.android_version.clone(),
        &app,
    )?;
    let outcome = sign_apk(
        &app,
        &unsigned,
        &signed,
        sv1,
        sv2,
        sv3,
        req.sign_mode != "custom" && sv1 && sv2 && sv3,
    )
    .await?;
    let sign_schemes = outcome.schemes;
    if let Some(note) = outcome.note {
        warning = Some(match warning.take() {
            Some(prev) => format!("{prev} {note}"),
            None => note,
        });
    }

    let _ = fs::remove_dir_all(&work);
    emit(&app, "done", "Android patch complete.", 7, 7);

    let _ = cfg;
    let _ = mod_files;
    Ok(AndroidPatchResult {
        output_path: signed.to_string_lossy().to_string(),
        total_overrides: applied,
        smali_files: smali_keys.len(),
        used_apktool,
        signed: true,
        sign_schemes,
        warning,
        app_name: display_name,
    })
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct IosPatchRequest {
    ipa_path: String,
    mods: Vec<ModSpec>,
    #[serde(default)]
    output_name: Option<String>,
    #[serde(default)]
    work_dir: Option<String>,
    #[serde(default)]
    app_name: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct IosPatchResult {
    output_path: String,
    app_dir: String,
    bundle_id: Option<String>,
    total_overrides: usize,
    stripped: usize,
    warning: Option<String>,
    app_name: Option<String>,
}

fn is_stale_ios_signature(app_dir: &str, name: &str) -> bool {
    let rel = name
        .strip_prefix(app_dir)
        .and_then(|s| s.strip_prefix('/'))
        .unwrap_or(name);
    if rel == "_CodeSignature"
        || rel.starts_with("_CodeSignature/")
        || rel == "embedded.mobileprovision"
        || rel.starts_with("SC_Info/")
    {
        return true;
    }
    if rel
        .split('/')
        .any(|seg| seg == "_CodeSignature" || seg == "SC_Info")
    {
        return true;
    }
    if rel.rsplit('/').next() == Some("embedded.mobileprovision") {
        return true;
    }
    false
}

fn customize_ios_plist(plist_bytes: &[u8], app_name: Option<&str>) -> Option<Vec<u8>> {
    let is_binary = plist_bytes.starts_with(b"bplist");
    let mut dict = match plist::Value::from_reader(std::io::Cursor::new(plist_bytes)) {
        Ok(plist::Value::Dictionary(d)) => d,
        _ => return None,
    };
    let mut changed = false;
    if dict.remove("UISupportedDevices").is_some() {
        changed = true;
    }
    if let Some(name) = app_name.map(str::trim).filter(|s| !s.is_empty()) {
        for key in ["CFBundleDisplayName", "CFBundleName"] {
            let fresh = plist::Value::String(name.to_string());
            if dict.get(key) != Some(&fresh) {
                dict.insert(key.to_string(), fresh);
                changed = true;
            }
        }
    }
    if !changed {
        return None;
    }
    let value = plist::Value::Dictionary(dict);
    let mut out = Vec::new();
    let ok = if is_binary {
        plist::to_writer_binary(&mut out, &value).is_ok()
    } else {
        plist::to_writer_xml(&mut out, &value).is_ok()
    };
    if ok {
        Some(out)
    } else {
        None
    }
}

fn ios_main_executable(archive: &mut zip::ZipArchive<File>, app_dir: &str) -> Option<String> {
    let plist_name = format!("{app_dir}/Info.plist");
    let mut entry = archive.by_name(&plist_name).ok()?;
    let mut bytes = Vec::new();
    entry.read_to_end(&mut bytes).ok()?;
    drop(entry);
    let value = plist::Value::from_reader(std::io::Cursor::new(&bytes)).ok()?;
    let dict = value.into_dictionary()?;
    let exe = dict.get("CFBundleExecutable")?.as_string()?.to_string();
    Some(format!("{app_dir}/{exe}"))
}

fn collect_mod_files_indexed(mods_dir: &Path) -> HashMap<String, PathBuf> {
    index_dir_files(mods_dir)
        .into_iter()
        .filter(|(k, _)| k != "enmapatcher.cfg.json")
        .collect()
}

#[tauri::command]
async fn patch_ios(app: AppHandle, req: IosPatchRequest) -> Result<IosPatchResult, String> {
    emit(&app, "start", "Starting iOS patch...", 0, 5);
    let ipa_path = PathBuf::from(&req.ipa_path);
    if !ipa_path.exists() {
        return Err(".ipa file not found.".to_string());
    }
    let info = inspect_ipa(req.ipa_path.clone())?;
    let work = match &req.work_dir {
        Some(d) if !d.trim().is_empty() => {
            let p = PathBuf::from(d);
            fs::create_dir_all(&p).map_err(|e| format!("Could not create work dir: {e}"))?;
            p
        }
        _ => unique_work_dir(&app, "ios")?,
    };

    emit(&app, "download", "Downloading mods...", 1, 5);
    let mods_dir = work.join("mods");
    fs::create_dir_all(&mods_dir).map_err(|e| format!("{e}"))?;
    let dl = download_mods_to_dir(&app, &req.mods, &mods_dir, "ios").await?;
    let mut warnings: Vec<String> = Vec::new();
    if !dl.skipped.is_empty() {
        warnings.push(format!(
            "Skipped {} mod(s) not enabled for iOS: {}",
            dl.skipped.len(),
            dl.skipped.join(", ")
        ));
    }
    if let Some(game_v) = info.version.as_deref() {
        for r in &dl.reports {
            if r.config.incompatible_versions.iter().any(|v| v == game_v) {
                warnings.push(format!(
                    "{} is incompatible with game version {}",
                    r.label, game_v
                ));
            }
        }
    }
    let mod_index = collect_mod_files_indexed(&mods_dir);

    emit(&app, "apply", "Patching the IPA...", 2, 5);
    let mut archive = zip::ZipArchive::new(File::open(&ipa_path).map_err(|e| format!("{e}"))?)
        .map_err(|_| "Invalid IPA".to_string())?;

    let mut entry_names: Vec<String> = Vec::new();
    for i in 0..archive.len() {
        if let Ok(f) = archive.by_index(i) {
            if !f.is_dir() {
                entry_names.push(f.name().replace('\\', "/"));
            }
        }
    }
    let entry_set: HashSet<&str> = entry_names.iter().map(|s| s.as_str()).collect();
    let main_exe = ios_main_executable(&mut archive, &info.app_dir);
    if main_exe.is_none() {
        warnings.push("Could not determine the main executable from Info.plist.".to_string());
    }

    let mut dest_for_mod: HashMap<String, String> = HashMap::new();
    let should_rename = dl.config.rename_assets.unwrap_or(true);
    for mod_rel in mod_index.keys() {
        let mut candidates = Vec::new();
        if should_rename {
            if let Some(stripped) = mod_rel.strip_prefix("assets/") {
                candidates.push(format!("{}/data/{}", info.app_dir, stripped));
            }
        }
        candidates.push(mod_rel.clone());
        candidates.push(format!("{}/{}", info.app_dir, mod_rel));
        let mut chosen: Option<String> = None;
        for c in &candidates {
            if entry_set.contains(c.as_str()) {
                chosen = Some(c.clone());
                break;
            }
        }
        if chosen.is_none() && mod_rel.starts_with("Payload/") {
            chosen = Some(mod_rel.clone());
        }
        if chosen.is_none() {
            chosen = Some(format!("{}/{}", info.app_dir, mod_rel));
        }
        dest_for_mod.insert(mod_rel.clone(), chosen.unwrap());
    }

    let mut dest_bytes: HashMap<String, Vec<u8>> = HashMap::new();
    for (mod_rel, dest) in &dest_for_mod {
        if is_stale_ios_signature(&info.app_dir, dest) {
            continue;
        }
        let bytes = fs::read(&mod_index[mod_rel]).map_err(|e| format!("{e}"))?;
        dest_bytes.insert(dest.clone(), bytes);
    }

    let display_name: Option<String> = req
        .app_name
        .clone()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .or_else(|| {
            dl.config
                .app_name
                .clone()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
        });
    {
        let plist_name = format!("{}/Info.plist", info.app_dir);
        let base: Option<Vec<u8>> = dest_bytes.get(&plist_name).cloned().or_else(|| {
            archive.by_name(&plist_name).ok().and_then(|mut f| {
                let mut b = Vec::new();
                f.read_to_end(&mut b).ok().map(|_| b)
            })
        });
        match base {
            Some(bytes) => {
                if let Some(fixed) = customize_ios_plist(&bytes, display_name.as_deref()) {
                    dest_bytes.insert(plist_name, fixed);
                    emit(&app, "apply", "Info.plist updated.", 1, 1);
                }
            }
            None => warnings.push("Info.plist not found in IPA.".to_string()),
        }
    }
    let warning = if warnings.is_empty() {
        None
    } else {
        Some(warnings.join(" "))
    };

    let out_dir = resolve_output_dir(&app, "ios")?;
    let stem = ipa_path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "juego".to_string());
    let out_name = req
        .output_name
        .clone()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| format!("{stem}_patched.ipa"));
    let out_path = out_dir.join(out_name);
    let mut stripped = 0usize;

    {
        use zip::write::SimpleFileOptions;
        use zip::CompressionMethod;
        let out_file = File::create(&out_path).map_err(|e| format!("{e}"))?;
        let mut writer = zip::ZipWriter::new(out_file);
        let mut written: HashSet<String> = HashSet::new();

        for i in 0..archive.len() {
            let mut entry = archive.by_index(i).map_err(|e| format!("{e}"))?;
            let name = entry.name().replace('\\', "/");
            if is_stale_ios_signature(&info.app_dir, &name) {
                stripped += 1;
                continue;
            }
            if entry.is_dir() {
                let perms = entry.unix_mode().unwrap_or(0o755);
                writer
                    .add_directory(
                        name.clone(),
                        SimpleFileOptions::default().unix_permissions(perms),
                    )
                    .map_err(|e| format!("{e}"))?;
                written.insert(name);
                continue;
            }
            let method = entry.compression();
            let mut perms = entry.unix_mode().unwrap_or(0o644);
            if main_exe.as_deref() == Some(name.as_str()) {
                perms |= 0o111;
            }
            let opts = SimpleFileOptions::default().unix_permissions(perms);
            if let Some(repl) = dest_bytes.get(&name) {
                let m = if method == CompressionMethod::Stored {
                    CompressionMethod::Stored
                } else {
                    CompressionMethod::Deflated
                };
                writer
                    .start_file(name.clone(), opts.compression_method(m))
                    .map_err(|e| format!("{e}"))?;
                writer.write_all(repl).map_err(|e| format!("{e}"))?;
            } else {
                writer
                    .start_file(name.clone(), opts.compression_method(method))
                    .map_err(|e| format!("{e}"))?;
                std::io::copy(&mut entry, &mut writer).map_err(|e| format!("{e}"))?;
            }
            written.insert(name);
        }
        let mut extra: Vec<&String> = dest_bytes
            .keys()
            .filter(|k| !written.contains(*k))
            .collect();
        extra.sort();
        for dest in extra {
            let mut perms = 0o644;
            if main_exe.as_deref() == Some(dest.as_str()) {
                perms |= 0o111;
            }
            writer
                .start_file(
                    dest,
                    SimpleFileOptions::default()
                        .unix_permissions(perms)
                        .compression_method(zip::CompressionMethod::Deflated),
                )
                .map_err(|e| format!("{e}"))?;
            writer
                .write_all(&dest_bytes[dest])
                .map_err(|e| format!("{e}"))?;
        }
        writer
            .finish()
            .map_err(|e| format!("Error writing IPA: {e}"))?;
    }

    if let Some(exe) = &main_exe {
        let mut check = zip::ZipArchive::new(File::open(&out_path).map_err(|e| format!("{e}"))?)
            .map_err(|_| "Cannot re-read patched IPA".to_string())?;
        let mode = check
            .by_name(exe)
            .map(|e| e.unix_mode().unwrap_or(0))
            .unwrap_or(0);
        if mode & 0o111 == 0 {
            return Err(format!(
                "Patched IPA lost the executable bit on {exe}; refusing to hand out a build that iOS cannot launch."
            ));
        }
    }

    let applied = dest_bytes.len();
    let _ = fs::remove_dir_all(&work);
    emit(&app, "done", "iOS patch complete.", 5, 5);

    Ok(IosPatchResult {
        output_path: out_path.to_string_lossy().to_string(),
        app_dir: info.app_dir,
        bundle_id: info.bundle_id,
        total_overrides: applied,
        stripped,
        warning,
        app_name: display_name,
    })
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
struct AppleAccount {
    #[serde(default)]
    id: String,
    #[serde(default)]
    label: String,
    #[serde(default)]
    apple_id: String,
    #[serde(default)]
    app_password: String,
    #[serde(default)]
    note: String,
    #[serde(default)]
    p12_password: String,
    #[serde(default)]
    has_identity: bool,
}

#[derive(Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct AppleAccountStore {
    #[serde(default)]
    accounts: Vec<AppleAccount>,
    #[serde(default)]
    active_id: String,
}

fn accounts_path(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(app_work_dir(app, "account")?.join("apple_accounts.json"))
}

fn obfuscate(s: &str) -> String {
    const KEY: u8 = 0x5A;
    s.bytes()
        .map(|b| format!("{:02x}", b ^ KEY))
        .collect::<String>()
}

fn deobfuscate(s: &str) -> String {
    const KEY: u8 = 0x5A;
    let bytes: Vec<u8> = (0..s.len())
        .step_by(2)
        .filter_map(|i| u8::from_str_radix(&s[i..i + 2.min(s.len() - i)], 16).ok())
        .map(|b| b ^ KEY)
        .collect();
    String::from_utf8_lossy(&bytes).to_string()
}

fn account_signing_paths(app: &AppHandle, id: &str) -> Result<(PathBuf, PathBuf), String> {
    let safe: String = id
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect();
    if safe.is_empty() {
        return Err("Invalid account id.".to_string());
    }
    let dir = app_work_dir(app, "account")?;
    Ok((
        dir.join(format!("{safe}.p12")),
        dir.join(format!("{safe}.mobileprovision")),
    ))
}

fn identity_present(app: &AppHandle, id: &str) -> bool {
    account_signing_paths(app, id)
        .map(|(p12, prov)| p12.exists() && prov.exists())
        .unwrap_or(false)
}

fn legacy_account_path(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(app_work_dir(app, "account")?.join("apple_account.json"))
}

fn read_account_store(app: &AppHandle) -> Result<AppleAccountStore, String> {
    let p = accounts_path(app)?;
    if !p.exists() {
        let legacy = legacy_account_path(app)?;
        if legacy.exists() {
            if let Ok(text) = fs::read_to_string(&legacy) {
                let v: serde_json::Value = serde_json::from_str(&text).unwrap_or_default();
                let apple_id = v
                    .get("apple_id")
                    .and_then(|x| x.as_str())
                    .unwrap_or_default();
                if !apple_id.is_empty() {
                    let acc = AppleAccount {
                        id: "acc-1".to_string(),
                        label: v
                            .get("note")
                            .and_then(|x| x.as_str())
                            .unwrap_or(apple_id)
                            .to_string(),
                        apple_id: apple_id.to_string(),
                        app_password: deobfuscate(
                            v.get("app_password")
                                .and_then(|x| x.as_str())
                                .unwrap_or_default(),
                        ),
                        note: v
                            .get("note")
                            .and_then(|x| x.as_str())
                            .unwrap_or_default()
                            .to_string(),
                        p12_password: String::new(),
                        has_identity: false,
                    };
                    let store = AppleAccountStore {
                        accounts: vec![acc],
                        active_id: "acc-1".to_string(),
                    };
                    write_account_store(app, &store)?;
                    let _ = fs::remove_file(&legacy);
                    return Ok(store);
                }
            }
            let _ = fs::remove_file(&legacy);
        }
        return Ok(AppleAccountStore::default());
    }
    let text = fs::read_to_string(&p).map_err(|e| format!("{e}"))?;
    let v: serde_json::Value = serde_json::from_str(&text).unwrap_or_default();
    let mut store = AppleAccountStore::default();
    if let Some(arr) = v.get("accounts").and_then(|x| x.as_array()) {
        for a in arr {
            let id = a
                .get("id")
                .and_then(|x| x.as_str())
                .unwrap_or_default()
                .to_string();
            store.accounts.push(AppleAccount {
                id: id.clone(),
                label: a
                    .get("label")
                    .and_then(|x| x.as_str())
                    .unwrap_or_default()
                    .to_string(),
                apple_id: a
                    .get("apple_id")
                    .and_then(|x| x.as_str())
                    .unwrap_or_default()
                    .to_string(),
                app_password: deobfuscate(
                    a.get("app_password")
                        .and_then(|x| x.as_str())
                        .unwrap_or_default(),
                ),
                note: a
                    .get("note")
                    .and_then(|x| x.as_str())
                    .unwrap_or_default()
                    .to_string(),
                p12_password: deobfuscate(
                    a.get("p12_password")
                        .and_then(|x| x.as_str())
                        .unwrap_or_default(),
                ),
                has_identity: false,
            });
        }
    }
    for a in store.accounts.iter_mut() {
        a.has_identity = identity_present(app, &a.id);
    }
    store.active_id = v
        .get("active_id")
        .and_then(|x| x.as_str())
        .unwrap_or_default()
        .to_string();
    if !store.accounts.iter().any(|a| a.id == store.active_id) {
        store.active_id = store
            .accounts
            .first()
            .map(|a| a.id.clone())
            .unwrap_or_default();
    }
    Ok(store)
}

fn write_account_store(app: &AppHandle, store: &AppleAccountStore) -> Result<(), String> {
    let p = accounts_path(app)?;
    let arr: Vec<serde_json::Value> = store
        .accounts
        .iter()
        .map(|a| {
            serde_json::json!({
                "id": a.id,
                "label": a.label,
                "apple_id": a.apple_id,
                "app_password": obfuscate(&a.app_password),
                "note": a.note,
                "p12_password": obfuscate(&a.p12_password),
            })
        })
        .collect();
    let stored = serde_json::json!({ "accounts": arr, "active_id": store.active_id });
    fs::write(
        &p,
        serde_json::to_string_pretty(&stored).unwrap_or_default(),
    )
    .map_err(|e| format!("Could not save accounts: {e}"))?;
    Ok(())
}

#[tauri::command]
fn list_apple_accounts(app: AppHandle) -> Result<AppleAccountStore, String> {
    read_account_store(&app)
}

#[tauri::command]
fn save_apple_account(app: AppHandle, account: AppleAccount) -> Result<AppleAccountStore, String> {
    if account.apple_id.trim().is_empty() {
        return Err("Apple ID is empty.".to_string());
    }
    let mut store = read_account_store(&app)?;
    let mut acc = account;
    acc.apple_id = acc.apple_id.trim().to_string();
    if acc.label.trim().is_empty() {
        acc.label = acc.apple_id.clone();
    } else {
        acc.label = acc.label.trim().to_string();
    }
    if acc.id.trim().is_empty() {
        acc.id = format!("acc-{}", chrono::Local::now().format("%Y%m%d%H%M%S"));
    }
    match store.accounts.iter_mut().find(|a| a.id == acc.id) {
        Some(existing) => {
            if acc.p12_password.is_empty() {
                acc.p12_password = existing.p12_password.clone();
            }
            *existing = acc.clone();
        }
        None => store.accounts.push(acc.clone()),
    }
    if store.active_id.is_empty() {
        store.active_id = acc.id.clone();
    }
    write_account_store(&app, &store)?;
    read_account_store(&app)
}

#[tauri::command]
fn delete_apple_account(app: AppHandle, id: String) -> Result<AppleAccountStore, String> {
    let mut store = read_account_store(&app)?;
    store.accounts.retain(|a| a.id != id);
    if store.active_id == id {
        store.active_id = store
            .accounts
            .first()
            .map(|a| a.id.clone())
            .unwrap_or_default();
    }
    if let Ok((p12, prov)) = account_signing_paths(&app, &id) {
        let _ = fs::remove_file(p12);
        let _ = fs::remove_file(prov);
    }
    write_account_store(&app, &store)?;
    read_account_store(&app)
}

#[tauri::command]
fn import_signing_identity(
    app: AppHandle,
    id: String,
    p12_src: String,
    prov_src: String,
    p12_password: String,
) -> Result<AppleAccountStore, String> {
    let mut store = read_account_store(&app)?;
    let acc = store
        .accounts
        .iter_mut()
        .find(|a| a.id == id)
        .ok_or("Unknown account.".to_string())?;
    let (p12_dst, prov_dst) = account_signing_paths(&app, &id)?;
    let p12 = PathBuf::from(&p12_src);
    let prov = PathBuf::from(&prov_src);
    if !p12.exists() {
        return Err("The .p12 file does not exist.".to_string());
    }
    if !prov.exists() {
        return Err("The .mobileprovision file does not exist.".to_string());
    }
    fs::copy(&p12, &p12_dst).map_err(|e| format!("Could not copy .p12: {e}"))?;
    fs::copy(&prov, &prov_dst).map_err(|e| format!("Could not copy profile: {e}"))?;
    acc.p12_password = p12_password;
    acc.has_identity = true;
    write_account_store(&app, &store)?;
    read_account_store(&app)
}

#[tauri::command]
fn delete_signing_identity(app: AppHandle, id: String) -> Result<AppleAccountStore, String> {
    let mut store = read_account_store(&app)?;
    let acc = store
        .accounts
        .iter_mut()
        .find(|a| a.id == id)
        .ok_or("Unknown account.".to_string())?;
    if let Ok((p12, prov)) = account_signing_paths(&app, &id) {
        let _ = fs::remove_file(p12);
        let _ = fs::remove_file(prov);
    }
    acc.p12_password.clear();
    acc.has_identity = false;
    write_account_store(&app, &store)?;
    read_account_store(&app)
}

#[cfg(target_os = "windows")]
fn ensure_zipalign(app: &AppHandle, uber: &Path) -> Result<PathBuf, String> {
    let tools = tools_dir(app)?;
    let exe = tools.join("zipalign.exe");
    let dll = tools.join("libwinpthread-1.dll");
    if exe.exists() && dll.exists() {
        return Ok(exe);
    }
    emit(app, "sign", "Extracting zipalign...", 0, 1);
    let mut archive =
        zip::ZipArchive::new(File::open(uber).map_err(|e| format!("Cannot open signer: {e}"))?)
            .map_err(|_| "Cannot read signer".to_string())?;
    let mut got_exe = false;
    let mut got_dll = false;
    for i in 0..archive.len() {
        let mut f = archive.by_index(i).map_err(|e| format!("{e}"))?;
        if f.is_dir() {
            continue;
        }
        let n = f.name().replace('\\', "/");
        let dest = if n.starts_with("win-zipalign") && n.ends_with(".exe") {
            got_exe = true;
            Some(exe.clone())
        } else if n.ends_with("libwinpthread-1.dll") {
            got_dll = true;
            Some(dll.clone())
        } else {
            None
        };
        if let Some(d) = dest {
            let mut out = File::create(&d).map_err(|e| format!("{e}"))?;
            std::io::copy(&mut f, &mut out).map_err(|e| format!("{e}"))?;
        }
    }
    if !(got_exe && got_dll) {
        return Err("zipalign not found inside uber-apk-signer.".to_string());
    }
    if Command::new(&exe).arg("--version").output().is_err() {
        let _ = fs::remove_file(&exe);
        return Err(
            "zipalign cannot start (antivirus is likely blocking it). Restore it from quarantine or exclude the Tools folder, then retry."
                .to_string(),
        );
    }
    Ok(exe)
}

fn zsign_bin(app: &AppHandle) -> Result<PathBuf, String> {
    let bin = tools_dir(app)?.join(if cfg!(target_os = "windows") {
        "zsign.exe"
    } else {
        "zsign"
    });
    if bin.exists() {
        Ok(bin)
    } else {
        Err("zsign is missing. Install it from the Tools tab.".to_string())
    }
}

fn zsign_args(
    zsign: &Path,
    p12: &Path,
    password: &str,
    prov: &Path,
    input: &Path,
    output: &Path,
) -> Vec<String> {
    let mut args = vec![
        "-k".to_string(),
        p12.to_string_lossy().to_string(),
        "-m".to_string(),
        prov.to_string_lossy().to_string(),
        "-z".to_string(),
        "9".to_string(),
        "-o".to_string(),
        output.to_string_lossy().to_string(),
    ];
    if !password.is_empty() {
        args.push("-p".to_string());
        args.push(password.to_string());
    }
    args.push(input.to_string_lossy().to_string());
    let _ = zsign;
    args
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct IosSignResult {
    signed_path: String,
    account_id: String,
}

#[tauri::command]
async fn sign_ios(
    app: AppHandle,
    ipa_path: String,
    account_id: String,
    output_name: Option<String>,
) -> Result<IosSignResult, String> {
    let zsign = zsign_bin(&app)?;
    let store = read_account_store(&app)?;
    let acc = if account_id.trim().is_empty() {
        store
            .accounts
            .iter()
            .find(|a| a.id == store.active_id)
            .ok_or("No active Apple account. Add one on the Account tab.".to_string())?
    } else {
        store
            .accounts
            .iter()
            .find(|a| a.id == account_id)
            .ok_or("Unknown account.".to_string())?
    };
    let (p12, prov) = account_signing_paths(&app, &acc.id)?;
    if !p12.exists() || !prov.exists() {
        return Err("This account has no signing identity. Import the .p12 and .mobileprovision on the Account tab.".to_string());
    }
    let input = PathBuf::from(&ipa_path);
    if !input.exists() {
        return Err(".ipa file not found.".to_string());
    }
    let out_dir = resolve_output_dir(&app, "ios")?;
    let stem = input
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "juego".to_string());
    let out_name = output_name
        .clone()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| format!("{stem}_signed.ipa"));
    let output = out_dir.join(out_name);

    emit(&app, "sign", "Signing IPA with zsign...", 0, 1);
    let args = zsign_args(&zsign, &p12, &acc.p12_password, &prov, &input, &output);
    let out = Command::new(&zsign)
        .args(&args)
        .output()
        .map_err(|e| format!("Could not run zsign: {e}"))?;
    if !out.status.success() {
        let log = format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        let tail: String = log
            .lines()
            .rev()
            .take(5)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect::<Vec<_>>()
            .join(" ");
        return Err(format!("zsign failed: {tail}"));
    }
    if !output.exists() {
        return Err("zsign produced no output.".to_string());
    }
    emit(&app, "sign", "IPA signed.", 1, 1);
    Ok(IosSignResult {
        signed_path: output.to_string_lossy().to_string(),
        account_id: acc.id.clone(),
    })
}

#[tauri::command]
fn set_active_apple_account(app: AppHandle, id: String) -> Result<AppleAccountStore, String> {
    let mut store = read_account_store(&app)?;
    if !store.accounts.iter().any(|a| a.id == id) {
        return Err("Unknown account.".to_string());
    }
    store.active_id = id;
    write_account_store(&app, &store)?;
    read_account_store(&app)
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AdbDevices {
    adb: Option<String>,
    devices: Vec<String>,
    raw: String,
}

#[tauri::command]
fn adb_devices(app: AppHandle) -> Result<AdbDevices, String> {
    let adb =
        find_adb(&app).ok_or("adb not found. Download platform-tools or add adb to the PATH.")?;
    let out = Command::new(&adb)
        .arg("devices")
        .output()
        .map_err(|e| format!("Could not run adb: {e}"))?;
    let raw = String::from_utf8_lossy(&out.stdout).to_string();
    let mut devices = Vec::new();
    for line in raw.lines().skip(1) {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 2 && parts[1] == "device" {
            devices.push(parts[0].to_string());
        }
    }
    Ok(AdbDevices {
        adb: Some(adb),
        devices,
        raw,
    })
}

#[tauri::command]
fn android_install(app: AppHandle, apk_path: String) -> Result<String, String> {
    let adb = find_adb(&app).ok_or("adb not found.")?;
    let devs = adb_devices(app)?;
    if devs.devices.is_empty() {
        return Err(
            "No Android devices connected. Enable USB debugging and accept the authorization."
                .to_string(),
        );
    }
    let target = &devs.devices[0];
    let out = Command::new(&adb)
        .args(["-s", target, "install", "-r", &apk_path])
        .output()
        .map_err(|e| format!("{e}"))?;
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    if out.status.success() && stdout.contains("Success") {
        Ok(format!("Installed on {target}."))
    } else {
        Err(format!("adb install failed: {stdout} {stderr}"))
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct IosInstallAttempt {
    ok: bool,
    tool: Option<String>,
    message: String,
}

#[tauri::command]
fn ios_install_attempt(ipa_path: String) -> Result<IosInstallAttempt, String> {
    if let Ok(out) = Command::new("pymobiledevice3").arg("--version").output() {
        if out.status.success() {
            let inst = Command::new("pymobiledevice3")
                .args(["apps", "install", &ipa_path])
                .output()
                .map_err(|e| format!("{e}"))?;
            let log = format!(
                "{}{}",
                String::from_utf8_lossy(&inst.stdout),
                String::from_utf8_lossy(&inst.stderr)
            );
            if inst.status.success() {
                return Ok(IosInstallAttempt {
                    ok: true,
                    tool: Some("pymobiledevice3".to_string()),
                    message: "IPA installed on the device.".to_string(),
                });
            }
            return Ok(IosInstallAttempt {
                ok: false,
                tool: Some("pymobiledevice3".to_string()),
                message: format!("Installer returned an error: {log}"),
            });
        }
    }
    if Command::new("ideviceinstaller").arg("-h").output().is_ok() {
        let inst = Command::new("ideviceinstaller")
            .args(["-i", &ipa_path])
            .output()
            .map_err(|e| format!("{e}"))?;
        let log = format!(
            "{}{}",
            String::from_utf8_lossy(&inst.stdout),
            String::from_utf8_lossy(&inst.stderr)
        );
        return Ok(IosInstallAttempt {
            ok: inst.status.success(),
            tool: Some("ideviceinstaller".to_string()),
            message: if inst.status.success() {
                "IPA installed on the device.".to_string()
            } else {
                format!("ideviceinstaller failed: {log}")
            },
        });
    }
    Ok(IosInstallAttempt {
        ok: false,
        tool: None,
        message: "No iOS installer available. Export the .ipa and sign it with your account (Sideloadly, AltStore, Scarlet or iLoader): sign in on the Account tab with your Apple ID and an app-specific password, sign the IPA, then install it."
            .to_string(),
    })
}

#[tauri::command]
fn set_app_logo(app: AppHandle, src_path: String) -> Result<String, String> {
    let src = PathBuf::from(&src_path);
    if !src.exists() {
        return Err("Selected image does not exist.".to_string());
    }
    let ext = src
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .unwrap_or_default();
    if !["png", "jpg", "jpeg", "webp", "bmp", "ico"].contains(&ext.as_str()) {
        return Err("Pick a PNG, JPG, WebP, BMP or ICO image.".to_string());
    }
    if fs::metadata(&src).map(|m| m.len()).unwrap_or(0) > 8 * 1024 * 1024 {
        return Err("Image is too large (8 MB max).".to_string());
    }
    let dir = app_work_dir(&app, "logo")?;
    if let Ok(entries) = fs::read_dir(&dir) {
        for e in entries.flatten() {
            let _ = fs::remove_file(e.path());
        }
    }
    let dest = dir.join(format!("custom-logo.{ext}"));
    fs::copy(&src, &dest).map_err(|e| format!("Could not copy image: {e}"))?;
    Ok(dest.to_string_lossy().to_string())
}

#[tauri::command]
fn clear_app_logo(app: AppHandle) -> Result<(), String> {
    let dir = app_work_dir(&app, "logo")?;
    if let Ok(entries) = fs::read_dir(&dir) {
        for e in entries.flatten() {
            let _ = fs::remove_file(e.path());
        }
    }
    Ok(())
}

#[tauri::command]
fn app_logo_path(app: AppHandle) -> Result<Option<String>, String> {
    let dir = app_work_dir(&app, "logo")?;
    if let Ok(entries) = fs::read_dir(&dir) {
        let mut files: Vec<PathBuf> = entries
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.is_file())
            .collect();
        files.sort();
        if let Some(first) = files.into_iter().next() {
            return Ok(Some(first.to_string_lossy().to_string()));
        }
    }
    Ok(None)
}

#[tauri::command]
fn export_file(src: String, dest: String) -> Result<String, String> {
    fs::copy(&src, &dest).map_err(|e| format!("Could not export: {e}"))?;
    Ok(dest)
}

#[tauri::command]
fn read_text_file(path: String) -> Result<String, String> {
    fs::read_to_string(Path::new(&path)).map_err(|e| format!("Cannot read file: {e}"))
}

#[tauri::command]
fn write_text_file(path: String, text: String) -> Result<(), String> {
    let p = Path::new(&path);
    if let Some(parent) = p.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent).map_err(|e| format!("{e}"))?;
        }
    }
    fs::write(p, text).map_err(|e| format!("Cannot write file: {e}"))?;
    Ok(())
}

#[tauri::command]
fn show_in_folder(path: String) -> Result<(), String> {
    let normalized = path.replace('/', std::path::MAIN_SEPARATOR_STR);
    let target = PathBuf::from(normalized);
    let show = if target.exists() {
        target
    } else {
        match target.parent() {
            Some(p) if !p.as_os_str().is_empty() => p.to_path_buf(),
            _ => target,
        }
    };
    let show_str = show.to_string_lossy().to_string();
    #[cfg(target_os = "windows")]
    {
        if show.is_file() {
            Command::new("explorer")
                .args(["/select,", &show_str])
                .spawn()
                .map_err(|e| format!("{e}"))?;
        } else {
            Command::new("explorer")
                .args([&show_str])
                .spawn()
                .map_err(|e| format!("{e}"))?;
        }
    }
    #[cfg(target_os = "macos")]
    {
        Command::new("open")
            .args(["-R", &path])
            .spawn()
            .map_err(|e| format!("{e}"))?;
    }
    #[cfg(target_os = "linux")]
    {
        let p = Path::new(&path);
        let dir = p.parent().unwrap_or(Path::new("."));
        Command::new("xdg-open")
            .arg(dir)
            .spawn()
            .map_err(|e| format!("{e}"))?;
    }
    Ok(())
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AppDirs {
    output: String,
    data: String,
    locales: String,
    account: String,
}

#[tauri::command]
fn app_dirs(app: AppHandle) -> Result<AppDirs, String> {
    let data = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Could not resolve data directory: {e}"))?;
    let locales = data.join("locales");
    fs::create_dir_all(&locales).map_err(|e| format!("{e}"))?;
    let output = default_output_dir(&app)?;
    Ok(AppDirs {
        output: output.to_string_lossy().to_string(),
        data: data.to_string_lossy().to_string(),
        locales: locales.to_string_lossy().to_string(),
        account: accounts_path(&app)?.to_string_lossy().to_string(),
    })
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExtraLocale {
    code: String,
    json: String,
}

#[tauri::command]
fn extra_locales(app: AppHandle) -> Result<Vec<ExtraLocale>, String> {
    let data = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Could not resolve data directory: {e}"))?;
    let dir = data.join("locales");
    if !dir.exists() {
        return Ok(vec![]);
    }
    let mut out = Vec::new();
    for entry in fs::read_dir(&dir).map_err(|e| format!("{e}"))? {
        let p = entry.map_err(|e| format!("{e}"))?.path();
        if p.file_name()
            .and_then(|s| s.to_str())
            .map(|s| s.starts_with('.'))
            .unwrap_or(true)
        {
            continue;
        }
        if p.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let code = p
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();
        if code.is_empty() {
            continue;
        }
        if let Ok(text) = fs::read_to_string(&p) {
            if serde_json::from_str::<serde_json::Value>(&text).is_ok() {
                out.push(ExtraLocale { code, json: text });
            }
        }
    }
    out.sort_by(|a, b| a.code.cmp(&b.code));
    Ok(out)
}

#[derive(Debug, Serialize, Deserialize, Default)]
struct SeedManifest {
    version: u32,
    #[serde(default)]
    hashes: HashMap<String, String>,
}

fn sha256_hex(data: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(data);
    format!("{:x}", hasher.finalize())
}

#[tauri::command]
fn seed_locales(app: AppHandle, files: Vec<ExtraLocale>, version: u32) -> Result<usize, String> {
    let data = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Could not resolve data directory: {e}"))?;
    let dir = data.join("locales");
    fs::create_dir_all(&dir).map_err(|e| format!("{e}"))?;
    let manifest_path = dir.join(".seed.json");
    let manifest: SeedManifest = fs::read_to_string(&manifest_path)
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default();
    let managed = manifest_path.exists();
    let mut written = 0usize;
    let mut hashes = HashMap::new();
    for f in files {
        if f.code.is_empty()
            || !f
                .code
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        {
            continue;
        }
        if serde_json::from_str::<serde_json::Value>(&f.json).is_err() {
            continue;
        }
        hashes.insert(f.code.clone(), sha256_hex(f.json.as_bytes()));
        let dest = dir.join(format!("{}.json", f.code));
        let should_write = if !dest.exists() {
            true
        } else if !managed || manifest.version != version {
            match fs::read(&dest) {
                Ok(disk) => {
                    !managed
                        || manifest
                            .hashes
                            .get(&f.code)
                            .map(|h| *h == sha256_hex(&disk))
                            .unwrap_or(true)
                }
                Err(_) => true,
            }
        } else {
            false
        };
        if should_write {
            fs::write(&dest, &f.json).map_err(|e| format!("{e}"))?;
            written += 1;
        }
    }
    manifest_write(&manifest_path, version, &hashes)?;
    Ok(written)
}

fn manifest_write(
    path: &Path,
    version: u32,
    hashes: &HashMap<String, String>,
) -> Result<(), String> {
    let manifest = SeedManifest {
        version,
        hashes: hashes.clone(),
    };
    let pretty = serde_json::to_string_pretty(&manifest).map_err(|e| format!("{e}"))?;
    fs::write(path, pretty).map_err(|e| format!("{e}"))?;
    Ok(())
}

#[tauri::command]
fn load_user_settings(app: AppHandle) -> Result<Option<serde_json::Value>, String> {
    let data = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Could not resolve data directory: {e}"))?;
    let p = data.join("settings.json");
    if !p.exists() {
        return Ok(None);
    }
    let text = fs::read_to_string(&p).map_err(|e| format!("{e}"))?;
    let v: serde_json::Value =
        serde_json::from_str(&text).map_err(|_| "settings.json is not valid JSON".to_string())?;
    Ok(Some(v))
}

#[tauri::command]
fn save_user_settings(app: AppHandle, json: String) -> Result<(), String> {
    let v: serde_json::Value =
        serde_json::from_str(&json).map_err(|_| "Invalid settings".to_string())?;
    if !v.is_object() {
        return Err("Invalid settings".to_string());
    }
    let data = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Could not resolve data directory: {e}"))?;
    let pretty = serde_json::to_string_pretty(&v).map_err(|e| format!("{e}"))?;
    fs::write(data.join("settings.json"), pretty).map_err(|e| format!("{e}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(name: &str) -> String {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join(name)
            .to_string_lossy()
            .to_string()
    }

    #[test]
    fn inspects_sample_apks() {
        let info = inspect_apks(sample("yw1m_1.0.13.apks")).expect("apks must parse");
        assert!(!info.single_apk);
        assert!(info.base_apk.is_some());
        assert!(info.splits.iter().any(|s| s.kind == "asset_pack"));
    }

    #[test]
    fn inspects_sample_drmb() {
        let info = inspect_drmb(sample("yw1m_1.0.13.drmb")).expect("drmb must parse");
        assert!(info.base_files > 0);
        assert!(info.split_files > 0);
        assert!(info.smali_files > 0);
    }

    #[test]
    fn excludes_match_doc_names_case_insensitively() {
        let exc = vec!["Configuración recomendada para jugar en Bluestacks.cfg".to_string()];
        assert!(!keep_path(
            "Configuración recomendada para jugar en BlueStacks.cfg",
            &[],
            &exc
        ));
        assert!(keep_path("assets/data/game.cfg", &[], &exc));
    }

    #[test]
    fn image_exclusion_covers_img_alias() {
        let mut exc = vec!["./image".to_string()];
        expand_image_excludes(&mut exc);
        assert!(!keep_path("img/Logo.png", &[], &exc));
        assert!(keep_path("assets/smp/fnt/ft_nrm.xf", &[], &exc));
    }

    #[test]
    fn stale_ios_signatures_include_nested_ones() {
        let app = "Payload/yw1_rom.app";
        assert!(is_stale_ios_signature(
            app,
            &format!("{app}/_CodeSignature/CodeResources")
        ));
        assert!(is_stale_ios_signature(
            app,
            &format!("{app}/embedded.mobileprovision")
        ));
        assert!(is_stale_ios_signature(
            app,
            &format!("{app}/Frameworks/Foo.framework/embedded.mobileprovision")
        ));
        assert!(!is_stale_ios_signature(app, &format!("{app}/yw1_rom")));
    }

    #[test]
    fn rename_assets_parses_and_merges() {
        let on = EnmaCfg::from_json(r#"{"renameAssets": true}"#);
        assert_eq!(on.rename_assets, Some(true));
        let off = EnmaCfg::from_json(r#"{"renameAssets": false}"#);
        assert_eq!(off.rename_assets, Some(false));
        let absent = EnmaCfg::from_json(r#"{"appName": "x"}"#);
        assert_eq!(absent.rename_assets, None);
        assert!(absent.rename_assets.unwrap_or(true));

        let mut merged = EnmaCfg::default();
        merge_mod_config(&mut merged, off);
        assert_eq!(merged.rename_assets, Some(false));
        merge_mod_config(&mut merged, on);
        assert_eq!(merged.rename_assets, Some(true));
    }

    #[test]
    fn ios_filters_skip_android_asset_pack_by_default() {
        let cfg = EnmaCfg::from_json(r#"{"appName": "x"}"#);
        let (inc, exc) = effective_filters(&cfg, "ios");
        assert!(!keep_path(
            "assets/android/snd/cri/yw_sound.acb",
            &inc,
            &exc
        ));
        assert!(keep_path("assets/smp/fnt/ft_nrm.xf", &inc, &exc));
        let cfg2 = EnmaCfg::from_json(r#"{"includeIos": ["assets/android/**"]}"#);
        let (inc2, exc2) = effective_filters(&cfg2, "ios");
        assert!(keep_path(
            "assets/android/snd/cri/yw_sound.acb",
            &inc2,
            &exc2
        ));
        let (inc3, exc3) = effective_filters(&cfg, "android");
        assert!(keep_path(
            "assets/android/snd/cri/yw_sound.acb",
            &inc3,
            &exc3
        ));
    }

    #[test]
    fn zsign_args_order_and_optional_password() {
        let z = PathBuf::from("zsign.exe");
        let full = zsign_args(
            &z,
            &PathBuf::from("a.p12"),
            "secret",
            &PathBuf::from("b.mobileprovision"),
            &PathBuf::from("in.ipa"),
            &PathBuf::from("out.ipa"),
        );
        assert_eq!(
            full,
            vec![
                "-k",
                "a.p12",
                "-m",
                "b.mobileprovision",
                "-z",
                "9",
                "-o",
                "out.ipa",
                "-p",
                "secret",
                "in.ipa"
            ]
        );
        let nopass = zsign_args(
            &z,
            &PathBuf::from("a.p12"),
            "",
            &PathBuf::from("b.mobileprovision"),
            &PathBuf::from("in.ipa"),
            &PathBuf::from("out.ipa"),
        );
        assert!(!nopass.contains(&"-p".to_string()));
        assert_eq!(nopass.last().unwrap(), "in.ipa");
    }

    #[test]
    fn strip_unsupported_devices_removes_key_and_keeps_rest() {
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleIdentifier</key><string>jp.co.level5.yws1</string>
<key>UISupportedDevices</key><array><string>iPhone8,1</string></array>
</dict></plist>"#;
        let fixed = customize_ios_plist(xml, None).expect("must strip");
        let v = plist::Value::from_reader(std::io::Cursor::new(&fixed)).unwrap();
        let d = v.into_dictionary().unwrap();
        assert!(!d.contains_key("UISupportedDevices"));
        assert_eq!(
            d.get("CFBundleIdentifier").and_then(|x| x.as_string()),
            Some("jp.co.level5.yws1")
        );
        let clean = br#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleIdentifier</key><string>x</string>
</dict></plist>"#;
        assert!(customize_ios_plist(clean, None).is_none());
    }

    #[test]
    fn mod_smali_path_detection() {
        assert!(mod_has_smali_path("smali/com/a/B.smali"));
        assert!(mod_has_smali_path("YW1MESP-main/smali/jp/Foo.smali"));
        assert!(mod_has_smali_path("assets/data/x.smali"));
        assert!(!mod_has_smali_path("assets/data/text/a.cfg.bin"));
        assert!(!mod_has_smali_path("README.md"));
    }

    #[test]
    fn verbatim_copy_strips_drmb_split_prefix() {
        use std::io::Cursor;
        let mut src_buf = Cursor::new(Vec::new());
        {
            let mut w = zip::ZipWriter::new(&mut src_buf);
            w.start_file(
                "split/assets/data/map/t.xc",
                zip::write::SimpleFileOptions::default()
                    .compression_method(zip::CompressionMethod::Stored),
            )
            .unwrap();
            w.write_all(b"mapbytes").unwrap();
            w.finish().unwrap();
        }
        src_buf.set_position(0);
        let mut src = zip::ZipArchive::new(src_buf).unwrap();
        let entry = src.by_name("split/assets/data/map/t.xc").unwrap();
        let mut out_buf = Cursor::new(Vec::new());
        {
            let mut w = zip::ZipWriter::new(&mut out_buf);
            copy_entry_verbatim(&mut w, entry, "assets/data/map/t.xc").unwrap();
            w.finish().unwrap();
        }
        out_buf.set_position(0);
        let mut out = zip::ZipArchive::new(out_buf).unwrap();
        assert!(out.by_name("assets/data/map/t.xc").is_ok());
        assert!(out.by_name("split/assets/data/map/t.xc").is_err());
        assert!(out
            .by_name("assets/data/map/t.xc")
            .unwrap()
            .read_to_end(&mut Vec::new())
            .is_ok());
    }

    #[test]
    fn customize_ios_plist_sets_name_and_strips_devices() {
        let xml = br#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleDisplayName</key><string>Old</string>
<key>UISupportedDevices</key><array><string>iPhone8,1</string></array>
</dict></plist>"#;
        let fixed = customize_ios_plist(xml, Some("  Yo-kai & Amigos  ")).expect("must change");
        let v = plist::Value::from_reader(std::io::Cursor::new(&fixed)).unwrap();
        let d = v.into_dictionary().unwrap();
        assert!(!d.contains_key("UISupportedDevices"));
        assert_eq!(
            d.get("CFBundleDisplayName").and_then(|x| x.as_string()),
            Some("Yo-kai & Amigos")
        );
        assert_eq!(
            d.get("CFBundleName").and_then(|x| x.as_string()),
            Some("Yo-kai & Amigos")
        );
        let clean = br#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleDisplayName</key><string>Same</string>
</dict></plist>"#;
        assert!(customize_ios_plist(clean, None).is_none());
        assert!(customize_ios_plist(clean, Some("Same")).is_some());
    }

    #[test]
    fn android_label_replaces_refs_with_escaped_literal() {
        assert_eq!(
            xml_attr_escape("A&B<C>\"Q\""),
            "A&amp;B&lt;C&gt;&quot;Q&quot;"
        );
    }

    #[test]
    fn error_helpers_surface_root_cause_and_disk_full() {
        let log = "line one\njava.lang.OutOfMemoryError: Java heap space\n\tat com.android.apksig.ApkSigner.sign(ApkSigner.java:431)\nCaused by: lack of memory\ntail line";
        let picked = pick_error_lines(log, 4);
        assert!(picked.contains("OutOfMemoryError"));
        assert!(picked.contains("Caused by"));
        let plain = "ok\nall good\nnothing here";
        assert_eq!(pick_error_lines(plain, 2), "all good nothing here");
        assert!(
            disk_full_hint("error 112: Espacio en disco insuficiente. (os error 112)").is_some()
        );
        assert!(disk_full_hint("write failed: No space left on device").is_some());
        assert!(disk_full_hint("some other failure").is_none());
        assert!(with_hint("sign failed:", "boom").contains("sign failed:"));
    }
}

#[derive(Debug, Clone, clap::Parser)]
#[command(
    name = "enma-patcher-desktop",
    about = "Enma Patcher Desktop (GUI + CLI)"
)]
struct Cli {
    #[command(subcommand)]
    command: Option<CliCommand>,
}

#[derive(Debug, Clone, clap::Subcommand)]
enum CliCommand {
    /// Patch an Android .apks/.apk with mods (+ optional .drmb) into a signed APK
    PatchAndroid {
        /// Input .apks bundle (or single .apk with --force-single-apk)
        #[arg(long)]
        apks: String,
        /// Your own .drmb (required for .apks with splits)
        #[arg(long)]
        drmb: Option<String>,
        /// Mod repo as owner/repo[@branch], repeatable
        #[arg(long = "mod")]
        mods: Vec<String>,
        /// Mod ZIP file, repeatable
        #[arg(long = "zip")]
        zips: Vec<String>,
        /// Output file name (default: <input>_patched.apk)
        #[arg(long)]
        out: Option<String>,
        /// Allow starting from a single .apk
        #[arg(long, default_value_t = false)]
        force_single_apk: bool,
        /// Signing: auto|custom (default auto = v1+v2+v3)
        #[arg(long, default_value = "auto")]
        sign_mode: String,
        /// Scratch directory (default: app data dir; use D: for big merges)
        #[arg(long)]
        work_dir: Option<String>,
        /// Display name for the installed app (default: mods' appName cfg)
        #[arg(long)]
        app_name: Option<String>,
    },
    /// Patch a decrypted iOS .ipa with mods
    PatchIos {
        /// Input decrypted .ipa
        #[arg(long)]
        ipa: String,
        /// Mod repo as owner/repo[@branch], repeatable
        #[arg(long = "mod")]
        mods: Vec<String>,
        /// Mod ZIP file, repeatable
        #[arg(long = "zip")]
        zips: Vec<String>,
        /// Output file name (default: <input>_patched.ipa)
        #[arg(long)]
        out: Option<String>,
        /// Scratch directory (default: app data dir)
        #[arg(long)]
        work_dir: Option<String>,
        /// Display name for the installed app (default: mods' appName cfg)
        #[arg(long)]
        app_name: Option<String>,
    },
    /// Sign an .ipa with zsign using an account's imported identity
    SignIos {
        /// Input .ipa (patched, unsigned)
        #[arg(long)]
        ipa: String,
        /// Account id (default: active account)
        #[arg(long)]
        account: Option<String>,
        /// Output file name (default: <input>_signed.ipa)
        #[arg(long)]
        out: Option<String>,
    },
    /// Inspect an .apks/.apk bundle
    InspectApks { path: String },
    /// Inspect a .drmb file
    InspectDrmb { path: String },
    /// Inspect an .ipa file
    InspectIpa { path: String },
    /// Check whether an APK looks patched
    CheckApk {
        path: String,
        /// Extra marker strings to look for
        #[arg(long = "marker")]
        markers: Vec<String>,
    },
    /// Show external tool status (apktool, uber-apk-signer, adb, java)
    ToolStatus,
    /// Download an external tool (apktool, uber-apk-signer)
    DownloadTool { name: String },
    /// List connected Android devices via adb
    AdbDevices,
    /// Install an APK on the first connected Android device
    AndroidInstall { apk: String },
}

fn parse_cli_mods(mods: &[String], zips: &[String]) -> Result<Vec<ModSpec>, String> {
    let mut out = Vec::new();
    for m in mods {
        let (repo, branch) = match m.split_once('@') {
            Some((r, b)) => (r.trim(), b.trim()),
            None => (m.trim(), "main"),
        };
        if !repo.contains('/') || repo.is_empty() {
            return Err(format!(
                "Invalid mod repo (expected owner/repo[@branch]): {m}"
            ));
        }
        out.push(ModSpec {
            kind: "github".to_string(),
            repo: repo.to_string(),
            branch: if branch.is_empty() {
                "main".to_string()
            } else {
                branch.to_string()
            },
            path: String::new(),
            enabled: true,
        });
    }
    for z in zips {
        out.push(ModSpec {
            kind: "zip".to_string(),
            repo: String::new(),
            branch: String::new(),
            path: z.clone(),
            enabled: true,
        });
    }
    if out.is_empty() {
        return Ok(out);
    }
    Ok(out)
}

fn cli_phase(step: &str) {
    eprintln!("[enma-cli] {step}");
}

fn print_json<T: serde::Serialize>(value: &T) {
    println!(
        "{}",
        serde_json::to_string_pretty(value).unwrap_or_else(|_| "{}".to_string())
    );
}

fn run_cli(cmd: CliCommand) -> Result<(), String> {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .build(tauri::generate_context!())
        .map_err(|e| format!("Could not initialize app context: {e}"))?;
    let handle = app.handle().clone();
    match cmd {
        CliCommand::PatchAndroid {
            apks,
            drmb,
            mods,
            zips,
            out,
            force_single_apk,
            sign_mode,
            work_dir,
            app_name,
        } => {
            cli_phase("patch-android start");
            let req = AndroidPatchRequest {
                apks_path: apks,
                drmb_path: drmb,
                mods: parse_cli_mods(&mods, &zips)?,
                output_name: out,
                force_single_apk,
                work_dir,
                app_name,
                sign_mode,
                sign_v1: true,
                sign_v2: true,
                sign_v3: true,
                android_version: None,
            };
            let res = tauri::async_runtime::block_on(patch_android(handle, req))?;
            cli_phase("patch-android done");
            print_json(&res);
            Ok(())
        }
        CliCommand::PatchIos {
            ipa,
            mods,
            zips,
            out,
            work_dir,
            app_name,
        } => {
            let req = IosPatchRequest {
                ipa_path: ipa,
                mods: parse_cli_mods(&mods, &zips)?,
                output_name: out,
                work_dir,
                app_name,
            };
            let res = tauri::async_runtime::block_on(patch_ios(handle, req))?;
            print_json(&res);
            Ok(())
        }
        CliCommand::SignIos { ipa, account, out } => {
            let res = tauri::async_runtime::block_on(sign_ios(
                handle,
                ipa,
                account.unwrap_or_default(),
                out,
            ))?;
            print_json(&res);
            Ok(())
        }
        CliCommand::InspectApks { path } => {
            print_json(&inspect_apks(path)?);
            Ok(())
        }
        CliCommand::InspectDrmb { path } => {
            print_json(&inspect_drmb(path)?);
            Ok(())
        }
        CliCommand::InspectIpa { path } => {
            print_json(&inspect_ipa(path)?);
            Ok(())
        }
        CliCommand::CheckApk { path, markers } => {
            print_json(&check_apk_patched(path, markers)?);
            Ok(())
        }
        CliCommand::ToolStatus => {
            print_json(&tool_status(handle)?);
            Ok(())
        }
        CliCommand::DownloadTool { name } => {
            let dest = tauri::async_runtime::block_on(download_tool(handle, name))?;
            println!("{dest}");
            Ok(())
        }
        CliCommand::AdbDevices => {
            print_json(&adb_devices(handle)?);
            Ok(())
        }
        CliCommand::AndroidInstall { apk } => {
            let msg = android_install(handle, apk)?;
            println!("{msg}");
            Ok(())
        }
    }
}

fn main() {
    #[cfg(target_os = "windows")]
    if std::env::args().len() > 1 {
        unsafe {
            windows_sys::Win32::System::Console::AttachConsole(
                windows_sys::Win32::System::Console::ATTACH_PARENT_PROCESS,
            );
        }
    }
    match Cli::try_parse() {
        Ok(cli) if cli.command.is_some() => {
            if let Err(e) = run_cli(cli.command.unwrap()) {
                eprintln!("Error: {e}");
                std::process::exit(1);
            }
            return;
        }
        _ => {}
    }
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            inspect_apks,
            inspect_single_apk,
            check_apk_patched,
            inspect_drmb,
            inspect_ipa,
            list_github_files,
            fetch_remote_config,
            mod_info,
            inspect_mod_smali,
            patch_android,
            patch_ios,
            tool_status,
            download_tool,
            import_tool_file,
            save_apple_account,
            list_apple_accounts,
            delete_apple_account,
            set_active_apple_account,
            import_signing_identity,
            delete_signing_identity,
            sign_ios,
            adb_devices,
            android_install,
            apk_abis,
            check_device_compat,
            ios_install_attempt,
            export_file,
            set_app_logo,
            clear_app_logo,
            app_logo_path,
            show_in_folder,
            app_dirs,
            extra_locales,
            seed_locales,
            load_user_settings,
            save_user_settings,
            get_output_dir,
            set_output_dir,
            read_text_file,
            write_text_file
        ])
        .run(tauri::generate_context!())
        .expect("error starting Enma Patcher Desktop");
}
