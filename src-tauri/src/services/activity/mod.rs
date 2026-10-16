// Service « activité » (cf. docs/internal/activity-spec.md) : ce qui tourne et ce que tu
// fais — jeu en cours, app au premier plan, temps passé. UN service à base d'événements :
//
//   fil `activity-fg`      : hook `EVENT_SYSTEM_FOREGROUND` (boucle de messages, coût nul
//                            au repos) → classement + icône à chaque changement de premier plan ;
//   fil `activity-scan`    : toutes les 2 s, relevé Toolhelp (pid + nom, aucun OpenProcess)
//                            → fin du jeu, temps passé, heuristique GPU, écriture chaque minute ;
//   fil `activity-catalog` : catalogue des launchers + base Discord (démarrage, rafraîchissements).
//
// Classement (par exe, mis en cache) : correction utilisateur > catalogue > Discord >
// heuristique > app. `activity://changed` (ActivityState) n'est émis qu'à l'overlay,
// dédoublonné. Permission `activity` pour les extensions (`activity_ext_*`).

use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};

mod catalog;
mod discord;
mod gpu;
mod history;
mod scan;

use catalog::{norm_path, Catalog};
use discord::DiscordDb;
use history::{Bucket, History, Stats};

pub const EVENT: &str = "activity://changed";
/// Inactivité au-delà de laquelle le temps d'app s'arrête (pas le temps de jeu).
const IDLE_MS: u64 = 5 * 60 * 1000;
/// Plafond d'un intervalle compté (veille, fil gelé…).
const MAX_STEP_MS: u64 = 10_000;
const TICK: Duration = Duration::from_secs(2);

// --- Types du contrat (camelCase côté JS) --------------------------------------------------

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Game,
    App,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum Source {
    Steam,
    Epic,
    Gog,
    Ea,
    Ubisoft,
    Battlenet,
    Xbox,
    Discord,
    Heuristic,
    User,
    App,
}

