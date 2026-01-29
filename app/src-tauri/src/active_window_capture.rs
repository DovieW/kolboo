#![allow(dead_code)]

#[derive(Debug, Clone)]
pub struct ActiveWindowCapture {
    pub captured_at_ms: u64,
    pub window_title: Option<String>,
    pub image_png_bytes: Vec<u8>,
    pub image_width_px: u32,
    pub image_height_px: u32,
    /// Which capture method produced the final image.
    pub capture_method: CaptureMethod,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureMethod {
    PrintWindow,
    BitBlt,
}

impl std::fmt::Display for CaptureMethod {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CaptureMethod::PrintWindow => write!(f, "PrintWindow"),
            CaptureMethod::BitBlt => write!(f, "BitBlt"),
        }
    }
}

/// Result of validating a captured image for OCR quality.
#[derive(Debug, Clone)]
pub enum ImageValidation {
    /// Image passed validation.
    Valid,
    /// Image is blank (all pixels are near-black or transparent).
    Blank,
    /// Image is uniform color (low variance, would cause OCR hallucinations).
    UniformColor { dominant_rgb: (u8, u8, u8) },
}

/// Detailed validation result with metrics for logging.
#[derive(Debug, Clone)]
pub struct ImageValidationResult {
    pub validation: ImageValidation,
    pub variance: u64,
    pub threshold: u64,
    pub mean_rgb: (u8, u8, u8),
}

impl ImageValidation {
    pub fn is_valid(&self) -> bool {
        matches!(self, ImageValidation::Valid)
    }

    pub fn reason(&self) -> Option<String> {
        match self {
            ImageValidation::Valid => None,
            ImageValidation::Blank => Some("captured image is blank".to_string()),
            ImageValidation::UniformColor { dominant_rgb } => Some(format!(
                "captured image is uniform color (RGB: {}, {}, {})",
                dominant_rgb.0, dominant_rgb.1, dominant_rgb.2
            )),
        }
    }
}

/// Quick sanity check: is the image mostly blank (all near-black)?
/// This is fast and catches obvious capture failures.
pub fn quick_blank_check(rgba_buffer: &[u8]) -> bool {
    // Sample every 50th pixel to check for non-black content.
    let mut non_black = 0usize;
    let step = 50 * 4; // 50 pixels * 4 bytes per pixel
    let mut sampled = 0usize;
    for chunk in rgba_buffer.chunks(step).filter_map(|c| c.get(0..4)) {
        sampled += 1;
        // Check if R, G, or B is above a threshold (not pure black).
        if chunk[0] > 10 || chunk[1] > 10 || chunk[2] > 10 {
            non_black += 1;
        }
    }
    // If less than 5% of sampled pixels are non-black, consider it blank.
    sampled > 0 && non_black * 100 / sampled < 5
}

