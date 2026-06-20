// Service `clipboard` : lecture/écriture du presse-papiers (texte + image).
// Cross-platform via `arboard`. Gardé par la permission `clipboard` (lire le
// presse-papiers de l'utilisateur = donnée potentiellement sensible).

use arboard::Clipboard;
use base64::Engine as _;
use tauri::AppHandle;

use crate::ext::require_perm;

#[tauri::command]
pub fn clipboard_read_text(app: AppHandle, ext_id: String) -> Result<String, String> {
    require_perm!(&app, &ext_id, "clipboard");
    Clipboard::new()
        .and_then(|mut c| c.get_text())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn clipboard_write_text(app: AppHandle, ext_id: String, text: String) -> Result<(), String> {
    require_perm!(&app, &ext_id, "clipboard");
    Clipboard::new()
        .and_then(|mut c| c.set_text(text))
        .map_err(|e| e.to_string())
}

/// Image du presse-papiers → PNG en data URL, ou `None` s'il n'y a pas d'image.
#[tauri::command]
pub fn clipboard_read_image(app: AppHandle, ext_id: String) -> Result<Option<String>, String> {
    require_perm!(&app, &ext_id, "clipboard");
    let mut cb = Clipboard::new().map_err(|e| e.to_string())?;
    let img = match cb.get_image() {
        Ok(i) => i,
        Err(arboard::Error::ContentNotAvailable) => return Ok(None),
        Err(e) => return Err(e.to_string()),
    };
    // arboard renvoie du RGBA → on encode en PNG.
    let mut png = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut png, img.width as u32, img.height as u32);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        let mut w = enc.write_header().map_err(|e| e.to_string())?;
        w.write_image_data(&img.bytes).map_err(|e| e.to_string())?;
    }
    Ok(Some(format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(&png)
    )))
}

/// Écrit une image (PNG en data URL, ex. `canvas.toDataURL()`) dans le presse-papiers.
#[tauri::command]
pub fn clipboard_write_image(app: AppHandle, ext_id: String, data_url: String) -> Result<(), String> {
    require_perm!(&app, &ext_id, "clipboard");
    let b64 = data_url.rsplit(',').next().unwrap_or(&data_url);
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(b64)
        .map_err(|e| e.to_string())?;

    let decoder = png::Decoder::new(std::io::Cursor::new(bytes));
    let mut reader = decoder.read_info().map_err(|e| e.to_string())?;
    let mut buf = vec![0u8; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf).map_err(|e| e.to_string())?;
    let raw = &buf[..info.buffer_size()];

    // arboard veut du RGBA 8-bit. On convertit les cas courants (RGBA passe tel quel,
    // RGB se voit ajouter un alpha opaque) ; les autres formats sont refusés.
    let rgba: Vec<u8> = match info.color_type {
        png::ColorType::Rgba => raw.to_vec(),
        png::ColorType::Rgb => raw.chunks_exact(3).flat_map(|p| [p[0], p[1], p[2], 255]).collect(),
        other => return Err(format!("clipboard: format PNG non supporté ({other:?})")),
    };

    let img = arboard::ImageData {
        width: info.width as usize,
        height: info.height as usize,
        bytes: rgba.into(),
    };
    Clipboard::new()
        .and_then(|mut c| c.set_image(img))
        .map_err(|e| e.to_string())
}
