use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Mutex;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_dialog::DialogExt;

const KEYRING_SERVICE: &str = "winn-lince";
const PLAY_SESSION_KEY: &str = "play-session";

pub struct AppState {
    play: Mutex<Option<PlaySession>>,
}

#[derive(Clone, Serialize)]
struct PlaySession {
    email: String,
    token: String,
    device: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct BrandInfo {
    name: String,
    platform: String,
}

#[derive(Serialize, Clone)]
struct Account {
    name: String,
    email: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LoginResult {
    needs2fa: bool,
    account: Option<Account>,
    message: String,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct StoreApp {
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    bundle_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    package: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    price: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    icon: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    developer: Option<String>,
}

#[derive(Serialize)]
struct SaveFolder {
    path: String,
    which: String,
}

fn brand_name() -> &'static str {
    if cfg!(windows) { "WINN" } else { "LINCE" }
}

fn emit_status(app: &AppHandle, msg: &str) {
    let _ = app.emit("helper-status", msg);
}

fn sidecar_path(name: &str) -> Result<PathBuf, String> {
    let file = if cfg!(windows) {
        format!("{name}-{}.exe", env!("TARGET_TRIPLE"))
    } else {
        format!("{name}-{}", env!("TARGET_TRIPLE"))
    };
    let dev = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("binaries").join(&file);
    if dev.exists() {
        return Ok(dev);
    }
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let dir = exe.parent().ok_or("missing executable directory")?;
    let bundled = if cfg!(windows) {
        dir.join(format!("{name}.exe"))
    } else {
        dir.join(name)
    };
    if bundled.exists() {
        return Ok(bundled);
    }
    Err(format!(
        "{name} sidecar is missing (looked for {} and {})",
        dev.display(),
        bundled.display()
    ))
}

fn tools_dir(app: &AppHandle) -> Option<PathBuf> {
    if let Ok(p) = app.path().resource_dir() {
        for c in [p.join("AndroidTools"), p.join("resources/AndroidTools")] {
            if c.join("APKEditor.jar").exists() || c.join("apkeditor/APKEditor.jar").exists() {
                return Some(c);
            }
        }
    }
    let dev = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../helpers/Vendor");
    if dev.join("apkeditor/APKEditor.jar").exists() {
        return Some(dev);
    }
    None
}

fn allowed_roots() -> Result<Vec<PathBuf>, String> {
    let mut roots = Vec::new();
    if let Some(p) = dirs::download_dir() {
        roots.push(p);
    }
    if let Some(p) = dirs::desktop_dir() {
        roots.push(p);
    }
    if roots.is_empty() {
        if let Some(home) = dirs::home_dir() {
            roots.push(home.join("Downloads"));
            roots.push(home.join("Desktop"));
        }
    }
    if roots.is_empty() {
        return Err("couldn't find your Downloads or Desktop folder".into());
    }
    Ok(roots)
}

fn is_desktop_path(p: &Path) -> bool {
    p.file_name()
        .and_then(|n| n.to_str())
        .map(|n| n.eq_ignore_ascii_case("desktop"))
        .unwrap_or(false)
}

fn resolve_dest(which: &str) -> Result<(String, PathBuf), String> {
    let roots = allowed_roots()?;
    let want_desktop = which.eq_ignore_ascii_case("desktop");
    let (label, path) = if want_desktop {
        if let Some(p) = roots.iter().find(|p| is_desktop_path(p)) {
            ("desktop", p.clone())
        } else if let Some(p) = dirs::desktop_dir() {
            ("desktop", p)
        } else {
            ("downloads", roots[0].clone())
        }
    } else if let Some(p) = dirs::download_dir() {
        ("downloads", p)
    } else {
        ("downloads", roots[0].clone())
    };
    std::fs::create_dir_all(&path).map_err(|e| e.to_string())?;
    Ok((label.to_string(), path))
}

fn is_under_allowed(path: &Path) -> bool {
    let path = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    let roots = allowed_roots().unwrap_or_default();
    roots.iter().any(|root| {
        let root = root.canonicalize().unwrap_or_else(|_| root.clone());
        path.starts_with(&root)
    })
}

fn ensure_saved_path(path: &Path) -> Result<String, String> {
    if is_under_allowed(path) {
        return Ok(path.to_string_lossy().into());
    }
    if let Some(parent) = path.parent() {
        if is_under_allowed(parent) {
            return Ok(path.to_string_lossy().into());
        }
    }
    Err("downloads can only be saved to your Downloads or Desktop folder".into())
}

fn run_captured(cmd: &mut Command) -> Result<(i32, String, String), String> {
    let out = cmd.output().map_err(|e| format!("couldn't start helper: {e}"))?;
    let code = out.status.code().unwrap_or(-1);
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    Ok((code, stdout, stderr))
}

fn random_passphrase() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("winn-lince-{nanos}-{:x}", std::process::id())
}

fn ipatool_base(_app: &AppHandle) -> Result<Command, String> {
    let path = sidecar_path("ipatool")?;
    let mut cmd = Command::new(path);
    cmd.arg("--non-interactive").arg("--format").arg("json");
    if let Ok(entry) = keyring::Entry::new(KEYRING_SERVICE, "ipatool-keychain-passphrase") {
        match entry.get_password() {
            Ok(p) if !p.is_empty() => {
                cmd.arg("--keychain-passphrase").arg(p);
            }
            _ => {
                let phrase = random_passphrase();
                if entry.set_password(&phrase).is_ok() {
                    cmd.arg("--keychain-passphrase").arg(phrase);
                }
            }
        }
    }
    Ok(cmd)
}

fn parse_json_lines(text: &str) -> Vec<serde_json::Value> {
    text.lines()
        .filter_map(|l| {
            let l = l.trim();
            if l.starts_with('{') {
                serde_json::from_str(l).ok()
            } else {
                None
            }
        })
        .collect()
}

fn json_msg(v: &serde_json::Value) -> String {
    v.get("message")
        .or_else(|| v.get("error"))
        .and_then(|x| x.as_str())
        .or_else(|| v.get("msg").and_then(|x| x.as_str()))
        .unwrap_or("")
        .to_string()
}

fn looks_like_2fa(text: &str) -> bool {
    let t = text.to_lowercase();
    t.contains("2fa")
        || t.contains("auth-code")
        || t.contains("auth code")
        || t.contains("two-factor")
        || t.contains("two factor")
}

fn account_from_json(rows: &[serde_json::Value]) -> Option<Account> {
    for v in rows.iter().rev() {
        let email = v.get("email").and_then(|x| x.as_str()).unwrap_or("");
        let name = v.get("name").and_then(|x| x.as_str()).unwrap_or(email);
        let success = v.get("success").and_then(|x| x.as_bool()).unwrap_or(false);
        if !email.is_empty() && (success || v.get("name").is_some()) {
            return Some(Account {
                name: name.to_string(),
                email: email.to_string(),
            });
        }
    }
    None
}

fn helper_error(stdout: &str, stderr: &str, code: i32) -> String {
    let mut parts = String::new();
    for v in parse_json_lines(stdout) {
        let m = json_msg(&v);
        if !m.is_empty() {
            parts.push_str(&m);
            parts.push('\n');
        }
    }
    if parts.is_empty() {
        parts = stderr.trim().to_string();
    }
    if parts.is_empty() {
        parts = stdout.trim().to_string();
    }
    if parts.is_empty() {
        format!("helper failed (exit {code})")
    } else {
        parts.trim().to_string()
    }
}

#[tauri::command]
fn app_brand() -> BrandInfo {
    BrandInfo {
        name: brand_name().into(),
        platform: if cfg!(windows) {
            "windows".into()
        } else {
            "linux".into()
        },
    }
}

#[tauri::command]
fn save_folder(which: String) -> Result<SaveFolder, String> {
    let (label, path) = resolve_dest(&which)?;
    Ok(SaveFolder {
        path: path.to_string_lossy().into(),
        which: label,
    })
}

#[tauri::command]
fn java_available() -> bool {
    let exe = if cfg!(windows) { "java.exe" } else { "java" };
    if let Ok(h) = std::env::var("JAVA_HOME") {
        if PathBuf::from(h).join("bin").join(exe).exists() {
            return true;
        }
    }
    if let Some(path) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&path) {
            if dir.join(exe).is_file() {
                return true;
            }
        }
    }
    false
}