/// Robust validation for hallucination protection.
/// Checks if the image has enough variance to be useful for OCR.
/// Returns a result with validation status and metrics for logging.
///
/// `variance_threshold`: minimum combined RGB variance. Real screenshots with text/UI
/// typically have variance 5000+. Near-solid images have variance 0-3000. Default: 2000.
pub fn validate_image_for_ocr(
    rgba_buffer: &[u8],
    variance_threshold: u64,
) -> ImageValidationResult {
    if rgba_buffer.len() < 4 {
        return ImageValidationResult {
            validation: ImageValidation::Blank,
            variance: 0,
            threshold: variance_threshold,
            mean_rgb: (0, 0, 0),
        };
    }

    // Sample pixels to calculate statistics.
    let step = 20 * 4; // Sample every 20th pixel
    let mut r_sum: u64 = 0;
    let mut g_sum: u64 = 0;
    let mut b_sum: u64 = 0;
    let mut r_sq_sum: u64 = 0;
    let mut g_sq_sum: u64 = 0;
    let mut b_sq_sum: u64 = 0;
    let mut count: u64 = 0;
    let mut non_black = 0u64;

    for chunk in rgba_buffer.chunks(step).filter_map(|c| c.get(0..4)) {
        let r = chunk[0] as u64;
        let g = chunk[1] as u64;
        let b = chunk[2] as u64;

        r_sum += r;
        g_sum += g;
        b_sum += b;
        r_sq_sum += r * r;
        g_sq_sum += g * g;
        b_sq_sum += b * b;
        count += 1;

        if chunk[0] > 10 || chunk[1] > 10 || chunk[2] > 10 {
            non_black += 1;
        }
    }

    if count == 0 {
        return ImageValidationResult {
            validation: ImageValidation::Blank,
            variance: 0,
            threshold: variance_threshold,
            mean_rgb: (0, 0, 0),
        };
    }

    // Check for mostly blank (near-black).
    if count > 0 && non_black * 100 / count < 5 {
        return ImageValidationResult {
            validation: ImageValidation::Blank,
            variance: 0,
            threshold: variance_threshold,
            mean_rgb: (0, 0, 0),
        };
    }

    // Calculate mean.
    let r_mean = r_sum / count;
    let g_mean = g_sum / count;
    let b_mean = b_sum / count;
    let mean_rgb = (r_mean as u8, g_mean as u8, b_mean as u8);

    // Calculate variance: E[X²] - E[X]²
    let r_var = (r_sq_sum / count).saturating_sub(r_mean * r_mean);
    let g_var = (g_sq_sum / count).saturating_sub(g_mean * g_mean);
    let b_var = (b_sq_sum / count).saturating_sub(b_mean * b_mean);

    // Combined variance across all channels.
    let total_var = r_var + g_var + b_var;

    // If variance is very low, the image is essentially a solid color.
    // A real screenshot with text/UI elements will have significant variance (typically 500+).

    log::debug!(
        "OCR image validation: mean RGB=({},{},{}), variance R={} G={} B={} total={}, threshold={}",
        r_mean,
        g_mean,
        b_mean,
        r_var,
        g_var,
        b_var,
        total_var,
        variance_threshold
    );

    if total_var < variance_threshold {
        return ImageValidationResult {
            validation: ImageValidation::UniformColor {
                dominant_rgb: mean_rgb,
            },
            variance: total_var,
            threshold: variance_threshold,
            mean_rgb,
        };
    }

    ImageValidationResult {
        validation: ImageValidation::Valid,
        variance: total_var,
        threshold: variance_threshold,
        mean_rgb,
    }
}

#[cfg(target_os = "windows")]
#[allow(dead_code)]
pub fn capture_active_window_png(
    max_dimension: u32,
    resize_filter: &str,
) -> Result<ActiveWindowCapture, String> {
    use windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow;

    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd == Default::default() {
            return Err("No foreground window".to_string());
        }
        capture_window_png_impl(hwnd, max_dimension, resize_filter)
    }
}

#[cfg(target_os = "windows")]
#[allow(dead_code)]
pub fn capture_window_png(
    hwnd: windows::Win32::Foundation::HWND,
    max_dimension: u32,
    resize_filter: &str,
) -> Result<ActiveWindowCapture, String> {
    capture_window_png_impl(hwnd, max_dimension, resize_filter)
}

