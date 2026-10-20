// Conditions « Visible quand… » des calques : moment de la journée réel (d'après le lever et
// le coucher du soleil de l'outil Météo), plage horaire, ciel réel (code météo WMO). Le
// moteur s'en sert pour faire apparaître / disparaître un calque en fondu ; l'aperçu de
// l'éditeur reçoit le même état (`wallpaper_conditions`) et peut le simuler.

use chrono::{Local, TimeZone, Timelike};
use serde::Serialize;

use super::scene::When;
use super::text::var;

/// Demi-largeur de l'aube et du crépuscule autour du lever / coucher (minutes).
const TWILIGHT: i32 = 45;
/// Sans météo : lever et coucher supposés (minutes depuis minuit).
const DEFAULT_RISE: i32 = 7 * 60;
const DEFAULT_SET: i32 = 19 * 60;

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Now {
    /// "dawn" | "day" | "dusk" | "night"
    pub moment: String,
    /// "clear" | "cloudy" | "fog" | "rain" | "snow" | "storm" ; None sans météo.
    pub sky: Option<String>,
    /// Minutes depuis minuit (heure locale).
    pub minutes: i32,
}

/// Classe de ciel d'un code météo WMO.
pub fn sky_of(code: i64) -> &'static str {
    match code {
        0 | 1 => "clear",
        2 | 3 => "cloudy",
        45 | 48 => "fog",
        71..=77 | 85 | 86 => "snow",
        95..=99 => "storm",
        51..=67 | 80..=82 => "rain",
        _ => "cloudy",
    }
}

/// Minutes depuis minuit (heure locale) d'un horodatage Unix.
fn local_minutes(unix: i64) -> Option<i32> {
    let t = Local.timestamp_opt(unix, 0).single()?;
    Some((t.hour() * 60 + t.minute()) as i32)
}

pub fn moment(minutes: i32, rise: i32, set: i32) -> &'static str {
    if (minutes - rise).abs() <= TWILIGHT {
        "dawn"
    } else if (minutes - set).abs() <= TWILIGHT {
        "dusk"
    } else if minutes > rise && minutes < set {
        "day"
    } else {
        "night"
    }
}

/// État réel du moment : heure locale, soleil et ciel poussés par l'outil Météo.
pub fn now() -> Now {
    let t = Local::now();
    let minutes = (t.hour() * 60 + t.minute()) as i32;
    let num = |k: &str| var(k).and_then(|v| v.parse::<i64>().ok());
    // Heures du lever / coucher (et non leurs dates) : juste même si la météo date d'hier.
    let rise = num("sun.rise").and_then(local_minutes).unwrap_or(DEFAULT_RISE);
    let set = num("sun.set").and_then(local_minutes).unwrap_or(DEFAULT_SET);
    let (rise, set) = if rise < set { (rise, set) } else { (DEFAULT_RISE, DEFAULT_SET) };
    Now { moment: moment(minutes, rise, set).into(), sky: num("weather.code").map(|c| sky_of(c).to_string()), minutes }
}

fn parse_hhmm(v: &str) -> Option<i32> {
    let (h, m) = v.split_once(':')?;
    Some(h.trim().parse::<i32>().ok()? * 60 + m.trim().parse::<i32>().ok()?)
}

/// Le calque doit-il être visible maintenant ?
pub fn visible(w: &When, n: &Now) -> bool {
    let time_ok = match (w.from.as_deref().and_then(parse_hhmm), w.to.as_deref().and_then(parse_hhmm)) {
        // Plage qui peut passer minuit (22:00 → 06:00).
        (Some(a), Some(b)) if a <= b => n.minutes >= a && n.minutes < b,
        (Some(a), Some(b)) => n.minutes >= a || n.minutes < b,
        _ => w.moments.is_empty() || w.moments.iter().any(|m| *m == n.moment),
    };
    // Sans météo connue, une condition de ciel n'empêche pas l'affichage.
    let sky_ok = w.sky.is_empty() || n.sky.as_ref().is_none_or(|s| w.sky.contains(s));
    time_ok && sky_ok
}

/// État réel, pour l'aperçu de l'éditeur.
#[tauri::command]
pub fn wallpaper_conditions() -> Now {
    now()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(minutes: i32, sky: Option<&str>) -> Now {
        Now { moment: moment(minutes, 7 * 60, 19 * 60).into(), sky: sky.map(String::from), minutes }
    }

    #[test]
    fn moments_follow_the_sun() {
        let (rise, set) = (7 * 60, 19 * 60);
        assert_eq!(moment(6 * 60 + 30, rise, set), "dawn");
        assert_eq!(moment(12 * 60, rise, set), "day");
        assert_eq!(moment(19 * 60 + 40, rise, set), "dusk");
        assert_eq!(moment(23 * 60, rise, set), "night");
        assert_eq!(moment(3 * 60, rise, set), "night");
        assert_eq!(sky_of(63), "rain");
        assert_eq!(sky_of(75), "snow");
        assert_eq!(sky_of(0), "clear");
    }

    #[test]
    fn conditions_combine_time_and_sky() {
        let night_rain = When { moments: vec!["night".into()], sky: vec!["rain".into(), "storm".into()], ..Default::default() };
        assert!(visible(&night_rain, &at(23 * 60, Some("rain"))));
        assert!(!visible(&night_rain, &at(23 * 60, Some("clear"))));
        assert!(!visible(&night_rain, &at(12 * 60, Some("rain"))));
        // Météo inconnue : le ciel ne bloque pas.
        assert!(visible(&night_rain, &at(23 * 60, None)));
        // Plage horaire qui passe minuit, prioritaire sur les moments.
        let late = When { from: Some("22:00".into()), to: Some("06:00".into()), moments: vec!["day".into()], ..Default::default() };
        assert!(visible(&late, &at(23 * 60, None)) && visible(&late, &at(5 * 60, None)));
        assert!(!visible(&late, &at(12 * 60, None)));
        assert!(visible(&When::default(), &at(12 * 60, Some("snow"))));
    }
}
