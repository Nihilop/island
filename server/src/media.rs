//! Médias de la fiche (convention `store/` d'`island-formats::media`) : à la publication,
//! chaque image est DÉCODÉE puis RÉ-ENCODÉE en JPEG à taille bornée — tout ce qui n'est pas
//! des pixels (métadonnées, contenu caché, formats exotiques) disparaît, et le catalogue sert
//! des fichiers légers.
//!
//!   vignette   recadrée en 960 × 540 (16:9)
//!   bannière   recadrée en 1920 × 640 (3:1)
//!   captures   réduites à 1920 × 1080 au plus, sans recadrage

use image::imageops::FilterType;
use image::{DynamicImage, GenericImageView, RgbImage};
use island_formats::media::Media;

pub const THUMBNAIL: (u32, u32) = (960, 540);
pub const BANNER: (u32, u32) = (1920, 640);
pub const SCREENSHOT_MAX: (u32, u32) = (1920, 1080);
const QUALITY: u8 = 85;

/// Médias prêts à stocker (JPEG).
pub struct Processed {
    pub thumbnail: Vec<u8>,
    pub banner: Option<Vec<u8>>,
    /// (numéro, JPEG), dans l'ordre.
    pub screenshots: Vec<(u8, Vec<u8>)>,
}

/// JPEG d'une image ; la transparence est posée sur du noir (le fond des aperçus).
pub fn jpeg(img: &DynamicImage) -> Result<Vec<u8>, String> {
    let (w, h) = img.dimensions();
    let rgba = img.to_rgba8();
    let rgb = RgbImage::from_fn(w, h, |x, y| {
        let p = rgba.get_pixel(x, y).0;
        let a = p[3] as u16;
        image::Rgb([(p[0] as u16 * a / 255) as u8, (p[1] as u16 * a / 255) as u8, (p[2] as u16 * a / 255) as u8])
    });
    let mut out = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, QUALITY).encode_image(&rgb).map_err(|e| e.to_string())?;
    Ok(out)
}

fn decode(path: &str, bytes: &[u8]) -> Result<DynamicImage, String> {
    image::load_from_memory(bytes).map_err(|_| format!("invalidImage:{path}"))
}

/// Ré-encode les médias fournis. Sans vignette, `fallback` en fabrique une (rendu approché).
pub fn process(media: &Media, fallback: impl FnOnce() -> image::RgbaImage) -> Result<Processed, String> {
    let thumbnail = match &media.thumbnail {
        Some((p, b)) => decode(p, b)?.resize_to_fill(THUMBNAIL.0, THUMBNAIL.1, FilterType::Triangle),
        None => DynamicImage::ImageRgba8(fallback()).resize_to_fill(THUMBNAIL.0, THUMBNAIL.1, FilterType::Triangle),
    };
    let banner = match &media.banner {
        Some((p, b)) => Some(jpeg(&decode(p, b)?.resize_to_fill(BANNER.0, BANNER.1, FilterType::Triangle))?),
        None => None,
    };
    let mut screenshots = Vec::new();
    for (n, p, b) in &media.screenshots {
        let img = decode(p, b)?;
        let (w, h) = img.dimensions();
        let img = if w > SCREENSHOT_MAX.0 || h > SCREENSHOT_MAX.1 { img.resize(SCREENSHOT_MAX.0, SCREENSHOT_MAX.1, FilterType::Triangle) } else { img };
        screenshots.push((*n, jpeg(&img)?));
    }
    Ok(Processed { thumbnail: jpeg(&thumbnail)?, banner, screenshots })
}

impl Processed {
    /// Les médias tels qu'ils repartent dans le paquet téléchargé (`store/…`, JPEG).
    pub fn as_media(&self, readme: Option<String>) -> Media {
        Media {
            thumbnail: Some(("store/thumbnail.jpg".into(), self.thumbnail.clone())),
            banner: self.banner.as_ref().map(|b| ("store/banner.jpg".into(), b.clone())),
            screenshots: self.screenshots.iter().map(|(n, b)| (*n, format!("store/screenshots/{n}.jpg"), b.clone())).collect(),
            readme,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png(w: u32, h: u32) -> Vec<u8> {
        let mut out = std::io::Cursor::new(Vec::new());
        image::RgbaImage::from_pixel(w, h, image::Rgba([200, 30, 30, 128])).write_to(&mut out, image::ImageFormat::Png).unwrap();
        out.into_inner()
    }

    #[test]
    fn media_are_reencoded_to_bounded_jpegs() {
        let mut m = Media::default();
        m.add("store/thumbnail.png", png(1600, 1200)).unwrap();
        m.add("store/banner.png", png(800, 200)).unwrap();
        m.add("store/screenshots/1.png", png(4000, 2000)).unwrap();
        let p = process(&m, || unreachable!("vignette fournie")).unwrap();
        let dims = |b: &[u8]| image::load_from_memory_with_format(b, image::ImageFormat::Jpeg).unwrap().dimensions();
        assert_eq!(dims(&p.thumbnail), THUMBNAIL);
        assert_eq!(dims(p.banner.as_ref().unwrap()), BANNER);
        assert_eq!(dims(&p.screenshots[0].1), (1920, 960));
    }

    #[test]
    fn missing_thumbnail_uses_the_fallback() {
        let p = process(&Media::default(), || image::RgbaImage::from_pixel(10, 10, image::Rgba([0, 0, 0, 255]))).unwrap();
        assert!(image::load_from_memory(&p.thumbnail).is_ok());
        assert!(p.banner.is_none() && p.screenshots.is_empty());
    }
}