#[tauri::command]
fn apple_status(app: AppHandle) -> Result<Option<Account>, String> {
    let mut cmd = ipatool_base(&app)?;
    cmd.args(["auth", "info"]);
    let (code, stdout, _stderr) = run_captured(&mut cmd)?;
    if code != 0 {
        return Ok(None);
    }
    Ok(account_from_json(&parse_json_lines(&stdout)))
}

#[tauri::command]
fn apple_login(
    app: AppHandle,
    email: String,
    password: String,
    auth_code: Option<String>,
) -> Result<LoginResult, String> {
    if email.trim().is_empty() || password.is_empty() {
        return Err("enter your Apple ID and password".into());
    }
    emit_status(&app, "Signing in to the App Store…");
    let mut cmd = ipatool_base(&app)?;
    cmd.args(["auth", "login", "--email"]).arg(email.trim());
    cmd.arg("--password").arg(&password);
    if let Some(code) = auth_code
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        cmd.arg("--auth-code").arg(code);
    }
    let (code, stdout, stderr) = run_captured(&mut cmd)?;
    let combined = format!("{stdout}\n{stderr}");
    if looks_like_2fa(&combined) && !combined.to_lowercase().contains("\"success\":true") {
        return Ok(LoginResult {
            needs2fa: true,
            account: None,
            message: "A 2FA code is required.".into(),
        });
    }
    let rows = parse_json_lines(&stdout);
    if let Some(account) = account_from_json(&rows) {
        emit_status(&app, "Signed in.");
        return Ok(LoginResult {
            needs2fa: false,
            account: Some(account),
            message: "Signed in. Your session is in the OS keyring.".into(),
        });
    }
    if code == 0 {
        if let Some(account) = apple_status(app)? {
            return Ok(LoginResult {
                needs2fa: false,
                account: Some(account),
                message: "Signed in.".into(),
            });
        }
    }
    Err(helper_error(&stdout, &stderr, code))
}