impl Source {
    fn as_str(self) -> &'static str {
        match self {
            Source::Steam => "steam",
            Source::Epic => "epic",
            Source::Gog => "gog",
            Source::Ea => "ea",
            Source::Ubisoft => "ubisoft",
            Source::Battlenet => "battlenet",
            Source::Xbox => "xbox",
            Source::Discord => "discord",
            Source::Heuristic => "heuristic",
            Source::User => "user",
            Source::App => "app",
        }
    }
}

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct App {
    pub pid: u32,
    pub exe: String,
    pub path: String,
    pub title: String,
    pub name: String,
    pub kind: Kind,
    pub source: Source,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub art: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GameSession {
    pub key: String,
    pub name: String,
    pub source: Source,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub art: Option<String>,
    pub pid: u32,
    /// ms epoch.
    pub started_at: u64,
}

#[derive(Serialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ActivityState {
    pub foreground: Option<App>,
    pub game: Option<GameSession>,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum OvKind {
    Game,
    App,
    Ignore,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Override {
    pub path: String,
    pub kind: OvKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

/// `<app_data>/activity/settings.json`.
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase", default)]
struct Settings {
    track_apps: bool,
    discord_db: bool,
    /// Clé = chemin normalisé (minuscules).
    overrides: BTreeMap<String, Override>,
    /// Dernière tentative de téléchargement de la base Discord (s epoch).
    discord_checked_at: u64,
}

impl Default for Settings {
    fn default() -> Self {
        Self { track_apps: false, discord_db: true, overrides: BTreeMap::new(), discord_checked_at: 0 }
    }
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PublicSettings {
    pub track_apps: bool,
    pub discord_db: bool,
}

#[derive(Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct SettingsPatch {
    #[serde(default)]
    pub track_apps: Option<bool>,
    #[serde(default)]
    pub discord_db: Option<bool>,
}

// --- Classement (pur, testé) ---------------------------------------------------------------

/// Classement de base d'un exe (sans correction utilisateur).
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Class {
    kind: Kind,
    source: Source,
    app_id: Option<String>,
    name: String,
    art: Option<String>,
    /// Éligible à l'heuristique GPU (app inconnue, ni système ni exclue).
    candidate: bool,
}

impl Class {
    /// Clé stable : `source:appId`, sinon le chemin normalisé.
    fn key(&self, lower: &str) -> String {
        match &self.app_id {
            Some(id) => format!("{}:{id}", self.source.as_str()),
            None => lower.to_string(),
        }
    }
}

/// Sondes disque d'un exe (une fois par chemin) : jamais le process lui-même.
#[derive(Clone, Debug, Default)]
pub(crate) struct Probe {
    description: Option<String>,
    xbox: Option<(String, Option<String>)>,
    /// Signature de moteur (`*-Win64-Shipping.exe`, Unity).
    engine: bool,
}

pub(crate) struct Ctx<'a> {
    catalog: &'a Catalog,
    discord: Option<&'a DiscordDb>,
    /// Nom retenu si l'heuristique a déjà désigné cet exe comme jeu.
    heuristic: Option<&'a str>,
    own_exe: &'a str,
    windir: &'a str,
}

fn file_name(lower: &str) -> &str {
    lower.rsplit(['\\', '/']).next().unwrap_or(lower)
}

fn stem(path: &str) -> String {
    let f = path.rsplit(['\\', '/']).next().unwrap_or(path);
    let s = f.strip_suffix(".exe").or_else(|| f.strip_suffix(".EXE")).unwrap_or(f);
    // `Foo-Win64-Shipping` → `Foo`.
    match s.to_lowercase().find("-win64-shipping").or_else(|| s.to_lowercase().find("-wingdk-shipping")) {
        Some(i) if i > 0 => s.get(..i).unwrap_or(s).to_string(),
        _ => s.to_string(),
    }
}

/// Exclus d'office : système, Island, navigateurs, explorateur, launchers.
fn excluded(lower: &str, ctx: &Ctx) -> bool {
    let exe = file_name(lower);
    lower.starts_with(ctx.windir)
        || lower == ctx.own_exe
        || exe == "explorer.exe"
        || catalog::BROWSERS.contains(&exe)
        || catalog::LAUNCHERS.contains(&exe)
}

pub(crate) fn classify(path: &str, probe: &Probe, ctx: &Ctx) -> Class {
    let lower = norm_path(path);
    let exe = file_name(&lower);
    let fallback = probe.description.clone().unwrap_or_else(|| stem(path));
    let app = |candidate: bool| Class {
        kind: Kind::App,
        source: Source::App,
        app_id: None,
        name: fallback.clone(),
        art: None,
        candidate,
    };
    if excluded(&lower, ctx) || catalog::is_helper(exe) {
        return app(false);
    }
    let game = |source, app_id, name: String, art| Class { kind: Kind::Game, source, app_id, name, art, candidate: false };
    if let Some(e) = ctx.catalog.lookup(&lower) {
        return game(e.source, e.app_id.clone(), e.name.clone(), e.art.clone());
    }
    if let Some((name, id)) = &probe.xbox {
        return game(Source::Xbox, id.clone(), name.clone(), None);
    }
    if let Some(d) = ctx.discord.and_then(|d| d.find(&lower)) {
        return game(Source::Discord, Some(d.id.clone()), d.name.clone(), d.art.clone());
    }
    if let Some(n) = ctx.heuristic {
        let name = if n.trim().is_empty() { fallback.clone() } else { n.to_string() };
        return game(Source::Heuristic, None, name, None);
    }
    app(true)
}

/// Correction utilisateur (prioritaire). `None` = ignoré.
pub(crate) fn apply_override(base: &Class, ov: Option<&Override>) -> Option<Class> {
    let Some(ov) = ov else { return Some(base.clone()) };
    let mut c = base.clone();
    match ov.kind {
        OvKind::Ignore => return None,
        // Déjà un jeu connu : on garde son identité (clé, jaquette), seul le nom change.
        OvKind::Game if c.kind == Kind::Game => {}
        OvKind::Game | OvKind::App => {
            c.kind = if ov.kind == OvKind::Game { Kind::Game } else { Kind::App };
            c.source = Source::User;
            c.app_id = None;
            c.art = None;
        }
    }
    if let Some(n) = ov.name.as_deref().map(str::trim).filter(|n| !n.is_empty()) {
        c.name = n.to_string();
    }
    c.candidate = false;
    Some(c)
}

fn probe(path: &str, system: bool) -> Probe {
    let description = scan::file_description(path);
    if system {
        return Probe { description, ..Default::default() };
    }
    let p = std::path::Path::new(path);
    let lower = path.to_lowercase();
    let engine = lower.ends_with("-shipping.exe")
        || p.parent().is_some_and(|d| {
            d.join("UnityPlayer.dll").is_file()
                || p.file_stem().is_some_and(|s| d.join(format!("{}_Data", s.to_string_lossy())).is_dir())
        });
    Probe { description, xbox: catalog::xbox_probe(path), engine }
}

// --- État partagé --------------------------------------------------------------------------

struct Fg {
    pid: u32,
    hwnd: isize,
    path: String,
    lower: String,
    title: String,
    icon: Option<String>,
    base: Class,
}

struct GameRef {
    session: GameSession,
    exe: String,
    lower: String,
}

struct Core {
    dir: Option<PathBuf>,
    settings: Settings,
    catalog: Arc<Catalog>,
    discord: Option<Arc<DiscordDb>>,
    probes: HashMap<String, Arc<Probe>>,
    classes: HashMap<String, Class>,
    heuristic: HashMap<String, String>,
    icons: HashMap<String, Option<String>>,
    fg: Option<Fg>,
    game: Option<GameRef>,
    /// Dernier relevé : pid → nom d'exe (minuscules).
    procs: HashMap<u32, String>,
    history: History,
    offset_s: i64,
    last_ms: u64,
    last_sig: String,
    own_exe: String,
    windir: String,
}

static CORE: OnceLock<Mutex<Core>> = OnceLock::new();
static APP: OnceLock<AppHandle> = OnceLock::new();

fn lock() -> Option<MutexGuard<'static, Core>> {
    CORE.get().map(|m| m.lock().unwrap_or_else(|p| p.into_inner()))
}

fn now_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

fn app_of(pid: u32, path: &str, title: &str, icon: Option<String>, c: &Class) -> App {
    App {
        pid,
        exe: path.rsplit(['\\', '/']).next().unwrap_or(path).to_string(),
        path: path.to_string(),
        title: title.to_string(),
        name: c.name.clone(),
        kind: c.kind,
        source: c.source,
        app_id: c.app_id.clone(),
        art: c.art.clone(),
        icon,
    }
}

impl Core {
    fn effective(&self, lower: &str, base: &Class) -> Option<Class> {
        apply_override(base, self.settings.overrides.get(lower))
    }

    fn state(&self) -> ActivityState {
        let foreground = self.fg.as_ref().and_then(|f| {
            self.effective(&f.lower, &f.base).map(|c| app_of(f.pid, &f.path, &f.title, f.icon.clone(), &c))
        });
        ActivityState { foreground, game: self.game.as_ref().map(|g| g.session.clone()) }
    }

    /// État à émettre s'il a changé (identité du premier plan ou du jeu ; pas le titre).
    fn change(&mut self) -> Option<ActivityState> {
        let s = self.state();
        let sig = format!(
            "{:?}|{:?}",
            s.foreground.as_ref().map(|a| (a.pid, &a.path, a.kind, a.source, &a.name, &a.app_id)),
            s.game.as_ref().map(|g| (&g.key, g.pid, g.started_at, &g.name)),
        );
        if sig == self.last_sig {
            return None;
        }
        self.last_sig = sig;
        Some(s)
    }

    /// Compte le temps écoulé depuis le dernier passage au premier plan courant.
    fn account(&mut self, now: u64, idle: bool) {
        let from = std::mem::replace(&mut self.last_ms, now);
        if from == 0 || now <= from {
            return;
        }
        let from = from.max(now - MAX_STEP_MS);
        let Some(f) = &self.fg else { return };
        let Some(c) = self.effective(&f.lower, &f.base) else { return };
        let key = c.key(&f.lower);
        match c.kind {
            Kind::Game => self.history.add(Bucket::Games, &key, &c.name, from, now, self.offset_s),
            Kind::App if self.settings.track_apps && !idle => {
                self.history.add(Bucket::Apps, &key, &c.name, from, now, self.offset_s)
            }
            Kind::App => {}
        }
    }

    /// Premier plan = jeu → session (même clé : on garde le début, sinon nouvelle).
    fn update_game(&mut self, now: u64) {
        // Le jeu courant a pu être corrigé en app / ignoré.
        if let Some(g) = &self.game {
            let still = self.classes.get(&g.lower).and_then(|b| self.effective(&g.lower, b)).map(|c| c.kind);
            if still.is_some_and(|k| k != Kind::Game) || (still.is_none() && self.settings.overrides.contains_key(&g.lower)) {
                self.game = None;
            }
        }
        let Some(f) = &self.fg else { return };
        let Some(c) = self.effective(&f.lower, &f.base).filter(|c| c.kind == Kind::Game) else { return };
        let key = c.key(&f.lower);
        let started_at = match &self.game {
            Some(g) if g.session.key == key => g.session.started_at,
            _ => now,
        };
        self.game = Some(GameRef {
            session: GameSession {
                key,
                name: c.name.clone(),
                source: c.source,
                app_id: c.app_id.clone(),
                art: c.art.clone(),
                pid: f.pid,
                started_at,
            },
            exe: file_name(&f.lower).to_string(),
            lower: f.lower.clone(),
        });
    }

    fn save_settings(&self) {
        let Some(d) = &self.dir else { return };
        if let Ok(json) = serde_json::to_vec_pretty(&self.settings) {
            if let Err(e) = history::write_atomic(&d.join("settings.json"), &json) {
                eprintln!("[activity] réglages : {e}");
            }
        }
    }

    fn public_settings(&self) -> PublicSettings {
        PublicSettings { track_apps: self.settings.track_apps, discord_db: self.settings.discord_db }
    }

    fn flush(&mut self) {
        let ym = history::date_of(now_ms(), self.offset_s)[..7].to_string();
        self.history.flush(&ym);
    }
}

fn emit(s: Option<ActivityState>) {
    if let (Some(s), Some(app)) = (s, APP.get()) {
        let _ = app.emit_to("overlay", EVENT, s);
        // Le launcher se suspend quand une app exclue passe au premier plan : réévaluation
        // immédiate, sans attendre son propre relevé.
        crate::launcher::hotkey::reevaluate(app);
    }
}

/// Exécutable (minuscules) de l'app au premier plan, hors fenêtres d'Island.
pub fn foreground_exe() -> Option<String> {
    lock().and_then(|c| c.state().foreground.map(|f| f.exe.to_lowercase()))
}

/// Classement de base d'un exe (cache par chemin ; sondes disque hors verrou).
fn base_class(path: &str) -> Class {
    let lower = norm_path(path);
    let (catalog, discord, heur, probe, own, windir) = {
        let Some(c) = lock() else {
            return Class { kind: Kind::App, source: Source::App, app_id: None, name: stem(path), art: None, candidate: false };
        };
        if let Some(cl) = c.classes.get(&lower) {
            return cl.clone();
        }
        (
            c.catalog.clone(),
            c.discord.clone(),
            c.heuristic.get(&lower).cloned(),
            c.probes.get(&lower).cloned(),
            c.own_exe.clone(),
            c.windir.clone(),
        )
    };
    let probe = probe.unwrap_or_else(|| Arc::new(self::probe(path, lower.starts_with(&windir))));
    let ctx = Ctx { catalog: &catalog, discord: discord.as_deref(), heuristic: heur.as_deref(), own_exe: &own, windir: &windir };
    let cl = classify(path, &probe, &ctx);
    if let Some(mut c) = lock() {
        if c.probes.len() > 2048 {
            c.probes.clear();
        }
        if c.classes.len() > 2048 {
            c.classes.clear();
        }
        c.probes.insert(lower.clone(), probe);
        c.classes.insert(lower, cl.clone());
    }
    cl
}

/// Icône (cache par chemin). COM doit être initialisé sur le fil appelant.
fn icon_for(path: &str) -> Option<String> {
    let lower = norm_path(path);
    if let Some(i) = lock().and_then(|c| c.icons.get(&lower).cloned()) {
        return i;
    }
    let icon = scan::icon(path);
    if let Some(mut c) = lock() {
        if c.icons.len() > 256 {
            c.icons.clear();
        }
        c.icons.insert(lower, icon.clone());
    }
    icon
}

// --- Fil premier plan ----------------------------------------------------------------------

fn on_foreground(hwnd: isize) {
    let pid = scan::window_pid(hwnd);
    if pid == 0 || pid == std::process::id() {
        return; // Island lui-même : le premier plan « sous » l'île reste
    }
    let path = if scan::is_shell_window(hwnd) { None } else { scan::image_path(pid) };
    let fg = path.map(|path| {
        let base = base_class(&path);
        Fg {
            pid,
            hwnd,
            lower: norm_path(&path),
            title: scan::window_title(hwnd),
            icon: icon_for(&path),
            base,
            path,
        }
    });
    let idle = scan::idle_ms() > IDLE_MS;
    let out = lock().and_then(|mut c| {
        let now = now_ms();
        c.account(now, idle);
        c.fg = fg;
        c.update_game(now);
        c.change()
    });
    emit(out);
}

/// Reclasse le premier plan après un changement de catalogue / base / heuristique.
fn reclassify(clear: bool) {
    let path = lock().and_then(|mut c| {
        if clear {
            c.classes.clear();
        }
        c.fg.as_ref().map(|f| (f.pid, f.path.clone()))
    });
    let Some((pid, path)) = path else { return };
    let base = base_class(&path);
    let out = lock().and_then(|mut c| {
        match c.fg.as_mut() {
            Some(f) if f.pid == pid => f.base = base,
            _ => return None,
        }
        c.update_game(now_ms());
        c.change()
    });
    emit(out);
}

// --- Fil de relevé -------------------------------------------------------------------------

#[derive(Default)]
struct Heur {
    gpu: Option<gpu::Gpu>,
    sustain: gpu::Sustain,
}

/// Fin du jeu : son pid a disparu → autre process du même exe et de même identité
/// (relance), sinon plus de jeu.
fn check_game(c: &mut Core) {
    let Some(g) = &c.game else { return };
    if c.procs.contains_key(&g.session.pid) {
        return;
    }
    let (exe, lower, key) = (g.exe.clone(), g.lower.clone(), g.session.key.clone());
    let same: Vec<u32> = c.procs.iter().filter(|(_, e)| **e == exe).map(|(p, _)| *p).collect();
    for pid in same {
        let Some(p) = scan::image_path(pid) else { continue };
        let pl = norm_path(&p);
        let k = c.classes.get(&pl).and_then(|b| c.effective(&pl, b)).map(|cl| cl.key(&pl));
        if pl == lower || k.as_deref() == Some(key.as_str()) {
            if let Some(g) = c.game.as_mut() {
                g.session.pid = pid;
            }
            return;
        }
    }
    c.game = None;
}

fn tick(h: &mut Heur) {
    let snap = scan::snapshot();
    let idle = scan::idle_ms() > IDLE_MS;
    let offset = scan::local_offset_secs();
    let fg_hwnd = lock().and_then(|c| c.fg.as_ref().map(|f| f.hwnd));
    let title = fg_hwnd.map(scan::window_title);

    // Candidat heuristique : app inconnue au premier plan.
    let (out, cand) = match lock() {
        Some(mut c) => {
            c.offset_s = offset;
            c.procs = snap.into_iter().map(|(p, e)| (p, e.to_lowercase())).collect();
            c.account(now_ms(), idle);
            if let (Some(f), Some(t)) = (c.fg.as_mut(), title) {
                if !t.is_empty() {
                    f.title = t;
                }
            }
            check_game(&mut *c);
            let cand = c.fg.as_ref().and_then(|f| {
                let ov = c.settings.overrides.contains_key(&f.lower);
                (f.base.candidate && !ov).then(|| {
                    let engine = c.probes.get(&f.lower).is_some_and(|p| p.engine);
                    (f.pid, f.hwnd, f.path.clone(), f.title.clone(), engine)
                })
            });
            (c.change(), cand)
        }
        None => (None, None),
    };
    emit(out);

    // Heuristique GPU : requête PDH ouverte seulement si le candidat est plein écran ou
    // porte une signature de moteur.
    match cand.filter(|(_, hwnd, _, _, engine)| *engine || scan::is_fullscreen(*hwnd)) {
        Some((pid, _, path, title, _)) => {
            if h.gpu.is_none() {
                h.gpu = gpu::Gpu::open();
                h.sustain.reset();
                return; // premier relevé = référence
            }
            let util = h.gpu.as_ref().map(|g| g.sample().get(&pid).copied().unwrap_or(0.0)).unwrap_or(0.0);
            if h.sustain.feed(pid, util) {
                h.gpu = None;
                h.sustain.reset();
                if let Some(mut c) = lock() {
                    let lower = norm_path(&path);
                    c.heuristic.insert(lower.clone(), title);
                    c.classes.remove(&lower);
                }
                reclassify(false);
            }
        }
        None => {
            h.gpu = None;
            h.sustain.reset();
        }
    }
}

fn scan_loop() {
    let mut h = Heur::default();
    let mut last_flush = Instant::now();
    let mut last_day = String::new();
    let mut last_discord = Instant::now();
    loop {
        std::thread::sleep(TICK);
        tick(&mut h);
        if last_flush.elapsed() >= Duration::from_secs(60) {
            last_flush = Instant::now();
            let day = lock().map(|mut c| {
                if c.history.is_dirty() {
                    c.flush();
                }
                history::date_of(now_ms(), c.offset_s)
            });
            // Purge au démarrage puis à chaque changement de jour.
            if let Some(day) = day.filter(|d| *d != last_day) {
                if let Some(mut c) = lock() {
                    c.history.purge(&day[..7]);
                }
                last_day = day;
            }
        }
        if last_discord.elapsed() >= Duration::from_secs(discord::RETRY_SECS) {
            last_discord = Instant::now();
            std::thread::spawn(discord_job);
        }
    }
}

// --- Catalogue & base Discord --------------------------------------------------------------

fn build_catalog() -> usize {
    let cat = Arc::new(catalog::build());
    let n = cat.entries.len();
    if let Some(mut c) = lock() {
        c.catalog = cat;
    }
    reclassify(true);
    n
}

fn secs() -> u64 {
    now_ms() / 1000
}

/// Charge le cache, et le rafraîchit au plus une fois par semaine (réglage `discordDb`).
fn discord_job() {
    static BUSY: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    if BUSY.swap(true, std::sync::atomic::Ordering::SeqCst) {
        return;
    }
    struct Done;
    impl Drop for Done {
        fn drop(&mut self) {
            BUSY.store(false, std::sync::atomic::Ordering::SeqCst);
        }
    }
    let _done = Done;

    let Some((enabled, dir, checked, loaded)) =
        lock().map(|c| (c.settings.discord_db, c.dir.clone(), c.settings.discord_checked_at, c.discord.is_some()))
    else {
        return;
    };
    if !enabled {
        if loaded {
            if let Some(mut c) = lock() {
                c.discord = None;
            }
            reclassify(true);
        }
        return;
    }
    let Some(dir) = dir else { return };
    let file = dir.join("detectable.json");
    if !loaded {
        if let Some(list) = discord::load_cache(&file) {
            let db = Arc::new(DiscordDb::build(list));
            if let Some(mut c) = lock() {
                c.discord = Some(db);
            }
            reclassify(true);
        }
    }
    let age = std::fs::metadata(&file)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.elapsed().ok())
        .map(|d| d.as_secs())
        .unwrap_or(u64::MAX);
    if age < discord::MAX_AGE_SECS || secs().saturating_sub(checked) < discord::RETRY_SECS {
        return;
    }
    if let Some(mut c) = lock() {
        c.settings.discord_checked_at = secs();
        c.save_settings();
    }
    match discord::download() {
        Ok(list) => {
            if let Ok(json) = serde_json::to_vec(&list) {
                let _ = history::write_atomic(&file, &json);
            }
            let db = Arc::new(DiscordDb::build(list));
            let still = lock().map(|mut c| {
                let on = c.settings.discord_db;
                if on {
                    c.discord = Some(db);
                }
                on
            });
            if still == Some(true) {
                reclassify(true);
            }
        }
        // Échec : on garde l'ancien cache.
        Err(e) => eprintln!("[activity] base Discord : {e}"),
    }
}

