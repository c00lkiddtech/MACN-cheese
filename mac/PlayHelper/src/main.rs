//! macn-play: the Google Play helper bundled inside MACN-cheese.app.
//!
//! MACN-cheese (a Mac Catalyst app) runs this as a child process and reads one
//! JSON object per line from its stdout. The Play protocol itself is
//! `gpapi` / `googleplay-protobuf` from EFForg/rs-google-play (MIT, vendored in
//! mac/Vendor/rs-google-play); the login / ToS / split-APK download flow follows
//! EFForg/apkeep's google_play.rs (MIT). Anonymous sessions come from the public
//! Aurora OSS token dispenser that Aurora Store uses, so no personal Google
//! account is ever needed or touched.
//!
//! Commands
//!   macn-play dispense
//!   macn-play details  <package|play url>
//!   macn-play search   <keywords> [--limit N]
//!   macn-play download <package|play url> --out DIR [--version-code N]
//!                      [--no-splits] [--obb] [--format auto|apk|apks|xapk|folder]
//!
//! Session (everything except `dispense`) comes from the environment, never argv:
//!   MACN_PLAY_EMAIL, MACN_PLAY_AUTH_TOKEN, optional MACN_PLAY_DEVICE (px_9a),
//!   MACN_PLAY_LOCALE (en_US), MACN_PLAY_TIMEZONE (UTC).
//!
//! Exit codes: 0 ok, 2 session rejected (caller should dispense a new one), 1 other error.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::{Duration, Instant};

use googleplay_protobuf::Item;
use gpapi::error::ErrorKind as GpErrorKind;
use gpapi::Gpapi;
use serde_json::{json, Value};

const DEFAULT_DEVICE: &str = "px_9a";
const DISPENSER_URL: &str = "https://auroraoss.com/api/auth";
/// The dispenser only answers clients that identify like Aurora Store does.
const DISPENSER_USER_AGENT: &str = "com.aurora.store-4.8.4-76";
const WEB_USER_AGENT: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.0 Safari/605.1.15";
const DEVICE_PROPERTIES: &str = include_str!("../../Vendor/rs-google-play/gpapi/device.properties");

// ---------------------------------------------------------------------------
// Output

fn emit(v: Value) {
    let mut out = io::stdout().lock();
    let _ = writeln!(out, "{}", v);
    let _ = out.flush();
}

#[derive(Debug)]
struct Failure {
    kind: &'static str,
    message: String,
}

impl Failure {
    fn new(kind: &'static str, message: impl Into<String>) -> Self {
        Failure { kind, message: message.into() }
    }
    fn exit_code(&self) -> i32 {
        if self.kind == "auth" { 2 } else { 1 }
    }
}

type Res<T> = Result<T, Failure>;

// ---------------------------------------------------------------------------
// Arguments

struct Args {
    positional: Vec<String>,
    flags: BTreeMap<String, Option<String>>,
}

const VALUE_FLAGS: &[&str] = &["--out", "--version-code", "--format", "--limit", "--device"];

fn parse_args(raw: Vec<String>) -> Res<Args> {
    let mut positional = Vec::new();
    let mut flags = BTreeMap::new();
    let mut it = raw.into_iter();
    while let Some(a) = it.next() {
        if a.starts_with("--") {
            if VALUE_FLAGS.contains(&a.as_str()) {
                let v = it.next().ok_or_else(|| Failure::new("usage", format!("{} needs a value", a)))?;
                flags.insert(a, Some(v));
            } else {
                flags.insert(a, None);
            }
        } else {
            positional.push(a);
        }
    }
    Ok(Args { positional, flags })
}

impl Args {
    fn value(&self, k: &str) -> Option<&str> {
        self.flags.get(k).and_then(|v| v.as_deref())
    }
    fn has(&self, k: &str) -> bool {
        self.flags.contains_key(k)
    }
}

