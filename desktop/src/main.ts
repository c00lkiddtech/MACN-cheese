import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";

type BrandInfo = { name: string; platform: string };
type Account = { name: string; email: string } | null;
type LoginResult = { needs2fa: boolean; account: Account; message: string };
type StoreApp = {
  name: string;
  bundleId?: string;
  package?: string;
  version?: string;
  id?: number | string;
  price?: string;
  icon?: string;
  developer?: string;
};
type Dest = "downloads" | "desktop";

const app = document.querySelector("#app")!;
let brand: BrandInfo = { name: "WINN-LINCE", platform: "unknown" };
let tab: "apple" | "android" | "credits" = "apple";
let dest: Dest = "downloads";
let destPath = "";
let account: Account = null;
let needs2fa = false;
let appleQuery = "";
let appleResults: StoreApp[] = [];
let playQuery = "";
let playResults: StoreApp[] = [];
let status = "";
let error = "";
let okmsg = "";
let busy = false;
let hasJava = false;
let playReady = false;

const $ = (html: string) => {
  const t = document.createElement("template");
  t.innerHTML = html.trim();
  return t.content;
};

function esc(s: string | undefined | null): string {
  return (s ?? "")
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}

async function refreshMeta() {
  brand = await invoke<BrandInfo>("app_brand");
  document.title = brand.name;
  const folder = await invoke<{ path: string; which: Dest }>("save_folder", { which: dest });
  destPath = folder.path;
  dest = folder.which;
  account = await invoke<Account>("apple_status");
  hasJava = await invoke<boolean>("java_available");
  playReady = await invoke<boolean>("play_session_ready");
}

function setMsg(ok: string, err: string, st = "") {
  okmsg = ok;
  error = err;
  status = st;
}

function render() {
  const letter = brand.name.slice(0, 1) || "W";
  const subtitle = brand.name === "WINN" ? "Windows" : brand.name === "LINCE" ? "Linux" : brand.platform;
  app.innerHTML = "";
  app.append($(
    `<header class="top">
      <div class="brand">
        <div class="mark">${esc(letter)}</div>
        <div>
          <div class="wordmark">${esc(brand.name)}</div>
          <div class="tagline">${esc(subtitle)} · downloads only</div>
        </div>
      </div>
      <nav class="tabs">
        <button data-tab="apple" class="${tab === "apple" ? "active" : ""}">App Store</button>
        <button data-tab="android" class="${tab === "android" ? "active" : ""}">Android</button>
        <button data-tab="credits" class="${tab === "credits" ? "active" : ""}">Credits</button>
      </nav>
    </header>
    <section class="panel" id="panel"></section>
    <div class="statusbar">
      <div>${esc(status)}${error ? `<div class="err">${esc(error)}</div>` : ""}${okmsg ? `<div class="okmsg">${esc(okmsg)}</div>` : ""}</div>
      <div>Save to <select id="dest"><option value="downloads"${dest === "downloads" ? " selected" : ""}>Downloads</option><option value="desktop"${dest === "desktop" ? " selected" : ""}>Desktop</option></select>
      <span class="path">${esc(destPath)}</span></div>
    </div>`
  ));

  const panel = app.querySelector("#panel")!;
  if (tab === "apple") panel.append(appleView());
  else if (tab === "android") panel.append(androidView());
  else panel.append(creditsView());

  app.querySelectorAll("nav.tabs button").forEach((b) => {
    b.addEventListener("click", () => {
      tab = (b as HTMLButtonElement).dataset.tab as typeof tab;
      render();
    });
  });
  app.querySelector("#dest")?.addEventListener("change", async (e) => {
    dest = (e.target as HTMLSelectElement).value as Dest;
    const folder = await invoke<{ path: string; which: Dest }>("save_folder", { which: dest });
    destPath = folder.path;
    dest = folder.which;
    render();
  });
}