#[tauri::command]
fn apple_logout(app: AppHandle) -> Result<(), String> {
    let mut cmd = ipatool_base(&app)?;
    cmd.args(["auth", "revoke"]);
    let (code, stdout, stderr) = run_captured(&mut cmd)?;
    if code != 0 {
        return Err(helper_error(&stdout, &stderr, code));
    }
    emit_status(&app, "Signed out.");
    Ok(())
}

fn parse_apple_apps(rows: &[serde_json::Value]) -> Vec<StoreApp> {
    let mut apps = Vec::new();
    for v in rows {
        let list = v
            .get("apps")
            .or_else(|| v.get("Apps"))
            .or_else(|| v.get("results"))
            .cloned()
            .or_else(|| {
                if v.get("bundleID").is_some()
                    || v.get("bundleId").is_some()
                    || v.get("bundle_id").is_some()
                {
                    Some(serde_json::Value::Array(vec![v.clone()]))
                } else {
                    None
                }
            });
        let Some(serde_json::Value::Array(items)) = list else {
            continue;
        };
        for item in items {
            let bundle = item
                .get("bundleID")
                .or_else(|| item.get("bundleId"))
                .or_else(|| item.get("bundle_id"))
                .and_then(|x| x.as_str())
                .map(|s| s.to_string());
            let name = item
                .get("name")
                .or_else(|| item.get("Name"))
                .and_then(|x| x.as_str())
                .unwrap_or("App")
                .to_string();
            let version = item
                .get("version")
                .or_else(|| item.get("Version"))
                .and_then(|x| x.as_str())
                .map(|s| s.to_string());
            let developer = item
                .get("developer")
                .or_else(|| item.get("sellerName"))
                .or_else(|| item.get("artistName"))
                .and_then(|x| x.as_str())
                .map(|s| s.to_string());
            let price = item
                .get("price")
                .or_else(|| item.get("formattedPrice"))
                .map(|x| match x {
                    serde_json::Value::String(s) => s.clone(),
                    serde_json::Value::Number(n) => n.to_string(),
                    _ => String::new(),
                })
                .filter(|s| !s.is_empty());
            let icon = item
                .get("icon")
                .or_else(|| item.get("artworkUrl100"))
                .or_else(|| item.get("iconURL"))
                .and_then(|x| x.as_str())
                .map(|s| s.to_string());
            let id = item
                .get("id")
                .or_else(|| item.get("ID"))
                .or_else(|| item.get("trackId"))
                .cloned();
            apps.push(StoreApp {
                name,
                bundle_id: bundle,
                package: None,
                version,
                id,
                price,
                icon,
                developer,
            });
        }
    }
    apps
}

