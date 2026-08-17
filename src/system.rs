// ! coleta de métricas do sistema lendo diretamente de /proc
// ! Não usamos nenhuma crate externa aqui de propósito: /proc é uma
// ! interface estável do kernel Linux e ler os arquivos na mão deixa o
// ! binário mais leve, mais rápido de compilar e mais fácil de entender

use anyhow::{Context, Result};
use std::collections::HashMap;
use std::fs;
use std::time::Instant;

#[derive(Debug, Clone, Copy, Default)]
struct CpuTimes {
    idle: u64,
    total: u64,
}

impl CpuTimes {
    fn usage_since(&self, prev: &CpuTimes) -> f64 {
        let idle_delta = self.idle.saturating_sub(prev.idle) as f64;
        let total_delta = self.total.saturating_sub(prev.total) as f64;
        if total_delta <= 0.0 {
            return 0.0;
        }
        (1.0 - idle_delta / total_delta).clamp(0.0, 1.0) * 100.0
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct MemInfo {
    pub total: u64,
    pub available: u64,
    pub used: u64,
    pub swap_total: u64,
    pub swap_used: u64,
}

impl MemInfo {
    pub fn used_pct(&self) -> f64 {
        if self.total == 0 {
            0.0
        } else {
            (self.used as f64 / self.total as f64) * 100.0
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct NetTotals {
    pub rx_bytes: u64,
    pub tx_bytes: u64,
}

#[derive(Debug, Clone)]
pub struct ProcInfo {
    pub pid: u32,
    pub name: String,
    pub cpu_pct: f64,
    pub mem_bytes: u64,
}

/// estadopersistente entre "ticks" de coleta, precisamos guardar a
/// leitura anterior pra calcular deltas (CPU %, taxa de rede etc)
pub struct Collector {
    prev_cpu_total: CpuTimes,
    prev_cpu_per_core: Vec<CpuTimes>,
    prev_net: NetTotals,
    prev_proc_times: HashMap<u32, u64>,
    prev_instant: Instant,
    clock_ticks_per_sec: u64,
}

pub struct Snapshot {
    pub cpu_total_pct: f64,
    pub cpu_per_core_pct: Vec<f64>,
    pub mem: MemInfo,
    pub net: NetTotals,
    pub net_rx_rate: f64,
    pub net_tx_rate: f64,
    pub load_avg: (f64, f64, f64),
    pub uptime_secs: u64,
    pub top_procs: Vec<ProcInfo>,
}

impl Collector {
    pub fn new() -> Result<Self> {
        let clock_ticks_per_sec = 100;
        let (total, per_core) = read_cpu_times()?;
        let net = read_net_totals()?;
        Ok(Self {
            prev_cpu_total: total,
            prev_cpu_per_core: per_core,
            prev_net: net,
            prev_proc_times: HashMap::new(),
            prev_instant: Instant::now(),
            clock_ticks_per_sec,
        })
    }

    pub fn tick(&mut self) -> Result<Snapshot> {
        let now = Instant::now();
        let elapsed = now
            .duration_since(self.prev_instant)
            .as_secs_f64()
            .max(0.001);

        let (cpu_total, cpu_per_core) = read_cpu_times()?;
        let cpu_total_pct = cpu_total.usage_since(&self.prev_cpu_total);
        let cpu_per_core_pct: Vec<f64> = cpu_per_core
            .iter()
            .zip(self.prev_cpu_per_core.iter())
            .map(|(now, prev)| now.usage_since(prev))
            .collect();

        let mem = read_mem_info()?;
        let net = read_net_totals()?;
        let net_rx_rate = (net.rx_bytes.saturating_sub(self.prev_net.rx_bytes)) as f64 / elapsed;
        let net_tx_rate = (net.tx_bytes.saturating_sub(self.prev_net.tx_bytes)) as f64 / elapsed;

        let load_avg = read_load_avg().unwrap_or((0.0, 0.0, 0.0));
        let uptime_secs = read_uptime().unwrap_or(0);

        let (top_procs, new_proc_times) =
            read_top_processes(&self.prev_proc_times, elapsed, self.clock_ticks_per_sec)
                .unwrap_or_default();

        self.prev_cpu_total = cpu_total;
        self.prev_cpu_per_core = cpu_per_core;
        self.prev_net = net;
        self.prev_proc_times = new_proc_times;
        self.prev_instant = now;

        Ok(Snapshot {
            cpu_total_pct,
            cpu_per_core_pct,
            mem,
            net,
            net_rx_rate,
            net_tx_rate,
            load_avg,
            uptime_secs,
            top_procs,
        })
    }
}

fn read_cpu_times() -> Result<(CpuTimes, Vec<CpuTimes>)> {
    let content = fs::read_to_string("/proc/stat").context("lendo /proc/stat")?;
    let mut total = CpuTimes::default();
    let mut per_core = Vec::new();

    for line in content.lines() {
        if let Some(rest) = line.strip_prefix("cpu ") {
            total = parse_cpu_line(rest);
        } else if line.starts_with("cpu") {
            if let Some(space_idx) = line.find(' ') {
                let rest = &line[space_idx + 1..];
                per_core.push(parse_cpu_line(rest));
            }
        } else {
            break;
        }
    }

    Ok((total, per_core))
}

fn parse_cpu_line(rest: &str) -> CpuTimes {
    let fields: Vec<u64> = rest
        .split_whitespace()
        .filter_map(|f| f.parse::<u64>().ok())
        .collect();
    let idle = fields.get(3).copied().unwrap_or(0) + fields.get(4).copied().unwrap_or(0);
    let total: u64 = fields.iter().take(8).sum();
    CpuTimes { idle, total }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_cpu_line_and_computes_usage() {
        let t0 = parse_cpu_line("100 0 100 800 0 0 0 0");
        let t1 = parse_cpu_line("125 0 125 850 0 0 0 0");
        let usage = t1.usage_since(&t0);
        assert!((usage - 50.0).abs() < 0.01);
    }

    #[test]
    fn zero_delta_gives_zero_usage() {
        let t = parse_cpu_line("100 0 100 800 0 0 0 0");
        assert_eq!(t.usage_since(&t), 0.0);
    }

    #[test]
    fn mem_used_pct_is_correct() {
        let mem = MemInfo {
            total: 1000,
            available: 400,
            used: 600,
            swap_total: 0,
            swap_used: 0,
        };
        assert!((mem.used_pct() - 60.0).abs() < 0.01);
    }
}

fn read_mem_info() -> Result<MemInfo> {
    let content = fs::read_to_string("/proc/meminfo").context("lendo /proc/meminfo")?;
    let mut map = HashMap::new();
    for line in content.lines() {
        if let Some((key, rest)) = line.split_once(':') {
            let value_kb = rest
                .trim()
                .split_whitespace()
                .next()
                .and_then(|v| v.parse::<u64>().ok())
                .unwrap_or(0);
            map.insert(key.to_string(), value_kb * 1024);
        }
    }
    let total = *map.get("MemTotal").unwrap_or(&0);
    let available = *map.get("MemAvailable").unwrap_or(&0);
    let swap_total = *map.get("SwapTotal").unwrap_or(&0);
    let swap_free = *map.get("SwapFree").unwrap_or(&0);
    Ok(MemInfo {
        total,
        available,
        used: total.saturating_sub(available),
        swap_total,
        swap_used: swap_total.saturating_sub(swap_free),
    })
}

fn read_net_totals() -> Result<NetTotals> {
    let content = fs::read_to_string("/proc/net/dev").context("lendo /proc/net/dev")?;
    let mut totals = NetTotals::default();
    for line in content.lines().skip(2) {
        let Some((iface, rest)) = line.split_once(':') else {
            continue;
        };
        let iface = iface.trim();
        if iface == "lo" {
            continue;
        }
        let fields: Vec<u64> = rest
            .split_whitespace()
            .filter_map(|f| f.parse::<u64>().ok())
            .collect();
        if let (Some(rx), Some(tx)) = (fields.first(), fields.get(8)) {
            totals.rx_bytes += rx;
            totals.tx_bytes += tx;
        }
    }
    Ok(totals)
}

fn read_load_avg() -> Result<(f64, f64, f64)> {
    let content = fs::read_to_string("/proc/loadavg")?;
    let parts: Vec<&str> = content.split_whitespace().collect();
    let one = parts.first().and_then(|v| v.parse().ok()).unwrap_or(0.0);
    let five = parts.get(1).and_then(|v| v.parse().ok()).unwrap_or(0.0);
    let fifteen = parts.get(2).and_then(|v| v.parse().ok()).unwrap_or(0.0);
    Ok((one, five, fifteen))
}

fn read_uptime() -> Result<u64> {
    let content = fs::read_to_string("/proc/uptime")?;
    let secs = content
        .split_whitespace()
        .next()
        .and_then(|v| v.parse::<f64>().ok())
        .unwrap_or(0.0);
    Ok(secs as u64)
}

fn read_top_processes(
    prev_times: &HashMap<u32, u64>,
    elapsed_secs: f64,
    clock_ticks_per_sec: u64,
) -> Result<(Vec<ProcInfo>, HashMap<u32, u64>)> {
    let mut procs = Vec::new();
    let mut new_times = HashMap::new();

    for entry in fs::read_dir("/proc")? {
        let Ok(entry) = entry else { continue };
        let file_name = entry.file_name();
        let Some(pid_str) = file_name.to_str() else {
            continue;
        };
        let Ok(pid) = pid_str.parse::<u32>() else {
            continue;
        };

        let stat_path = entry.path().join("stat");
        let Ok(stat) = fs::read_to_string(&stat_path) else {
            continue;
        };

        let Some(close_paren) = stat.rfind(')') else {
            continue;
        };
        let name_start = stat.find('(').map(|i| i + 1).unwrap_or(0);
        let name = stat[name_start..close_paren].to_string();

        let rest: Vec<&str> = stat[close_paren + 1..].split_whitespace().collect();
        let utime: u64 = rest.get(11).and_then(|v| v.parse().ok()).unwrap_or(0);
        let stime: u64 = rest.get(12).and_then(|v| v.parse().ok()).unwrap_or(0);
        let total_jiffies = utime + stime;

        new_times.insert(pid, total_jiffies);

        let cpu_pct = match prev_times.get(&pid) {
            Some(&prev) if elapsed_secs > 0.0 => {
                let delta_jiffies = total_jiffies.saturating_sub(prev) as f64;
                let delta_secs = delta_jiffies / clock_ticks_per_sec as f64;
                (delta_secs / elapsed_secs) * 100.0
            }
            _ => 0.0,
        };

        let mem_bytes = fs::read_to_string(entry.path().join("status"))
            .ok()
            .and_then(|status| {
                status.lines().find_map(|l| {
                    l.strip_prefix("VmRSS:").map(|v| {
                        v.trim()
                            .split_whitespace()
                            .next()
                            .and_then(|n| n.parse::<u64>().ok())
                            .unwrap_or(0)
                            * 1024
                    })
                })
            })
            .unwrap_or(0);

        procs.push(ProcInfo {
            pid,
            name,
            cpu_pct,
            mem_bytes,
        });
    }

    procs.sort_by(|a, b| b.cpu_pct.partial_cmp(&a.cpu_pct).unwrap());
    procs.truncate(50);

    Ok((procs, new_times))
}