/// Accepts `com.example.app`, `com.example.app@123`, or any Play Store URL with `id=`.
pub fn normalize_package(input: &str) -> Option<String> {
    let s = input.trim();
    let candidate = if let Some(pos) = s.find("id=") {
        let rest = &s[pos + 3..];
        rest.split(|c| c == '&' || c == '#' || c == '/').next().unwrap_or("").to_string()
    } else {
        s.to_string()
    };
    let re = regex::Regex::new(r"^[A-Za-z][A-Za-z0-9_]*(\.[A-Za-z0-9_]+)+$").unwrap();
    if re.is_match(&candidate) { Some(candidate) } else { None }
}

// ---------------------------------------------------------------------------
// Session

fn device_name() -> String {
    std::env::var("MACN_PLAY_DEVICE").ok().filter(|s| !s.is_empty()).unwrap_or_else(|| DEFAULT_DEVICE.into())
}

/// The [section] of gpapi's device.properties as a flat map, which is the body
/// the dispenser expects (Aurora Store posts its spoofed `Properties` the same way).
pub fn device_properties_json(device: &str) -> Option<Value> {
    let mut map = serde_json::Map::new();
    let mut in_section = false;
    for line in DEVICE_PROPERTIES.lines() {
        let line = line.trim_end();
        if line.starts_with('[') && line.ends_with(']') {
            if in_section { break; }
            in_section = &line[1..line.len() - 1] == device;
            continue;
        }
        if !in_section || line.starts_with('#') || line.is_empty() { continue; }
        if let Some((k, v)) = line.split_once('=') {
            map.insert(k.to_string(), Value::String(v.replace("\\:", ":")));
        }
    }
    if map.is_empty() { None } else { Some(Value::Object(map)) }
}

/// Asks the Aurora OSS dispenser for an anonymous session and announces it on
/// stdout as {"event":"session",...} so MACN-cheese can keep it in the Keychain.
async fn dispense_session() -> Res<(String, String)> {
    // The public dispenser hiccups (HTTP 5xx) now and then; try a few times.
    let mut last = Failure::new("dispenser", "token dispenser unavailable");
    for attempt in 0..3u64 {
        if attempt > 0 { tokio::time::sleep(Duration::from_millis(1500 * attempt)).await; }
        match dispense_once().await {
            Ok(v) => return Ok(v),
            Err(f) if f.kind == "usage" => return Err(f),
            Err(f) => last = f,
        }
    }
    Err(last)
}

async fn dispense_once() -> Res<(String, String)> {
    let device = device_name();
    let body = device_properties_json(&device)
        .ok_or_else(|| Failure::new("usage", format!("unknown device profile {}", device)))?;
    let client = reqwest::Client::builder()
        .user_agent(DISPENSER_USER_AGENT)
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|e| Failure::new("network", e.to_string()))?;
    let resp = client
        .post(DISPENSER_URL)
        .header("Content-Type", "application/json")
        .body(body.to_string())
        .send()
        .await
        .map_err(|e| Failure::new("network", format!("token dispenser unreachable: {}", e)))?;
    let status = resp.status();
    let text = resp.text().await.map_err(|e| Failure::new("network", e.to_string()))?;
    if !status.is_success() {
        let kind = if status.as_u16() == 429 { "rateLimited" } else { "dispenser" };
        return Err(Failure::new(kind, format!("token dispenser answered HTTP {}", status.as_u16())));
    }
    let v: Value = serde_json::from_str(&text).map_err(|_| Failure::new("dispenser", "token dispenser sent an unexpected reply"))?;
    let email = v.get("email").and_then(Value::as_str).unwrap_or_default().to_string();
    let token = v.get("authToken").and_then(Value::as_str).unwrap_or_default().to_string();
    if email.is_empty() || token.is_empty() {
        return Err(Failure::new("dispenser", "token dispenser reply had no session"));
    }
    emit(json!({"event": "session", "email": email, "authToken": token, "device": device}));
    Ok((email, token))
}