#[tauri::command]
fn apple_search(
    app: AppHandle,
    query: String,
    platform: Option<String>,
) -> Result<Vec<StoreApp>, String> {
    let q = query.trim();
    if q.is_empty() {
        return Err("type something to search for".into());
    }
    emit_status(&app, "Searching the App Store…");
    let mut cmd = ipatool_base(&app)?;
    cmd.args(["search", q, "--limit", "20"]);
    if let Some(p) = platform.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        cmd.arg("--platform").arg(p);
    }
    let (code, stdout, stderr) = run_captured(&mut cmd)?;
    if code != 0 {
        return Err(helper_error(&stdout, &stderr, code));
    }
    let apps = parse_apple_apps(&parse_json_lines(&stdout));
    emit_status(&app, &format!("Found {} app(s).", apps.len()));
    Ok(apps)
}

fn newest_file(dir: &Path, exts: &[&str]) -> Option<PathBuf> {
    let mut best: Option<(std::time::SystemTime, PathBuf)> = None;
    let entries = std::fs::read_dir(dir).ok()?;
    for e in entries.flatten() {
        let p = e.path();
        let ext = p
            .extension()
            .and_then(|x| x.to_str())
            .unwrap_or("")
            .to_lowercase();
        if !exts.iter().any(|x| x.eq_ignore_ascii_case(&ext)) {
            continue;
        }
        let t = e.metadata().ok().and_then(|m| m.modified().ok())?;
        if best.as_ref().map(|(bt, _)| t > *bt).unwrap_or(true) {
            best = Some((t, p));
        }
    }
    best.map(|(_, p)| p)
}

#[tauri::command]
fn apple_download(
    app: AppHandle,
    bundle_id: String,
    platform: Option<String>,
    dest: String,
) -> Result<String, String> {
    let bundle = bundle_id.trim();
    if bundle.is_empty() {
        return Err("missing bundle id".into());
    }
    let (_label, dir) = resolve_dest(&dest)?;
    emit_status(&app, "Downloading IPA… this can take a while.");
    let mut cmd = ipatool_base(&app)?;
    cmd.args(["download", "--bundle-identifier", bundle, "--output"])
        .arg(&dir);
    if let Some(p) = platform.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        cmd.arg("--platform").arg(p);
    }
    let (code, stdout, stderr) = run_captured(&mut cmd)?;
    if code != 0 {
        let msg = helper_error(&stdout, &stderr, code);
        if msg.to_lowercase().contains("license") {
            return Err("This Apple ID doesn't have a license for that app. Get it on the App Store first (this app never purchases paid apps).".into());
        }
        return Err(msg);
    }
    let mut path = None;
    for v in parse_json_lines(&stdout) {
        if let Some(p) = v
            .get("output")
            .or_else(|| v.get("destination"))
            .or_else(|| v.get("path"))
            .and_then(|x| x.as_str())
        {
            path = Some(p.to_string());
        }
    }
    let saved = if let Some(p) = path {
        PathBuf::from(p)
    } else {
        newest_file(&dir, &["ipa", "pkg"])
            .ok_or_else(|| "download finished but the IPA path wasn't reported".to_string())?
    };
    let s = ensure_saved_path(&saved)?;
    emit_status(&app, &format!("Saved {s}"));
    Ok(s)
}

fn play_entry() -> Result<keyring::Entry, String> {
    keyring::Entry::new(KEYRING_SERVICE, PLAY_SESSION_KEY).map_err(|e| e.to_string())
}

fn load_play_session(state: &AppState) -> Option<PlaySession> {
    if let Some(s) = state.play.lock().ok().and_then(|g| g.clone()) {
        return Some(s);
    }
    let entry = play_entry().ok()?;
    let raw = entry.get_password().ok()?;
    let v: serde_json::Value = serde_json::from_str(&raw).ok()?;
    let email = v.get("email")?.as_str()?.to_string();
    let token = v.get("token")?.as_str()?.to_string();
    let device = v
        .get("device")
        .and_then(|x| x.as_str())
        .unwrap_or("px_9a")
        .to_string();
    if email.is_empty() || token.is_empty() {
        return None;
    }
    let s = PlaySession {
        email,
        token,
        device,
    };
    if let Ok(mut g) = state.play.lock() {
        *g = Some(s.clone());
    }
    Some(s)
}