function appleView() {
  const wrap = document.createElement("div");
  wrap.style.display = "flex";
  wrap.style.flexDirection = "column";
  wrap.style.gap = "14px";
  wrap.style.minHeight = "0";
  wrap.style.flex = "1";

  if (!account) {
    wrap.append($(`
      <div class="hint">Sign in with an Apple ID that already uses the App Store. The session is stored in the OS keyring by <code>ipatool</code>. This app never installs anything, and it does not remove FairPlay from IPAs.</div>
      <div class="row">
        <label class="field grow">Apple ID
          <input id="apple-email" type="text" autocomplete="username" placeholder="name@example.com" />
        </label>
        <label class="field grow">Password
          <input id="apple-pass" type="password" autocomplete="current-password" />
        </label>
        ${needs2fa ? `<label class="field">2FA code<input id="apple-2fa" type="text" inputmode="numeric" placeholder="000000" /></label>` : ""}
        <button class="btn primary" id="apple-login" ${busy ? "disabled" : ""}>${needs2fa ? "Finish sign-in" : "Sign in"}</button>
      </div>
    `));
  } else {
    wrap.append($(`
      <div class="row spread">
        <div class="pill ok">Signed in as ${esc(account.name || account.email)} · ${esc(account.email)}</div>
        <button class="btn ghost danger" id="apple-logout" ${busy ? "disabled" : ""}>Sign out</button>
      </div>
      <div class="row">
        <label class="field grow">Search the App Store
          <input id="apple-q" type="search" placeholder="App name or bundle id" value="${esc(appleQuery)}" />
        </label>
        <label class="field">Platform
          <select id="apple-platform">
            <option value="iphone">iPhone</option>
            <option value="ipad">iPad</option>
            <option value="macos">Mac</option>
          </select>
        </label>
        <button class="btn primary" id="apple-search" ${busy ? "disabled" : ""}>Search</button>
      </div>
    `));
  }

  const results = document.createElement("div");
  results.className = "results";
  if (appleResults.length === 0) {
    results.append($(`<div class="hint">Search for an app you already own, then download the IPA to ${esc(destPath) || "your Downloads or Desktop folder"}.</div>`));
  } else {
    for (const a of appleResults) {
      const card = document.createElement("div");
      card.className = "card";
      const initial = (a.name || "?").slice(0, 1).toUpperCase();
      card.innerHTML = `
        ${a.icon ? `<img src="${esc(a.icon)}" alt="" />` : `<div class="ph">${esc(initial)}</div>`}
        <div>
          <h3>${esc(a.name)}</h3>
          <p>${esc(a.bundleId || "")}${a.version ? ` · ${esc(a.version)}` : ""}${a.price ? ` · ${esc(a.price)}` : ""}</p>
          <p class="meta">${esc(a.developer || "")}</p>
        </div>
        <button class="btn primary" data-dl="${esc(a.bundleId || "")}" ${busy || !a.bundleId ? "disabled" : ""}>Download IPA</button>`;
      results.append(card);
    }
  }
  wrap.append(results);

  wrap.querySelector("#apple-login")?.addEventListener("click", onAppleLogin);
  wrap.querySelector("#apple-logout")?.addEventListener("click", onAppleLogout);
  wrap.querySelector("#apple-search")?.addEventListener("click", onAppleSearch);
  wrap.querySelector("#apple-q")?.addEventListener("keydown", (e) => {
    if ((e as KeyboardEvent).key === "Enter") onAppleSearch();
  });
  results.querySelectorAll("[data-dl]").forEach((b) => {
    b.addEventListener("click", () => onAppleDownload((b as HTMLButtonElement).dataset.dl || ""));
  });
  return wrap;
}

