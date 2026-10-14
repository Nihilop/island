// Conversion de fichiers (outil intégré). Sortie À CÔTÉ de l'original, jamais d'écrasement
// (suffixe « (1) », « (2) »…) ; l'original n'est jamais modifié ; fichier partiel supprimé
// en cas d'échec ; conversion séquentielle, progression sur `convert://progress`.
//   - images (crate `image`) : PNG, JPG (aplati sur fond blanc, qualité), WebP (sans perte),
//     ICO (plafonné à 256 px), BMP ; redimensionnement Lanczos3 si taille max ;
//   - audio / vidéo : ffmpeg externe (PATH, sinon le lien winget), sans fenêtre console.

use std::path::{Path, PathBuf};
use std::process::Command;

use tauri::{AppHandle, Emitter};

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct Progress {
    path: String,
    /// running | done | error | skipped
    status: &'static str,
    output: Option<String>,
    error: Option<String>,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Summary {
    converted: Vec<String>,
    failed: usize,
    first_error: Option<String>,
}

fn ext_of(p: &Path) -> String {
    p.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default()
}

/// Chemin de sortie libre à côté de l'original.
fn output_path(input: &Path, ext: &str) -> PathBuf {
    let dir = input.parent().unwrap_or(Path::new("."));
    let stem = input.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| "converted".into());
    let mut out = dir.join(format!("{stem}.{ext}"));
    let mut n = 1;
    while out.exists() {
        out = dir.join(format!("{stem} ({n}).{ext}"));
        n += 1;
    }
    out
}

fn convert_image(input: &Path, out: &Path, target: &str, quality: u8, max: Option<u32>) -> Result<(), String> {
    use image::imageops::FilterType;
    let mut img = image::open(input).map_err(|e| e.to_string())?;
    let cap = if target == "ico" { Some(max.unwrap_or(256).min(256)) } else { max };
    if let Some(m) = cap {
        if img.width().max(img.height()) > m {
            img = img.resize(m, m, FilterType::Lanczos3);
        }
    }
    let file = std::fs::File::create(out).map_err(|e| e.to_string())?;
    let mut w = std::io::BufWriter::new(file);
    match target {
        "jpg" => {
            // Pas de transparence en JPG : aplati sur fond blanc.
            let rgba = img.to_rgba8();
            let mut rgb = image::RgbImage::new(rgba.width(), rgba.height());
            for (x, y, p) in rgba.enumerate_pixels() {
                let a = p[3] as u32;
                let mix = |c: u8| ((c as u32 * a + 255 * (255 - a)) / 255) as u8;
                rgb.put_pixel(x, y, image::Rgb([mix(p[0]), mix(p[1]), mix(p[2])]));
            }
            image::codecs::jpeg::JpegEncoder::new_with_quality(&mut w, quality)
                .encode_image(&rgb)
                .map_err(|e| e.to_string())
        }
        "webp" => image::codecs::webp::WebPEncoder::new_lossless(&mut w).encode(
            img.to_rgba8().as_raw(),
            img.width(),
            img.height(),
            image::ExtendedColorType::Rgba8,
        )
        .map_err(|e| e.to_string()),
        "png" => img.write_to(&mut w, image::ImageFormat::Png).map_err(|e| e.to_string()),
        "bmp" => img.write_to(&mut w, image::ImageFormat::Bmp).map_err(|e| e.to_string()),
        "ico" => img.write_to(&mut w, image::ImageFormat::Ico).map_err(|e| e.to_string()),
        _ => Err("unsupported".into()),
    }
}

/// ffmpeg : PATH, sinon l'emplacement des liens winget.
fn ffmpeg() -> Option<PathBuf> {
    let probe = |p: &Path| {
        let mut c = Command::new(p);
        c.arg("-version");
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            c.creation_flags(0x0800_0000);
        }
        c.output().map(|o| o.status.success()).unwrap_or(false)
    };
    if probe(Path::new("ffmpeg")) {
        return Some(PathBuf::from("ffmpeg"));
    }
    let winget = std::env::var_os("LOCALAPPDATA").map(|d| PathBuf::from(d).join("Microsoft").join("WinGet").join("Links").join("ffmpeg.exe"))?;
    probe(&winget).then_some(winget)
}

#[tauri::command]
pub fn convert_ffmpeg_available() -> bool {
    ffmpeg().is_some()
}

