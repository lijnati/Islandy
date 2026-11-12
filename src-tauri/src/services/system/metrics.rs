//! CPU and memory utilization.
//!
//! CPU utilization is a *difference* of two cumulative time samples, so the
//! first sample after sampling (re)starts only sets a baseline. Sampling runs
//! only while the system overview is visible (see `monitor.rs`).
//!
//! - Windows: `GetSystemTimes` (kernel time *includes* idle time) and
//!   `GlobalMemoryStatusEx` (`ullTotalPhys`, `ullAvailPhys`).
//! - Linux (development host only): `/proc/stat` and `/proc/meminfo`.

use super::models::{Availability, MemoryUsage, MetricsState};

/// Cumulative CPU time across all processors, in any consistent unit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CpuTimes {
    pub idle: u64,
    pub total: u64,
}

impl CpuTimes {
    /// From `GetSystemTimes` (100 ns units). `kernel` already contains `idle`.
    #[cfg_attr(not(windows), allow(dead_code))]
    pub fn from_windows(idle: u64, kernel: u64, user: u64) -> Self {
        Self {
            idle,
            total: kernel.saturating_add(user),
        }
    }
}

/// One raw reading.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RawSample {
    pub cpu: Option<CpuTimes>,
    /// (total, available) physical memory in bytes.
    pub memory: Option<(u64, u64)>,
}

/// Busy share of the elapsed CPU time between two samples, 0–100.
/// `None` when no time elapsed or the counters went backwards.
pub fn cpu_percent(previous: CpuTimes, next: CpuTimes) -> Option<u8> {
    let total = next.total.checked_sub(previous.total)?;
    if total == 0 {
        return None;
    }
    let idle = next.idle.checked_sub(previous.idle)?.min(total);
    let busy = total - idle;
    Some(((busy as f64 * 100.0 / total as f64).round()) as u8)
}

pub fn memory_usage(total: u64, available: u64) -> Option<MemoryUsage> {
    if total == 0 {
        return None;
    }
    let available = available.min(total);
    let used = total - available;
    Some(MemoryUsage {
        total_bytes: total,
        available_bytes: available,
        used_percent: ((used as f64 * 100.0 / total as f64).round()) as u8,
    })
}

/// Turns raw samples into [`MetricsState`], keeping the CPU baseline.
#[derive(Debug, Default)]
pub struct Sampler {
    previous: Option<CpuTimes>,
}

impl Sampler {
    /// Forgets the baseline (sampling paused: an old baseline would average
    /// over the pause).
    pub fn reset(&mut self) {
        self.previous = None;
    }

    pub fn sample(
        &mut self,
        raw: RawSample,
        now_ms: u64,
        interval_ms: Option<u64>,
    ) -> MetricsState {
        let cpu_percent = match (self.previous, raw.cpu) {
            (Some(previous), Some(next)) => cpu_percent(previous, next),
            _ => None,
        };
        if raw.cpu.is_some() {
            self.previous = raw.cpu;
        }
        let memory = raw
            .memory
            .and_then(|(total, available)| memory_usage(total, available));
        MetricsState {
            status: if raw.cpu.is_none() && memory.is_none() {
                Availability::Unavailable
            } else {
                Availability::Ready
            },
            cpu_percent,
            memory,
            sampled_at_ms: now_ms,
            interval_ms,
        }
    }
}

/// Parses the aggregate `cpu` line of `/proc/stat`.
/// `user nice system idle iowait irq softirq steal [guest guest_nice]`;
/// guest time is already part of user time, so it is not added again.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
pub fn parse_proc_stat(text: &str) -> Option<CpuTimes> {
    let line = text.lines().find(|l| l.starts_with("cpu "))?;
    let values: Vec<u64> = line
        .split_whitespace()
        .skip(1)
        .take(8)
        .map(|v| v.parse().ok())
        .collect::<Option<_>>()?;
    if values.len() < 4 {
        return None;
    }
    let idle = values[3] + values.get(4).copied().unwrap_or(0);
    Some(CpuTimes {
        idle,
        total: values.iter().sum(),
    })
}

