use crate::app::{App, Tab};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Gauge, Paragraph, Row, Sparkline, Table, Tabs},
    Frame,
};

fn heat_color(pct: f64) -> Color {
    match pct as u64 {
        0..=49 => Color::Green,
        50..=79 => Color::Yellow,
        _ => Color::Red,
    }
}

fn format_bytes(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut value = bytes as f64;
    let mut unit_idx = 0;
    while value >= 1024.0 && unit_idx < UNITS.len() - 1 {
        value /= 1024.0;
        unit_idx += 1;
    }
    format!("{:.1}{}", value, UNITS[unit_idx])
}

fn format_rate(bytes_per_sec: f64) -> String {
    format!("{}/s", format_bytes(bytes_per_sec as u64))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_bytes_human_readable() {
        assert_eq!(format_bytes(512), "512.0B");
        assert_eq!(format_bytes(2048), "2.0KB");
        assert_eq!(format_bytes(5 * 1024 * 1024), "5.0MB");
    }

    #[test]
    fn heat_color_thresholds() {
        assert_eq!(heat_color(10.0), Color::Green);
        assert_eq!(heat_color(60.0), Color::Yellow);
        assert_eq!(heat_color(95.0), Color::Red);
    }
}

pub fn draw(f: &mut Frame, app: &App) {
    let root = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(f.size());

    draw_header(f, app, root[0]);

    match app.tab {
        Tab::Overview => draw_overview(f, app, root[1]),
        Tab::Processes => draw_processes(f, app, root[1]),
        Tab::Containers => draw_containers(f, app, root[1]),
    }

    draw_footer(f, root[2]);
}

fn draw_header(f: &mut Frame, app: &App, area: Rect) {
    let titles = [Tab::Overview, Tab::Processes, Tab::Containers]
        .iter()
        .map(|t| Line::from(t.title()))
        .collect::<Vec<_>>();

    let selected = match app.tab {
        Tab::Overview => 0,
        Tab::Processes => 1,
        Tab::Containers => 2,
    };

    let spinner = app.spinner_char();
    let title = format!(" {spinner} rustop-tui — monitor do sistema ");

    let tabs = Tabs::new(titles)
        .select(selected)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(title)
                .title_style(Style::default().add_modifier(Modifier::BOLD)),
        )
        .style(Style::default().fg(Color::White))
        .highlight_style(
            Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .divider("│");

    f.render_widget(tabs, area);
}

fn draw_footer(f: &mut Frame, area: Rect) {
    let help = Line::from(vec![
        Span::styled(" q ", Style::default().fg(Color::Black).bg(Color::Gray)),
        Span::raw(" sair   "),
        Span::styled(
            " ←/→ ou Tab ",
            Style::default().fg(Color::Black).bg(Color::Gray),
        ),
        Span::raw(" trocar aba   "),
        Span::styled(" ↑/↓ ", Style::default().fg(Color::Black).bg(Color::Gray)),
        Span::raw(" navegar processos"),
    ]);
    f.render_widget(Paragraph::new(help), area);
}

fn draw_overview(f: &mut Frame, app: &App, area: Rect) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
        .split(area);

    let left = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(6),
            Constraint::Length(6),
        ])
        .split(cols[0]);

    let cpu_pct = app.snapshot.cpu_total_pct;
    let cpu_gauge = Gauge::default()
        .block(Block::default().borders(Borders::ALL).title("CPU total"))
        .gauge_style(Style::default().fg(heat_color(cpu_pct)))
        .ratio((cpu_pct / 100.0).clamp(0.0, 1.0))
        .label(format!("{cpu_pct:.1}%"));
    f.render_widget(cpu_gauge, left[0]);

    draw_per_core(f, app, left[1]);
    draw_memory(f, app, left[2]);

    let right = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(7),
            Constraint::Length(7),
            Constraint::Min(0),
        ])
        .split(cols[1]);

    let cpu_history: Vec<u64> = app.history.cpu.iter().map(|v| *v as u64).collect();
    let cpu_spark = Sparkline::default()
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Histórico de CPU (%)"),
        )
        .data(&cpu_history)
        .style(Style::default().fg(Color::Cyan));
    f.render_widget(cpu_spark, right[0]);

    let net_area = right[1];
    let net_cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(net_area);

    let rx_history: Vec<u64> = app.history.net_rx.iter().map(|v| *v as u64).collect();
    let rx_spark = Sparkline::default()
        .block(Block::default().borders(Borders::ALL).title(format!(
            "↓ Download {}",
            format_rate(app.snapshot.net_rx_rate)
        )))
        .data(&rx_history)
        .style(Style::default().fg(Color::Green));
    f.render_widget(rx_spark, net_cols[0]);

    let tx_history: Vec<u64> = app.history.net_tx.iter().map(|v| *v as u64).collect();
    let tx_spark = Sparkline::default()
        .block(Block::default().borders(Borders::ALL).title(format!(
            "↑ Upload {}",
            format_rate(app.snapshot.net_tx_rate)
        )))
        .data(&tx_history)
        .style(Style::default().fg(Color::Magenta));
    f.render_widget(tx_spark, net_cols[1]);

    draw_system_info(f, app, right[2]);
}

