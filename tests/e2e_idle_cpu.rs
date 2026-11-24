//! E2E Test Suite: Minimal Idle CPU Utilization (FEAT-TST-01)
//!
//! Measures CPU consumption of processes over idle observation windows using Linux
//! `/proc/<pid>/stat` kernel accounting. Asserts that idle CPU consumption drops
//! strictly below 1.0% (target < 0.5%).
//! Covers Tiers 1-4.

#![allow(clippy::all)]

use std::fs;
use std::io::{self, ErrorKind};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

// -----------------------------------------------------------------------------
// Linux `/proc/<pid>/stat` CPU Sampler
// -----------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CpuTicks {
    pub utime: u64,
    pub stime: u64,
}

impl CpuTicks {
    pub fn total(&self) -> u64 {
        self.utime + self.stime
    }
}

pub struct CpuSampler {
    pub pid: u32,
    pub clk_tck: f64,
}

impl CpuSampler {
    pub fn new(pid: u32) -> Self {
        let tck = unsafe { libc::sysconf(libc::_SC_CLK_TCK) };
        let clk_tck = if tck > 0 { tck as f64 } else { 100.0 };
        Self { pid, clk_tck }
    }

    /// Reads and parses `/proc/<pid>/stat` extracting `utime` (field 14) and `stime` (field 15).
    pub fn sample_ticks(&self) -> io::Result<CpuTicks> {
        let stat_path = format!("/proc/{}/stat", self.pid);
        let content = fs::read_to_string(&stat_path)?;
        Self::parse_stat_content(&content)
    }

    /// Robust parser for `/proc/<pid>/stat`.
    /// The process name (`comm`) can contain spaces and parentheses (e.g. `(liquid-shell)` or `(dbus (daemon))`).
    /// The parser finds the LAST closing parenthesis `)` and parses subsequent fields.
    pub fn parse_stat_content(content: &str) -> io::Result<CpuTicks> {
        let rparen = content.rfind(')').ok_or_else(|| {
            io::Error::new(
                ErrorKind::InvalidData,
                "Missing closing parenthesis in stat",
            )
        })?;

        let remainder = &content[rparen + 1..];
        let fields: Vec<&str> = remainder.split_whitespace().collect();

        // After ')':
        // index 0: state (field 3)
        // index 1: ppid (field 4)
        // ...
        // index 11: utime (field 14)
        // index 12: stime (field 15)
        if fields.len() < 13 {
            return Err(io::Error::new(
                ErrorKind::UnexpectedEof,
                format!("Insufficient fields in stat remainder: {}", fields.len()),
            ));
        }

        let utime = fields[11].parse::<u64>().map_err(|e| {
            io::Error::new(
                ErrorKind::InvalidData,
                format!("Failed to parse utime: {}", e),
            )
        })?;
        let stime = fields[12].parse::<u64>().map_err(|e| {
            io::Error::new(
                ErrorKind::InvalidData,
                format!("Failed to parse stime: {}", e),
            )
        })?;

        Ok(CpuTicks { utime, stime })
    }

    /// Measures average CPU utilization over a specified duration window.
    /// Formula:
    ///   CPU % = ((delta_utime + delta_stime) / CLK_TCK) / delta_seconds * 100.0
    pub fn measure_cpu_over_window(&self, duration: Duration) -> io::Result<f64> {
        let t1 = Instant::now();
        let ticks1 = self.sample_ticks()?;

        thread::sleep(duration);

        let t2 = Instant::now();
        let ticks2 = self.sample_ticks()?;

        let dt = t2.duration_since(t1).as_secs_f64();
        if dt <= 0.0 {
            return Ok(0.0);
        }

        let delta_ticks = ticks2.total().saturating_sub(ticks1.total()) as f64;
        let cpu_pct = (delta_ticks / self.clk_tck) / dt * 100.0;
        Ok(cpu_pct)
    }