async fn dispense() -> Res<()> {
    dispense_session().await.map(|_| ())
}

async fn login_with(email: &str, token: &str) -> Res<Gpapi> {
    let mut gpa = Gpapi::new(device_name(), email.to_string());
    if let Ok(l) = std::env::var("MACN_PLAY_LOCALE") { if !l.is_empty() { gpa.set_locale(l); } }
    if let Ok(t) = std::env::var("MACN_PLAY_TIMEZONE") { if !t.is_empty() { gpa.set_timezone(t); } }
    gpa.set_auth_token(token.to_string());
    if let Err(err) = gpa.login().await {
        match err.kind() {
            GpErrorKind::TermsOfService => {
                // Same as apkeep --accept-tos: the dispensed account is anonymous and shared.
                gpa.accept_tos().await.map_err(|e| Failure::new("tos", format!("couldn't accept Google Play terms: {}", e)))?;
            }
            _ => return Err(Failure::new("auth", format!("Google Play rejected the session: {}", err))),
        }
    }
    Ok(gpa)
}

/// Logs in with the session from the environment, or a freshly dispensed one when
/// there is none or Google rejects it. Returns whether the session is fresh.
async fn login() -> Res<(Gpapi, bool)> {
    let email = std::env::var("MACN_PLAY_EMAIL").unwrap_or_default();
    let token = std::env::var("MACN_PLAY_AUTH_TOKEN").unwrap_or_default();
    if !email.is_empty() && !token.is_empty() {
        match login_with(&email, &token).await {
            Ok(gpa) => return Ok((gpa, false)),
            Err(f) if f.kind == "auth" => {
                emit(json!({"event": "status", "stage": "login", "message": "Refreshing the anonymous Google Play session…"}));
            }
            Err(f) => return Err(f),
        }
    }
    let (e, t) = dispense_session().await?;
    Ok((login_with(&e, &t).await?, true))
}

async fn fresh_login() -> Res<Gpapi> {
    let (e, t) = dispense_session().await?;
    login_with(&e, &t).await
}

// ---------------------------------------------------------------------------
// Details / search

const IMAGE_TYPE_ICON: i32 = 4;

pub fn item_summary(item: &Item) -> Value {
    let app = item.details.as_ref().and_then(|d| d.app_details.as_ref());
    let icon = item
        .image
        .iter()
        .find(|i| i.image_type == Some(IMAGE_TYPE_ICON))
        .or_else(|| item.image.first())
        .and_then(|i| i.image_url.clone());
    let offer = item.offer.first();
    let micros = offer.and_then(|o| o.micros).unwrap_or(0);
    json!({
        "package": app.and_then(|a| a.package_name.clone()).or_else(|| item.id.clone()),
        "title": item.title.clone().or_else(|| app.and_then(|a| a.title.clone())),
        "developer": app.and_then(|a| a.developer_name.clone()).or_else(|| item.creator.clone()),
        "versionCode": app.and_then(|a| a.version_code),
        "versionName": app.and_then(|a| a.version_string.clone()),
        "size": app.and_then(|a| a.info_download_size),
        "updated": app.and_then(|a| a.info_updated_on.clone()),
        "downloads": app.and_then(|a| a.download_label_abbreviated.clone().or(a.download_label.clone())),
        "targetSdk": app.and_then(|a| a.target_sdk_version),
        "category": app.and_then(|a| a.category_name.clone()),
        "rating": item.aggregate_rating.as_ref().and_then(|r| r.star_rating),
        "icon": icon,
        "free": micros == 0,
        "price": offer.and_then(|o| o.formatted_amount.clone()),
        "splits": app.map(|a| a.split_id.clone()).unwrap_or_default(),
        "description": item.promotional_description.clone(),
    })
}

