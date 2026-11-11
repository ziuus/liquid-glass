pub mod capabilities {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct GlassCapabilities {
        pub background_sampling: bool,
        pub compositor_blur: bool,
        pub kde_blur: bool,
        pub ext_background_effect: bool,
        pub refraction: bool,
        pub layer_surface: bool,
    }

    impl Default for GlassCapabilities {
        fn default() -> Self {
            Self {
                background_sampling: false, // Assume false until compositor confirms
                compositor_blur: false,
                kde_blur: false,
                ext_background_effect: false,
                refraction: true,     // Software fallback works
                layer_surface: false, // Needs wlr-layer-shell
            }
        }
    }
}

pub mod wallpaper;
pub use wallpaper::WallpaperManager;

pub mod wayland {
    use super::capabilities::GlassCapabilities;
    use wayland_protocols_plasma::blur::client::org_kde_kwin_blur_manager::OrgKdeKwinBlurManager;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum BlurCompositorProtocol {
        None,
        OrgKdeKwinBlur,
        ExtBackgroundEffect,
    }

    pub struct WaylandContext {
        pub layer_shell_available: bool,
        pub kde_blur_manager: Option<OrgKdeKwinBlurManager>,
        pub ext_background_effect_available: bool,
    }

    impl Default for WaylandContext {
        fn default() -> Self {
            Self {
                layer_shell_available: true,
                kde_blur_manager: None,
                ext_background_effect_available: false,
            }
        }
    }

    impl WaylandContext {
        pub fn init() -> Result<Self, Box<dyn std::error::Error>> {
            let is_kde = Self::is_plasma_desktop();

            Ok(Self {
                layer_shell_available: true,
                kde_blur_manager: None,
                ext_background_effect_available: is_kde,
            })
        }

        pub fn is_plasma_desktop() -> bool {
            std::env::var("XDG_CURRENT_DESKTOP")
                .map(|d| {
                    let d = d.to_ascii_lowercase();
                    d.contains("kde") || d.contains("plasma")
                })
                .unwrap_or(false)
                || std::env::var("KDE_FULL_SESSION")
                    .map(|v| v == "true")
                    .unwrap_or(false)
        }

        pub fn with_kde_blur(mut self, enabled: bool) -> Self {
            self.ext_background_effect_available = enabled;
            self
        }

        pub fn active_blur_protocol(&self) -> BlurCompositorProtocol {
            if self.kde_blur_manager.is_some() {
                BlurCompositorProtocol::OrgKdeKwinBlur
            } else if self.ext_background_effect_available || Self::is_plasma_desktop() {
                BlurCompositorProtocol::ExtBackgroundEffect
            } else {
                BlurCompositorProtocol::None
            }
        }

        pub fn check_capabilities(&self) -> GlassCapabilities {
            let has_kde_blur = self.kde_blur_manager.is_some()
                || self.ext_background_effect_available
                || Self::is_plasma_desktop();

            GlassCapabilities {
                layer_surface: self.layer_shell_available,
                compositor_blur: has_kde_blur,
                kde_blur: has_kde_blur,
                ext_background_effect: self.ext_background_effect_available,
                refraction: true,
                background_sampling: false,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::capabilities::GlassCapabilities;
    use super::wayland::{BlurCompositorProtocol, WaylandContext};

    #[test]
    fn test_default_capabilities() {
        let caps = GlassCapabilities::default();
        assert!(!caps.compositor_blur);
        assert!(!caps.kde_blur);
        assert!(!caps.ext_background_effect);
        assert!(!caps.layer_surface);
        assert!(caps.refraction);
    }

    #[test]
    fn test_wayland_context_capabilities_and_kde_blur() {
        let mut ctx = WaylandContext::default();
        assert!(ctx.layer_shell_available);
        assert_eq!(ctx.active_blur_protocol(), BlurCompositorProtocol::None);

        let caps = ctx.check_capabilities();
        assert!(caps.layer_surface);
        assert!(!caps.kde_blur);

        // Enable KDE blur / background effect
        ctx = ctx.with_kde_blur(true);
        assert_eq!(
            ctx.active_blur_protocol(),
            BlurCompositorProtocol::ExtBackgroundEffect
        );
        let kde_caps = ctx.check_capabilities();
        assert!(kde_caps.compositor_blur);
        assert!(kde_caps.kde_blur);
        assert!(kde_caps.ext_background_effect);
    }
}
