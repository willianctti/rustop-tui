//! coleta de estatísticas de containers Docker
//! Em vez de linkar a crate `bollard` (cliente async da API do Docker,
//! que traz `tokio` e uma árvore de dependências pesada), chamamos o próprio
//! binário `docker` via `docker stats --no-stream`. .mais simples, não
//! exige runtime async, e funciona em qualquer máquina que já tenha o
//! Docker instalado — que é exatamente onde esse painel importa

use std::process::Command;
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct ContainerStat {
    pub name: String,
    pub cpu_pct: f64,
    pub mem_used_bytes: u64,
    pub mem_limit_bytes: u64,
    pub net_rx_bytes: u64,
    pub net_tx_bytes: u64,
}

pub struct DockerWatcher {
    docker_available: Option<bool>,
    last_check: Instant,
}

impl DockerWatcher {
    pub fn new() -> Self {
        Self {
            docker_available: None,
            last_check: Instant::now() - Duration::from_secs(60),
        }
    }

    pub fn is_available(&mut self) -> bool {
        if self.docker_available.is_none() || self.last_check.elapsed() > Duration::from_secs(30) {
            self.docker_available = Some(
                Command::new("docker")
                    .arg("info")
                    .output()
                    .map(|o| o.status.success())
                    .unwrap_or(false),
            );
            self.last_check = Instant::now();
        }
        self.docker_available.unwrap_or(false)
    }

    pub fn collect(&mut self) -> Vec<ContainerStat> {
        if !self.is_available() {
            return Vec::new();
        }

        let output = Command::new("docker")
            .args([
                "stats",
                "--no-stream",
                "--format",
                "{{.Name}}\t{{.CPUPerc}}\t{{.MemUsage}}\t{{.NetIO}}",
            ])
            .output();

        let Ok(output) = output else {
            return Vec::new();
        };
        if !output.status.success() {
            return Vec::new();
        }

        let text = String::from_utf8_lossy(&output.stdout);
        text.lines().filter_map(parse_stats_line).collect()
    }
}

fn parse_stats_line(line: &str) -> Option<ContainerStat> {
    let cols: Vec<&str> = line.split('\t').collect();
    if cols.len() < 4 {
        return None;
    }
    let name = cols[0].to_string();
    let cpu_pct = cols[1].trim_end_matches('%').parse::<f64>().unwrap_or(0.0);

    let (mem_used_bytes, mem_limit_bytes) = cols[2]
        .split_once('/')
        .map(|(used, limit)| (parse_size(used.trim()), parse_size(limit.trim())))
        .unwrap_or((0, 0));

    let (net_rx_bytes, net_tx_bytes) = cols[3]
        .split_once('/')
        .map(|(rx, tx)| (parse_size(rx.trim()), parse_size(tx.trim())))
        .unwrap_or((0, 0));

    Some(ContainerStat {
        name,
        cpu_pct,
        mem_used_bytes,
        mem_limit_bytes,
        net_rx_bytes,
        net_tx_bytes,
    })
}

fn parse_size(s: &str) -> u64 {
    let s = s.trim();
    let split_at = s.find(|c: char| c.is_alphabetic()).unwrap_or(s.len());
    let (num_part, unit_part) = s.split_at(split_at);
    let num: f64 = num_part.trim().parse().unwrap_or(0.0);
    let unit = unit_part.trim().to_lowercase();

    let multiplier: f64 = match unit.as_str() {
        "b" => 1.0,
        "kb" => 1_000.0,
        "kib" => 1_024.0,
        "mb" => 1_000_000.0,
        "mib" => 1_024.0 * 1_024.0,
        "gb" => 1_000_000_000.0,
        "gib" => 1_024.0 * 1_024.0 * 1_024.0,
        _ => 1.0,
    };

    (num * multiplier) as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_mebibytes() {
        assert_eq!(parse_size("12.5MiB"), (12.5 * 1024.0 * 1024.0) as u64);
    }

    #[test]
    fn parses_kilobytes() {
        assert_eq!(parse_size("3.4kB"), 3400);
    }

    #[test]
    fn parses_plain_bytes() {
        assert_eq!(parse_size("512B"), 512);
    }

    #[test]
    fn parses_full_stats_line() {
        let line = "meu_container\t12.34%\t100MiB / 2GiB\t1.2kB / 3.4kB";
        let stat = parse_stats_line(line).expect("linha válida deve parsear");
        assert_eq!(stat.name, "meu_container");
        assert!((stat.cpu_pct - 12.34).abs() < f64::EPSILON);
        assert_eq!(stat.mem_used_bytes, (100.0 * 1024.0 * 1024.0) as u64);
        assert_eq!(
            stat.mem_limit_bytes,
            (2.0 * 1024.0 * 1024.0 * 1024.0) as u64
        );
    }

    #[test]
    fn rejects_malformed_line() {
        assert!(parse_stats_line("linha sem colunas suficientes").is_none());
    }
}