async fn details(args: &Args) -> Res<()> {
    let pkg = args.positional.get(1).and_then(|s| normalize_package(s))
        .ok_or_else(|| Failure::new("usage", "give a package name or a Play Store link"))?;
    let (gpa, _) = login().await?;
    let v = fetch_details(&gpa, &pkg).await?;
    emit(json!({"event": "details", "app": v}));
    Ok(())
}

async fn fetch_details(gpa: &Gpapi, pkg: &str) -> Res<Value> {
    let resp = gpa.details(pkg).await.map_err(|e| Failure::new("network", e.to_string()))?;
    let item = resp.and_then(|r| r.item)
        .ok_or_else(|| Failure::new("notFound", format!("{} isn't on Google Play (or not in this region)", pkg)))?;
    Ok(item_summary(&item))
}

pub fn url_encode(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
            b' ' => "+".to_string(),
            _ => format!("%{:02X}", b),
        })
        .collect()
}

/// Package ids, in order, from a play.google.com search results page.
pub fn parse_search_ids(html: &str, limit: usize) -> Vec<String> {
    let re = regex::Regex::new(r"/store/apps/details\?id=([A-Za-z0-9._]+)").unwrap();
    let mut seen = std::collections::HashSet::new();
    let mut ids = Vec::new();
    for cap in re.captures_iter(html) {
        let id = cap[1].to_string();
        if seen.insert(id.clone()) {
            ids.push(id);
            if ids.len() >= limit { break; }
        }
    }
    ids
}

async fn search(args: &Args) -> Res<()> {
    let query = args.positional[1..].join(" ");
    if query.trim().is_empty() {
        return Err(Failure::new("usage", "type something to search for"));
    }
    // A link or package name skips the keyword search.
    if let Some(pkg) = normalize_package(&query).filter(|_| query.contains('.') && !query.contains(' ')) {
        let (gpa, _) = login().await?;
        let v = fetch_details(&gpa, &pkg).await?;
        emit(json!({"event": "search", "results": [v]}));
        return Ok(());
    }
    let limit: usize = args.value("--limit").and_then(|s| s.parse().ok()).unwrap_or(20).clamp(1, 50);
    let client = reqwest::Client::builder()
        .user_agent(WEB_USER_AGENT)
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|e| Failure::new("network", e.to_string()))?;
    let html = client
        .get(format!("https://play.google.com/store/search?q={}&c=apps&hl=en&gl=US", url_encode(&query)))
        .send()
        .await
        .map_err(|e| Failure::new("network", e.to_string()))?
        .text()
        .await
        .map_err(|e| Failure::new("network", e.to_string()))?;
    let ids = parse_search_ids(&html, limit);
    if ids.is_empty() {
        emit(json!({"event": "search", "results": []}));
        return Ok(());
    }
    let (gpa, _) = login().await?;
    let refs: Vec<&str> = ids.iter().map(String::as_str).collect();
    let bulk = gpa.bulk_details(&refs).await.map_err(|e| Failure::new("network", e.to_string()))?;
    let mut by_id: BTreeMap<String, Value> = BTreeMap::new();
    for entry in bulk.map(|b| b.entry).unwrap_or_default() {
        if let Some(item) = entry.item {
            let v = item_summary(&item);
            if let Some(p) = v["package"].as_str() { by_id.insert(p.to_string(), v); }
        }
    }
    let results: Vec<Value> = ids
        .iter()
        .map(|id| by_id.remove(id).unwrap_or_else(|| json!({"package": id})))
        .collect();
    emit(json!({"event": "search", "results": results}));
    Ok(())
}

// ---------------------------------------------------------------------------
// Download

pub fn safe_name(s: &str) -> String {
    let cleaned: String = s
        .chars()
        .map(|c| if "/\\:?%*|\"<>".contains(c) || c.is_control() { ' ' } else { c })
        .collect();
    let trimmed = cleaned.split_whitespace().collect::<Vec<_>>().join(" ");
    let t: String = trimmed.chars().take(120).collect();
    if t.is_empty() { "app".into() } else { t }
}