function androidView() {
  const wrap = document.createElement("div");
  wrap.style.display = "flex";
  wrap.style.flexDirection = "column";
  wrap.style.gap = "14px";
  wrap.style.minHeight = "0";
  wrap.style.flex = "1";
  wrap.append($(`
    <div class="row spread">
      <div class="pill ${playReady ? "ok" : ""}">${playReady ? "Anonymous Aurora session ready" : "Anonymous Aurora session (no Google account)"}</div>
      <div class="pill ${hasJava ? "ok" : "warn"}">${hasJava ? "Java found — split APKs will be merged into one .apk" : "No Java — split apps will be saved as .apks. Install a JRE to merge."}</div>
    </div>
    <div class="hint">Downloads use a public Aurora OSS session, never your Google account. Files are only saved to Downloads or Desktop. Nothing is installed.</div>
    <div class="row">
      <label class="field grow">Search Google Play or paste a package / Play URL
        <input id="play-q" type="search" placeholder="com.example.app or app name" value="${esc(playQuery)}" />
      </label>
      <button class="btn primary" id="play-search" ${busy ? "disabled" : ""}>Search</button>
      <button class="btn" id="play-merge" ${busy ? "disabled" : ""}>Convert .apks / .xapk…</button>
    </div>
  `));
  const results = document.createElement("div");
  results.className = "results";
  if (playResults.length === 0) {
    results.append($(`<div class="hint">Free apps only. Paid Play apps can't be downloaded with the anonymous session.</div>`));
  } else {
    for (const a of playResults) {
      const card = document.createElement("div");
      card.className = "card";
      const initial = (a.name || "?").slice(0, 1).toUpperCase();
      const pkg = a.package || "";
      card.innerHTML = `
        ${a.icon ? `<img src="${esc(a.icon)}" alt="" />` : `<div class="ph">${esc(initial)}</div>`}
        <div>
          <h3>${esc(a.name)}</h3>
          <p>${esc(pkg)}${a.version ? ` · ${esc(a.version)}` : ""}</p>
          <p class="meta">${esc(a.developer || a.price || "")}</p>
        </div>
        <button class="btn primary" data-dl="${esc(pkg)}" ${busy || !pkg ? "disabled" : ""}>Download</button>`;
      results.append(card);
    }
  }
  wrap.append(results);
  wrap.querySelector("#play-search")?.addEventListener("click", onPlaySearch);
  wrap.querySelector("#play-q")?.addEventListener("keydown", (e) => {
    if ((e as KeyboardEvent).key === "Enter") onPlaySearch();
  });
  wrap.querySelector("#play-merge")?.addEventListener("click", onPlayMerge);
  results.querySelectorAll("[data-dl]").forEach((b) => {
    b.addEventListener("click", () => onPlayDownload((b as HTMLButtonElement).dataset.dl || ""));
  });
  return wrap;
}

function creditsView() {
  const el = document.createElement("div");
  el.className = "credits";
  el.innerHTML = `
    <div>
      <h2>Credits &amp; licenses</h2>
      <p>${esc(brand.name)} is a Windows/Linux companion to <strong>MACN-cheese</strong>, itself a macOS port of iPadn. It only downloads packages. It never installs apps, and it never strips FairPlay or other DRM.</p>
    </div>
    <div>
      <h3>iPadn</h3>
      <p>The original app is <a href="https://github.com/mengangs/ipadn" target="_blank" rel="noreferrer">iPadn</a> by mengangs and contributors, licensed <strong>GPL-2.0-or-later</strong>. ${esc(brand.name)} is an unofficial companion and isn't affiliated with iPadn or Apple.</p>
    </div>
    <div>
      <h3>ipatool</h3>
      <p>App Store sign-in, search and IPA downloads are performed by a bundled copy of <a href="https://github.com/majd/ipatool" target="_blank" rel="noreferrer">ipatool</a> by Majd Alfhaily, licensed <strong>MIT</strong>. It is a separate program. Apple ID sessions stay in the OS keyring.</p>
    </div>
    <div>
      <h3>macn-play (Google Play helper)</h3>
      <p>Android downloads run in a separate <code>macn-play</code> process (GPL-3.0-or-later) because it embeds Aurora OSS device profiles.</p>
      <ul>
        <li><a href="https://github.com/EFForg/apkeep" target="_blank" rel="noreferrer">apkeep</a> by William Budington / EFF — <strong>MIT</strong>. The download flow follows apkeep.</li>
        <li><a href="https://github.com/EFForg/rs-google-play" target="_blank" rel="noreferrer">rs-google-play</a> (<code>gpapi</code>, <code>googleplay-protobuf</code>) by Bill Budington / EFF and David Weinstein — <strong>MIT</strong>.</li>
        <li><a href="https://gitlab.com/AuroraOSS/AuroraStore" target="_blank" rel="noreferrer">Aurora Store</a> by AuroraOSS — device profiles and the anonymous session dispenser, <strong>GPL-3.0-or-later</strong>. No personal Google account is used.</li>
        <li><a href="https://github.com/REAndroid/APKEditor" target="_blank" rel="noreferrer">APKEditor</a> by REAndroid — <strong>Apache-2.0</strong>. Merges base + split APKs into one .apk when Java is installed.</li>
        <li><a href="https://github.com/patrickfav/uber-apk-signer" target="_blank" rel="noreferrer">uber-apk-signer</a> by Patrick Favre-Bulle — <strong>Apache-2.0</strong>. Zipaligns and signs the merged .apk with a local debug key.</li>
      </ul>
    </div>
    <div>
      <h3>This app</h3>
      <p>${esc(brand.name)} itself is licensed <strong>GPL-2.0-or-later</strong>, same family as iPadn and MACN-cheese. macn-play remains a separate GPL-3.0-or-later program shipped alongside it. Source, notices and license texts are in the <a href="https://github.com/c00lkiddtech/WINN-LINCE" target="_blank" rel="noreferrer">WINN-LINCE repository</a>.</p>
    </div>
  `;
  return el;
}

