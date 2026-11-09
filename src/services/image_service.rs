//! Reads images for the model: from a file the person names, from a path typed in a prompt, or
//! from an MCP tool result. Each one is identified by its bytes, scaled down when its long edge is
//! over the limit, and kept as base64 text.

use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use anyhow::{anyhow, bail, Context, Result};
use base64::Engine;
use image::{imageops::FilterType, ImageFormat};
use regex::Regex;
use serde_json::Value;

use crate::models::config::ImagesConfig;
use crate::models::image::ImageData;

/// The file extensions taken as images when a path is typed in a prompt.
const IMAGE_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "gif", "webp"];

/// The JPEG quality used when a JPEG is scaled down.
const JPEG_QUALITY: u8 = 85;

/// Reads an image file and prepares it for the model.
/// @param path - the file
/// @param settings - the scaling and size limits
pub fn load_image(path: &Path, settings: &ImagesConfig) -> Result<ImageData> {
    let bytes = std::fs::read(path).with_context(|| format!("reading image {}", path.display()))?;
    image_from_bytes(bytes, &path.to_string_lossy(), settings).with_context(|| format!("image {}", path.display()))
}

/// Prepares image bytes for the model: checks the type, scales the image down when it is too
/// big, and checks the size of the result.
/// @param bytes - the encoded image
/// @param source - where it came from, recorded with the image
/// @param settings - the scaling and size limits
pub fn image_from_bytes(bytes: Vec<u8>, source: &str, settings: &ImagesConfig) -> Result<ImageData> {
    let format = sniff_format(&bytes).ok_or_else(|| anyhow!("not a PNG, JPEG, GIF or WebP image"))?;
    let (width, height) = image::ImageReader::with_format(Cursor::new(&bytes), format).into_dimensions().context("could not read the image size")?;
    let (bytes, format, width, height) = if needs_scaling(width, height, settings.max_long_edge) { scale_down(&bytes, format, settings.max_long_edge)? } else { (bytes, format, width, height) };
    let data = base64::engine::general_purpose::STANDARD.encode(&bytes);
    if settings.max_bytes > 0 && data.len() > settings.max_bytes {
        bail!("the image is {} bytes as base64, more than the {} byte limit ([images] max_bytes)", data.len(), settings.max_bytes);
    }
    Ok(ImageData { media_type: format.to_mime_type().to_string(), data, width, height, source: source.to_string() })
}

/// Identifies an image by its first bytes, never by its file name.
/// @param bytes - the encoded image
pub fn sniff_format(bytes: &[u8]) -> Option<ImageFormat> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some(ImageFormat::Png)
    } else if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some(ImageFormat::Jpeg)
    } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        Some(ImageFormat::Gif)
    } else if bytes.len() >= 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        Some(ImageFormat::WebP)
    } else {
        None
    }
}

/// Returns true when the long edge is over the limit. A limit of 0 never scales.
/// @param width - the width in pixels
/// @param height - the height in pixels
/// @param max_long_edge - the limit
fn needs_scaling(width: u32, height: u32, max_long_edge: u32) -> bool {
    max_long_edge > 0 && width.max(height) > max_long_edge
}

/// Decodes the image, scales it to fit the long edge, and encodes it again: a JPEG as a JPEG,
/// anything else as a PNG. A GIF keeps only its first frame, which is all the models use.
/// @param bytes - the encoded image
/// @param format - its format
/// @param max_long_edge - the long edge to fit
fn scale_down(bytes: &[u8], format: ImageFormat, max_long_edge: u32) -> Result<(Vec<u8>, ImageFormat, u32, u32)> {
    let decoded = image::load_from_memory_with_format(bytes, format).context("could not decode the image to scale it down")?;
    let scaled = decoded.resize(max_long_edge, max_long_edge, FilterType::Lanczos3);
    let mut out = Vec::new();
    let format = if format == ImageFormat::Jpeg {
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, JPEG_QUALITY).encode_image(&scaled.to_rgb8()).context("could not encode the scaled image")?;
        ImageFormat::Jpeg
    } else {
        scaled.write_to(&mut Cursor::new(&mut out), ImageFormat::Png).context("could not encode the scaled image")?;
        ImageFormat::Png
    };
    Ok((out, format, scaled.width(), scaled.height()))
}

