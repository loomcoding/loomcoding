//! Lectura del portapapeles y pegado de imágenes y texto.

use super::*;

/// Variable que desactiva la lectura de imágenes del portapapeles real. Cabe señalar que la fijan
/// el entorno aislado de las pruebas e2e y que, en las pruebas unitarias, la lectura está apagada
/// siempre: un resultado no debe depender de lo que la persona tenga copiado en ese momento.
pub const NO_CLIPBOARD_ENV: &str = "LOOMCODING_NO_CLIPBOARD";

pub(super) fn read_clipboard_image() -> Result<Option<Vec<u8>>, String> {
    if cfg!(test) || std::env::var_os(NO_CLIPBOARD_ENV).is_some() {
        return Ok(None);
    }
    let mut clipboard = arboard::Clipboard::new().map_err(|error| error.to_string())?;
    let Ok(image) = clipboard.get_image() else {
        return Ok(None);
    };
    let raw_len = image
        .width
        .checked_mul(image.height)
        .and_then(|pixels| pixels.checked_mul(4))
        .ok_or_else(|| "Clipboard image dimensions are invalid".to_owned())?;
    if raw_len > 32 * 1024 * 1024 || image.bytes.len() != raw_len {
        return Err("Clipboard image is too large".into());
    }
    let pixels = image::RgbaImage::from_raw(
        image.width as u32,
        image.height as u32,
        image.bytes.into_owned(),
    )
    .ok_or_else(|| "Clipboard image data is invalid".to_owned())?;
    let mut png = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(pixels)
        .write_to(&mut png, image::ImageFormat::Png)
        .map_err(|error| error.to_string())?;
    Ok(Some(png.into_inner()))
}

/// Retorna true cuando el portapapeles contenía imagen y el pegado quedó atendido.
pub(super) fn paste_clipboard_image(app: &mut App) -> bool {
    match read_clipboard_image() {
        Ok(Some(bytes)) => match app.attach_image_bytes(&bytes, "clipboard.png") {
            Ok(()) => {
                app.slash_feedback = Some("Image pasted from clipboard".into());
                true
            }
            Err(error) => {
                app.slash_feedback = Some(format!("Clipboard image rejected: {error}"));
                true
            }
        },
        Ok(None) => false,
        Err(error) => {
            app.slash_feedback = Some(format!("Clipboard image read failed: {error}"));
            true
        }
    }
}

pub(super) fn paste_from_clipboard(app: &mut App) {
    if paste_clipboard_image(app) {
        return;
    }
    match arboard::Clipboard::new().and_then(|mut clipboard| clipboard.get_text()) {
        Ok(text) => {
            app.paste(&text);
            app.slash_feedback = Some("Text pasted from clipboard".into());
        }
        Err(error) => app.slash_feedback = Some(format!("Clipboard paste failed: {error}")),
    }
}
