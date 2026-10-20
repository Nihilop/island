// Calques texte des fonds animés : variables (`{time}`, `{date:EEEE d MMMM}`,
// `{time@Europe/Paris}`, `{weather.temp}`, `{media.title}`…) et rendu en image RGBA.
//
// Le MÊME rendu sert au fond (moteur) et à l'aperçu de l'éditeur (`wallpaper_text_raster`) :
// texte mis en forme par cosmic-text (polices du système, ligatures, alphabets), puis
// contour et ombre calculés autour. L'image est faite à la hauteur réelle de l'écran : le
// moteur la dessine pixel pour pixel, sans mise à l'échelle.
//
// Variables : l'heure et la date sont calculées ici (fuseaux : chrono-tz) ; la musique vient
// du service média ; la météo et le soleil sont poussés par l'outil Météo
// (`wallpaper_set_vars`). Une variable inconnue reste écrite telle quelle (`{oups}`), pour
// que l'erreur se voie.

use std::collections::BTreeMap;
use std::sync::{Mutex, OnceLock};

use chrono::{DateTime, Datelike, Local, TimeZone, Timelike, Utc};
use cosmic_text::{Align, Attrs, Buffer, Color, Family, FontSystem, Metrics, Shaping, Style, SwashCache, Weight};

use super::scene::{hex, TextStyle};

// --- Variables ----------------------------------------------------------------------------

/// Valeurs poussées par le front (météo, soleil), par nom (`weather.temp`…).
static VARS: Mutex<BTreeMap<String, String>> = Mutex::new(BTreeMap::new());

/// L'outil Météo pousse ses valeurs à chaque mise à jour.
#[tauri::command]
pub fn wallpaper_set_vars(vars: BTreeMap<String, String>) {
    let mut v = VARS.lock().unwrap_or_else(|p| p.into_inner());
    for (k, val) in vars {
        if k.len() <= 40 && val.chars().count() <= 200 {
            v.insert(k, val);
        }
    }
}

/// Valeur brute poussée par le front (conditions de calques : soleil, météo).
pub fn var(name: &str) -> Option<String> {
    VARS.lock().unwrap_or_else(|p| p.into_inner()).get(name).cloned()
}

const DAYS_FR: [&str; 7] = ["lundi", "mardi", "mercredi", "jeudi", "vendredi", "samedi", "dimanche"];
const DAYS_EN: [&str; 7] = ["Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday", "Sunday"];
const MONTHS_FR: [&str; 12] = ["janvier", "février", "mars", "avril", "mai", "juin", "juillet", "août", "septembre", "octobre", "novembre", "décembre"];
const MONTHS_EN: [&str; 12] = ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];

/// Formate une date : `yyyy yy MMMM MMM MM M dd d EEEE EEE HH H hh h mm m ss s a`, texte entre
/// apostrophes laissé tel quel (`'à' HH:mm`).
fn format_time<Tz: TimeZone>(t: &DateTime<Tz>, fmt: &str, fr: bool) -> String {
    let (days, months) = if fr { (DAYS_FR, MONTHS_FR) } else { (DAYS_EN, MONTHS_EN) };
    let day = days[t.weekday().num_days_from_monday() as usize];
    let month = months[t.month0() as usize];
    let short = |s: &str| s.chars().take(3).collect::<String>();
    let h12 = if t.hour() % 12 == 0 { 12 } else { t.hour() % 12 };
    let mut out = String::new();
    let chars: Vec<char> = fmt.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '\'' {
            let end = chars[i + 1..].iter().position(|&x| x == '\'').map(|p| i + 1 + p).unwrap_or(chars.len());
            out.extend(&chars[i + 1..end.min(chars.len())]);
            i = end + 1;
            continue;
        }
        let run = chars[i..].iter().take_while(|&&x| x == c).count();
        let piece = match (c, run) {
            ('y', 2) => format!("{:02}", t.year() % 100),
            ('y', _) => t.year().to_string(),
            ('M', 1) => t.month().to_string(),
            ('M', 2) => format!("{:02}", t.month()),
            ('M', 3) => short(month),
            ('M', _) => month.to_string(),
            ('d', 1) => t.day().to_string(),
            ('d', _) => format!("{:02}", t.day()),
            ('E', 1..=3) => short(day),
            ('E', _) => day.to_string(),
            ('H', 1) => t.hour().to_string(),
            ('H', _) => format!("{:02}", t.hour()),
            ('h', 1) => h12.to_string(),
            ('h', _) => format!("{h12:02}"),
            ('m', 1) => t.minute().to_string(),
            ('m', _) => format!("{:02}", t.minute()),
            ('s', 1) => t.second().to_string(),
            ('s', _) => format!("{:02}", t.second()),
            ('a', _) => (if t.hour() < 12 { "AM" } else { "PM" }).to_string(),
            _ => chars[i..i + run].iter().collect(),
        };
        out.push_str(&piece);
        i += run;
    }
    out
}