/// Finds the image files named in typed text: words that, without a leading `@` and quotes, name
/// an existing file with an image extension. Relative paths are taken from the working folder.
/// @param text - what the person typed
/// @param cwd - the working folder
pub fn find_image_paths(text: &str, cwd: &Path) -> Vec<PathBuf> {
    static WORDS: OnceLock<Regex> = OnceLock::new();
    let words = WORDS.get_or_init(|| Regex::new(r#"@?"([^"]+)"|@?'([^']+)'|(\S+)"#).expect("valid regex"));
    let mut found: Vec<PathBuf> = Vec::new();
    for captures in words.captures_iter(text) {
        let Some(word) = captures.get(1).or(captures.get(2)).or(captures.get(3)) else { continue };
        let Some(path) = image_path(word.as_str(), cwd) else { continue };
        if !found.contains(&path) {
            found.push(path);
        }
    }
    found
}

/// Turns one word into an image path when it names an existing image file.
/// @param word - one word or quoted string from the text
/// @param cwd - the working folder
fn image_path(word: &str, cwd: &Path) -> Option<PathBuf> {
    let word = word.trim_start_matches('@').trim_end_matches([',', ';', ':', ')', ']', '!', '?']);
    let word = word.strip_suffix('.').filter(|w| has_image_extension(w)).unwrap_or(word);
    if !has_image_extension(word) {
        return None;
    }
    let path = cwd.join(git_bash_path(word));
    path.is_file().then_some(path)
}

/// Returns true when a name ends in one of the image extensions.
/// @param name - a file name or path
fn has_image_extension(name: &str) -> bool {
    Path::new(name).extension().and_then(|e| e.to_str()).is_some_and(|e| IMAGE_EXTENSIONS.contains(&e.to_lowercase().as_str()))
}

/// On Windows, turns a Git Bash path such as `/c/Users/x.png` into `C:/Users/x.png`, which is
/// what a file dragged into a Git Bash terminal pastes.
/// @param word - the path as typed
fn git_bash_path(word: &str) -> String {
    let bytes = word.as_bytes();
    if cfg!(windows) && bytes.len() > 3 && bytes[0] == b'/' && bytes[1].is_ascii_alphabetic() && bytes[2] == b'/' {
        return format!("{}:{}", (bytes[1] as char).to_ascii_uppercase(), &word[2..]);
    }
    word.to_string()
}

/// Loads every image named in typed text. An image that cannot be used is reported in the
/// returned notices and left out.
/// @param text - what the person typed
/// @param cwd - the working folder
/// @param settings - the image settings
pub fn images_in_text(text: &str, cwd: &Path, settings: &ImagesConfig) -> (Vec<ImageData>, Vec<String>) {
    if !settings.detect_paths {
        return (Vec::new(), Vec::new());
    }
    let mut images = Vec::new();
    let mut notices = Vec::new();
    for path in find_image_paths(text, cwd) {
        match load_image(&path, settings) {
            Ok(image) => images.push(image),
            Err(error) => notices.push(format!("{error:#}, so it was not attached")),
        }
    }
    (images, notices)
}

/// Converts one MCP `image` content item. Returns an error for bad base64 or an unknown type.
/// @param item - the content item, `{"type": "image", "mimeType": ..., "data": ...}`
/// @param source - the tool that returned it
/// @param settings - the image settings
pub fn image_from_mcp(item: &Value, source: &str, settings: &ImagesConfig) -> Result<ImageData> {
    let data = item["data"].as_str().ok_or_else(|| anyhow!("the image has no data"))?;
    let bytes = base64::engine::general_purpose::STANDARD.decode(data.trim()).context("the image data is not base64")?;
    image_from_bytes(bytes, source, settings)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Encodes a plain image of the given size as a PNG.
    fn png(width: u32, height: u32) -> Vec<u8> {
        let mut out = Vec::new();
        image::DynamicImage::new_rgb8(width, height).write_to(&mut Cursor::new(&mut out), ImageFormat::Png).unwrap();
        out
    }

    #[test]
    fn identifies_images_by_their_bytes() {
        assert_eq!(sniff_format(&png(2, 2)), Some(ImageFormat::Png));
        assert_eq!(sniff_format(&[0xFF, 0xD8, 0xFF, 0xE0]), Some(ImageFormat::Jpeg));
        assert_eq!(sniff_format(b"GIF89a...."), Some(ImageFormat::Gif));
        assert_eq!(sniff_format(b"RIFF\0\0\0\0WEBPVP8 "), Some(ImageFormat::WebP));
        assert_eq!(sniff_format(b"hello, I am text"), None);
    }

    #[test]
    fn scales_big_images_and_keeps_small_ones() {
        let settings = ImagesConfig::default();
        let big = image_from_bytes(png(3000, 1500), "big", &settings).unwrap();
        assert_eq!((big.width, big.height, big.media_type.as_str()), (1568, 784, "image/png"));
        let small = image_from_bytes(png(40, 30), "small", &settings).unwrap();
        assert_eq!((small.width, small.height), (40, 30));
        let never = ImagesConfig { max_long_edge: 0, ..ImagesConfig::default() };
        assert_eq!(image_from_bytes(png(3000, 10), "x", &never).unwrap().width, 3000);
        assert!(image_from_bytes(b"not an image".to_vec(), "x", &settings).is_err());
        let tiny_limit = ImagesConfig { max_bytes: 10, ..ImagesConfig::default() };
        assert!(image_from_bytes(png(40, 30), "x", &tiny_limit).is_err());
    }

    #[test]
    fn finds_image_paths_in_typed_text() {
        let dir = std::env::temp_dir().join(format!("umbra-image-paths-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("sub dir")).unwrap();
        std::fs::write(dir.join("a.png"), png(2, 2)).unwrap();
        std::fs::write(dir.join("sub dir").join("b.JPG"), b"x").unwrap();
        std::fs::write(dir.join("notes.txt"), b"x").unwrap();
        let text = "compare @a.png with \"sub dir/b.JPG\", and a.png. Ignore notes.txt and missing.png";
        let found = find_image_paths(text, &dir);
        assert_eq!(found, vec![dir.join("a.png"), dir.join("sub dir/b.JPG")]);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