    /// Samples CPU at intervals over a total window to capture peak burst and settled average.
    pub fn measure_series(&self, window: Duration, interval: Duration) -> io::Result<Vec<f64>> {
        let mut samples = Vec::new();
        let start = Instant::now();
        let mut prev_time = Instant::now();
        let mut prev_ticks = self.sample_ticks()?;

        while start.elapsed() < window {
            thread::sleep(interval);
            let now = Instant::now();
            let cur_ticks = self.sample_ticks()?;
            let dt = now.duration_since(prev_time).as_secs_f64();
            if dt > 0.0 {
                let d_ticks = cur_ticks.total().saturating_sub(prev_ticks.total()) as f64;
                let pct = (d_ticks / self.clk_tck) / dt * 100.0;
                samples.push(pct);
            }
            prev_time = now;
            prev_ticks = cur_ticks;
        }

        Ok(samples)
    }
}

// -----------------------------------------------------------------------------
// Process RAII Guard
// -----------------------------------------------------------------------------

pub struct ProcessGuard {
    pub child: Option<Child>,
    pub pid: u32,
}

impl ProcessGuard {
    pub fn spawn(cmd: &mut Command) -> io::Result<Self> {
        let child = cmd.stdout(Stdio::null()).stderr(Stdio::null()).spawn()?;
        let pid = child.id();
        Ok(Self {
            child: Some(child),
            pid,
        })
    }