fn variable(expr: &str, now: DateTime<Utc>, fr: bool) -> Option<String> {
    // `nom[:format][@fuseau]`
    let (body, zone) = match expr.rsplit_once('@') {
        Some((b, z)) => (b, Some(z.trim())),
        None => (expr, None),
    };
    let (name, fmt) = match body.split_once(':') {
        Some((n, f)) => (n.trim(), Some(f)),
        None => (body.trim(), None),
    };
    let time = |default: &str| -> Option<String> {
        let f = fmt.unwrap_or(default);
        match zone {
            Some(z) => Some(format_time(&now.with_timezone(&z.parse::<chrono_tz::Tz>().ok()?), f, fr)),
            None => Some(format_time(&now.with_timezone(&Local), f, fr)),
        }
    };
    match name {
        "time" => time("HH:mm"),
        "date" => time(if fr { "EEEE d MMMM" } else { "EEEE, MMMM d" }),
        "media.title" | "media.artist" => {
            let np = crate::services::media::np_state().filter(|n| n.active);
            Some(np.map(|n| if name == "media.title" { n.title } else { n.artist }).unwrap_or_default())
        }
        n if n.starts_with("weather.") || n.starts_with("sun.") => Some(var(n).unwrap_or_default()),
        _ => None,
    }
}

/// Remplace les variables d'un texte ; `{{` / `}}` donnent des accolades.
pub fn resolve(content: &str, now: DateTime<Utc>, fr: bool) -> String {
    let mut out = String::new();
    let mut rest = content;
    while let Some(i) = rest.find(['{', '}']) {
        out.push_str(&rest[..i]);
        let tail = &rest[i..];
        if tail.starts_with("{{") || tail.starts_with("}}") {
            out.push_str(&tail[..1]);
            rest = &tail[2..];
        } else if tail.starts_with('{') {
            match tail.find('}') {
                Some(end) => {
                    let expr = &tail[1..end];
                    match variable(expr, now, fr) {
                        Some(v) => out.push_str(&v),
                        None => out.push_str(&tail[..=end]),
                    }
                    rest = &tail[end + 1..];
                }
                None => {
                    out.push_str(tail);
                    rest = "";
                }
            }
        } else {
            out.push('}');
            rest = &tail[1..];
        }
    }
    out.push_str(rest);
    out
}

/// Langue d'Island (réglage `lang`), pour les noms de jours et de mois.
pub fn french(app: &tauri::AppHandle) -> bool {
    !crate::store::read_store(app, "__app__").get("lang").and_then(|v| v.as_str()).unwrap_or("fr").starts_with("en")
}

// --- Rendu ---------------------------------------------------------------------------------

fn fonts() -> &'static Mutex<FontSystem> {
    static FONTS: OnceLock<Mutex<FontSystem>> = OnceLock::new();
    FONTS.get_or_init(|| Mutex::new(FontSystem::new()))
}

/// Familles de polices installées (sélecteur de l'éditeur).
#[tauri::command]
pub async fn wallpaper_fonts() -> Vec<String> {
    tauri::async_runtime::spawn_blocking(|| {
        let fs = fonts().lock().unwrap_or_else(|p| p.into_inner());
        let mut names: Vec<String> = fs.db().faces().filter_map(|f| f.families.first().map(|(n, _)| n.clone())).collect();
        names.sort_by_key(|n| n.to_lowercase());
        names.dedup();
        names
    })
    .await
    .unwrap_or_default()
}

pub struct Raster {
    pub width: u32,
    pub height: u32,
    /// RGBA, alpha non prémultiplié (comme les images des calques).
    pub rgba: Vec<u8>,
}

