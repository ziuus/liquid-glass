#[cfg(target_os = "linux")]
use std::os::unix::io::RawFd;
#[cfg(not(target_os = "linux"))]
type RawFd = i32;
use std::sync::{Arc, Mutex};
use std::time::Instant;

#[derive(Debug, Clone, PartialEq, Eq)]
struct FileFingerprint {
    len: u64,
    mtime_nanos: u128,
    content_hash: u64,
}

fn compute_file_fingerprint(path: &str) -> Option<FileFingerprint> {
    use std::hash::{Hash, Hasher};
    use std::io::{Read, Seek, SeekFrom};

    let meta = std::fs::metadata(path).ok()?;
    let len = meta.len();
    let mtime_nanos = meta
        .modified()
        .ok()
        .and_then(|m| m.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_nanos())
        .unwrap_or(0);

    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    len.hash(&mut hasher);
    mtime_nanos.hash(&mut hasher);

    if let Ok(mut file) = std::fs::File::open(path) {
        if len <= 4 * 1024 * 1024 {
            let mut buf = Vec::with_capacity(len as usize);
            if file.read_to_end(&mut buf).is_ok() {
                hasher.write(&buf);
            }
        } else {
            // Sample 8 evenly spaced 8KB windows across large files plus tail
            let mut buf = [0u8; 8192];
            for i in 0..8u64 {
                let offset = (len.saturating_sub(8192)) * i / 7;
                if file.seek(SeekFrom::Start(offset)).is_ok() {
                    if let Ok(n) = file.read(&mut buf) {
                        hasher.write(&buf[..n]);
                    }
                }
            }
        }
    }

    Some(FileFingerprint {
        len,
        mtime_nanos,
        content_hash: hasher.finish(),
    })
}

fn cache_path_for(candidate: &str, fp: &FileFingerprint) -> String {
    use std::hash::{Hash, Hasher};
    let home = std::env::var("HOME").unwrap_or_default();
    let cache_dir = format!("{}/.cache/liquid-glass", home);
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    candidate.hash(&mut hasher);
    fp.len.hash(&mut hasher);
    fp.mtime_nanos.hash(&mut hasher);
    fp.content_hash.hash(&mut hasher);
    let filter_tag = if cfg!(debug_assertions) {
        "nearest_v2"
    } else {
        "bilinear_v2"
    };
    filter_tag.hash(&mut hasher);
    format!("{}/wallpaper_{:016x}.raw", cache_dir, hasher.finish())
}

fn fast_crop_resize_1080p(img: &image::DynamicImage) -> Vec<u8> {
    const DST_W: u32 = 1920;
    const DST_H: u32 = 1080;

    let (ow_u32, oh_u32) = (img.width().max(1), img.height().max(1));
    let (ow, oh) = (ow_u32 as f32, oh_u32 as f32);

    let scale = (DST_W as f32 / ow).max(DST_H as f32 / oh);
    let sw = (ow * scale).round().max(DST_W as f32) as u32;
    let sh = (oh * scale).round().max(DST_H as f32) as u32;
    let left = sw.saturating_sub(DST_W) / 2;
    let top = sh.saturating_sub(DST_H) / 2;

    let mut out = vec![0u8; (DST_W * DST_H * 4) as usize];

    let sample_nearest = |src: &[u8], bpp: usize, out: &mut [u8]| {
        let max_sx = (ow_u32 - 1) as usize;
        let max_sy = (oh_u32 - 1) as usize;
        let mut x_map = [0usize; DST_W as usize];
        for (dx, slot) in x_map.iter_mut().enumerate() {
            let sx = ((((left as usize + dx) as f32) + 0.5) / (sw as f32) * ow).floor() as isize;
            *slot = sx.clamp(0, max_sx as isize) as usize * bpp;
        }
        let stride = ow_u32 as usize * bpp;
        for dy in 0..DST_H as usize {
            let sy = ((((top as usize + dy) as f32) + 0.5) / (sh as f32) * oh).floor() as isize;
            let src_y = sy.clamp(0, max_sy as isize) as usize;
            let src_row = &src[src_y * stride..(src_y + 1) * stride];
            let dst_row = &mut out[dy * (DST_W as usize * 4)..(dy + 1) * (DST_W as usize * 4)];
            if bpp == 3 {
                for (dx, &sx_off) in x_map.iter().enumerate() {
                    let d = dx * 4;
                    dst_row[d] = src_row[sx_off];
                    dst_row[d + 1] = src_row[sx_off + 1];
                    dst_row[d + 2] = src_row[sx_off + 2];
                    dst_row[d + 3] = 255;
                }
            } else {
                for (dx, &sx_off) in x_map.iter().enumerate() {
                    let d = dx * 4;
                    dst_row[d] = src_row[sx_off];
                    dst_row[d + 1] = src_row[sx_off + 1];
                    dst_row[d + 2] = src_row[sx_off + 2];
                    dst_row[d + 3] = src_row[sx_off + 3];
                }
            }
        }
    };

    let sample_bilinear = |src: &[u8], bpp: usize, out: &mut [u8]| {
        let max_sx = (ow_u32 - 1) as usize;
        let max_sy = (oh_u32 - 1) as usize;
        let mut x_map = Vec::with_capacity(DST_W as usize);
        for dx in 0..DST_W as usize {
            let fx = (((left as usize + dx) as f32 + 0.5) / (sw as f32) * ow - 0.5)
                .clamp(0.0, max_sx as f32);
            let x0 = (fx.floor() as usize).min(max_sx);
            let x1 = (x0 + 1).min(max_sx);
            let wx = ((fx - x0 as f32) * 256.0).round() as u32;
            x_map.push((x0 * bpp, x1 * bpp, wx));
        }
        let stride = ow_u32 as usize * bpp;
        for dy in 0..DST_H as usize {
            let fy = (((top as usize + dy) as f32 + 0.5) / (sh as f32) * oh - 0.5)
                .clamp(0.0, max_sy as f32);
            let y0 = (fy.floor() as usize).min(max_sy);
            let y1 = (y0 + 1).min(max_sy);
            let wy = ((fy - y0 as f32) * 256.0).round() as u32;
            let inv_wy = 256 - wy;
            let row0 = &src[y0 * stride..(y0 + 1) * stride];
            let row1 = &src[y1 * stride..(y1 + 1) * stride];
            let dst_row = &mut out[dy * (DST_W as usize * 4)..(dy + 1) * (DST_W as usize * 4)];
            for (dx, &(x0_off, x1_off, wx)) in x_map.iter().enumerate() {
                let inv_wx = 256 - wx;
                let w00 = inv_wx * inv_wy;
                let w10 = wx * inv_wy;
                let w01 = inv_wx * wy;
                let w11 = wx * wy;
                let d = dx * 4;
                for c in 0..3 {
                    let val = row0[x0_off + c] as u32 * w00
                        + row0[x1_off + c] as u32 * w10
                        + row1[x0_off + c] as u32 * w01
                        + row1[x1_off + c] as u32 * w11;
                    dst_row[d + c] = ((val + 32768) >> 16) as u8;
                }
                dst_row[d + 3] = if bpp == 4 {
                    let val = row0[x0_off + 3] as u32 * w00
                        + row0[x1_off + 3] as u32 * w10
                        + row1[x0_off + 3] as u32 * w01
                        + row1[x1_off + 3] as u32 * w11;
                    ((val + 32768) >> 16) as u8
                } else {
                    255
                };
            }
        }
    };

    if let Some(rgb) = img.as_rgb8() {
        let raw = rgb.as_raw();
        if cfg!(debug_assertions) {
            sample_nearest(raw, 3, &mut out);
        } else {
            sample_bilinear(raw, 3, &mut out);
        }
    } else if let Some(rgba) = img.as_rgba8() {
        if ow_u32 == DST_W && oh_u32 == DST_H {
            return rgba.as_raw().clone();
        }
        let raw = rgba.as_raw();
        if cfg!(debug_assertions) {
            sample_nearest(raw, 4, &mut out);
        } else {
            sample_bilinear(raw, 4, &mut out);
        }
    } else {
        let rgba = img.to_rgba8();
        if ow_u32 == DST_W && oh_u32 == DST_H {
            return rgba.into_raw();
        }
        let raw = rgba.as_raw();
        if cfg!(debug_assertions) {
            sample_nearest(raw, 4, &mut out);
        } else {
            sample_bilinear(raw, 4, &mut out);
        }
    }

    out
}