pub fn unique_path(dir: &Path, stem: &str, ext: Option<&str>) -> PathBuf {
    let make = |n: usize| {
        let base = if n == 1 { stem.to_string() } else { format!("{} ({})", stem, n) };
        match ext { Some(e) => dir.join(format!("{}.{}", base, e)), None => dir.join(base) }
    };
    let mut n = 1;
    loop {
        let p = make(n);
        if !p.exists() { return p; }
        n += 1;
    }
}

#[derive(Default)]
struct Progress {
    files: BTreeMap<String, (u64, u64)>,
    last: Option<Instant>,
}

fn collect_files(dir: &Path, out: &mut Vec<PathBuf>) -> io::Result<()> {
    for e in fs::read_dir(dir)? {
        let p = e?.path();
        if p.is_dir() { collect_files(&p, out)?; } else { out.push(p); }
    }
    Ok(())
}

struct Downloaded {
    base: PathBuf,
    splits: Vec<(String, PathBuf)>,
    obbs: Vec<PathBuf>,
}

/// Sorts what gpapi wrote: `<pkg>.apk`, `<pkg>.<split>.apk`, and `*.obb`.
pub fn classify(pkg: &str, files: Vec<PathBuf>) -> Option<(PathBuf, Vec<(String, PathBuf)>, Vec<PathBuf>)> {
    let mut base = None;
    let mut splits = Vec::new();
    let mut obbs = Vec::new();
    let base_name = format!("{}.apk", pkg);
    let prefix = format!("{}.", pkg);
    for f in files {
        let name = f.file_name().and_then(|n| n.to_str()).unwrap_or_default().to_string();
        if name == base_name {
            base = Some(f);
        } else if name.ends_with(".apk") && name.starts_with(&prefix) {
            let split = name[prefix.len()..name.len() - 4].to_string();
            splits.push((split, f));
        } else if name.ends_with(".obb") {
            obbs.push(f);
        }
    }
    splits.sort_by(|a, b| a.0.cmp(&b.0));
    base.map(|b| (b, splits, obbs))
}

fn zip_files(target: &Path, entries: &[(String, PathBuf)], manifest: Option<&Value>) -> io::Result<()> {
    use zip::write::SimpleFileOptions;
    let file = File::create(target)?;
    let mut zip = zip::ZipWriter::new(file);
    if let Some(m) = manifest {
        zip.start_file("manifest.json", SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated))
            .map_err(io::Error::other)?;
        zip.write_all(serde_json::to_string_pretty(m).unwrap().as_bytes())?;
    }
    // APKs are already compressed; storing them keeps the archive fast to build and install.
    let stored = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored).large_file(true);
    for (name, path) in entries {
        zip.start_file(name.as_str(), stored).map_err(io::Error::other)?;
        let mut f = File::open(path)?;
        io::copy(&mut f, &mut zip)?;
    }
    zip.finish().map_err(io::Error::other)?;
    Ok(())
}