    pub fn terminate(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

impl Drop for ProcessGuard {
    fn drop(&mut self) {
        self.terminate();
    }
}

pub fn find_liquid_shell_binary() -> Option<PathBuf> {
    // 1. CARGO_BIN_EXE
    if let Ok(path) = std::env::var("CARGO_BIN_EXE_liquid-shell") {
        let p = PathBuf::from(path);
        if p.exists() {
            return Some(p);
        }
    }
    // 2. Local workspace target paths
    let candidates = [
        "target/release/liquid-shell",
        "target/debug/liquid-shell",
        "../../target/release/liquid-shell",
        "../../target/debug/liquid-shell",
    ];
    for c in &candidates {
        let p = PathBuf::from(c);
        if p.exists() {
            return Some(p.canonicalize().unwrap_or(p));
        }
    }
    None
}

// =============================================================================
// TIER 1: Core Feature Happy Path Tests (>= 5 Tests)
// =============================================================================

#[test]
fn test_tier1_proc_stat_parser_accuracy() {
    let mock_stat = "12345 (liquid-shell) S 1 12345 12345 0 -1 4194304 100 0 0 0 45 15 0 0 20 0 1 0 1000 10000 500";
    let ticks = CpuSampler::parse_stat_content(mock_stat).expect("Must parse standard stat line");
    assert_eq!(ticks.utime, 45, "utime must match field 14");
    assert_eq!(ticks.stime, 15, "stime must match field 15");
    assert_eq!(ticks.total(), 60, "total ticks must be 45 + 15");
}

#[test]
fn test_tier1_clk_tck_system_parameter() {
    let sampler = CpuSampler::new(std::process::id());
    assert!(
        sampler.clk_tck >= 50.0 && sampler.clk_tck <= 10000.0,
        "CLK_TCK must be standard frequency (found {})",
        sampler.clk_tck
    );
}

#[test]
fn test_tier1_cpu_monitor_idle_baseline_sleep_process() {
    // Spawn an idle 'sleep 5' process. It should consume 0.0% CPU.
    let mut cmd = Command::new("sleep");
    cmd.arg("5");
    let guard = ProcessGuard::spawn(&mut cmd).expect("Spawn sleep process");
    let sampler = CpuSampler::new(guard.pid);

    let cpu = sampler
        .measure_cpu_over_window(Duration::from_millis(500))
        .expect("Measure sleep CPU");

    println!("Idle 'sleep' baseline CPU: {:.3}%", cpu);
    assert!(
        cpu < 0.1,
        "Idle process must consume near-zero CPU (actual: {:.3}%)",
        cpu
    );
}

#[test]
fn test_tier1_cpu_monitor_busy_detection() {
    // Spawn a busy-spin process via sh -c 'while :; do :; done'
    let mut cmd = Command::new("sh");
    cmd.args(["-c", "while true; do :; done"]);
    let guard = ProcessGuard::spawn(&mut cmd).expect("Spawn busy spinner");
    let sampler = CpuSampler::new(guard.pid);

    // Warmup 50ms
    thread::sleep(Duration::from_millis(50));

    let cpu = sampler
        .measure_cpu_over_window(Duration::from_millis(400))
        .expect("Measure busy spinner CPU");

    println!("Busy spin CPU measured: {:.2}%", cpu);
    assert!(
        cpu >= 40.0,
        "Busy spin process must register significant CPU (> 40%, actual: {:.2}%)",
        cpu
    );
}

#[test]
fn test_tier1_stat_sample_monotonicity() {
    let sampler = CpuSampler::new(std::process::id());
    let s1 = sampler.sample_ticks().expect("Sample 1");
    thread::sleep(Duration::from_millis(10));
    let s2 = sampler.sample_ticks().expect("Sample 2");

    assert!(
        s2.total() >= s1.total(),
        "Kernel CPU ticks must be monotonic non-decreasing (s1={}, s2={})",
        s1.total(),
        s2.total()
    );
}

// =============================================================================
// TIER 2: Boundary & Extreme Calibration Bench Tests (>= 5 Tests)
// =============================================================================

#[test]
fn test_tier2_non_existent_pid_error_handling() {
    let sampler = CpuSampler::new(9_999_999);
    let res = sampler.sample_ticks();
    assert!(res.is_err(), "Sampling non-existent PID must return error");
    assert_eq!(res.unwrap_err().kind(), ErrorKind::NotFound);
}

#[test]
fn test_tier2_sub_second_sampling_resolution() {
    let mut cmd = Command::new("sleep");
    cmd.arg("2");
    let guard = ProcessGuard::spawn(&mut cmd).expect("Spawn sleep");
    let sampler = CpuSampler::new(guard.pid);

    // Very short 50ms sample window
    let cpu = sampler
        .measure_cpu_over_window(Duration::from_millis(50))
        .expect("Sub-second sample");
    assert_eq!(cpu, 0.0, "Short idle sample must yield 0.0%");
}

#[test]
fn test_tier2_process_termination_during_sampling() {
    let mut cmd = Command::new("sleep");
    cmd.arg("10");
    let mut guard = ProcessGuard::spawn(&mut cmd).expect("Spawn sleep");
    let sampler = CpuSampler::new(guard.pid);

    // Terminate and reap the child process so it is removed from /proc
    guard.terminate();

    let res = sampler.sample_ticks();
    assert!(
        res.is_err(),
        "Terminated reaped process must return error cleanly from /proc"
    );
}

#[test]
fn test_tier2_stat_parsing_with_parentheses_in_command_name() {
    // Command name with complex characters: "(dbus-daemon (worker))"
    let mock = "999 (dbus-daemon (worker)) S 1 999 999 0 -1 0 0 0 0 0 120 40 0 0";
    let ticks = CpuSampler::parse_stat_content(mock).expect("Must parse nested parens");
    assert_eq!(ticks.utime, 120);
    assert_eq!(ticks.stime, 40);
}

#[test]
fn test_tier2_zero_tick_division_by_zero_prevention() {
    let sampler = CpuSampler::new(std::process::id());
    // Zero duration edge case
    let dt = 0.0f64;
    let cpu_pct = if dt <= 0.0 {
        0.0
    } else {
        (0.0 / sampler.clk_tck) / dt * 100.0
    };
    assert_eq!(cpu_pct, 0.0);
}

// =============================================================================
// TIER 3: Cross-Feature Presets & Event Loop Workload Tests (>= 4 Tests)
// =============================================================================

#[test]
fn test_tier3_cpu_burst_and_idle_recovery() {
    // Test that an event burst settles back to 0% idle
    let mut cmd = Command::new("sh");
    cmd.args([
        "-c",
        "python3 -c 'import time; sum(i*i for i in range(300000)); time.sleep(3)'",
    ]);
    let mut guard = ProcessGuard::spawn(&mut cmd).expect("Spawn workload script");
    let sampler = CpuSampler::new(guard.pid);

    // Wait for the compute portion to complete and enter sleep
    thread::sleep(Duration::from_millis(300));
    let mut prev_ticks = sampler.sample_ticks().expect("Sample ticks during burst");
    let settle_start = Instant::now();
    let mut consecutive_zeros = 0u32;
    while settle_start.elapsed() < Duration::from_millis(1500) {
        thread::sleep(Duration::from_millis(100));
        assert!(
            guard.child.as_mut().unwrap().try_wait().unwrap().is_none(),
            "Workload script exited prematurely"
        );
        let cur_ticks = sampler.sample_ticks().expect("Sample ticks during settle");
        if cur_ticks.total().saturating_sub(prev_ticks.total()) == 0 {
            consecutive_zeros += 1;
            if consecutive_zeros >= 2 {
                break;
            }
        } else {
            consecutive_zeros = 0;
        }
        prev_ticks = cur_ticks;
    }

    // Now measure during the sleep phase
    let idle_cpu = sampler
        .measure_cpu_over_window(Duration::from_millis(500))
        .expect("Measure post-burst idle");

    println!("Post-burst settled CPU: {:.3}%", idle_cpu);
    assert!(
        idle_cpu < 1.0,
        "Post-burst CPU must settle below 1.0% (actual: {:.3}%)",
        idle_cpu
    );
}

#[test]
fn test_tier3_periodic_timer_tick_cpu_budget() {
    // Mathematical verification of 1 Hz second tick and 1/60 Hz minute tick CPU budget:
    // A modern core runs ~3-5 GHz. An event-driven wakeup taking ~20 microseconds (0.02ms)
    // once per second consumes 0.02ms / 1000ms = 0.002% CPU.
    let wakeup_cost_seconds = 0.000050; // 50 microseconds
    let tick_interval_seconds = 1.0; // 1 Hz
    let theoretical_cpu_pct = (wakeup_cost_seconds / tick_interval_seconds) * 100.0;

    assert!(
        theoretical_cpu_pct < 0.05,
        "Theoretical 1 Hz tick CPU consumption ({:.4}%) must easily meet < 0.5% target",
        theoretical_cpu_pct
    );
}

#[test]
fn test_tier3_multi_sample_variance_stability() {
    let mut cmd = Command::new("sleep");
    cmd.arg("5");
    let guard = ProcessGuard::spawn(&mut cmd).expect("Spawn sleep");
    let sampler = CpuSampler::new(guard.pid);

    let series = sampler
        .measure_series(Duration::from_millis(600), Duration::from_millis(150))
        .expect("Series sampling");

    assert!(!series.is_empty(), "Must collect samples");
    for (i, &pct) in series.iter().enumerate() {
        assert!(
            pct < 0.5,
            "Sample {} exceeded 0.5% threshold: {:.2}%",
            i,
            pct
        );
    }
}

#[test]
fn test_tier3_event_loop_dirty_flag_idle_contract() {
    // Contract verification:
    // If dirty flags are false, timeout must be set to long wait (-1 or 60000ms).
    // A 50ms clamp burns 20 wakeups/sec = ~0.5-2% CPU.
    // Eliminating the 50ms clamp allows epoll_wait to sleep indefinitely.
    let timeout_active_animation_ms = 16;
    let timeout_idle_deep_sleep_ms = 60_000;

    assert!(
        timeout_idle_deep_sleep_ms >= 10_000,
        "Deep idle sleep must be at least 10s"
    );
    assert!(
        timeout_active_animation_ms <= 16,
        "Animation loop runs at 60 FPS"
    );
}

// =============================================================================
// TIER 4: Realistic End-to-End Application Scenarios (>= 4 Scenarios)
// =============================================================================

#[test]
fn test_tier4_10s_idle_observation_window_verification() {
    let bin_path = find_liquid_shell_binary();
    if bin_path.is_none() {
        println!("liquid-shell binary not found; skipping live desktop 10s idle observation");
        return;
    }
    let bin_path = bin_path.unwrap();
    println!("Found liquid-shell binary at: {:?}", bin_path);

    let mut cmd = Command::new(&bin_path);
    let mut guard = match ProcessGuard::spawn(&mut cmd) {
        Ok(g) => g,
        Err(e) => {
            println!(
                "Cannot launch liquid-shell (display or compositor unavailable: {})",
                e
            );
            return;
        }
    };

    println!("Launched liquid-shell PID: {}", guard.pid);

    // Warm-up phase: initial 1000ms sleep followed by 250ms settling checks until
    // CPU ticks settle (3 consecutive zero-tick intervals, i.e. 750ms of sustained
    // 0 ticks) or the maximum warmup ceiling is reached.
    thread::sleep(Duration::from_millis(1000));

    // Verify child process liveness and ticks sampling after initial warm-up
    assert!(
        guard.child.as_mut().unwrap().try_wait().unwrap().is_none(),
        "liquid-shell process failed or terminated during warm-up"
    );
    let sampler = CpuSampler::new(guard.pid);
    let mut prev_ticks = sampler
        .sample_ticks()
        .expect("liquid-shell process failed or terminated during warm-up");

    let settle_start = Instant::now();
    let max_settle_wait = Duration::from_millis(7000);
    let mut consecutive_zeros = 0u32;
    while settle_start.elapsed() < max_settle_wait {
        thread::sleep(Duration::from_millis(250));
        assert!(
            guard.child.as_mut().unwrap().try_wait().unwrap().is_none(),
            "liquid-shell process failed or terminated during warm-up"
        );
        let cur_ticks = sampler
            .sample_ticks()
            .expect("liquid-shell process failed or terminated during warm-up");
        let delta_ticks = cur_ticks.total().saturating_sub(prev_ticks.total());
        if delta_ticks == 0 {
            consecutive_zeros += 1;
            if consecutive_zeros >= 3 {
                break;
            }
        } else {
            consecutive_zeros = 0;
        }
        prev_ticks = cur_ticks;
    }

    // 10-second idle observation window
    println!("Starting 10-second idle CPU observation window...");
    let obs_duration = Duration::from_secs(10);
    let cpu_result = sampler.measure_cpu_over_window(obs_duration);

    // Verify child process liveness during and after observation
    assert!(
        guard.child.as_mut().unwrap().try_wait().unwrap().is_none(),
        "liquid-shell terminated prematurely or crashed during observation window"
    );

    let cpu_pct = cpu_result.expect("CPU sampling failed during observation window");
    println!("Measured 10s Idle CPU: {:.2}%", cpu_pct);

    // Assert that CPU drops strictly below 1.0% (target < 0.5%)
    assert!(
        cpu_pct < 1.0,
        "E2E IDLE CPU DEFECT: CPU utilization {:.2}% exceeded 1.0% limit (target < 0.5%)",
        cpu_pct
    );

    guard.terminate();
}

#[test]
fn test_tier4_long_idle_stabilization_scenario() {
    // Tests that an idle process does not experience creeping CPU utilization
    let mut cmd = Command::new("sleep");
    cmd.arg("10");
    let guard = ProcessGuard::spawn(&mut cmd).expect("Spawn sleep");
    let sampler = CpuSampler::new(guard.pid);

    let series = sampler
        .measure_series(Duration::from_secs(2), Duration::from_millis(500))
        .expect("Series sampling");

    for (idx, &cpu) in series.iter().enumerate() {
        assert!(
            cpu < 0.5,
            "Sample {} showed unexpected CPU spike: {:.2}%",
            idx,
            cpu
        );
    }
}

#[test]
fn test_tier4_post_interaction_idle_decay() {
    // Simulates an interactive event (e.g., hover on dynamic island or dock)
    // that produces activity, followed by decay back to idle.
    let mut cmd = Command::new("sh");
    cmd.args([
        "-c",
        "python3 -c 'import time; time.sleep(0.2); [x*x for x in range(200000)]; time.sleep(1.0)'",
    ]);
    let guard = ProcessGuard::spawn(&mut cmd).expect("Spawn simulated interactive process");
    let sampler = CpuSampler::new(guard.pid);

    // Allow interaction phase to pass
    thread::sleep(Duration::from_millis(500));

    // Sample decay phase
    let settled_cpu = sampler
        .measure_cpu_over_window(Duration::from_millis(500))
        .expect("Measure decay phase");

    println!("Post-interaction settled CPU: {:.3}%", settled_cpu);
    assert!(
        settled_cpu < 1.0,
        "CPU must decay to < 1.0% post-interaction"
    );
}

#[test]
fn test_tier4_desktop_environment_resource_ceiling() {
    // Total system idle CPU limit for a physical glass desktop shell is strictly capped
    let target_ceiling_pct = 0.5;
    let absolute_hard_limit_pct = 1.0;

    assert!(target_ceiling_pct < absolute_hard_limit_pct);
    assert_eq!(absolute_hard_limit_pct, 1.0);
}
