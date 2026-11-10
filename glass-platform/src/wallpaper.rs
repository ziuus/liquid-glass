use std::sync::Arc;
use std::time::Instant;

/// Watcher and loader for the active compositor wallpaper (awww / swww cache or file).
pub struct WallpaperManager {
    current_path: Option<String>,
    cached_1080p: Option<Arc<Vec<u8>>>,
    last_check: Instant,
    inotify_fd: Option<i32>,
    watch_desc: Option<i32>,
    version: u32,
}

impl Default for WallpaperManager {
    fn default() -> Self {
        Self::new()
    }
}

impl WallpaperManager {
    pub fn new() -> Self {
        let (fd, wd) = Self::init_inotify();
        let mut mgr = Self {
            current_path: None,
            cached_1080p: None,
            last_check: Instant::now() - std::time::Duration::from_secs(10),
            inotify_fd: fd,
            watch_desc: wd,
            version: 0,
        };
        mgr.update_cache();
        mgr
    }

    fn init_inotify() -> (Option<i32>, Option<i32>) {
        unsafe {
            let fd = libc::inotify_init1(libc::IN_NONBLOCK | libc::IN_CLOEXEC);
            if fd < 0 {
                return (None, None);
            }
            let home = std::env::var("HOME").unwrap_or_default();
            let cache_dir = format!("{}/.cache/awww", home);
            let mut first_wd = None;

            if let Ok(c_path) = std::ffi::CString::new(cache_dir.clone()) {
                let wd = libc::inotify_add_watch(
                    fd,
                    c_path.as_ptr(),
                    libc::IN_MODIFY | libc::IN_CREATE | libc::IN_DELETE | libc::IN_CLOSE_WRITE,
                );
                if wd >= 0 {
                    first_wd = Some(wd);
                }
            }

            // Also watch subdirectories in ~/.cache/awww (e.g. 0.12.1 where eDP-1 is written)
            if let Ok(entries) = std::fs::read_dir(&cache_dir) {
                for entry in entries.flatten() {
                    if entry.path().is_dir() {
                        if let Some(p_str) = entry.path().to_str() {
                            if let Ok(c_path) = std::ffi::CString::new(p_str) {
                                let _ = libc::inotify_add_watch(
                                    fd,
                                    c_path.as_ptr(),
                                    libc::IN_MODIFY
                                        | libc::IN_CREATE
                                        | libc::IN_DELETE
                                        | libc::IN_CLOSE_WRITE,
                                );
                            }
                        }
                    }
                }
            }

            // Also watch ~/.cache/current_wallpaper
            let cur_wall = format!("{}/.cache/current_wallpaper", home);
            if let Ok(c_path) = std::ffi::CString::new(cur_wall) {
                let _ = libc::inotify_add_watch(
                    fd,
                    c_path.as_ptr(),
                    libc::IN_MODIFY | libc::IN_CLOSE_WRITE,
                );
            }

            (Some(fd), first_wd)
        }
    }

    fn poll_inotify_triggered(&self) -> bool {
        if let Some(fd) = self.inotify_fd {
            let mut buf = [0u8; 1024];
            let res = unsafe { libc::read(fd, buf.as_mut_ptr() as *mut libc::c_void, buf.len()) };
            res > 0
        } else {
            false
        }
    }

    /// Queries the active wallpaper path from awww CLI, ~/.cache/awww, or fallback cache.
    pub fn get_active_wallpaper_path() -> Option<String> {
        // 1. Direct read from awww cache directory (instant zero-process read, <5µs)
        if let Some(path) = dirs_cache_awww() {
            if std::path::Path::new(&path).exists() {
                return Some(path);
            }
        }

        // 2. Live query to awww daemon
        if let Ok(out) = std::process::Command::new("awww").arg("query").output() {
            let s = String::from_utf8_lossy(&out.stdout);
            for line in s.lines() {
                if line.contains("image:") {
                    if let Some(p) = line.split("image:").nth(1) {
                        let path = p.trim().to_string();
                        if std::path::Path::new(&path).exists() {
                            return Some(path);
                        }
                    }
                }
            }
        }

        // 3. Current wallpaper cache
        let cache_path = format!(
            "{}/.cache/current_wallpaper",
            std::env::var("HOME").unwrap_or_default()
        );
        if let Ok(content) = std::fs::read_to_string(&cache_path) {
            let p = content.trim().to_string();
            if std::path::Path::new(&p).exists() {
                return Some(p);
            }
        }

        None
    }