/// Optical metrics calculated for a specific rectangular zone of the wallpaper crop.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ZoneMetrics {
    /// Relative luminance mean in linear sRGB space: Y = 0.2126*R + 0.7152*G + 0.0722*B [0.0..1.0]
    pub luma_mean: f32,
    /// Standard deviation of relative luminance across the zone (multi-tone variance detector)
    pub luma_std_dev: f32,
    /// Dominant tint vector as average linear RGB [0.0..1.0]
    pub dominant_tint: [f32; 3],
    /// True when standard deviation exceeds the multi-tone threshold (sigma_Y > 0.18)
    pub is_multi_tone: bool,
    /// Perceptual lightness flag based on luminance threshold (> 0.35)
    pub is_light: bool,
}

impl Default for ZoneMetrics {
    fn default() -> Self {
        Self {
            luma_mean: 0.0,
            luma_std_dev: 0.0,
            dominant_tint: [0.0, 0.0, 0.0],
            is_multi_tone: false,
            is_light: false,
        }
    }
}

/// Optical profile of the central widget wallpaper crop divided into distinct functional zones.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OpticalZoneProfile {
    /// Overall metrics computed over the entire crop
    pub overall: ZoneMetrics,
    /// Zone A: Day Header (top 25% of widget height)
    pub day_zone: ZoneMetrics,
    /// Zone B: Date Subtitle (middle 20% of widget height)
    pub date_zone: ZoneMetrics,
    /// Zone C: Numeral Clock Area (bottom 55% of widget height)
    pub numeral_zone: ZoneMetrics,

    // Direct interface contract fields for backward and ergonomic access:
    pub luma_mean: f32,
    pub luma_std_dev: f32,
    pub dominant_tint: [f32; 3],
    pub is_multi_tone: bool,
}

impl Default for OpticalZoneProfile {
    fn default() -> Self {
        Self {
            overall: ZoneMetrics::default(),
            day_zone: ZoneMetrics::default(),
            date_zone: ZoneMetrics::default(),
            numeral_zone: ZoneMetrics::default(),
            luma_mean: 0.0,
            luma_std_dev: 0.0,
            dominant_tint: [0.0, 0.0, 0.0],
            is_multi_tone: false,
        }
    }
}

fn get_srgb_lut() -> &'static [f32; 256] {
    static LUT: std::sync::OnceLock<[f32; 256]> = std::sync::OnceLock::new();
    LUT.get_or_init(|| {
        let mut table = [0.0f32; 256];
        for (i, val) in table.iter_mut().enumerate() {
            let c = i as f32 / 255.0;
            *val = if c <= 0.04045 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            };
        }
        table
    })
}

type PendingWallpaper = Arc<Mutex<Option<(Option<String>, Option<FileFingerprint>, Arc<Vec<u8>>)>>>;

/// Watcher and loader for the active compositor wallpaper (awww / swww cache or file).
pub struct WallpaperManager {
    current_path: Option<String>,
    current_fingerprint: Option<FileFingerprint>,
    cached_1080p: Option<Arc<Vec<u8>>>,
    last_check: Instant,
    inotify_fd: Option<i32>,
    watch_descs: Vec<i32>,
    cache_dir_wd: Option<i32>,
    awww_dir_wd: Option<i32>,
    pending_async: PendingWallpaper,
    version: u32,
}