/// Flou en boîte séparable (rayon `r`), trois passes ≈ gaussien.
fn blur(src: &[f32], w: usize, h: usize, r: usize) -> Vec<f32> {
    if r == 0 {
        return src.to_vec();
    }
    let mut a = src.to_vec();
    let mut b = vec![0f32; a.len()];
    let pass = |from: &[f32], to: &mut [f32], horizontal: bool| {
        let (n, lines) = if horizontal { (w, h) } else { (h, w) };
        let at = |line: usize, i: usize| if horizontal { line * w + i } else { i * w + line };
        let norm = 1.0 / (2 * r + 1) as f32;
        for line in 0..lines {
            let mut sum = 0f32;
            for i in 0..=r.min(n - 1) {
                sum += from[at(line, i)];
            }
            for i in 0..n {
                to[at(line, i)] = sum * norm;
                if i + r + 1 < n {
                    sum += from[at(line, i + r + 1)];
                }
                if i >= r {
                    sum -= from[at(line, i - r)];
                }
            }
        }
    };
    for _ in 0..3 {
        pass(&a, &mut b, true);
        pass(&b, &mut a, false);
    }
    a
}

/// Image d'un texte (déjà résolu) pour un écran de `screen_h` pixels de haut.
pub fn raster(style: &TextStyle, text: &str, screen_h: f32) -> Raster {
    let empty = Raster { width: 1, height: 1, rgba: vec![0; 4] };
    let text = if style.uppercase { text.to_uppercase() } else { text.to_string() };
    if text.trim().is_empty() {
        return empty;
    }
    let px = (style.size.clamp(0.005, 0.6) * screen_h).clamp(4.0, 1600.0);
    let line = px * style.line_height.clamp(0.6, 3.0);
    let mut guard = fonts().lock().unwrap_or_else(|p| p.into_inner());
    let fs = &mut *guard;
    let mut cache = SwashCache::new();
    let mut buf = Buffer::new(fs, Metrics::new(px, line));
    let family = if style.font.trim().is_empty() { Family::SansSerif } else { Family::Name(style.font.trim()) };
    let attrs = Attrs::new()
        .family(family)
        .weight(Weight(style.weight.clamp(100, 900)))
        .style(if style.italic { Style::Italic } else { Style::Normal })
        .letter_spacing(style.letter_spacing.clamp(-0.2, 1.0));
    let align = match style.align.as_str() {
        "left" => Align::Left,
        "right" => Align::Right,
        _ => Align::Center,
    };
    // Mesure sans largeur imposée, puis mise en page à la largeur du texte (alignement).
    buf.set_size(None, None);
    buf.set_text(&text, &attrs, Shaping::Advanced, Some(align));
    buf.shape_until_scroll(fs, false);
    let width = buf.layout_runs().map(|r| r.line_w).fold(0f32, f32::max).ceil();
    let height = buf.layout_runs().map(|r| r.line_top + r.line_height).fold(0f32, f32::max).ceil();
    if width < 1.0 || height < 1.0 {
        return empty;
    }
    buf.set_size(Some(width), None);
    buf.shape_until_scroll(fs, false);

    let outline = (style.outline.clamp(0.0, 0.3) * px).round() as usize;
    let shadow = style.shadow.as_ref().filter(|s| s.opacity > 0.0);
    let (sblur, sx, sy) = shadow.map_or((0, 0, 0), |s| ((s.blur.clamp(0.0, 1.0) * px) as usize, (s.x.clamp(-1.0, 1.0) * px) as i32, (s.y.clamp(-1.0, 1.0) * px) as i32));
    let pad = outline + sblur * 2 + sx.unsigned_abs().max(sy.unsigned_abs()) as usize + 2;
    let (w, h) = (width as usize + pad * 2, height as usize + pad * 2);
    if w * h > 4096 * 4096 {
        return empty;
    }
    // Couverture des glyphes (0..1).
    let mut cover = vec![0f32; w * h];
    buf.draw(fs, &mut cache, Color::rgba(255, 255, 255, 255), |x, y, gw, gh, c| {
        let a = c.a() as f32 / 255.0;
        for yy in y.max(0)..(y + gh as i32) {
            for xx in x.max(0)..(x + gw as i32) {
                let (px_, py_) = (xx as usize + pad, yy as usize + pad);
                if px_ < w && py_ < h {
                    let v = &mut cover[py_ * w + px_];
                    *v = v.max(a);
                }
            }
        }
    });
    drop(guard);

    // Contour : couverture élargie (flou puis seuil doux → bords arrondis).
    let outer = if outline > 0 { blur(&cover, w, h, outline).iter().map(|v| (v * 3.0).min(1.0)).collect() } else { cover.clone() };
    let (fill, line_c) = (hex(&style.color), hex(&style.outline_color));
    let mut px_out = vec![[0f32; 4]; w * h];
    let over = |dst: &mut [f32; 4], rgb: [f32; 3], a: f32| {
        // « Par-dessus », alpha non prémultiplié.
        let out_a = a + dst[3] * (1.0 - a);
        if out_a > 0.0 {
            for i in 0..3 {
                dst[i] = (rgb[i] * a + dst[i] * dst[3] * (1.0 - a)) / out_a;
            }
        }
        dst[3] = out_a;
    };
    if let Some(s) = shadow {
        let soft = blur(&outer, w, h, sblur);
        let rgb = hex(&s.color);
        for y in 0..h {
            for x in 0..w {
                let (fx, fy) = (x as i32 - sx, y as i32 - sy);
                if fx >= 0 && fy >= 0 && (fx as usize) < w && (fy as usize) < h {
                    let a = soft[fy as usize * w + fx as usize] * s.opacity.clamp(0.0, 1.0);
                    over(&mut px_out[y * w + x], rgb, a);
                }
            }
        }
    }
    for i in 0..w * h {
        if outline > 0 {
            over(&mut px_out[i], line_c, outer[i]);
        }
        over(&mut px_out[i], fill, cover[i]);
    }
    let rgba = px_out.iter().flat_map(|p| [(p[0] * 255.0) as u8, (p[1] * 255.0) as u8, (p[2] * 255.0) as u8, (p[3] * 255.0) as u8]).collect();
    Raster { width: w as u32, height: h as u32, rgba }
}