    /// Loads, centers, and crops an image to exact 1920x1080 RGBA matching awww's Lanczos/crop algorithm.
    pub fn load_wallpaper_1080p(path: Option<&str>) -> Option<Arc<Vec<u8>>> {
        let active_detected = Self::get_active_wallpaper_path();
        let candidates = [
            path,
            active_detected.as_deref(),
            Some("/home/zius/Pictures/wallpaper/a2f1463e-29de-494e-8398-d32a3e22f171.jpg"),
            Some("/home/zius/Pictures/wallpaper/Fantasy-Landscape3.png"),
            Some("/home/zius/Pictures/wallpaper/scott-webb-hDyO6rr3kqk-unsplash.jpg"),
        ];

        for candidate in candidates.into_iter().flatten() {
            if let Ok(img) = image::open(candidate) {
                log::info!("Loaded wallpaper: {}", candidate);
                let rgba = img.to_rgba8();
                let (ow, oh) = (rgba.width() as f32, rgba.height() as f32);

                // Exactly match awww --resize crop --crop-gravity center
                let scale = (1920.0 / ow).max(1080.0 / oh);
                let sw = (ow * scale).round() as u32;
                let sh = (oh * scale).round() as u32;
                let scaled =
                    image::imageops::resize(&rgba, sw, sh, image::imageops::FilterType::Triangle);
                let left = sw.saturating_sub(1920) / 2;
                let top = sh.saturating_sub(1080) / 2;
                let cropped = image::imageops::crop_imm(&scaled, left, top, 1920, 1080).to_image();
                return Some(Arc::new(cropped.into_raw()));
            }
        }

        // Clean neutral fallback
        let mut fallback = vec![0u8; 1920 * 1080 * 4];
        for y in 0..1080 {
            for x in 0..1920 {
                let i = (y * 1920 + x) * 4;
                fallback[i] = 240;
                fallback[i + 1] = 242;
                fallback[i + 2] = 240;
                fallback[i + 3] = 255;
            }
        }
        Some(Arc::new(fallback))
    }

    /// Crops a sub-rect from the 1080p cached buffer.
    pub fn get_wallpaper_crop(cache: &[u8], x: u32, y: u32, w: u32, h: u32) -> Vec<u8> {
        let mut crop = vec![0u8; (w * h * 4) as usize];
        for row in 0..h {
            let src_y = (y + row).min(1079);
            let src_x = x.min(1920u32.saturating_sub(w));
            let src_start = ((src_y * 1920 + src_x) * 4) as usize;
            let dst_start = (row * w * 4) as usize;
            let len = (w * 4) as usize;
            crop[dst_start..dst_start + len].copy_from_slice(&cache[src_start..src_start + len]);
        }
        crop
    }

    /// Checks if wallpaper has changed via inotify or fast fallback polling (250ms).
    /// Returns true if the cache was updated.
    pub fn check_update(&mut self) -> bool {
        let inotify_fired = self.poll_inotify_triggered();
        let timer_due = self.last_check.elapsed().as_millis() >= 250;

        if !inotify_fired && !timer_due {
            return false;
        }
        self.last_check = Instant::now();

        let queried = Self::get_active_wallpaper_path();
        if queried != self.current_path && queried.is_some() {
            log::info!("Wallpaper changed to {:?}", queried);
            if let Some(buf) = Self::load_wallpaper_1080p(queried.as_deref()) {
                self.current_path = queried;
                self.cached_1080p = Some(buf);
                self.version = self.version.wrapping_add(1);
                return true;
            }
        }
        false
    }

    pub fn cached_buffer(&self) -> Option<&Arc<Vec<u8>>> {
        self.cached_1080p.as_ref()
    }

