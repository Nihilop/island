// Copie PRIVÉE : le texte est posé dans le presse-papiers avec les marqueurs qui le font
// ignorer par les moniteurs de presse-papiers (dont notre historique), par Win+V et par
// le presse-papiers cloud. Les secrets sont effacés après 30 s, seulement si rien d'autre
// n'a été copié entre-temps (numéro de séquence inchangé).

use std::time::Duration;

const CLEAR_AFTER: Duration = Duration::from_secs(30);

#[cfg(target_os = "windows")]
pub fn copy_private(text: &str, clear: bool) -> Result<(), &'static str> {
    use windows::core::w;
    use windows::Win32::Foundation::{HANDLE, HGLOBAL};
    use windows::Win32::System::DataExchange::{
        CloseClipboard, EmptyClipboard, GetClipboardSequenceNumber, OpenClipboard, RegisterClipboardFormatW,
        SetClipboardData,
    };
    use windows::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE};

    const CF_UNICODETEXT: u32 = 13;

    unsafe fn global<T: Copy>(data: &[T]) -> Option<HGLOBAL> {
        let bytes = std::mem::size_of_val(data);
        let h = GlobalAlloc(GMEM_MOVEABLE, bytes.max(1)).ok()?;
        let p = GlobalLock(h) as *mut T;
        if p.is_null() {
            return None;
        }
        std::ptr::copy_nonoverlapping(data.as_ptr(), p, data.len());
        let _ = GlobalUnlock(h);
        Some(h)
    }

    unsafe {
        // Le presse-papiers peut être tenu par une autre app : 10 tentatives espacées de 20 ms.
        let mut opened = false;
        for _ in 0..10 {
            if OpenClipboard(None).is_ok() {
                opened = true;
                break;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        if !opened {
            return Err("clipboardBusy");
        }
        let _ = EmptyClipboard();
        let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
        let ok = global(&wide).map(|h| SetClipboardData(CF_UNICODETEXT, Some(HANDLE(h.0))).is_ok()).unwrap_or(false);
        let exclude = RegisterClipboardFormatW(w!("ExcludeClipboardContentFromMonitorProcessing"));
        let history = RegisterClipboardFormatW(w!("CanIncludeInClipboardHistory"));
        let cloud = RegisterClipboardFormatW(w!("CanUploadToCloudClipboard"));
        if let Some(h) = global(&[0u8]) {
            let _ = SetClipboardData(exclude, Some(HANDLE(h.0)));
        }
        for fmt in [history, cloud] {
            if let Some(h) = global(&[0u32]) {
                let _ = SetClipboardData(fmt, Some(HANDLE(h.0)));
            }
        }
        let seq = GetClipboardSequenceNumber();
        let _ = CloseClipboard();
        if !ok {
            return Err("clipboardBusy");
        }
        if clear {
            std::thread::spawn(move || {
                std::thread::sleep(CLEAR_AFTER);
                if GetClipboardSequenceNumber() == seq && OpenClipboard(None).is_ok() {
                    let _ = EmptyClipboard();
                    let _ = CloseClipboard();
                }
            });
        }
        Ok(())
    }
}

#[cfg(not(target_os = "windows"))]
pub fn copy_private(text: &str, _clear: bool) -> Result<(), &'static str> {
    arboard::Clipboard::new().and_then(|mut c| c.set_text(text.to_string())).map_err(|_| "clipboardBusy")
}

/// Image du presse-papiers (capture Win+Maj+S) → contenu d'un QR, s'il y en a un.
pub fn scan_qr() -> Result<String, &'static str> {
    let img = arboard::Clipboard::new()
        .and_then(|mut c| c.get_image())
        .map_err(|_| "noImage")?;
    let (w, h) = (img.width, img.height);
    let px = img.bytes;
    let mut prepared = rqrr::PreparedImage::prepare_from_greyscale(w, h, |x, y| {
        let i = (y * w + x) * 4;
        // Luminance approchée (RGBA).
        ((px[i] as u32 * 77 + px[i + 1] as u32 * 150 + px[i + 2] as u32 * 29) >> 8) as u8
    });
    let grids = prepared.detect_grids();
    let grid = grids.first().ok_or("noQr")?;
    grid.decode().map(|(_, content)| content).map_err(|_| "noQr")
}
