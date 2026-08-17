use crate::docker::{ContainerStat, DockerWatcher};
use crate::system::{Collector, Snapshot};
use anyhow::Result;
use std::collections::VecDeque;
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::Duration;

pub const HISTORY_LEN: usize = 120;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Overview,
    Processes,
    Containers,
}

impl Tab {
    pub fn title(&self) -> &'static str {
        match self {
            Tab::Overview => "Visão geral",
            Tab::Processes => "Processos",
            Tab::Containers => "Containers",
        }
    }

    pub fn next(self) -> Self {
        match self {
            Tab::Overview => Tab::Processes,
            Tab::Processes => Tab::Containers,
            Tab::Containers => Tab::Overview,
        }
    }

    pub fn prev(self) -> Self {
        match self {
            Tab::Overview => Tab::Containers,
            Tab::Processes => Tab::Overview,
            Tab::Containers => Tab::Processes,
        }
    }
}

// sparklines
pub struct History {
    pub cpu: VecDeque<f64>,
    pub mem: VecDeque<f64>,
    pub net_rx: VecDeque<f64>,
    pub net_tx: VecDeque<f64>,
}

impl History {
    fn new() -> Self {
        Self {
            cpu: VecDeque::with_capacity(HISTORY_LEN),
            mem: VecDeque::with_capacity(HISTORY_LEN),
            net_rx: VecDeque::with_capacity(HISTORY_LEN),
            net_tx: VecDeque::with_capacity(HISTORY_LEN),
        }
    }

    fn push(buf: &mut VecDeque<f64>, value: f64) {
        if buf.len() == HISTORY_LEN {
            buf.pop_front();
        }
        buf.push_back(value);
    }
}

pub struct App {
    pub collector: Collector,
    pub snapshot: Snapshot,
    pub history: History,
    pub tab: Tab,
    pub spinner_frame: usize,
    pub selected_process: usize,
    pub containers: Vec<ContainerStat>,
    docker_rx: Receiver<Vec<ContainerStat>>,
    pub docker_installed: bool,
    pub should_quit: bool,
}

const SPINNER_FRAMES: &[char] = &['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];

impl App {
    pub fn new() -> Result<Self> {
        let mut collector = Collector::new()?;
        let snapshot = collector.tick()?;

        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            let mut watcher = DockerWatcher::new();
            loop {
                let stats = watcher.collect();
                if tx.send(stats).is_err() {
                    break;
                }
                thread::sleep(Duration::from_millis(2000));
            }
        });

        Ok(Self {
            collector,
            snapshot,
            history: History::new(),
            tab: Tab::Overview,
            spinner_frame: 0,
            selected_process: 0,
            containers: Vec::new(),
            docker_rx: rx,
            docker_installed: false,
            should_quit: false,
        })
    }

    pub fn spinner_char(&self) -> char {
        SPINNER_FRAMES[self.spinner_frame % SPINNER_FRAMES.len()]
    }

    pub fn on_animation_tick(&mut self) {
        self.spinner_frame = self.spinner_frame.wrapping_add(1);
        if let Ok(stats) = self.docker_rx.try_recv() {
            self.docker_installed = true;
            self.containers = stats;
        }
    }

    pub fn on_metrics_tick(&mut self) -> Result<()> {
        self.snapshot = self.collector.tick()?;
        History::push(&mut self.history.cpu, self.snapshot.cpu_total_pct);
        History::push(&mut self.history.mem, self.snapshot.mem.used_pct());
        History::push(&mut self.history.net_rx, self.snapshot.net_rx_rate);
        History::push(&mut self.history.net_tx, self.snapshot.net_tx_rate);
        Ok(())
    }

    pub fn next_tab(&mut self) {
        self.tab = self.tab.next();
    }

    pub fn prev_tab(&mut self) {
        self.tab = self.tab.prev();
    }

    pub fn move_selection(&mut self, delta: i32) {
        let len = self.snapshot.top_procs.len();
        if len == 0 {
            return;
        }
        let new_index = self.selected_process as i32 + delta;
        self.selected_process = new_index.clamp(0, len as i32 - 1) as usize;
    }
}