/// Aperçu de l'éditeur : le texte du calque, résolu et rendu pour un canevas de `height` px.
/// Réponse binaire : largeur et hauteur (u32 LE), puis les pixels RGBA.
#[tauri::command]
pub async fn wallpaper_text_raster(app: tauri::AppHandle, style: TextStyle, height: f32) -> Result<tauri::ipc::Response, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let text = resolve(&style.content, Utc::now(), french(&app));
        let r = raster(&style, &text, height.clamp(64.0, 4320.0));
        let mut out = Vec::with_capacity(8 + r.rgba.len());
        out.extend_from_slice(&r.width.to_le_bytes());
        out.extend_from_slice(&r.height.to_le_bytes());
        out.extend_from_slice(&r.rgba);
        tauri::ipc::Response::new(out)
    })
    .await
    .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn variables_and_escapes() {
        let now = Utc.with_ymd_and_hms(2026, 10, 20, 13, 5, 9).unwrap();
        assert_eq!(resolve("{time:HH:mm:ss@UTC}", now, true), "13:05:09");
        assert_eq!(resolve("{time@Europe/Paris}", now, true), "15:05");
        assert_eq!(resolve("{date:EEEE d MMMM yyyy@UTC}", now, true), "mardi 20 octobre 2026");
        assert_eq!(resolve("{date:EEE, MMM d@UTC}", now, false), "Tue, Oct 20");
        assert_eq!(resolve("{time:h:mm a@UTC}", now, false), "1:05 PM");
        assert_eq!(resolve("{time:'il est' H'h'mm@UTC}", now, true), "il est 13h05");
        assert_eq!(resolve("{{ok}} {oups} {time@Pas/Un_Fuseau}", now, true), "{ok} {oups} {time@Pas/Un_Fuseau}");
        wallpaper_set_vars(BTreeMap::from([("weather.temp".into(), "15°".into())]));
        assert_eq!(resolve("Grenoble {weather.temp} {weather.inconnu}", now, true), "Grenoble 15° ");
    }

    #[test]
    fn renders_text_with_outline_and_shadow() {
        let mut style = TextStyle { content: "12:34".into(), outline: 0.05, ..Default::default() };
        style.shadow = Some(Default::default());
        let r = raster(&style, "12:34", 1080.0);
        assert!(r.width > 100 && r.height > 50, "{}×{}", r.width, r.height);
        assert_eq!(r.rgba.len(), (r.width * r.height * 4) as usize);
        assert!(r.rgba.chunks_exact(4).any(|p| p[3] == 255), "du texte opaque");
        assert!(r.rgba.chunks_exact(4).any(|p| p[3] == 0), "du vide autour");
        let empty = raster(&style, "   ", 1080.0);
        assert_eq!((empty.width, empty.height), (1, 1));
    }
}