/// Parses `MemTotal` and `MemAvailable` (kB) from `/proc/meminfo`, in bytes.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
pub fn parse_meminfo(text: &str) -> Option<(u64, u64)> {
    let field = |name: &str| -> Option<u64> {
        let line = text.lines().find(|l| l.starts_with(name))?;
        let kb: u64 = line[name.len()..]
            .trim()
            .trim_end_matches("kB")
            .trim()
            .parse()
            .ok()?;
        kb.checked_mul(1024)
    };
    Some((field("MemTotal:")?, field("MemAvailable:")?))
}

/// Reads `/proc` (Linux development host).
#[cfg(target_os = "linux")]
pub fn read_linux() -> RawSample {
    RawSample {
        cpu: std::fs::read_to_string("/proc/stat")
            .ok()
            .and_then(|t| parse_proc_stat(&t)),
        memory: std::fs::read_to_string("/proc/meminfo")
            .ok()
            .and_then(|t| parse_meminfo(&t)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn computes_cpu_from_time_deltas() {
        let a = CpuTimes::from_windows(800, 1_000, 1_000); // kernel includes idle
        let b = CpuTimes::from_windows(1_100, 1_400, 1_600);
        // elapsed = (1400+1600) - 2000 = 1000, idle = 300 → 70% busy
        assert_eq!(cpu_percent(a, b), Some(70));
        assert_eq!(cpu_percent(a, a), None, "no time elapsed");
        assert_eq!(cpu_percent(b, a), None, "counters went backwards");
        let idle_only = CpuTimes {
            idle: a.idle + 500,
            total: a.total + 500,
        };
        assert_eq!(cpu_percent(a, idle_only), Some(0));
        let busy = CpuTimes {
            idle: a.idle,
            total: a.total + 500,
        };
        assert_eq!(cpu_percent(a, busy), Some(100));
        // Idle advancing more than total (clock skew) is clamped.
        let skew = CpuTimes {
            idle: a.idle + 900,
            total: a.total + 500,
        };
        assert_eq!(cpu_percent(a, skew), Some(0));
    }

    #[test]
    fn computes_memory_usage() {
        let gib = 1u64 << 30;
        let m = memory_usage(16 * gib, 4 * gib).unwrap();
        assert_eq!(m.used_percent, 75);
        assert_eq!(m.available_bytes, 4 * gib);
        assert_eq!(memory_usage(0, 0), None);
        assert_eq!(memory_usage(8, 20).unwrap().used_percent, 0, "clamped");
    }

    #[test]
    fn first_sample_is_only_a_baseline() {
        let mut s = Sampler::default();
        let raw = |idle, total| RawSample {
            cpu: Some(CpuTimes { idle, total }),
            memory: Some((100, 40)),
        };
        let first = s.sample(raw(50, 100), 1, Some(2000));
        assert_eq!(first.cpu_percent, None);
        assert_eq!(first.memory.unwrap().used_percent, 60);
        assert_eq!(first.status, Availability::Ready);
        assert_eq!(s.sample(raw(75, 200), 2, Some(2000)).cpu_percent, Some(75));
        s.reset();
        assert_eq!(s.sample(raw(80, 300), 3, Some(2000)).cpu_percent, None);
        let nothing = s.sample(
            RawSample {
                cpu: None,
                memory: None,
            },
            4,
            None,
        );
        assert_eq!(nothing.status, Availability::Unavailable);
    }

    #[test]
    fn parses_proc_files() {
        let stat = "cpu  100 5 50 800 20 1 2 3 7 0\ncpu0 1 2 3 4\nintr 1";
        assert_eq!(
            parse_proc_stat(stat),
            Some(CpuTimes {
                idle: 820,
                total: 981
            })
        );
        assert_eq!(parse_proc_stat("cpu0 1 2 3 4"), None);
        assert_eq!(parse_proc_stat("cpu  x y z"), None);
        let meminfo = "MemTotal:       16000 kB\nMemFree: 1 kB\nMemAvailable:    4000 kB\n";
        assert_eq!(parse_meminfo(meminfo), Some((16000 * 1024, 4000 * 1024)));
        assert_eq!(parse_meminfo("MemTotal: 1 kB"), None);
    }
}