#[cfg(target_os = "windows")]
fn capture_window_png_impl(
    hwnd: windows::Win32::Foundation::HWND,
    max_dimension: u32,
    resize_filter: &str,
) -> Result<ActiveWindowCapture, String> {
    use image::{imageops::FilterType, DynamicImage, ImageBuffer, ImageFormat, Rgba};
    use std::io::Cursor;
    use std::time::{Instant, SystemTime, UNIX_EPOCH};
    use windows::Win32::Graphics::Gdi::{
        BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, DeleteObject, GetDIBits,
        ReleaseDC, SelectObject, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, CAPTUREBLT, DIB_RGB_COLORS,
        HBITMAP, HDC, HGDIOBJ, RGBQUAD, SRCCOPY,
    };
    use windows::Win32::Storage::Xps::{PrintWindow, PRINT_WINDOW_FLAGS};
    use windows::Win32::UI::WindowsAndMessaging::{GetWindowRect, GetWindowTextW};

    let capture_start = Instant::now();

    unsafe {
        if hwnd == Default::default() {
            return Err("No window handle".to_string());
        }

        let mut rect = Default::default();
        GetWindowRect(hwnd, &mut rect).map_err(|_| "Failed to get window rect".to_string())?;

        let width = rect.right - rect.left;
        let height = rect.bottom - rect.top;
        if width <= 0 || height <= 0 {
            return Err("Invalid window size".to_string());
        }

        let window_dc: HDC = windows::Win32::Graphics::Gdi::GetWindowDC(Some(hwnd));
        if window_dc == HDC::default() {
            return Err("Failed to acquire window DC".to_string());
        }

        let memory_dc: HDC = CreateCompatibleDC(Some(window_dc));
        if memory_dc == HDC::default() {
            let _ = ReleaseDC(Some(hwnd), window_dc);
            return Err("Failed to create compatible DC".to_string());
        }

        let bitmap: HBITMAP = CreateCompatibleBitmap(window_dc, width, height);
        if bitmap == HBITMAP::default() {
            let _ = DeleteDC(memory_dc);
            let _ = ReleaseDC(Some(hwnd), window_dc);
            return Err("Failed to create compatible bitmap".to_string());
        }

        let old_object: HGDIOBJ = SelectObject(memory_dc, bitmap.into());
        if old_object == HGDIOBJ::default() {
            let _ = DeleteObject(bitmap.into());
            let _ = DeleteDC(memory_dc);
            let _ = ReleaseDC(Some(hwnd), window_dc);
            return Err("Failed to select bitmap into DC".to_string());
        }

        // PW_RENDERFULLCONTENT (0x2) is required for DWM-composited / hardware-accelerated
        // windows (e.g., VS Code, browsers). Without it, PrintWindow often returns a blank image.
        const PW_RENDERFULLCONTENT: u32 = 2;
        let print_start = Instant::now();
        let print_ok =
            PrintWindow(hwnd, memory_dc, PRINT_WINDOW_FLAGS(PW_RENDERFULLCONTENT)).as_bool();
        let print_elapsed_ms = print_start.elapsed().as_millis();
        log::debug!(
            "OCR capture timing: PrintWindow took {}ms (ok={})",
            print_elapsed_ms,
            print_ok
        );

        // Helper to read the bitmap into a buffer for inspection (BGRA format).
        let read_bitmap_bgra = |dc: HDC, bmp: HBITMAP, w: i32, h: i32| -> Option<Vec<u8>> {
            let mut info = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: w,
                    biHeight: -h,
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB.0,
                    biSizeImage: 0,
                    biXPelsPerMeter: 0,
                    biYPelsPerMeter: 0,
                    biClrUsed: 0,
                    biClrImportant: 0,
                },
                bmiColors: [RGBQUAD::default(); 1],
            };
            let mut buf = vec![0u8; (w as usize) * (h as usize) * 4];
            let lines = GetDIBits(
                dc,
                bmp,
                0,
                h as u32,
                Some(buf.as_mut_ptr().cast()),
                &mut info,
                DIB_RGB_COLORS,
            );
            if lines > 0 {
                Some(buf)
            } else {
                None
            }
        };

        // Convert BGRA buffer to RGBA for validation (swaps B and R).
        let bgra_to_rgba = |buf: &[u8]| -> Vec<u8> {
            let mut rgba = buf.to_vec();
            for chunk in rgba.chunks_exact_mut(4) {
                chunk.swap(0, 2);
            }
            rgba
        };

        // Check if PrintWindow produced a blank image; if so, retry with BitBlt.
        let mut capture_method = CaptureMethod::PrintWindow;
        let mut need_bitblt_fallback = !print_ok;
        if print_ok {
            if let Some(bgra_buf) = read_bitmap_bgra(memory_dc, bitmap, width, height) {
                let rgba_buf = bgra_to_rgba(&bgra_buf);
                if quick_blank_check(&rgba_buf) {
                    log::debug!("PrintWindow produced blank image, falling back to BitBlt");
                    need_bitblt_fallback = true;
                }
            }
        }

        if need_bitblt_fallback {
            capture_method = CaptureMethod::BitBlt;
            // Fallback to BitBlt for older windows or when PrintWindow fails.
            BitBlt(
                memory_dc,
                0,
                0,
                width,
                height,
                Some(window_dc),
                0,
                0,
                SRCCOPY | CAPTUREBLT,
            )
            .map_err(|_| "BitBlt failed".to_string())?;

            // Check if BitBlt also produced a blank image.
            if let Some(bgra_buf) = read_bitmap_bgra(memory_dc, bitmap, width, height) {
                let rgba_buf = bgra_to_rgba(&bgra_buf);
                if quick_blank_check(&rgba_buf) {
                    // Both methods failed - cleanup and return error.
                    let _ = SelectObject(memory_dc, old_object);
                    let _ = DeleteObject(bitmap.into());
                    let _ = DeleteDC(memory_dc);
                    let _ = ReleaseDC(Some(hwnd), window_dc);
                    return Err("Both PrintWindow and BitBlt produced blank images".to_string());
                }
            }
        }

        let mut bitmap_info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: width,
                biHeight: -height,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                biSizeImage: 0,
                biXPelsPerMeter: 0,
                biYPelsPerMeter: 0,
                biClrUsed: 0,
                biClrImportant: 0,
            },
            bmiColors: [RGBQUAD::default(); 1],
        };

        let mut buffer = vec![0u8; (width as usize) * (height as usize) * 4];
        let scan_lines = GetDIBits(
            memory_dc,
            bitmap,
            0,
            height as u32,
            Some(buffer.as_mut_ptr().cast()),
            &mut bitmap_info,
            DIB_RGB_COLORS,
        );

        let _ = SelectObject(memory_dc, old_object);
        let _ = DeleteObject(bitmap.into());
        let _ = DeleteDC(memory_dc);
        let _ = ReleaseDC(Some(hwnd), window_dc);

        if scan_lines == 0 {
            return Err("GetDIBits failed".to_string());
        }

        // Convert BGRA to RGBA and ensure alpha is opaque.
        // PrintWindow/BitBlt often leave alpha at 0 even when RGB values are valid.
        for chunk in buffer.chunks_exact_mut(4) {
            chunk.swap(0, 2); // BGR -> RGB
            chunk[3] = 255; // Force alpha to opaque
        }

        let mut image = ImageBuffer::<Rgba<u8>, _>::from_raw(width as u32, height as u32, buffer)
            .ok_or_else(|| "Failed to build image buffer".to_string())?;

        if max_dimension > 0 {
            let current_max = image.width().max(image.height());
            if current_max > max_dimension {
                let scale = max_dimension as f32 / current_max as f32;
                let target_w = (image.width() as f32 * scale).round().max(1.0) as u32;
                let target_h = (image.height() as f32 * scale).round().max(1.0) as u32;
                let resize_start = Instant::now();
                let filter = match resize_filter {
                    "triangle" => FilterType::Triangle,
                    "catmullrom" => FilterType::CatmullRom,
                    "lanczos3" => FilterType::Lanczos3,
                    _ => FilterType::Nearest, // Default to fastest
                };
                image = image::imageops::resize(&image, target_w, target_h, filter);
                log::debug!(
                    "OCR capture timing: resize ({}x{} -> {}x{}, filter={}) took {}ms",
                    width,
                    height,
                    target_w,
                    target_h,
                    resize_filter,
                    resize_start.elapsed().as_millis()
                );
            }
        }

        let png_start = Instant::now();
        let mut png_bytes: Vec<u8> = Vec::new();
        let mut cursor = Cursor::new(&mut png_bytes);
        let dynamic_image = DynamicImage::ImageRgba8(image);
        dynamic_image
            .write_to(&mut cursor, ImageFormat::Png)
            .map_err(|e| format!("PNG encode failed: {}", e))?;
        log::debug!(
            "OCR capture timing: PNG encode took {}ms (bytes={})",
            png_start.elapsed().as_millis(),
            png_bytes.len()
        );
        let final_width = dynamic_image.width();
        let final_height = dynamic_image.height();

        log::debug!(
            "OCR capture timing: total {}ms",
            capture_start.elapsed().as_millis()
        );

        let window_title = {
            let mut buf = [0u16; 512];
            let len = GetWindowTextW(hwnd, &mut buf);
            if len > 0 {
                Some(String::from_utf16_lossy(&buf[..len as usize]))
            } else {
                None
            }
        };

        let captured_at_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        Ok(ActiveWindowCapture {
            captured_at_ms,
            window_title,
            image_png_bytes: png_bytes,
            image_width_px: final_width,
            image_height_px: final_height,
            capture_method,
        })
    }
}

#[cfg(not(target_os = "windows"))]
#[allow(dead_code)]
pub fn capture_active_window_png(
    _max_dimension: u32,
    _resize_filter: &str,
) -> Result<ActiveWindowCapture, String> {
    Err("Active window capture is only supported on Windows".to_string())
}