// --- Démarrage -----------------------------------------------------------------------------

pub fn start(app: AppHandle) {
    let dir = app.path().app_data_dir().ok().map(|d| d.join("activity"));
    let settings: Settings = dir
        .as_ref()
        .and_then(|d| std::fs::read(d.join("settings.json")).ok())
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default();
    let own_exe = std::env::current_exe().map(|p| norm_path(&p.to_string_lossy())).unwrap_or_default();
    let windir = format!(
        "{}\\",
        norm_path(&std::env::var("WINDIR").unwrap_or_else(|_| "C:\\Windows".into())).trim_end_matches('\\')
    );
    let core = Core {
        history: History::new(dir.clone()),
        dir,
        settings,
        catalog: Arc::new(Catalog::default()),
        discord: None,
        probes: HashMap::new(),
        classes: HashMap::new(),
        heuristic: HashMap::new(),
        icons: HashMap::new(),
        fg: None,
        game: None,
        procs: HashMap::new(),
        offset_s: scan::local_offset_secs(),
        last_ms: 0,
        last_sig: String::new(),
        own_exe,
        windir,
    };
    if CORE.set(Mutex::new(core)).is_err() {
        return; // déjà démarré
    }
    let _ = APP.set(app.clone());

    // Écriture de l'historique à la fermeture (mini-plugin : `lib.rs` n'a pas de boucle
    // d'événements à nous).
    let plugin = tauri::plugin::Builder::<tauri::Wry>::new("island-activity")
        .on_event(|_, ev| {
            if matches!(ev, tauri::RunEvent::Exit) {
                if let Some(mut c) = lock() {
                    let now = now_ms();
                    c.account(now, scan::idle_ms() > IDLE_MS);
                    c.flush();
                }
            }
        })
        .build();
    if let Err(e) = app.plugin(plugin) {
        eprintln!("[activity] hook de fermeture : {e}");
    }

    let _ = std::thread::Builder::new().name("activity-catalog".into()).spawn(|| {
        build_catalog();
        discord_job();
    });
    scan::spawn_foreground_hook(on_foreground);
    let _ = std::thread::Builder::new().name("activity-scan".into()).spawn(scan_loop);
}

