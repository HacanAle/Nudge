//! Backdrop for the "Vitrine" (glass) theme.
//!
//! A transparent webview can't blur the desktop behind it — CSS `backdrop-filter` only sees the page's
//! own content. So just before the alert is shown we copy the screen region it is about to cover,
//! shrink it (blurring hides the loss, and it keeps the payload around 40 KB), and hand it to the
//! page as a data-URL. The page blurs it behind the card, which gives true frosted glass.

use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use serde::Serialize;
use tauri::{AppHandle, Manager};

use crate::screen::Area;
use crate::{settings, AppState};

/// The snapshot is taken at 1/DOWNSCALE of the real pixel size.
const DOWNSCALE: i32 = 4;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    /// `data:image/bmp;base64,…`
    pub url: String,
    /// Logical (CSS) size the image should be stretched to.
    pub width: f64,
    pub height: f64,
}

/// A snapshot of `area` — but only when the glass theme is active (every other theme paints its own surface).
pub fn for_theme(app: &AppHandle, area: &Area, logical_w: f64, logical_h: f64) -> Option<Snapshot> {
    if settings::theme(&app.state::<AppState>().db) != settings::GLASS_THEME {
        return None;
    }
    capture(area.x, area.y, area.width, area.height, logical_w, logical_h)
}

/// Copy the physical-pixel rectangle `(x, y, w, h)` of the desktop.
/// `logical_w` / `logical_h` are the same area in CSS pixels.
pub fn capture(x: i32, y: i32, w: i32, h: i32, logical_w: f64, logical_h: f64) -> Option<Snapshot> {
    if w <= 0 || h <= 0 {
        return None;
    }
    let (dw, dh) = ((w / DOWNSCALE).max(1), (h / DOWNSCALE).max(1));
    let rows = grab(x, y, w, h, dw, dh)?;
    let bmp = bmp_24(dw as u32, dh as u32, &rows);
    Some(Snapshot {
        url: format!("data:image/bmp;base64,{}", STANDARD.encode(bmp)),
        width: logical_w,
        height: logical_h,
    })
}

/// 24-bit BMP from bottom-up, 4-byte-aligned BGR rows (exactly what `GetDIBits` produces).
fn bmp_24(width: u32, height: u32, rows: &[u8]) -> Vec<u8> {
    const HEADERS: u32 = 14 + 40;
    let mut out = Vec::with_capacity(HEADERS as usize + rows.len());
    out.extend_from_slice(b"BM");
    out.extend_from_slice(&(HEADERS + rows.len() as u32).to_le_bytes());
    out.extend_from_slice(&[0; 4]); // reserved
    out.extend_from_slice(&HEADERS.to_le_bytes()); // pixel data offset
    out.extend_from_slice(&40u32.to_le_bytes()); // BITMAPINFOHEADER size
    out.extend_from_slice(&(width as i32).to_le_bytes());
    out.extend_from_slice(&(height as i32).to_le_bytes()); // positive = bottom-up
    out.extend_from_slice(&1u16.to_le_bytes()); // planes
    out.extend_from_slice(&24u16.to_le_bytes()); // bits per pixel
    out.extend_from_slice(&[0; 4]); // BI_RGB
    out.extend_from_slice(&(rows.len() as u32).to_le_bytes());
    out.extend_from_slice(&[0; 16]); // resolution + palette fields
    out.extend_from_slice(rows);
    out
}

#[cfg(windows)]
fn grab(x: i32, y: i32, w: i32, h: i32, dw: i32, dh: i32) -> Option<Vec<u8>> {
    use std::mem::size_of;
    use windows::Win32::Graphics::Gdi::{
        CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, DeleteObject, GetDC, GetDIBits, ReleaseDC,
        SelectObject, SetStretchBltMode, StretchBlt, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, CAPTUREBLT,
        DIB_RGB_COLORS, HALFTONE, ROP_CODE, SRCCOPY,
    };

    // SAFETY: plain GDI calls on handles created and released in this function.
    unsafe {
        let screen = GetDC(None);
        if screen.is_invalid() {
            return None;
        }
        let mem = CreateCompatibleDC(Some(screen));
        let bitmap = CreateCompatibleBitmap(screen, dw, dh);
        let previous = SelectObject(mem, bitmap.into());

        SetStretchBltMode(mem, HALFTONE);
        // CAPTUREBLT includes layered (semi-transparent) windows in the copy.
        let copied = StretchBlt(
            mem,
            0,
            0,
            dw,
            dh,
            Some(screen),
            x,
            y,
            w,
            h,
            ROP_CODE(SRCCOPY.0 | CAPTUREBLT.0),
        )
        .as_bool();

        let mut info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: dw,
                biHeight: dh,
                biPlanes: 1,
                biBitCount: 24,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let stride = (dw as usize * 3).div_ceil(4) * 4;
        let mut pixels = vec![0u8; stride * dh as usize];
        let lines = GetDIBits(
            mem,
            bitmap,
            0,
            dh as u32,
            Some(pixels.as_mut_ptr().cast()),
            &mut info,
            DIB_RGB_COLORS,
        );

        SelectObject(mem, previous);
        let _ = DeleteObject(bitmap.into());
        let _ = DeleteDC(mem);
        ReleaseDC(None, screen);

        (copied && lines > 0).then_some(pixels)
    }
}

#[cfg(not(windows))]
fn grab(_x: i32, _y: i32, _w: i32, _h: i32, _dw: i32, _dh: i32) -> Option<Vec<u8>> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bmp_header_matches_the_pixel_data() {
        // 3×2 image: rows are padded to 12 bytes (3 px × 3 B = 9 → 12).
        let rows = vec![7u8; 12 * 2];
        let bmp = bmp_24(3, 2, &rows);

        assert_eq!(&bmp[0..2], b"BM");
        assert_eq!(u32::from_le_bytes(bmp[2..6].try_into().unwrap()) as usize, bmp.len());
        assert_eq!(u32::from_le_bytes(bmp[10..14].try_into().unwrap()), 54, "pixel data starts after both headers");
        assert_eq!(i32::from_le_bytes(bmp[18..22].try_into().unwrap()), 3);
        assert_eq!(i32::from_le_bytes(bmp[22..26].try_into().unwrap()), 2);
        assert_eq!(u16::from_le_bytes(bmp[28..30].try_into().unwrap()), 24);
        assert_eq!(bmp.len(), 54 + rows.len());
    }

    #[test]
    fn an_empty_region_is_not_captured() {
        assert!(capture(0, 0, 0, 100, 0.0, 100.0).is_none());
        assert!(capture(0, 0, 100, -5, 100.0, 0.0).is_none());
    }
}
