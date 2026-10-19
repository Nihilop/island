//! Retrait des métadonnées des images publiées, SANS recompression (l'image reste
//! identique au pixel près) : EXIF (dont la position GPS de l'auteur), XMP, IPTC,
//! commentaires et blocs texte. PNG, JPEG et WebP ; les autres formats sont rendus tels
//! quels. Une image qui ne se lit pas comme attendu est aussi rendue telle quelle : elle a
//! déjà été validée (décodée) par `island-formats`, ceci n'est qu'un nettoyage.

/// Image nettoyée selon son format (reconnu par sa signature, pas par son nom).
pub fn strip(bytes: &[u8]) -> Vec<u8> {
    let out = if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        png(bytes)
    } else if bytes.starts_with(&[0xFF, 0xD8]) {
        jpeg(bytes)
    } else if bytes.len() >= 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        webp(bytes)
    } else {
        None
    };
    out.unwrap_or_else(|| bytes.to_vec())
}

/// PNG : on retire les blocs `tEXt`, `zTXt`, `iTXt`, `eXIf`, `tIME`.
fn png(b: &[u8]) -> Option<Vec<u8>> {
    let mut out = b[..8].to_vec();
    let mut i = 8;
    while i + 12 <= b.len() {
        let len = u32::from_be_bytes(b[i..i + 4].try_into().ok()?) as usize;
        let end = i.checked_add(12)?.checked_add(len)?;
        if end > b.len() {
            return None;
        }
        let kind = &b[i + 4..i + 8];
        if !matches!(kind, b"tEXt" | b"zTXt" | b"iTXt" | b"eXIf" | b"tIME") {
            out.extend_from_slice(&b[i..end]);
        }
        i = end;
        if kind == b"IEND" {
            return Some(out);
        }
    }
    None
}

/// JPEG : on retire les segments APP1 (EXIF, XMP), APP13 (IPTC) et COM ; on garde le reste
/// (dont APP2, le profil de couleur). Tout ce qui suit le début de l'image (SOS) est copié.
fn jpeg(b: &[u8]) -> Option<Vec<u8>> {
    let mut out = vec![0xFF, 0xD8];
    let mut i = 2;
    loop {
        if i + 4 > b.len() || b[i] != 0xFF {
            return None;
        }
        let marker = b[i + 1];
        // Marqueurs sans longueur (remplissage, redémarrages).
        if marker == 0xFF {
            i += 1;
            continue;
        }
        if (0xD0..=0xD7).contains(&marker) || marker == 0x01 {
            out.extend_from_slice(&b[i..i + 2]);
            i += 2;
            continue;
        }
        let len = u16::from_be_bytes([b[i + 2], b[i + 3]]) as usize;
        let end = i + 2 + len;
        if len < 2 || end > b.len() {
            return None;
        }
        if marker == 0xDA {
            // Début des données de l'image : tout le reste tel quel.
            out.extend_from_slice(&b[i..]);
            return Some(out);
        }
        if !matches!(marker, 0xE1 | 0xED | 0xFE) {
            out.extend_from_slice(&b[i..end]);
        }
        i = end;
    }
}

/// WebP : on retire les blocs `EXIF` et `XMP ` et on éteint leurs drapeaux dans `VP8X`.
fn webp(b: &[u8]) -> Option<Vec<u8>> {
    let mut out = b"RIFF\0\0\0\0WEBP".to_vec();
    let mut i = 12;
    while i + 8 <= b.len() {
        let size = u32::from_le_bytes(b[i + 4..i + 8].try_into().ok()?) as usize;
        let end = i.checked_add(8)?.checked_add(size + (size & 1))?;
        if end > b.len() {
            return None;
        }
        let kind = &b[i..i + 4];
        if kind != b"EXIF" && kind != b"XMP " {
            let start = out.len();
            out.extend_from_slice(&b[i..end]);
            if kind == b"VP8X" && size >= 1 {
                out[start + 8] &= !(0x08 | 0x04); // drapeaux EXIF et XMP
            }
        }
        i = end;
    }
    let riff = (out.len() - 8) as u32;
    out[4..8].copy_from_slice(&riff.to_le_bytes());
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn encode(format: image::ImageFormat) -> Vec<u8> {
        let img = image::RgbaImage::from_fn(8, 8, |x, y| image::Rgba([x as u8 * 30, y as u8 * 30, 90, 255]));
        let mut out = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(img).to_rgb8().write_to(&mut out, format).unwrap();
        out.into_inner()
    }

    fn contains(hay: &[u8], needle: &[u8]) -> bool {
        hay.windows(needle.len()).any(|w| w == needle)
    }

    #[test]
    fn png_text_chunks_are_removed_and_pixels_kept() {
        let clean = encode(image::ImageFormat::Png);
        // Bloc tEXt « GPS » inséré après IHDR (8 + 25 octets).
        let data = b"GPS\0lat=48.8566";
        let mut chunk = (data.len() as u32).to_be_bytes().to_vec();
        chunk.extend_from_slice(b"tEXt");
        chunk.extend_from_slice(data);
        chunk.extend_from_slice(&[0, 0, 0, 0]); // CRC (non vérifié par le nettoyage)
        let mut dirty = clean[..33].to_vec();
        dirty.extend_from_slice(&chunk);
        dirty.extend_from_slice(&clean[33..]);
        let out = strip(&dirty);
        assert!(!contains(&out, b"lat=48.8566"));
        assert_eq!(out, clean);
    }

    #[test]
    fn jpeg_exif_is_removed_and_image_still_decodes() {
        let clean = encode(image::ImageFormat::Jpeg);
        let payload = b"Exif\0\0GPS 48.8566";
        let mut seg = vec![0xFF, 0xE1];
        seg.extend_from_slice(&((payload.len() + 2) as u16).to_be_bytes());
        seg.extend_from_slice(payload);
        let mut dirty = vec![0xFF, 0xD8];
        dirty.extend_from_slice(&seg);
        dirty.extend_from_slice(&clean[2..]);
        let out = strip(&dirty);
        assert!(!contains(&out, b"GPS 48.8566"));
        assert!(image::load_from_memory(&out).is_ok());
    }

    #[test]
    fn unknown_or_broken_files_are_returned_as_is() {
        assert_eq!(strip(b"GIF89a..."), b"GIF89a...".to_vec());
        let broken = b"\x89PNG\r\n\x1a\n\0\0\0\xffIHDR".to_vec();
        assert_eq!(strip(&broken), broken);
    }
}