fn save_play_session(state: &AppState, s: &PlaySession) {
    if let Ok(mut g) = state.play.lock() {
        *g = Some(s.clone());
    }
    if let Ok(entry) = play_entry() {
        let raw = serde_json::json!({"email": s.email, "token": s.token, "device": s.device})
            .to_string();
        let _ = entry.set_password(&raw);
    }
}

fn apply_play_env(cmd: &mut Command, app: &AppHandle, session: Option<&PlaySession>) {
    if let Some(s) = session {
        cmd.env("MACN_PLAY_EMAIL", &s.email);
        cmd.env("MACN_PLAY_AUTH_TOKEN", &s.token);
        if !s.device.is_empty() {
            cmd.env("MACN_PLAY_DEVICE", &s.device);
        }
    }
    if let Some(tools) = tools_dir(app) {
        cmd.env("MACN_PLAY_TOOLS", tools);
    }
}

fn ingest_play_session(state: &AppState, rows: &[serde_json::Value]) {
    for v in rows {
        if v.get("event").and_then(|x| x.as_str()) != Some("session") {
            continue;
        }
        let email = v
            .get("email")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string();
        let token = v
            .get("authToken")
            .or_else(|| v.get("token"))
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string();
        let device = v
            .get("device")
            .and_then(|x| x.as_str())
            .unwrap_or("px_9a")
            .to_string();
        if !email.is_empty() && !token.is_empty() {
            save_play_session(
                state,
                &PlaySession {
                    email,
                    token,
                    device,
                },
            );
        }
    }
}

fn run_play(
    app: &AppHandle,
    state: &AppState,
    args: &[&str],
    dest: Option<&Path>,
) -> Result<Vec<serde_json::Value>, String> {
    let path = sidecar_path("macn-play")?;
    let mut cmd = Command::new(path);
    cmd.args(args).stdout(Stdio::piped()).stderr(Stdio::piped());
    if let Some(dir) = dest {
        cmd.arg("--out").arg(dir);
    }
    apply_play_env(&mut cmd, app, load_play_session(state).as_ref());
    let mut child = cmd
        .spawn()
        .map_err(|e| format!("couldn't start macn-play: {e}"))?;
    let stdout = child.stdout.take().ok_or("macn-play stdout missing")?;
    let reader = BufReader::new(stdout);
    let mut rows = Vec::new();
    for line in reader.lines() {
        let Ok(line) = line else { continue };
        let line = line.trim().to_string();
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&line) {
            if v.get("event").and_then(|x| x.as_str()) == Some("status") {
                if let Some(msg) = v.get("message").and_then(|x| x.as_str()) {
                    emit_status(app, msg);
                }
            }
            rows.push(v);
        }
    }
    let out = child.wait_with_output().map_err(|e| e.to_string())?;
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    for v in parse_json_lines(&String::from_utf8_lossy(&out.stdout)) {
        if !rows.iter().any(|r| r == &v) {
            rows.push(v);
        }
    }
    ingest_play_session(state, &rows);
    let code = out.status.code().unwrap_or(-1);
    if code != 0 {
        return Err(helper_error(
            &rows
                .iter()
                .map(|v| v.to_string())
                .collect::<Vec<_>>()
                .join("\n"),
            &stderr,
            code,
        ));
    }
    Ok(rows)
}

fn parse_play_apps(rows: &[serde_json::Value]) -> Vec<StoreApp> {
    let mut apps = Vec::new();
    for v in rows {
        let list = v.get("results").cloned().or_else(|| {
            if v.get("event").and_then(|x| x.as_str()) == Some("details") {
                v.get("app")
                    .cloned()
                    .map(|a| serde_json::Value::Array(vec![a]))
            } else {
                None
            }
        });
        let Some(serde_json::Value::Array(items)) = list else {
            continue;
        };
        for item in items {
            let pkg = item
                .get("package")
                .or_else(|| item.get("packageName"))
                .or_else(|| item.get("id"))
                .and_then(|x| x.as_str())
                .map(|s| s.to_string());
            let name = item
                .get("title")
                .or_else(|| item.get("name"))
                .and_then(|x| x.as_str())
                .unwrap_or("App")
                .to_string();
            let version = item
                .get("versionName")
                .or_else(|| item.get("version"))
                .and_then(|x| x.as_str())
                .map(|s| s.to_string());
            let developer = item
                .get("developer")
                .or_else(|| item.get("author"))
                .and_then(|x| x.as_str())
                .map(|s| s.to_string());
            let icon = item
                .get("icon")
                .or_else(|| item.get("iconUrl"))
                .and_then(|x| x.as_str())
                .map(|s| s.to_string());
            let price = item.get("price").map(|x| x.to_string());
            apps.push(StoreApp {
                name,
                bundle_id: None,
                package: pkg,
                version,
                id: None,
                price,
                icon,
                developer,
            });
        }
    }
    apps
}

