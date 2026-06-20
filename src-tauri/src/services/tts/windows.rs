// Synthèse vocale Windows via SAPI (ISpVoice). Un thread COM dédié possède la voix
// et lit les messages dans l'ordre (Speak synchrone) → pas de chevauchement, et la
// voix (non-Send) ne quitte jamais son thread.

use std::sync::mpsc::Sender;
use std::sync::{Mutex, OnceLock};

use windows::core::PCWSTR;
use windows::Win32::Media::Speech::{ISpVoice, SpVoice};
use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CLSCTX_ALL, COINIT_MULTITHREADED};

static TX: OnceLock<Mutex<Sender<String>>> = OnceLock::new();

pub fn speak(text: &str) {
    let tx = TX.get_or_init(|| {
        let (tx, rx) = std::sync::mpsc::channel::<String>();
        std::thread::spawn(move || unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
            let voice: Option<ISpVoice> = CoCreateInstance(&SpVoice, None, CLSCTX_ALL).ok();
            for msg in rx {
                if let Some(v) = &voice {
                    let wide: Vec<u16> = msg.encode_utf16().chain(std::iter::once(0)).collect();
                    // dwFlags 0 = lecture SYNCHRONE → les messages sont lus dans l'ordre.
                    let _ = v.Speak(PCWSTR(wide.as_ptr()), 0, None);
                }
            }
        });
        Mutex::new(tx)
    });
    let _ = tx.lock().unwrap_or_else(|p| p.into_inner()).send(text.to_string());
}