async fn download(args: &Args) -> Res<()> {
    let pkg = args.positional.get(1).and_then(|s| normalize_package(s))
        .ok_or_else(|| Failure::new("usage", "give a package name or a Play Store link"))?;
    let out = PathBuf::from(args.value("--out").ok_or_else(|| Failure::new("usage", "--out DIR is required"))?);
    fs::create_dir_all(&out).map_err(|e| Failure::new("io", format!("can't create {}: {}", out.display(), e)))?;
    let version_code: Option<i64> = match args.value("--version-code") {
        Some(v) if !v.is_empty() => Some(v.parse().map_err(|_| Failure::new("usage", "version code must be a number"))?),
        _ => None,
    };
    let want_splits = !args.has("--no-splits");
    let want_obb = args.has("--obb");
    let format = args.value("--format").unwrap_or("auto").to_string();
    if !["auto", "apk", "apks", "xapk", "folder"].contains(&format.as_str()) {
        return Err(Failure::new("usage", "format must be auto, apk, apks, xapk or folder"));
    }

    emit(json!({"event": "status", "stage": "login", "message": "Connecting to Google Play…"}));
    let (mut gpa, mut fresh) = login().await?;
    emit(json!({"event": "status", "stage": "details", "message": "Reading app details…"}));
    let info = fetch_details(&gpa, &pkg).await?;
    if info["free"] == json!(false) {
        return Err(Failure::new("paid", format!("{} is a paid app; the anonymous Play session can only download free apps", pkg)));
    }
    let title = info["title"].as_str().unwrap_or(&pkg).to_string();
    let version_name = info["versionName"].as_str().unwrap_or("").to_string();
    let vc_display = version_code.or(info["versionCode"].as_i64());

    let tmp = out.join(format!(".macn-play-{}-{}", pkg, std::process::id()));
    let _ = fs::remove_dir_all(&tmp);
    fs::create_dir_all(&tmp).map_err(|e| Failure::new("io", e.to_string()))?;
    struct Cleanup(PathBuf);
    impl Drop for Cleanup { fn drop(&mut self) { let _ = fs::remove_dir_all(&self.0); } }
    let _cleanup = Cleanup(tmp.clone());

    emit(json!({"event": "status", "stage": "download", "message": "Downloading…"}));
    let progress = Rc::new(RefCell::new(Progress::default()));
    let p_outer = Rc::clone(&progress);
    let cb: Box<dyn Fn(String, u64) -> Box<dyn Fn(u64)>> = Box::new(move |name: String, total: u64| {
        p_outer.borrow_mut().files.insert(name.clone(), (0, total));
        let p_inner = Rc::clone(&p_outer);
        Box::new(move |pos: u64| {
            let mut p = p_inner.borrow_mut();
            if let Some(e) = p.files.get_mut(&name) { e.0 = pos.min(e.1.max(pos)); }
            let now = Instant::now();
            let due = p.last.map_or(true, |l| now.duration_since(l) >= Duration::from_millis(200));
            if due {
                p.last = Some(now);
                let (done, total) = p.files.values().fold((0u64, 0u64), |a, v| (a.0 + v.0, a.1 + v.1));
                emit(json!({"event": "progress", "file": name, "done": done, "total": total, "files": p.files.len()}));
            }
        })
    });
    let mut result = gpa.download(&pkg, version_code, want_splits, false, want_obb, &tmp, Some(&cb)).await;
    // Pooled dispenser accounts go stale: a reused session can start answering
    // "invalid app" for everything. Retry once on a brand-new session.
    if !fresh && matches!(result.as_ref().map_err(|e| e.kind()), Err(GpErrorKind::InvalidApp) | Err(GpErrorKind::LoginRequired) | Err(GpErrorKind::Authentication)) {
        emit(json!({"event": "status", "stage": "login", "message": "Refreshing the anonymous Google Play session…"}));
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).map_err(|e| Failure::new("io", e.to_string()))?;
        progress.borrow_mut().files.clear();
        gpa = fresh_login().await?;
        fresh = true;
        result = gpa.download(&pkg, version_code, want_splits, false, want_obb, &tmp, Some(&cb)).await;
    }
    let _ = fresh;
    if let Err(err) = result {
        return Err(match err.kind() {
            GpErrorKind::InvalidApp => Failure::new("notAvailable", match version_code {
                Some(v) => format!("Google Play won't serve version {} of {} (it usually only serves the current one)", v, pkg),
                None => format!("Google Play won't serve {} to this device profile or region", pkg),
            }),
            GpErrorKind::LoginRequired | GpErrorKind::Authentication => Failure::new("auth", err.to_string()),
            GpErrorKind::PermissionDenied => Failure::new("io", format!("can't write to {}", out.display())),
            _ => Failure::new("download", err.to_string()),
        });
    }
    {
        let p = progress.borrow();
        let (_, total) = p.files.values().fold((0u64, 0u64), |a, v| (a.0 + v.0, a.1 + v.1));
        emit(json!({"event": "progress", "file": "", "done": total, "total": total, "files": p.files.len()}));
    }

    let mut files = Vec::new();
    collect_files(&tmp, &mut files).map_err(|e| Failure::new("io", e.to_string()))?;
    let (base, splits, obbs) = classify(&pkg, files)
        .ok_or_else(|| Failure::new("download", "Google Play didn't send a base APK"))?;
    let got = Downloaded { base, splits, obbs };

    emit(json!({"event": "status", "stage": "package", "message": "Saving…"}));
    let stem = safe_name(&if version_name.is_empty() { title.clone() } else { format!("{} {}", title, version_name) });
    let effective = match format.as_str() {
        "auto" => if got.splits.is_empty() && got.obbs.is_empty() { "apk" } else { "apks" },
        "apk" if !got.splits.is_empty() => {
            emit(json!({"event": "status", "stage": "package", "message": "This app ships as split APKs, so it was saved as .apks (install with SAI or adb install-multiple)."}));
            "apks"
        }
        other => other,
    }.to_string();
    let io_err = |e: io::Error| Failure::new("io", e.to_string());
    let saved: PathBuf = match effective.as_str() {
        "apk" => {
            let target = unique_path(&out, &stem, Some("apk"));
            fs::rename(&got.base, &target).map_err(io_err)?;
            target
        }
        "apks" => {
            let target = unique_path(&out, &stem, Some("apks"));
            let mut entries = vec![("base.apk".to_string(), got.base.clone())];
            for (s, p) in &got.splits { entries.push((format!("split_{}.apk", s), p.clone())); }
            for p in &got.obbs {
                let n = p.file_name().unwrap().to_string_lossy().to_string();
                entries.push((format!("Android/obb/{}/{}", pkg, n), p.clone()));
            }
            zip_files(&target, &entries, None).map_err(io_err)?;
            target
        }
        "xapk" => {
            let target = unique_path(&out, &stem, Some("xapk"));
            let mut entries = vec![(format!("{}.apk", pkg), got.base.clone())];
            let mut split_list = vec![json!({"file": format!("{}.apk", pkg), "id": "base"})];
            for (s, p) in &got.splits {
                entries.push((format!("{}.apk", s), p.clone()));
                split_list.push(json!({"file": format!("{}.apk", s), "id": s}));
            }
            let mut expansions = Vec::new();
            for p in &got.obbs {
                let n = p.file_name().unwrap().to_string_lossy().to_string();
                let inner = format!("Android/obb/{}/{}", pkg, n);
                expansions.push(json!({"file": inner, "install_location": "EXTERNAL_STORAGE", "install_path": inner}));
                entries.push((inner, p.clone()));
            }
            let total: u64 = entries.iter().filter_map(|(_, p)| fs::metadata(p).ok()).map(|m| m.len()).sum();
            let manifest = json!({
                "xapk_version": 2,
                "package_name": pkg,
                "name": title,
                "version_code": vc_display.map(|v| v.to_string()).unwrap_or_default(),
                "version_name": version_name,
                "target_sdk_version": info["targetSdk"].as_i64().map(|v| v.to_string()).unwrap_or_default(),
                "split_configs": got.splits.iter().map(|(s, _)| s.clone()).collect::<Vec<_>>(),
                "split_apks": split_list,
                "expansions": expansions,
                "total_size": total,
            });
            zip_files(&target, &entries, Some(&manifest)).map_err(io_err)?;
            target
        }
        _ => {
            let target = unique_path(&out, &stem, None);
            fs::create_dir_all(&target).map_err(io_err)?;
            fs::rename(&got.base, target.join(format!("{}.apk", pkg))).map_err(io_err)?;
            for (s, p) in &got.splits { fs::rename(p, target.join(format!("{}.{}.apk", pkg, s))).map_err(io_err)?; }
            for p in &got.obbs { fs::rename(p, target.join(p.file_name().unwrap())).map_err(io_err)?; }
            target
        }
    };
    let size = if saved.is_dir() {
        let mut v = Vec::new();
        let _ = collect_files(&saved, &mut v);
        v.iter().filter_map(|p| fs::metadata(p).ok()).map(|m| m.len()).sum()
    } else {
        fs::metadata(&saved).map(|m| m.len()).unwrap_or(0)
    };
    emit(json!({
        "event": "done",
        "path": saved.to_string_lossy(),
        "format": effective,
        "size": size,
        "package": pkg,
        "title": title,
        "versionName": version_name,
        "versionCode": vc_display,
        "splitCount": got.splits.len(),
        "obbCount": got.obbs.len(),
    }));
    Ok(())
}