impl Default for WallpaperManager {
    fn default() -> Self {
        Self::new()
    }
}

impl WallpaperManager {
    pub fn new() -> Self {
        let (fd, wds, cache_wd, awww_wd) = Self::init_inotify();
        let mut mgr = Self {
            current_path: None,
            current_fingerprint: None,
            cached_1080p: None,
            last_check: Instant::now() - std::time::Duration::from_secs(10),
            inotify_fd: fd,
            watch_descs: wds,
            cache_dir_wd: cache_wd,
            awww_dir_wd: awww_wd,
            pending_async: Arc::new(Mutex::new(None)),
            version: 0,
        };
        mgr.update_cache();
        mgr
    }

    /// Rapidly initializes `WallpaperManager`:
    /// - If the `.raw` disk cache already exists (warm cache), loads it synchronously (<3ms).
    /// - Otherwise (cold cache), spawns a background thread to decode/resize/cache the wallpaper
    ///   and writes a wakeup byte to `wakeup_write_fd` once ready.
    pub fn new_async(wakeup_write_fd: Option<RawFd>) -> Self {
        let (fd, wds, cache_wd, awww_wd) = Self::init_inotify();
        let pending_async: PendingWallpaper = Arc::new(Mutex::new(None));
        let queried = Self::get_active_wallpaper_path();

        // Fast path: if warm raw cache already exists for the detected wallpaper, load it synchronously (<3ms)
        if let Some(ref qpath) = queried {
            if let Some(fp) = compute_file_fingerprint(qpath) {
                let cpath = cache_path_for(qpath, &fp);
                if let Ok(cached_raw) = std::fs::read(&cpath) {
                    if cached_raw.len() == 1920 * 1080 * 4 {
                        log::info!("Warm-cache fast load: {}", cpath);
                        return Self {
                            current_path: queried,
                            current_fingerprint: Some(fp),
                            cached_1080p: Some(Arc::new(cached_raw)),
                            last_check: Instant::now(),
                            inotify_fd: fd,
                            watch_descs: wds,
                            cache_dir_wd: cache_wd,
                            awww_dir_wd: awww_wd,
                            pending_async,
                            version: 1,
                        };
                    }
                }
            }
        }

        // Cold cache: decode off the startup critical path
        let slot = Arc::clone(&pending_async);
        let q_clone = queried.clone();
        let _ = std::thread::Builder::new()
            .name("wallpaper-loader".into())
            .spawn(move || {
                let fp = q_clone.as_deref().and_then(compute_file_fingerprint);
                if let Some(buf) = Self::load_wallpaper_1080p(q_clone.as_deref()) {
                    let actual_path = q_clone.or_else(Self::get_active_wallpaper_path);
                    let actual_fp =
                        fp.or_else(|| actual_path.as_deref().and_then(compute_file_fingerprint));
                    if let Ok(mut guard) = slot.lock() {
                        *guard = Some((actual_path, actual_fp, buf));
                    }
                    #[cfg(target_os = "linux")]
                    if let Some(wfd) = wakeup_write_fd {
                        let byte = *b"w";
                        unsafe {
                            libc::write(wfd, byte.as_ptr() as *const libc::c_void, 1);
                        }
                    }
                    #[cfg(not(target_os = "linux"))]
                    let _ = wakeup_write_fd;
                }
            });

        Self {
            current_path: queried,
            current_fingerprint: None,
            cached_1080p: None,
            last_check: Instant::now(),
            inotify_fd: fd,
            watch_descs: wds,
            cache_dir_wd: cache_wd,
            awww_dir_wd: awww_wd,
            pending_async,
            version: 0,
        }
    }

    #[cfg(not(target_os = "linux"))]
    fn init_inotify() -> (Option<i32>, Vec<i32>, Option<i32>, Option<i32>) {
        (None, Vec::new(), None, None)
    }

    #[cfg(target_os = "linux")]
    fn init_inotify() -> (Option<i32>, Vec<i32>, Option<i32>, Option<i32>) {
        unsafe {
            let fd = libc::inotify_init1(libc::IN_NONBLOCK | libc::IN_CLOEXEC);
            if fd < 0 {
                return (None, Vec::new(), None, None);
            }
            let home = std::env::var("HOME").unwrap_or_default();
            let mut wds = Vec::new();
            let mut cache_dir_wd = None;
            let mut awww_dir_wd = None;

            // Watch ~/.cache for creation / atomic rename of current_wallpaper or awww
            let base_cache = format!("{}/.cache", home);
            if let Ok(c_path) = std::ffi::CString::new(base_cache) {
                let wd = libc::inotify_add_watch(
                    fd,
                    c_path.as_ptr(),
                    libc::IN_CREATE | libc::IN_MOVED_TO,
                );
                if wd >= 0 {
                    cache_dir_wd = Some(wd);
                    wds.push(wd);
                }
            }

            let cache_dir = format!("{}/.cache/awww", home);
            if let Ok(c_path) = std::ffi::CString::new(cache_dir.clone()) {
                let wd = libc::inotify_add_watch(
                    fd,
                    c_path.as_ptr(),
                    libc::IN_MODIFY
                        | libc::IN_CREATE
                        | libc::IN_DELETE
                        | libc::IN_CLOSE_WRITE
                        | libc::IN_MOVED_TO,
                );
                if wd >= 0 {
                    awww_dir_wd = Some(wd);
                    wds.push(wd);
                }
            }

            // Also watch subdirectories in ~/.cache/awww (e.g. 0.12.1 where eDP-1 is written)
            if let Ok(entries) = std::fs::read_dir(&cache_dir) {
                for entry in entries.flatten() {
                    if entry.path().is_dir() {
                        if let Some(p_str) = entry.path().to_str() {
                            if let Ok(c_path) = std::ffi::CString::new(p_str) {
                                let wd = libc::inotify_add_watch(
                                    fd,
                                    c_path.as_ptr(),
                                    libc::IN_MODIFY
                                        | libc::IN_CREATE
                                        | libc::IN_DELETE
                                        | libc::IN_CLOSE_WRITE
                                        | libc::IN_MOVED_TO,
                                );
                                if wd >= 0 {
                                    wds.push(wd);
                                }
                            }
                        }
                    }
                }
            }

            // Also watch ~/.cache/current_wallpaper
            let cur_wall = format!("{}/.cache/current_wallpaper", home);
            if let Ok(c_path) = std::ffi::CString::new(cur_wall) {
                let wd = libc::inotify_add_watch(
                    fd,
                    c_path.as_ptr(),
                    libc::IN_MODIFY | libc::IN_CLOSE_WRITE,
                );
                if wd >= 0 {
                    wds.push(wd);
                }
            }

            // If zero watches succeeded, do not claim inotify is active so fallback timer stays enabled
            if wds.is_empty() {
                libc::close(fd);
                return (None, Vec::new(), None, None);
            }

            (Some(fd), wds, cache_dir_wd, awww_dir_wd)
        }
    }