async function withBusy(fn: () => Promise<void>) {
  if (busy) return;
  busy = true;
  render();
  try {
    await fn();
  } catch (e) {
    setMsg("", String(e));
  } finally {
    busy = false;
    render();
  }
}

async function onAppleLogin() {
  const email = (document.querySelector("#apple-email") as HTMLInputElement | null)?.value.trim() ?? "";
  const password = (document.querySelector("#apple-pass") as HTMLInputElement | null)?.value ?? "";
  const authCode = (document.querySelector("#apple-2fa") as HTMLInputElement | null)?.value.trim() ?? "";
  await withBusy(async () => {
    setMsg("", "", "Signing in…");
    const res = await invoke<LoginResult>("apple_login", { email, password, authCode });
    needs2fa = res.needs2fa;
    account = res.account;
    if (needs2fa) setMsg("", "Enter the 6-digit code from a trusted device, then finish sign-in.");
    else setMsg(res.message || "Signed in.", "");
  });
}

async function onAppleLogout() {
  await withBusy(async () => {
    await invoke("apple_logout");
    account = null;
    needs2fa = false;
    appleResults = [];
    setMsg("Signed out.", "");
  });
}

async function onAppleSearch() {
  appleQuery = (document.querySelector("#apple-q") as HTMLInputElement | null)?.value.trim() ?? appleQuery;
  const platform = (document.querySelector("#apple-platform") as HTMLSelectElement | null)?.value ?? "iphone";
  await withBusy(async () => {
    setMsg("", "", "Searching the App Store…");
    appleResults = await invoke<StoreApp[]>("apple_search", { query: appleQuery, platform });
    setMsg(appleResults.length ? `Found ${appleResults.length}` : "No apps found.", "");
  });
}

async function onAppleDownload(bundleId: string) {
  const platform = (document.querySelector("#apple-platform") as HTMLSelectElement | null)?.value ?? "iphone";
  await withBusy(async () => {
    setMsg("", "", "Downloading IPA…");
    const path = await invoke<string>("apple_download", { bundleId, platform, dest });
    setMsg(`Saved ${path}`, "");
  });
}

async function onPlaySearch() {
  playQuery = (document.querySelector("#play-q") as HTMLInputElement | null)?.value.trim() ?? playQuery;
  await withBusy(async () => {
    setMsg("", "", "Searching Google Play…");
    playResults = await invoke<StoreApp[]>("play_search", { query: playQuery });
    playReady = true;
    setMsg(playResults.length ? `Found ${playResults.length}` : "No apps found.", "");
  });
}

async function onPlayDownload(packageName: string) {
  await withBusy(async () => {
    setMsg("", "", "Downloading APK…");
    const path = await invoke<string>("play_download", { package: packageName, dest });
    setMsg(`Saved ${path}`, "");
  });
}

async function onPlayMerge() {
  const selected = await open({
    multiple: false,
    title: "Choose an .apks or .xapk bundle",
    filters: [{ name: "Split APK bundles", extensions: ["apks", "xapk"] }],
  });
  const input = Array.isArray(selected) ? selected[0] : selected;
  if (!input) return;
  await withBusy(async () => {
    setMsg("", "", "Merging split APKs…");
    const path = await invoke<string>("play_merge", { input, dest });
    setMsg(`Saved ${path}`, "");
  });
}

async function boot() {
  await listen<string>("helper-status", (e) => {
    status = e.payload;
    const bar = document.querySelector(".statusbar > div");
    if (bar) bar.childNodes[0].textContent = status;
  });
  try {
    await refreshMeta();
  } catch (e) {
    error = String(e);
  }
  render();
}

boot();