fn convert_media(bin: &Path, input: &Path, out: &Path, target: &str, quality: u8) -> Result<(), String> {
    let q = quality as f64;
    let crf = format!("{}", (35.0 - q * 0.17).round() as i32);
    let abr = format!("{}k", (96.0 + q * 2.24).round() as i32);
    let mut args: Vec<String> = vec!["-hide_banner".into(), "-n".into(), "-i".into(), input.to_string_lossy().into_owned()];
    let extra: Vec<&str> = match target {
        "mp3" => vec!["-vn", "-c:a", "libmp3lame", "-b:a", &abr],
        "m4a" => vec!["-vn", "-c:a", "aac", "-b:a", &abr],
        "wav" => vec!["-vn", "-c:a", "pcm_s16le"],
        "flac" => vec!["-vn", "-c:a", "flac"],
        "mp4" => vec!["-c:v", "libx264", "-preset", "medium", "-crf", &crf, "-c:a", "aac", "-b:a", &abr, "-movflags", "+faststart"],
        "webm" => vec!["-c:v", "libvpx-vp9", "-crf", &crf, "-b:v", "0", "-c:a", "libopus", "-b:a", &abr],
        "gif" => vec!["-vf", "fps=15,scale=480:-1:flags=lanczos", "-loop", "0"],
        _ => return Err("unsupported".into()),
    };
    args.extend(extra.iter().map(|s| s.to_string()));
    args.push(out.to_string_lossy().into_owned());
    let mut c = Command::new(bin);
    c.args(&args);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        c.creation_flags(0x0800_0000);
    }
    let o = c.output().map_err(|e| e.to_string())?;
    if o.status.success() {
        return Ok(());
    }
    let err = String::from_utf8_lossy(&o.stderr);
    Err(err.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("ffmpeg").chars().take(120).collect())
}

const IMAGE_IN: [&str; 7] = ["png", "jpg", "jpeg", "webp", "bmp", "gif", "ico"];
const IMAGE_OUT: [&str; 5] = ["png", "jpg", "webp", "ico", "bmp"];

/// Convertit `paths` vers `target`. Les fichiers déjà au format cible sont ignorés.
#[tauri::command]
pub async fn convert_files(app: AppHandle, paths: Vec<String>, target: String, quality: u8, max_size: Option<u32>) -> Summary {
    tauri::async_runtime::spawn_blocking(move || {
        let bin = ffmpeg();
        let mut converted = Vec::new();
        let mut failed = 0;
        let mut first_error = None;
        for p in paths {
            let input = PathBuf::from(&p);
            let ext = ext_of(&input);
            let same = ext == target || (ext == "jpeg" && target == "jpg");
            let send = |status, output: Option<String>, error: Option<String>| {
                let _ = app.emit("convert://progress", Progress { path: p.clone(), status, output, error });
            };
            if same {
                send("skipped", None, None);
                continue;
            }
            send("running", None, None);
            let out = output_path(&input, &target);
            let r = if IMAGE_IN.contains(&ext.as_str()) && IMAGE_OUT.contains(&target.as_str()) {
                convert_image(&input, &out, &target, quality, max_size)
            } else if let Some(b) = &bin {
                convert_media(b, &input, &out, &target, quality)
            } else {
                Err("noFfmpeg".into())
            };
            match r {
                Ok(()) => {
                    let o = out.to_string_lossy().into_owned();
                    send("done", Some(o.clone()), None);
                    converted.push(o);
                }
                Err(e) => {
                    let _ = std::fs::remove_file(&out); // fichier partiel
                    failed += 1;
                    first_error.get_or_insert(e.clone());
                    send("error", None, Some(e));
                }
            }
        }
        Summary { converted, failed, first_error }
    })
    .await
    .unwrap_or(Summary { converted: vec![], failed: 0, first_error: Some("failed".into()) })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn images_round_trip() {
        let dir = std::env::temp_dir().join("island-convert-test");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let src = dir.join("in.png");
        image::RgbaImage::from_fn(600, 300, |x, _| image::Rgba([x as u8, 80, 200, if x < 300 { 255 } else { 0 }])).save(&src).unwrap();
        for t in ["jpg", "webp", "ico", "bmp"] {
            let out = output_path(&src, t);
            convert_image(&src, &out, t, 85, Some(256)).unwrap();
            let back = image::open(&out).unwrap();
            assert!(back.width() <= 256, "{t}");
        }
        // Jamais d'écrasement.
        assert!(output_path(&src, "jpg").ends_with("in (1).jpg"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