#[tauri::command]
fn play_session_ready(state: State<AppState>) -> bool {
    load_play_session(&state).is_some()
}

#[tauri::command]
fn play_search(
    app: AppHandle,
    state: State<AppState>,
    query: String,
) -> Result<Vec<StoreApp>, String> {
    let q = query.trim();
    if q.is_empty() {
        return Err("type a package name, Play URL, or search words".into());
    }
    emit_status(&app, "Searching Google Play…");
    let rows = run_play(&app, &state, &["search", q, "--limit", "20"], None)?;
    let apps = parse_play_apps(&rows);
    emit_status(&app, &format!("Found {} app(s).", apps.len()));
    Ok(apps)
}

fn path_from_play_done(rows: &[serde_json::Value]) -> Option<PathBuf> {
    for v in rows {
        if v.get("event").and_then(|x| x.as_str()) == Some("done") {
            if let Some(p) = v.get("path").and_then(|x| x.as_str()) {
                return Some(PathBuf::from(p));
            }
        }
    }
    None
}

#[tauri::command]
fn play_download(
    app: AppHandle,
    state: State<AppState>,
    package: String,
    dest: String,
) -> Result<String, String> {
    let pkg = package.trim();
    if pkg.is_empty() {
        return Err("missing package name".into());
    }
    let (_label, dir) = resolve_dest(&dest)?;
    emit_status(&app, "Downloading APK…");
    let fmt = if java_available() { "auto" } else { "apks" };
    let rows = run_play(&app, &state, &["download", pkg, "--format", fmt], Some(&dir))?;
    let saved = if let Some(p) = path_from_play_done(&rows) {
        p
    } else {
        newest_file(&dir, &["apk", "apks", "xapk"])
            .ok_or_else(|| "download finished but the file path wasn't reported".to_string())?
    };
    let s = ensure_saved_path(&saved)?;
    emit_status(&app, &format!("Saved {s}"));
    Ok(s)
}

#[tauri::command]
fn play_merge(
    app: AppHandle,
    state: State<AppState>,
    input: String,
    dest: String,
) -> Result<String, String> {
    if !java_available() {
        return Err("Merging split APKs needs Java (a JRE or JDK). Without it, leave the .apks as they are.".into());
    }
    let input = PathBuf::from(input);
    if !input.exists() {
        return Err("that file doesn't exist".into());
    }
    let (_label, dir) = resolve_dest(&dest)?;
    emit_status(&app, "Merging split APKs…");
    let input_s = input.to_string_lossy().to_string();
    let rows = run_play(&app, &state, &["merge", &input_s], Some(&dir))?;
    let saved = if let Some(p) = path_from_play_done(&rows) {
        p
    } else {
        newest_file(&dir, &["apk"])
            .ok_or_else(|| "merge finished but the .apk path wasn't reported".to_string())?
    };
    let s = ensure_saved_path(&saved)?;
    emit_status(&app, &format!("Saved {s}"));
    Ok(s)
}

#[allow(dead_code)]
fn _dialog_touch(app: &AppHandle) {
    let _ = app.dialog();
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState {
            play: Mutex::new(None),
        })
        .setup(|app| {
            let title = brand_name();
            if let Some(w) = app.webview_windows().into_iter().next().map(|(_, w)| w) {
                let _ = w.set_title(title);
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            app_brand,
            save_folder,
            java_available,
            apple_status,
            apple_login,
            apple_logout,
            apple_search,
            apple_download,
            play_session_ready,
            play_search,
            play_download,
            play_merge
        ])
        .run(tauri::generate_context!())
        .expect("error while running WINN/LINCE");
}