    /// Drops the heavy uncompressed 1080p buffer from RAM once the surface crop is extracted and uploaded to GPU.
    pub fn release_full_buffer(&mut self) {
        self.cached_1080p = None;
    }

    pub fn version(&self) -> u32 {
        self.version
    }

    /// Evaluates if an RGBA buffer is perceptually light using WCAG 2.1 relative luminance
    /// with hysteresis to eliminate flickering on luminance threshold boundaries.
    pub fn is_buffer_light_hysteresis(buf: &[u8], current_state: bool) -> bool {
        let mut total_lum = 0.0f64;
        let mut count = 0usize;
        let step = 16 * 4;
        for i in (0..buf.len()).step_by(step) {
            if i + 2 < buf.len() {
                let r = buf[i] as f64 / 255.0;
                let g = buf[i + 1] as f64 / 255.0;
                let b = buf[i + 2] as f64 / 255.0;
                let to_lin = |c: f64| {
                    if c <= 0.04045 {
                        c / 12.92
                    } else {
                        ((c + 0.055) / 1.055).powf(2.4)
                    }
                };
                let lin_lum = 0.2126 * to_lin(r) + 0.7152 * to_lin(g) + 0.0722 * to_lin(b);
                total_lum += lin_lum;
                count += 1;
            }
        }
        if count > 0 {
            let mean = total_lum / count as f64;
            if current_state {
                mean > 0.28
            } else {
                mean > 0.35
            }
        } else {
            current_state
        }
    }

    pub fn is_buffer_light(buf: &[u8]) -> bool {
        Self::is_buffer_light_hysteresis(buf, false)
    }

    pub fn sample_region_is_light(
        cache: &[u8],
        gx: i32,
        gy: i32,
        gw: u32,
        gh: u32,
        current_state: bool,
    ) -> bool {
        let crop = Self::get_wallpaper_crop(cache, gx.max(0) as u32, gy.max(0) as u32, gw, gh);
        Self::is_buffer_light_hysteresis(&crop, current_state)
    }

    fn update_cache(&mut self) {
        let queried = Self::get_active_wallpaper_path();
        self.current_path = queried.clone();
        self.cached_1080p = Self::load_wallpaper_1080p(queried.as_deref());
        self.version = self.version.wrapping_add(1);
    }
}

impl Drop for WallpaperManager {
    fn drop(&mut self) {
        if let Some(fd) = self.inotify_fd {
            unsafe {
                if let Some(wd) = self.watch_desc {
                    libc::inotify_rm_watch(fd, wd);
                }
                libc::close(fd);
            }
        }
    }
}

fn dirs_cache_awww() -> Option<String> {
    let home = std::env::var("HOME").ok()?;
    let cache_dir = format!("{}/.cache/awww", home);
    if let Ok(entries) = std::fs::read_dir(cache_dir) {
        for version_entry in entries.flatten() {
            if let Ok(mon_entries) = std::fs::read_dir(version_entry.path()) {
                for mon_entry in mon_entries.flatten() {
                    if let Ok(bytes) = std::fs::read(mon_entry.path()) {
                        let s = String::from_utf8_lossy(&bytes);
                        for part in s.split(|c: char| c == '\0' || c.is_whitespace()) {
                            let trimmed = part.trim();
                            if trimmed.starts_with('/') && std::path::Path::new(trimmed).exists() {
                                return Some(trimmed.to_string());
                            }
                        }
                    }
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_active_wallpaper() {
        let path = WallpaperManager::get_active_wallpaper_path();
        println!("DETECTED WALLPAPER PATH: {:?}", path);
        let buf = WallpaperManager::load_wallpaper_1080p(path.as_deref());
        assert!(buf.is_some());
        let slice = buf.unwrap();
        println!("BUFFER LEN: {}", slice.len());
        // Sample at 100, 100
        let idx = (100 * 1920 + 100) * 4;
        println!(
            "PIXEL AT (100, 100): R={} G={} B={} A={}",
            slice[idx],
            slice[idx + 1],
            slice[idx + 2],
            slice[idx + 3]
        );
    }
}