fn draw_per_core(f: &mut Frame, app: &App, area: Rect) {
    let cores = &app.snapshot.cpu_per_core_pct;
    if cores.is_empty() {
        f.render_widget(
            Paragraph::new("lendo núcleos...").block(Block::default().borders(Borders::ALL)),
            area,
        );
        return;
    }

    let block = Block::default().borders(Borders::ALL).title("Núcleos");
    let inner = block.inner(area);
    f.render_widget(block, area);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints(vec![
            Constraint::Length(1);
            cores.len().min(inner.height as usize)
        ])
        .split(inner);

    for (i, row_area) in rows.iter().enumerate() {
        let Some(&pct) = cores.get(i) else { continue };
        let bar_width = row_area.width.saturating_sub(10) as usize;
        let filled = ((pct / 100.0) * bar_width as f64) as usize;
        let bar: String =
            "█".repeat(filled.min(bar_width)) + &"░".repeat(bar_width.saturating_sub(filled));
        let line = Line::from(vec![
            Span::raw(format!("{i:>2} ")),
            Span::styled(bar, Style::default().fg(heat_color(pct))),
            Span::raw(format!(" {pct:>5.1}%")),
        ]);
        f.render_widget(Paragraph::new(line), *row_area);
    }
}

fn draw_memory(f: &mut Frame, app: &App, area: Rect) {
    let mem = &app.snapshot.mem;
    let pct = mem.used_pct();
    let label = format!(
        "{} / {} ({:.1}%)",
        format_bytes(mem.used),
        format_bytes(mem.total),
        pct
    );
    let gauge = Gauge::default()
        .block(Block::default().borders(Borders::ALL).title("Memória RAM"))
        .gauge_style(Style::default().fg(heat_color(pct)))
        .ratio((pct / 100.0).clamp(0.0, 1.0))
        .label(label);
    f.render_widget(gauge, area);
}

fn draw_system_info(f: &mut Frame, app: &App, area: Rect) {
    let s = &app.snapshot;
    let uptime_h = s.uptime_secs / 3600;
    let uptime_m = (s.uptime_secs % 3600) / 60;
    let lines = vec![
        Line::from(format!(
            "Load average: {:.2}  {:.2}  {:.2}",
            s.load_avg.0, s.load_avg.1, s.load_avg.2
        )),
        Line::from(format!("Uptime: {uptime_h}h {uptime_m}m")),
        Line::from(format!(
            "Swap: {} / {}",
            format_bytes(s.mem.swap_used),
            format_bytes(s.mem.swap_total)
        )),
        Line::from(format!("Processos monitorados: {}", s.top_procs.len())),
    ];
    f.render_widget(
        Paragraph::new(lines).block(Block::default().borders(Borders::ALL).title("Sistema")),
        area,
    );
}

fn draw_processes(f: &mut Frame, app: &App, area: Rect) {
    let header = Row::new(vec!["PID", "Processo", "CPU %", "Memória"])
        .style(Style::default().add_modifier(Modifier::BOLD));

    let rows: Vec<Row> = app
        .snapshot
        .top_procs
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let style = if i == app.selected_process {
                Style::default().bg(Color::Cyan).fg(Color::Black)
            } else {
                Style::default().fg(heat_color(p.cpu_pct))
            };
            Row::new(vec![
                Cell::from(p.pid.to_string()),
                Cell::from(p.name.clone()),
                Cell::from(format!("{:.1}%", p.cpu_pct)),
                Cell::from(format_bytes(p.mem_bytes)),
            ])
            .style(if i == app.selected_process {
                style
            } else {
                Style::default()
            })
            .style(style)
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Length(8),
            Constraint::Min(20),
            Constraint::Length(10),
            Constraint::Length(12),
        ],
    )
    .header(header)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Top processos por CPU "),
    );

    f.render_widget(table, area);
}

fn draw_containers(f: &mut Frame, app: &App, area: Rect) {
    if !app.docker_installed {
        let msg = Paragraph::new(
            "Docker não encontrado (ou o daemon não está acessível).\n\
             Instale o Docker ou rode este painel como um usuário que tenha permissão\n\
             de executar `docker stats`.",
        )
        .block(Block::default().borders(Borders::ALL).title("Containers"))
        .alignment(Alignment::Center);
        f.render_widget(msg, area);
        return;
    }

    if app.containers.is_empty() {
        let msg = Paragraph::new("Nenhum container em execução no momento.")
            .block(Block::default().borders(Borders::ALL).title("Containers"))
            .alignment(Alignment::Center);
        f.render_widget(msg, area);
        return;
    }

    let header = Row::new(vec!["Nome", "CPU %", "Memória", "Rede RX/TX"])
        .style(Style::default().add_modifier(Modifier::BOLD));

    let rows: Vec<Row> = app
        .containers
        .iter()
        .map(|c| {
            Row::new(vec![
                Cell::from(c.name.clone()),
                Cell::from(format!("{:.1}%", c.cpu_pct)),
                Cell::from(format!(
                    "{} / {}",
                    format_bytes(c.mem_used_bytes),
                    format_bytes(c.mem_limit_bytes)
                )),
                Cell::from(format!(
                    "{} / {}",
                    format_bytes(c.net_rx_bytes),
                    format_bytes(c.net_tx_bytes)
                )),
            ])
            .style(Style::default().fg(heat_color(c.cpu_pct)))
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Min(16),
            Constraint::Length(10),
            Constraint::Length(20),
            Constraint::Length(20),
        ],
    )
    .header(header)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title(" docker stats (atualiza a cada ~2s) "),
    );

    f.render_widget(table, area);
}