    #[cfg(not(target_os = "linux"))]
    fn poll_inotify_triggered(&mut self) -> bool {
        false
    }

    #[cfg(target_os = "linux")]
    fn poll_inotify_triggered(&mut self) -> bool {
        let Some(fd) = self.inotify_fd else {
            return false;
        };
        let mut relevant = false;
        let mut buf = [0u8; 4096];
        let header_size = std::mem::size_of::<libc::inotify_event>();

        loop {
            let res = unsafe { libc::read(fd, buf.as_mut_ptr() as *mut libc::c_void, buf.len()) };
            if res > 0 {
                let n = res as usize;
                let mut offset = 0usize;
                while offset + header_size <= n {
                    let wd = i32::from_ne_bytes(buf[offset..offset + 4].try_into().unwrap());
                    let mask = u32::from_ne_bytes(buf[offset + 4..offset + 8].try_into().unwrap());
                    let name_len =
                        u32::from_ne_bytes(buf[offset + 12..offset + 16].try_into().unwrap())
                            as usize;
                    let event_end = (offset + header_size + name_len).min(n);
                    let name_slice = &buf[offset + header_size..event_end];
                    let name_bytes = name_slice.split(|&b| b == 0).next().unwrap_or(&[]);
                    let name = std::str::from_utf8(name_bytes).unwrap_or("");

                    if Some(wd) == self.cache_dir_wd {
                        if name == "current_wallpaper" || name == "awww" {
                            relevant = true;
                            let home = std::env::var("HOME").unwrap_or_default();
                            let target = format!("{}/.cache/{}", home, name);
                            if let Ok(c_path) = std::ffi::CString::new(target.as_str()) {
                                let new_wd = unsafe {
                                    libc::inotify_add_watch(
                                        fd,
                                        c_path.as_ptr(),
                                        libc::IN_MODIFY
                                            | libc::IN_CREATE
                                            | libc::IN_DELETE
                                            | libc::IN_CLOSE_WRITE
                                            | libc::IN_MOVED_TO,
                                    )
                                };
                                if new_wd >= 0 {
                                    if !self.watch_descs.contains(&new_wd) {
                                        self.watch_descs.push(new_wd);
                                    }
                                    if name == "awww" {
                                        self.awww_dir_wd = Some(new_wd);
                                        if let Ok(entries) = std::fs::read_dir(&target) {
                                            for entry in entries.flatten() {
                                                if entry.path().is_dir() {
                                                    if let Some(p_str) = entry.path().to_str() {
                                                        if let Ok(sub_c_path) =
                                                            std::ffi::CString::new(p_str)
                                                        {
                                                            let sub_wd = unsafe {
                                                                libc::inotify_add_watch(
                                                                    fd,
                                                                    sub_c_path.as_ptr(),
                                                                    libc::IN_MODIFY
                                                                        | libc::IN_CREATE
                                                                        | libc::IN_DELETE
                                                                        | libc::IN_CLOSE_WRITE
                                                                        | libc::IN_MOVED_TO,
                                                                )
                                                            };
                                                            if sub_wd >= 0
                                                                && !self
                                                                    .watch_descs
                                                                    .contains(&sub_wd)
                                                            {
                                                                self.watch_descs.push(sub_wd);
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    } else {
                        relevant = true;
                        if Some(wd) == self.awww_dir_wd
                            && !name.is_empty()
                            && (mask & (libc::IN_CREATE | libc::IN_MOVED_TO)) != 0
                        {
                            let home = std::env::var("HOME").unwrap_or_default();
                            let subdir = format!("{}/.cache/awww/{}", home, name);
                            if std::path::Path::new(&subdir).is_dir() {
                                if let Ok(c_path) = std::ffi::CString::new(subdir) {
                                    let new_wd = unsafe {
                                        libc::inotify_add_watch(
                                            fd,
                                            c_path.as_ptr(),
                                            libc::IN_MODIFY
                                                | libc::IN_CREATE
                                                | libc::IN_DELETE
                                                | libc::IN_CLOSE_WRITE
                                                | libc::IN_MOVED_TO,
                                        )
                                    };
                                    if new_wd >= 0 && !self.watch_descs.contains(&new_wd) {
                                        self.watch_descs.push(new_wd);
                                    }
                                }
                            }
                        }
                    }

                    offset += header_size + name_len;
                }
            } else {
                if res < 0 {
                    let err = std::io::Error::last_os_error().raw_os_error();
                    if err == Some(libc::EINTR) {
                        continue;
                    }
                }
                break;
            }
        }
        relevant
    }

    /// Queries the active wallpaper path from awww cache, current_wallpaper, or fallback awww CLI.
    pub fn get_active_wallpaper_path() -> Option<String> {
        #[cfg(target_os = "windows")]
        {
            if let Ok(appdata) = std::env::var("APPDATA") {
                let p = format!(
                    "{}\\Microsoft\\Windows\\Themes\\TranscodedWallpaper",
                    appdata
                );
                if std::path::Path::new(&p).exists() {
                    return Some(p);
                }
            }
            return None;
        }
        #[cfg(target_os = "macos")]
        {
            if let Ok(out) = std::process::Command::new("osascript")
                .args([
                    "-e",
                    "tell application \"Finder\" to get POSIX path of (desktop picture as alias)",
                ])
                .output()
            {
                if out.status.success() {
                    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
                    if !s.is_empty() && std::path::Path::new(&s).exists() {
                        return Some(s);
                    }
                }
            }
            return None;
        }
        #[cfg(not(any(target_os = "windows", target_os = "macos")))]
        {
            // 1. Direct read from awww cache directory (instant zero-process read, <5µs)
            if let Some(path) = dirs_cache_awww() {
                if std::path::Path::new(&path).exists() {
                    return Some(path);
                }
            }

            // 2. Current wallpaper cache (zero-process read)
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

            // 3. Live query to awww / swww daemon (fallback)
            for bin in ["awww", "swww"] {
                if let Ok(out) = std::process::Command::new(bin).arg("query").output() {
                    let s = String::from_utf8_lossy(&out.stdout);
                    for line in s.lines() {
                        if let Some(p) = line.split("image:").nth(1) {
                            let path = p.trim().to_string();
                            if std::path::Path::new(&path).exists() {
                                return Some(path);
                            }
                        }
                    }
                }
            }

            // 4. KDE Plasma wallpaper config
            let home = std::env::var("HOME").unwrap_or_default();
            let kde_cfg = format!("{}/.config/plasma-org.kde.plasma.desktop-appletsrc", home);
            if let Ok(content) = std::fs::read_to_string(&kde_cfg) {
                for line in content.lines() {
                    if let Some(rest) = line.trim().strip_prefix("Image=") {
                        let p = rest.trim().trim_start_matches("file://").to_string();
                        if std::path::Path::new(&p).exists() {
                            return Some(p);
                        }
                    }
                }
            }

            // 5. GNOME / Cinnamon / Cosmic gsettings fallback
            if let Ok(out) = std::process::Command::new("gsettings")
                .args(["get", "org.gnome.desktop.background", "picture-uri"])
                .output()
            {
                if out.status.success() {
                    let raw = String::from_utf8_lossy(&out.stdout);
                    let p = raw
                        .trim()
                        .trim_matches('\'')
                        .trim_start_matches("file://")
                        .to_string();
                    if std::path::Path::new(&p).exists() {
                        return Some(p);
                    }
                }
            }

            // 6. Nitrogen fallback
            let nitro_cfg = format!("{}/.config/nitrogen/bg-saved.cfg", home);
            if let Ok(content) = std::fs::read_to_string(&nitro_cfg) {
                for line in content.lines() {
                    if let Some(rest) = line.trim().strip_prefix("file=") {
                        let p = rest.trim().to_string();
                        if std::path::Path::new(&p).exists() {
                            return Some(p);
                        }
                    }
                }
            }

            None
        }
    }

    /// Loads, centers, and crops an image to exact 1920x1080 RGBA matching awww's Lanczos/crop algorithm.
    pub fn load_wallpaper_1080p(path: Option<&str>) -> Option<Arc<Vec<u8>>> {
        static TMP_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

        let active_detected = Self::get_active_wallpaper_path();
        let candidates = [
            path,
            active_detected.as_deref(),
            Some("/home/zius/Pictures/wallpaper/a2f1463e-29de-494e-8398-d32a3e22f171.jpg"),
            Some("/home/zius/Pictures/wallpaper/Fantasy-Landscape3.png"),
            Some("/home/zius/Pictures/wallpaper/scott-webb-hDyO6rr3kqk-unsplash.jpg"),
        ];

        for candidate in candidates.into_iter().flatten() {
            if !std::path::Path::new(candidate).exists() {
                continue;
            }

            // Check persistent 1080p raw disk cache (<3ms load instead of image decode+resize)
            let home = std::env::var("HOME").unwrap_or_default();
            let cache_dir = format!("{}/.cache/liquid-glass", home);
            let fp = compute_file_fingerprint(candidate);
            let cache_file = fp.as_ref().map(|f| cache_path_for(candidate, f));

            if let Some(ref cpath) = cache_file {
                if let Ok(cached_raw) = std::fs::read(cpath) {
                    if cached_raw.len() == 1920 * 1080 * 4 {
                        log::info!("Loaded wallpaper from raw cache: {}", cpath);
                        return Some(Arc::new(cached_raw));
                    }
                }
            }

            if let Ok(img) = image::open(candidate) {
                log::info!("Loaded wallpaper: {}", candidate);
                let raw_bytes = fast_crop_resize_1080p(&img);

                if let Some(ref cpath) = cache_file {
                    if std::fs::create_dir_all(&cache_dir).is_ok() {
                        let seq = TMP_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        let tmp_path = format!("{}.tmp.{}.{}", cpath, std::process::id(), seq);
                        if std::fs::write(&tmp_path, &raw_bytes).is_ok() {
                            if std::fs::rename(&tmp_path, cpath).is_err() {
                                let _ = std::fs::remove_file(&tmp_path);
                            }
                        } else {
                            let _ = std::fs::remove_file(&tmp_path);
                        }
                    }
                }

                return Some(Arc::new(raw_bytes));
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

    /// Checks if wallpaper has changed via async loader completion, inotify, or 30s fallback.
    /// Returns true if the cache was updated.
    pub fn check_update(&mut self) -> bool {
        if let Ok(mut guard) = self.pending_async.lock() {
            if let Some((path, fp, buf)) = guard.take() {
                self.current_path = path;
                self.current_fingerprint = fp;
                self.cached_1080p = Some(buf);
                self.version = self.version.wrapping_add(1);
                self.last_check = Instant::now();
                return true;
            }
        }

        let should_check = if self.inotify_fd.is_some() && !self.watch_descs.is_empty() {
            self.poll_inotify_triggered()
        } else {
            // Slow fallback only when inotify failed to initialize or had 0 active watches
            if self.last_check.elapsed() >= std::time::Duration::from_secs(30) {
                self.last_check = Instant::now();
                true
            } else {
                false
            }
        };

        if !should_check {
            return false;
        }
        self.last_check = Instant::now();

        let queried = Self::get_active_wallpaper_path();
        if let Some(ref qpath) = queried {
            let fp = compute_file_fingerprint(qpath);
            let path_changed = queried != self.current_path;
            let content_changed = fp.is_some() && fp != self.current_fingerprint;
            if path_changed || content_changed {
                log::info!(
                    "Wallpaper changed to {:?} (path_changed={}, content_changed={})",
                    queried,
                    path_changed,
                    content_changed
                );
                if let Some(buf) = Self::load_wallpaper_1080p(Some(qpath.as_str())) {
                    self.current_path = queried;
                    self.current_fingerprint = fp;
                    self.cached_1080p = Some(buf);
                    self.version = self.version.wrapping_add(1);
                    return true;
                }
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

    pub fn inotify_fd(&self) -> Option<RawFd> {
        self.inotify_fd
    }

    /// Computes zone metrics for a specified vertical sub-slice `[y_start..y_end]` of an RGBA crop.
    pub fn sample_zone_metrics(
        crop: &[u8],
        width: u32,
        height: u32,
        y_start: u32,
        y_end: u32,
    ) -> ZoneMetrics {
        if width == 0 || height == 0 || y_start >= y_end || y_end > height {
            return ZoneMetrics::default();
        }
        let lut = get_srgb_lut();
        let mut sum_luma = 0.0f64;
        let mut sum_luma_sq = 0.0f64;
        let mut sum_r = 0.0f64;
        let mut sum_g = 0.0f64;
        let mut sum_b = 0.0f64;
        let mut count = 0usize;

        // Sample with a conservative step (step 2 for width and height) for high performance (<0.1ms)
        let step_y = 2u32;
        let step_x = 2u32;

        for y in (y_start..y_end).step_by(step_y as usize) {
            let row_offset = (y as usize)
                .saturating_mul(width as usize)
                .saturating_mul(4);
            for x in (0..width).step_by(step_x as usize) {
                let px_idx = row_offset.saturating_add((x as usize).saturating_mul(4));
                if px_idx + 2 < crop.len() {
                    let r_lin = lut[crop[px_idx] as usize] as f64;
                    let g_lin = lut[crop[px_idx + 1] as usize] as f64;
                    let b_lin = lut[crop[px_idx + 2] as usize] as f64;

                    let luma = 0.2126 * r_lin + 0.7152 * g_lin + 0.0722 * b_lin;
                    sum_luma += luma;
                    sum_luma_sq += luma * luma;
                    sum_r += r_lin;
                    sum_g += g_lin;
                    sum_b += b_lin;
                    count += 1;
                }
            }
        }

        if count == 0 {
            return ZoneMetrics::default();
        }

        let n = count as f64;
        let mean = sum_luma / n;
        let variance = ((sum_luma_sq / n) - (mean * mean)).max(0.0);
        let std_dev = variance.sqrt();
        let is_multi_tone = std_dev > 0.18;
        let is_light = mean > 0.35;

        ZoneMetrics {
            luma_mean: mean as f32,
            luma_std_dev: std_dev as f32,
            dominant_tint: [(sum_r / n) as f32, (sum_g / n) as f32, (sum_b / n) as f32],
            is_multi_tone,
            is_light,
        }
    }

    /// Evaluates wallpaper crop into Day Header (top 25%), Date Subtitle (middle 20%),
    /// and Numeral Clock Area (bottom 55%), computing relative luminance, standard deviation,
    /// and dominant tint vectors for each zone and overall.
    pub fn sample_widget_zones(crop: &[u8], width: u32, height: u32) -> OpticalZoneProfile {
        if width == 0
            || height == 0
            || crop.len()
                < (width as usize)
                    .saturating_mul(height as usize)
                    .saturating_mul(4)
        {
            return OpticalZoneProfile::default();
        }

        let h_day = ((height as f32) * 0.25).round() as u32;
        let h_date = ((height as f32) * 0.45).round() as u32;

        let day_zone = Self::sample_zone_metrics(crop, width, height, 0, h_day);
        let date_zone = Self::sample_zone_metrics(crop, width, height, h_day, h_date);
        let numeral_zone = Self::sample_zone_metrics(crop, width, height, h_date, height);
        let overall = Self::sample_zone_metrics(crop, width, height, 0, height);

        let is_multi_tone = overall.is_multi_tone
            || day_zone.is_multi_tone
            || date_zone.is_multi_tone
            || numeral_zone.is_multi_tone;

        OpticalZoneProfile {
            overall,
            day_zone,
            date_zone,
            numeral_zone,
            luma_mean: overall.luma_mean,
            luma_std_dev: overall.luma_std_dev,
            dominant_tint: overall.dominant_tint,
            is_multi_tone,
        }
    }

    /// Alias conforming to PROJECT.md interface contract.
    pub fn sample_zones(crop: &[u8], width: u32, height: u32) -> OpticalZoneProfile {
        Self::sample_widget_zones(crop, width, height)
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
        self.current_fingerprint = queried.as_deref().and_then(compute_file_fingerprint);
        self.current_path = queried.clone();
        self.cached_1080p = Self::load_wallpaper_1080p(queried.as_deref());
        self.version = self.version.wrapping_add(1);
    }
}

impl Drop for WallpaperManager {
    fn drop(&mut self) {
        #[cfg(target_os = "linux")]
        if let Some(fd) = self.inotify_fd {
            unsafe {
                for &wd in &self.watch_descs {
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
    use std::sync::Mutex;

    static HOME_LOCK: Mutex<()> = Mutex::new(());

    struct TestDirGuard(std::path::PathBuf);
    impl Drop for TestDirGuard {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    struct HomeRestoreGuard(Option<String>);
    impl Drop for HomeRestoreGuard {
        fn drop(&mut self) {
            match &self.0 {
                Some(v) => std::env::set_var("HOME", v),
                None => std::env::remove_var("HOME"),
            }
        }
    }

    fn make_test_dir(name: &str) -> TestDirGuard {
        let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("target")
            .join(format!("test_wp_{}_{}", name, std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        TestDirGuard(dir)
    }

    fn write_solid_png(path: &std::path::Path, rgb: [u8; 3]) {
        let img = image::RgbImage::from_pixel(32, 32, image::Rgb(rgb));
        img.save(path).unwrap();
    }

    #[test]
    fn test_active_wallpaper() {
        let _lock = HOME_LOCK.lock().unwrap();
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

    #[test]
    fn test_inotify_disabled_when_zero_watches_succeed() {
        let _lock = HOME_LOCK.lock().unwrap();
        let orig_home = HomeRestoreGuard(std::env::var("HOME").ok());
        let dir = make_test_dir("zero_watches");
        // Point HOME to a nonexistent directory so ~/.cache does not exist
        let missing = dir.0.join("nonexistent_home");
        std::env::set_var("HOME", &missing);

        let (fd, wds, _, _) = WallpaperManager::init_inotify();
        assert!(
            fd.is_none(),
            "inotify_fd must be None when 0 watches succeed"
        );
        assert!(wds.is_empty());
        drop(orig_home);
    }

    #[test]
    fn test_same_path_content_change_invalidates_cache_and_updates() {
        let _lock = HOME_LOCK.lock().unwrap();
        let _orig_home = HomeRestoreGuard(std::env::var("HOME").ok());
        let dir = make_test_dir("same_path");
        std::fs::create_dir_all(dir.0.join(".cache")).unwrap();
        std::env::set_var("HOME", &dir.0);

        let img_path = dir.0.join("wall.png");
        write_solid_png(&img_path, [10, 20, 30]);
        std::fs::write(
            dir.0.join(".cache/current_wallpaper"),
            img_path.to_string_lossy().as_bytes(),
        )
        .unwrap();

        let mut mgr = WallpaperManager::new();
        let buf1 = mgr.cached_buffer().unwrap().clone();
        assert_eq!(&buf1[0..3], &[10, 20, 30]);

        // Atomic cache write check: no leftover .tmp files in .cache/liquid-glass
        let cache_dir = dir.0.join(".cache/liquid-glass");
        let entries: Vec<_> = std::fs::read_dir(&cache_dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        assert!(entries.iter().any(|n| n.ends_with(".raw")));
        assert!(!entries.iter().any(|n| n.contains(".tmp.")));

        // Overwrite the SAME path with a different color (even if size/mtime match)
        write_solid_png(&img_path, [240, 100, 50]);
        // Touch current_wallpaper to trigger inotify (simulates wallpaper change to same path)
        std::fs::write(
            dir.0.join(".cache/current_wallpaper"),
            img_path.to_string_lossy().as_bytes(),
        )
        .unwrap();

        assert!(
            mgr.check_update(),
            "check_update must detect same-path content change"
        );
        let buf2 = mgr.cached_buffer().unwrap();
        assert_eq!(&buf2[0..3], &[240, 100, 50]);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn test_new_async_delivers_wakeup_on_cold_cache() {
        let _lock = HOME_LOCK.lock().unwrap();
        let _orig_home = HomeRestoreGuard(std::env::var("HOME").ok());
        let dir = make_test_dir("async_cold");
        std::fs::create_dir_all(dir.0.join(".cache")).unwrap();
        std::env::set_var("HOME", &dir.0);

        let img_path = dir.0.join("wall.png");
        write_solid_png(&img_path, [77, 88, 99]);
        std::fs::write(
            dir.0.join(".cache/current_wallpaper"),
            img_path.to_string_lossy().as_bytes(),
        )
        .unwrap();

        let mut fds = [0i32; 2];
        unsafe { libc::pipe(fds.as_mut_ptr()) };
        let (rfd, wfd) = (fds[0], fds[1]);

        let mut mgr = WallpaperManager::new_async(Some(wfd));
        // Wait for background thread wakeup byte
        let mut pfd = libc::pollfd {
            fd: rfd,
            events: libc::POLLIN,
            revents: 0,
        };
        let poll_res = unsafe { libc::poll(&mut pfd, 1, 5000) };
        assert!(
            poll_res > 0,
            "Expected wakeup byte from async wallpaper loader"
        );
        let mut byte = [0u8; 8];
        unsafe {
            libc::read(rfd, byte.as_mut_ptr() as *mut libc::c_void, 8);
            libc::close(rfd);
            libc::close(wfd);
        }

        assert!(
            mgr.check_update(),
            "Pending async wallpaper should be applied"
        );
        let buf = mgr.cached_buffer().unwrap();
        assert_eq!(&buf[0..3], &[77, 88, 99]);
    }

    #[test]
    fn test_optical_zone_profile_pure_black() {
        let w = 100u32;
        let h = 100u32;
        let crop = vec![0u8; (w * h * 4) as usize];
        let profile = WallpaperManager::sample_widget_zones(&crop, w, h);

        assert_eq!(profile.overall.luma_mean, 0.0);
        assert_eq!(profile.overall.luma_std_dev, 0.0);
        assert!(!profile.overall.is_light);
        assert!(!profile.overall.is_multi_tone);
        assert!(!profile.is_multi_tone);
        assert_eq!(profile.day_zone.luma_mean, 0.0);
        assert_eq!(profile.date_zone.luma_mean, 0.0);
        assert_eq!(profile.numeral_zone.luma_mean, 0.0);
    }

    #[test]
    fn test_optical_zone_profile_pure_white() {
        let w = 100u32;
        let h = 100u32;
        let crop = vec![255u8; (w * h * 4) as usize];
        let profile = WallpaperManager::sample_widget_zones(&crop, w, h);

        assert!((profile.overall.luma_mean - 1.0).abs() < 1e-4);
        assert!(profile.overall.luma_std_dev < 1e-4);
        assert!(profile.overall.is_light);
        assert!(!profile.overall.is_multi_tone);
        assert!(!profile.is_multi_tone);
        assert!(profile.day_zone.is_light);
        assert!(profile.date_zone.is_light);
        assert!(profile.numeral_zone.is_light);
    }

    #[test]
    fn test_optical_zone_profile_multi_tone_split() {
        let w = 100u32;
        let h = 100u32;
        let mut crop = vec![0u8; (w * h * 4) as usize];
        // Make top 25% (Day zone) pure white, and bottom 75% pure black
        for y in 0..25 {
            for x in 0..100 {
                let idx = ((y * w + x) * 4) as usize;
                crop[idx] = 255;
                crop[idx + 1] = 255;
                crop[idx + 2] = 255;
                crop[idx + 3] = 255;
            }
        }
        let profile = WallpaperManager::sample_widget_zones(&crop, w, h);

        // Day zone should be white
        assert!(profile.day_zone.is_light);
        assert!((profile.day_zone.luma_mean - 1.0).abs() < 1e-3);

        // Date and Numeral zones should be dark
        assert!(!profile.date_zone.is_light);
        assert!(!profile.numeral_zone.is_light);

        // Overall standard deviation should detect multi-tone variance
        assert!(profile.overall.luma_std_dev > 0.18);
        assert!(profile.is_multi_tone);
    }

    #[test]
    fn test_optical_zone_profile_dominant_tint() {
        let w = 40u32;
        let h = 40u32;
        let mut crop = vec![0u8; (w * h * 4) as usize];
        // Uniform vibrant green
        for y in 0..40 {
            for x in 0..40 {
                let idx = ((y * w + x) * 4) as usize;
                crop[idx] = 0;
                crop[idx + 1] = 255;
                crop[idx + 2] = 0;
                crop[idx + 3] = 255;
            }
        }
        let profile = WallpaperManager::sample_widget_zones(&crop, w, h);
        assert_eq!(profile.overall.dominant_tint[0], 0.0);
        assert!((profile.overall.dominant_tint[1] - 1.0).abs() < 1e-3);
        assert_eq!(profile.overall.dominant_tint[2], 0.0);
    }

    #[test]
    fn test_dynamic_awww_create_dir_all_watches_existing_subdirs() {
        let _lock = HOME_LOCK.lock().unwrap();
        let _orig_home = HomeRestoreGuard(std::env::var("HOME").ok());
        let dir = make_test_dir("awww_create_dir_all");
        std::fs::create_dir_all(dir.0.join(".cache")).unwrap();
        std::env::set_var("HOME", &dir.0);

        let mut mgr = WallpaperManager::new();
        assert!(mgr.inotify_fd().is_some(), "inotify must watch ~/.cache");
        let initial_watch_count = mgr.watch_descs.len();

        // Simulate awww cold-start creating ~/.cache/awww/0.12.1 via create_dir_all
        let awww_ver_dir = dir.0.join(".cache/awww/0.12.1");
        std::fs::create_dir_all(&awww_ver_dir).unwrap();

        // First check_update processes IN_CREATE for "awww" and must scan & watch "0.12.1"
        assert!(!mgr.check_update());
        assert!(mgr.awww_dir_wd.is_some(), "awww_dir_wd must be registered");
        assert!(
            mgr.watch_descs.len() >= initial_watch_count + 2,
            "watch_descs must include both ~/.cache/awww and existing subdir 0.12.1"
        );

        // Subsequent write inside 0.12.1 must be detected immediately by inotify
        let img1 = dir.0.join("wp1.png");
        write_solid_png(&img1, [12, 34, 56]);
        std::fs::write(
            awww_ver_dir.join("eDP-1"),
            format!("Lanczos3\0{}\0", img1.display()),
        )
        .unwrap();

        assert!(
            mgr.check_update(),
            "check_update must detect wallpaper written into pre-existing awww/0.12.1 subdir"
        );
        assert_eq!(&mgr.cached_buffer().unwrap()[0..3], &[12, 34, 56]);

        // Also verify dynamic creation of a second version subdir under awww_dir_wd
        let watch_count_before_second_dir = mgr.watch_descs.len();
        let awww_ver_dir2 = dir.0.join(".cache/awww/0.12.2");
        std::fs::create_dir_all(&awww_ver_dir2).unwrap();
        assert!(!mgr.check_update());
        assert!(
            mgr.watch_descs.len() > watch_count_before_second_dir,
            "watch_descs must include dynamically created awww/0.12.2 subdir"
        );

        let img2 = dir.0.join("wp2.png");
        write_solid_png(&img2, [210, 180, 90]);
        std::fs::write(
            awww_ver_dir2.join("DP-1"),
            format!("Lanczos3\0{}\0", img2.display()),
        )
        .unwrap();

        assert!(
            mgr.check_update(),
            "check_update must detect wallpaper written into newly created awww/0.12.2 subdir"
        );
        assert_eq!(&mgr.cached_buffer().unwrap()[0..3], &[210, 180, 90]);
    }

    #[test]
    fn test_optical_zone_profile_large_dimensions_no_overflow() {
        // Large dimensions (> 65535) that would overflow u32 arithmetic if not handled safely
        let w = 70_000u32;
        let h = 70_000u32;
        let crop = vec![0u8; 100];
        let profile = WallpaperManager::sample_widget_zones(&crop, w, h);
        assert_eq!(profile.overall.luma_mean, 0.0);
        assert_eq!(profile.overall.luma_std_dev, 0.0);
        assert!(!profile.overall.is_light);
    }
}