// --- Commandes -----------------------------------------------------------------------------

fn require(app: &AppHandle, id: &str) -> Result<(), String> {
    crate::extensions::services::require(app, id, "activity")
}

#[tauri::command]
pub fn activity_state() -> ActivityState {
    lock().map(|c| c.state()).unwrap_or_default()
}

/// Apps avec fenêtre visible (une ligne par exe), pour corriger le classement. Les apps
/// ignorées apparaissent (kind `app`, source `user`) pour pouvoir être rétablies.
#[tauri::command]
pub async fn activity_running() -> Result<Vec<App>, String> {
    tauri::async_runtime::spawn_blocking(|| {
        scan::com_init();
        let me = std::process::id();
        let windir = lock().map(|c| c.windir.clone()).unwrap_or_default();
        let mut seen = std::collections::HashSet::new();
        let mut out = Vec::new();
        for (pid, _, title) in scan::visible_windows() {
            if pid == me {
                continue;
            }
            let Some(path) = scan::image_path(pid) else { continue };
            let lower = norm_path(&path);
            if (!windir.is_empty() && lower.starts_with(&windir)) || !seen.insert(lower.clone()) {
                continue;
            }
            let base = base_class(&path);
            let eff = lock().and_then(|c| c.effective(&lower, &base)).unwrap_or_else(|| Class {
                kind: Kind::App,
                source: Source::User,
                app_id: None,
                art: None,
                candidate: false,
                ..base.clone()
            });
            out.push(app_of(pid, &path, &title, icon_for(&path), &eff));
        }
        out.sort_by_key(|a| a.name.to_lowercase());
        out
    })
    .await
    .map_err(|e| e.to_string())
}

