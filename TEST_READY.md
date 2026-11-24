# E2E Test Suite Ready

## Test Runner
- **Rust Integration Test Suites**:
  - `cargo test --test e2e_wallpaper_contrast`: 28 tests (WCAG 2.1 AA legibility across synthetic benches and all 18 wallpapers in `~/Pictures/wallpaper/`)
  - `cargo test --test e2e_idle_cpu`: 18 tests (Linux `/proc/<pid>/stat` sampling, 10s idle observation window asserting CPU < 1.0%, child liveness verification)
  - `cargo test --test e2e_config_parsing`: 20 tests (TOML deserialization, default fallback, boundary clamps, hot-reload)
- **Turnkey Installation & Script Validation**:
  - `./tests/test_install_dry_run.sh`: 14 checks (verifies `--dry-run` and `--check` zero-modification behavior and prerequisite checks)
  - `./install.sh --check`: Exits 0 if prerequisites are satisfied
  - `./install.sh --dry-run`: Exits 0 displaying planned targets without modifying disk
- **Full Suite Compilation**:
  - `cargo test --no-run --tests`: Exits with code 0

## Coverage Summary
| Tier | Count | Description |
|------|------:|-------------|
| 1. Feature Coverage | 28 | Basic functionality across standard wallpapers, baseline CPU, TOML parsing, and install checks |
| 2. Boundary & Corner | 24 | Pure black `#000000`, pure white `#FFFFFF`, 50% gray `#808080`, splits, 1x1 buffer, 32:9 ultrawide, corrupt configs, zero tick clamp |
| 3. Cross-Feature | 14 | Optical contrast modes, rim boost, theme overrides, dynamic island dock interaction, config reload |
| 4. Real-World Application | 14 | All 18 user wallpapers verified, 10s live desktop idle window, wallpaper transitions, multi-monitor crop scaling |
| **Total** | **80** | **Comprehensive 4-tier E2E testing suite** |

## Feature Checklist
| Feature | Tier 1 | Tier 2 | Tier 3 | Tier 4 | Status |
|---------|:------:|:------:|:------:|:------:|:------:|
| Central Clock Dynamic Contrast (ORIGINAL_REQUEST §R1) | 5 | 8 | 5 | 10 | READY |
| Minimal Idle CPU Loop (<1%) (ORIGINAL_REQUEST §R2) | 5 | 5 | 4 | 4 | READY (Gatekeeper Active: DEFECT-M2-01 Isolated) |
| Multi-System Portability & Scaling (ORIGINAL_REQUEST §R3) | 4 | 3 | 3 | 2 | READY |
| Turnkey install.sh & Systemd (ORIGINAL_REQUEST §R4) | 6 | 4 | 2 | 2 | READY |
| Centralized TOML Config (ORIGINAL_REQUEST §R5) | 8 | 4 | 4 | 4 | READY |

## Known Implementation Defects Under Active Gatekeeping
- **DEFECT-M2-01**: `test_tier4_10s_idle_observation_window_verification` in `tests/e2e_idle_cpu.rs` accurately catches the current un-throttled render loop in `liquid-shell` (measuring 60.80% - 63.20% CPU vs < 1.0% limit). This test serves as the mandatory pass gate for Milestone 2 implementation.