// ---------------------------------------------------------------------------

fn usage() -> Failure {
    Failure::new("usage", "usage: macn-play dispense | details <pkg> | search <words> [--limit N] | download <pkg> --out DIR [--version-code N] [--no-splits] [--obb] [--format auto|apk|apks|xapk|folder]")
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let raw: Vec<String> = std::env::args().skip(1).collect();
    let outcome = async {
        let args = parse_args(raw)?;
        match args.positional.first().map(String::as_str) {
            Some("dispense") => dispense().await,
            Some("details") => details(&args).await,
            Some("search") => search(&args).await,
            Some("download") => download(&args).await,
            Some("version") | Some("--version") => {
                emit(json!({"event": "version", "version": env!("CARGO_PKG_VERSION")}));
                Ok(())
            }
            _ => Err(usage()),
        }
    }
    .await;
    if let Err(f) = outcome {
        emit(json!({"event": "error", "kind": f.kind, "message": f.message}));
        std::process::exit(f.exit_code());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packages_and_links() {
        assert_eq!(normalize_package("org.mozilla.firefox").as_deref(), Some("org.mozilla.firefox"));
        assert_eq!(
            normalize_package("https://play.google.com/store/apps/details?id=com.spotify.music&hl=en_US").as_deref(),
            Some("com.spotify.music")
        );
        assert_eq!(normalize_package("market://details?id=com.whatsapp").as_deref(), Some("com.whatsapp"));
        assert_eq!(normalize_package("not a package"), None);
        assert_eq!(normalize_package("firefox"), None);
    }

    #[test]
    fn search_ids_dedupe() {
        let html = r#"<a href="/store/apps/details?id=a.b">x</a><a href="/store/apps/details?id=a.b"></a><a href="/store/apps/details?id=c.d_e">"#;
        assert_eq!(parse_search_ids(html, 10), vec!["a.b", "c.d_e"]);
        assert_eq!(parse_search_ids(html, 1), vec!["a.b"]);
    }

    #[test]
    fn device_profile_loads() {
        let v = device_properties_json("px_9a").unwrap();
        assert_eq!(v["Build.MODEL"], "Pixel 9a");
        assert!(v["Build.FINGERPRINT"].as_str().unwrap().contains("tegu:15"));
        assert!(device_properties_json("nope").is_none());
    }

    #[test]
    fn classify_files() {
        let files = vec![
            PathBuf::from("/t/org.x/org.x.apk"),
            PathBuf::from("/t/org.x/org.x.config.arm64_v8a.apk"),
            PathBuf::from("/t/org.x/main.12.org.x.obb"),
        ];
        let (b, s, o) = classify("org.x", files).unwrap();
        assert!(b.ends_with("org.x.apk"));
        assert_eq!(s[0].0, "config.arm64_v8a");
        assert_eq!(o.len(), 1);
    }

    #[test]
    fn names() {
        assert_eq!(safe_name("Fire/fox: 1.0"), "Fire fox 1.0");
        assert_eq!(safe_name("   "), "app");
    }
}