/// Correction par chemin d'exe : `kind` = "game" | "app" | "ignore", ou null pour retirer.
#[tauri::command]
pub fn activity_set_override(path: String, kind: Option<String>, name: Option<String>) -> Result<(), String> {
    let lower = norm_path(&path);
    if lower.is_empty() {
        return Err("badPath".into());
    }
    let kind = match kind.as_deref() {
        None => None,
        Some("game") => Some(OvKind::Game),
        Some("app") => Some(OvKind::App),
        Some("ignore") => Some(OvKind::Ignore),
        Some(_) => return Err("badKind".into()),
    };
    let name = name.map(|n| n.trim().chars().take(120).collect::<String>()).filter(|n| !n.is_empty());
    let out = {
        let Some(mut c) = lock() else { return Err("notReady".into()) };
        match kind {
            Some(kind) => c.settings.overrides.insert(lower, Override { path: path.trim().to_string(), kind, name }),
            None => c.settings.overrides.remove(&lower),
        };
        c.save_settings();
        let now = now_ms();
        c.update_game(now);
        c.change()
    };
    emit(out);
    Ok(())
}

#[tauri::command]
pub fn activity_overrides() -> Vec<Override> {
    lock().map(|c| c.settings.overrides.values().cloned().collect()).unwrap_or_default()
}

#[tauri::command]
pub fn activity_stats(from: String, to: String) -> Result<Stats, String> {
    lock().ok_or("notReady")?.history.stats(&from, &to)
}

#[tauri::command]
pub fn activity_settings() -> PublicSettings {
    lock().map(|c| c.public_settings()).unwrap_or(PublicSettings { track_apps: false, discord_db: true })
}

#[tauri::command]
pub fn activity_settings_set(settings: SettingsPatch) -> Result<PublicSettings, String> {
    let (out, discord_changed) = {
        let mut c = lock().ok_or("notReady")?;
        if let Some(t) = settings.track_apps {
            c.settings.track_apps = t;
        }
        let changed = settings.discord_db.is_some_and(|d| d != c.settings.discord_db);
        if let Some(d) = settings.discord_db {
            c.settings.discord_db = d;
        }
        c.save_settings();
        (c.public_settings(), changed)
    };
    if discord_changed {
        std::thread::spawn(discord_job);
    }
    Ok(out)
}

/// Re-scan des launchers (léger) ; renvoie le nombre de jeux catalogués.
#[tauri::command]
pub async fn activity_refresh_catalog() -> Result<usize, String> {
    tauri::async_runtime::spawn_blocking(build_catalog).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub fn activity_ext_state(app: AppHandle, id: String) -> Result<ActivityState, String> {
    require(&app, &id)?;
    Ok(activity_state())
}

#[tauri::command]
pub fn activity_ext_stats(app: AppHandle, id: String, from: String, to: String) -> Result<Stats, String> {
    require(&app, &id)?;
    activity_stats(from, to)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cat() -> Catalog {
        let mut c = Catalog::default();
        c.push("D:\\SteamLibrary\\steamapps\\common\\ELDEN RING", "ELDEN RING", Source::Steam, Some("1245620".into()), Some("art".into()));
        // Dossier qui contient (à tort) un launcher : il ne doit pas devenir un jeu.
        c.push("C:\\Program Files (x86)\\Steam", "Steam", Source::Steam, Some("0".into()), None);
        c
    }

    fn db() -> DiscordDb {
        DiscordDb::build(vec![
            discord::CacheApp { id: "9".into(), name: "Elden (Discord)".into(), icon: None, exes: vec!["game/eldenring.exe".into()] },
            discord::CacheApp { id: "7".into(), name: "Hades".into(), icon: None, exes: vec!["hades/x64/hades.exe".into()] },
            discord::CacheApp { id: "8".into(), name: "Chrome?".into(), icon: None, exes: vec!["chrome.exe".into()] },
        ])
    }

    fn ctx<'a>(c: &'a Catalog, d: &'a DiscordDb, h: Option<&'a str>) -> Ctx<'a> {
        Ctx { catalog: c, discord: Some(d), heuristic: h, own_exe: "c:\\apps\\island\\island.exe", windir: "c:\\windows\\" }
    }

    const ER: &str = "D:\\SteamLibrary\\steamapps\\common\\ELDEN RING\\Game\\eldenring.exe";

    #[test]
    fn precedence() {
        let (c, d) = (cat(), db());
        let p = Probe::default();
        // Catalogue > Discord.
        let cl = classify(ER, &p, &ctx(&c, &d, Some("Heur")));
        assert_eq!((cl.kind, cl.source, cl.name.as_str()), (Kind::Game, Source::Steam, "ELDEN RING"));
        assert_eq!(cl.key(&norm_path(ER)), "steam:1245620");
        // Discord > heuristique.
        let hades = "E:\\Games\\Hades\\x64\\Hades.exe";
        let cl = classify(hades, &p, &ctx(&c, &d, Some("Heur")));
        assert_eq!((cl.source, cl.name.as_str()), (Source::Discord, "Hades"));
        // Heuristique > app.
        let x = "E:\\Indie\\Foo-Win64-Shipping.exe";
        let cl = classify(x, &p, &ctx(&c, &d, Some("Foo Game")));
        assert_eq!((cl.kind, cl.source, cl.name.as_str()), (Kind::Game, Source::Heuristic, "Foo Game"));
        assert_eq!(cl.key(&norm_path(x)), norm_path(x));
        let cl = classify(x, &p, &ctx(&c, &d, None));
        assert_eq!((cl.kind, cl.source, cl.name.as_str(), cl.candidate), (Kind::App, Source::App, "Foo", true));
        // Xbox (sonde) avant Discord.
        let px = Probe { xbox: Some(("Forza".into(), Some("MS.Forza".into()))), ..Default::default() };
        let cl = classify(hades, &px, &ctx(&c, &d, None));
        assert_eq!((cl.source, cl.key("")), (Source::Xbox, "xbox:MS.Forza".to_string()));

        // Correction > tout.
        let base = classify(ER, &p, &ctx(&c, &d, None));
        let ov = |kind, name: Option<&str>| Override { path: ER.into(), kind, name: name.map(String::from) };
        assert_eq!(apply_override(&base, Some(&ov(OvKind::Ignore, None))), None);
        let a = apply_override(&base, Some(&ov(OvKind::App, None))).unwrap();
        assert_eq!((a.kind, a.source, a.app_id), (Kind::App, Source::User, None));
        // Jeu déjà connu renommé : garde l'identité.
        let g = apply_override(&base, Some(&ov(OvKind::Game, Some("Elden")))).unwrap();
        assert_eq!((g.source, g.name.as_str(), g.key("x")), (Source::Steam, "Elden", "steam:1245620".to_string()));
        // App → jeu utilisateur.
        let app = classify(x, &p, &ctx(&c, &d, None));
        let g = apply_override(&app, Some(&ov(OvKind::Game, None))).unwrap();
        assert_eq!((g.kind, g.source, g.candidate), (Kind::Game, Source::User, false));
        assert_eq!(apply_override(&app, None), Some(app.clone()));
    }

    #[test]
    fn exclusions() {
        let (c, d) = (cat(), db());
        let p = Probe::default();
        for path in [
            "C:\\Program Files (x86)\\Steam\\steam.exe",
            "C:\\Program Files (x86)\\Steam\\bin\\cef\\cef.win7x64\\steamwebhelper.exe",
            "C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe",
            "C:\\Windows\\explorer.exe",
            "C:\\Windows\\System32\\notepad.exe",
            "C:\\Apps\\Island\\island.exe",
            "D:\\SteamLibrary\\steamapps\\common\\ELDEN RING\\Game\\UnityCrashHandler64.exe",
            "D:\\SteamLibrary\\steamapps\\common\\ELDEN RING\\_CommonRedist\\vc_redist.x64.exe",
        ] {
            let cl = classify(path, &p, &ctx(&c, &d, Some("heur")));
            assert_eq!((cl.kind, cl.candidate), (Kind::App, false), "{path}");
        }
        // Le launcher reste exclu même si son dossier est (à tort) catalogué.
        let cl = classify("C:\\Program Files (x86)\\Steam\\steam.exe", &p, &ctx(&c, &d, None));
        assert_eq!(cl.source, Source::App);
    }

    #[test]
    fn stem_and_state_json() {
        assert_eq!(stem("C:\\x\\Foo-Win64-Shipping.exe"), "Foo");
        assert_eq!(stem("C:\\x\\Code.exe"), "Code");
        let s = ActivityState {
            foreground: Some(App {
                pid: 1,
                exe: "a.exe".into(),
                path: "C:\\a.exe".into(),
                title: "t".into(),
                name: "A".into(),
                kind: Kind::Game,
                source: Source::Battlenet,
                app_id: Some("x".into()),
                art: None,
                icon: None,
            }),
            game: Some(GameSession {
                key: "battlenet:x".into(),
                name: "A".into(),
                source: Source::Battlenet,
                app_id: Some("x".into()),
                art: None,
                pid: 1,
                started_at: 5,
            }),
        };
        let v = serde_json::to_value(&s).unwrap();
        assert_eq!(v["foreground"]["kind"], "game");
        assert_eq!(v["foreground"]["source"], "battlenet");
        assert_eq!(v["foreground"]["appId"], "x");
        assert_eq!(v["game"]["startedAt"], 5);
        assert!(v["foreground"].get("art").is_none());
    }
}
