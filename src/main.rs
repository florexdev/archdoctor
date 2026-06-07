use std::process::Command;
use std::time::Duration;
use regex::Regex;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, BorderType, List, ListItem, ListState, Paragraph, LineGauge},
    Frame,
};

#[derive(Clone)]
struct DiagnosticResult {
    name: &'static str,
    passed: bool,
    score_deduction: i32,
    details: Vec<String>,
    suggestion: Option<String>,
}

#[derive(Clone)]
struct BootMetric {
    kernel: f32,
    initrd: f32,
    userspace: f32,
    total: f32,
}

struct App {
    health_score: i32,
    reports: Vec<DiagnosticResult>,
    boot_metrics: Option<BootMetric>,
    list_state: ListState,
    should_quit: bool,
}

impl App {
    fn new() -> Self {
        let mut app = Self {
            health_score: 100,
            reports: Vec::new(),
            boot_metrics: None,
            list_state: ListState::default(),
            should_quit: false,
        };
        app.run_all_diagnostics();
        app.list_state.select(Some(0));
        app
    }

    fn run_all_diagnostics(&mut self) {
        let mut reports = Vec::new();
        reports.push(check_failed_services());
        reports.push(check_disk_space());
        reports.push(check_orphan_packages());
        reports.push(check_journal_logs());

        let mut score = 100;
        for r in &reports {
            if !r.passed {
                score -= r.score_deduction;
            }
        }
        self.health_score = score.max(0);
        self.reports = reports;
        self.boot_metrics = get_boot_metrics();
    }

    fn next(&mut self) {
        let i = match self.list_state.selected() {
            Some(i) => {
                let total_items = self.reports.len() + 1;
                if i >= total_items - 1 {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.list_state.select(Some(i));
    }

    fn previous(&mut self) {
        let i = match self.list_state.selected() {
            Some(i) => {
                let total_items = self.reports.len() + 1;
                if i == 0 {
                    total_items - 1
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.list_state.select(Some(i));
    }
}

fn check_failed_services() -> DiagnosticResult {
    let output = Command::new("systemctl")
        .args(["--failed", "--output=json"])
        .output();

    let mut details = Vec::new();
    let mut passed = true;
    let mut suggestion = None;

    if let Ok(out) = output {
        let stdout = String::from_utf8_lossy(&out.stdout);
        let lines: Vec<&str> = stdout.lines().filter(|l| l.contains(".service")).collect();
        
        if !lines.is_empty() {
            passed = false;
            for line in lines {
                if let Some(service) = line.split_whitespace().next() {
                    details.push(format!("Failed service: {}", service));
                    if service.contains("docker") {
                        suggestion = Some("sudo systemctl restart docker.service".to_string());
                    } else if service.contains("lightdm") || service.contains("gdm") || service.contains("sddm") {
                        suggestion = Some("sudo systemctl restart display-manager".to_string());
                    }
                }
            }
        }
    }

    if passed {
        details.push("All systemd services running perfectly.".to_string());
    }

    DiagnosticResult {
        name: "Failed Services Check",
        passed,
        score_deduction: 15,
        details,
        suggestion,
    }
}

fn check_disk_space() -> DiagnosticResult {
    let output = Command::new("df")
        .arg("-h")
        .output();

    let mut details = Vec::new();
    let mut passed = true;
    let mut suggestion = None;

    if let Ok(out) = output {
        let stdout = String::from_utf8_lossy(&out.stdout);
        for line in stdout.lines() {
            if line.ends_with('/') {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 5 {
                    let use_pct_str = parts[4].replace('%', "");
                    if let Ok(use_pct) = use_pct_str.parse::<i32>() {
                        details.push(format!("Root partition usage is at {}% (mount: /)", use_pct));
                        if use_pct > 90 {
                            passed = false;
                            suggestion = Some("sudo pacman -Sc && sudo journalctl --vacuum-time=3d".to_string());
                        }
                    }
                }
            }
        }
    }

    DiagnosticResult {
        name: "Disk Space Check",
        passed,
        score_deduction: 15,
        details,
        suggestion,
    }
}

fn check_orphan_packages() -> DiagnosticResult {
    let output = Command::new("pacman")
        .args(["-Qdtq"])
        .output();

    let mut details = Vec::new();
    let mut passed = true;
    let mut suggestion = None;

    if let Ok(out) = output {
        let stdout = String::from_utf8_lossy(&out.stdout);
        let orphans: Vec<&str> = stdout.lines().filter(|l| !l.is_empty()).collect();
        
        if !orphans.is_empty() {
            passed = false;
            details.push(format!("Detected {} unneeded orphan packages.", orphans.len()));
            suggestion = Some("sudo pacman -Rns $(pacman -Qdtq)".to_string());
        } else {
            details.push("No orphan packages lingering around. System is clean!".to_string());
        }
    }

    DiagnosticResult {
        name: "Orphan Packages Check",
        passed,
        score_deduction: 5,
        details,
        suggestion,
    }
}

fn check_journal_logs() -> DiagnosticResult {
    let output = Command::new("journalctl")
        .args(["-p", "3", "-b"])
        .output();

    let mut details = Vec::new();
    let mut passed = true;
    let mut suggestion = None;
    let mut score_deduction = 0;

    if let Ok(out) = output {
        let stdout = String::from_utf8_lossy(&out.stdout);
        let lines: Vec<&str> = stdout.lines().filter(|l| !l.is_empty()).collect();
        let error_count = lines.len();

        if error_count > 0 {
            passed = false;
            score_deduction = 20;
            details.push(format!("Found {} critical hardware/boot logs in this session.", error_count));

            let nvidia_regex = Regex::new(r"(?i)nvidia|nouveau|nvidia-modeset").unwrap();
            let mut nvidia_issue_found = false;

            for line in &lines {
                if nvidia_regex.is_match(line) {
                    nvidia_issue_found = true;
                    details.push(format!("Matched Log: {}", line.trim()));
                    break;
                }
            }

            if nvidia_issue_found {
                suggestion = Some("sudo pacman -S nvidia nvidia-utils".to_string());
            } else {
                suggestion = Some("journalctl -p 3 -b".to_string());
            }
        } else {
            details.push("Zero critical errors recorded in local system log journals.".to_string());
        }
    }

    DiagnosticResult {
        name: "Journal Log Analysis",
        passed,
        score_deduction,
        details,
        suggestion,
    }
}

fn get_boot_metrics() -> Option<BootMetric> {
    let output = Command::new("systemd-analyze").output().ok()?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    
    let parse_time = |label: &str| -> f32 {
        let pattern = format!(r"(\d+(?:\.\d+)?)(s|ms)\s*\({}\)", label);
        if let Ok(re) = Regex::new(&pattern) {
            if let Some(caps) = re.captures(&stdout) {
                let val = caps.get(1).map_or(0.0, |m| m.as_str().parse::<f32>().unwrap_or(0.0));
                let unit = caps.get(2).map_or("s", |m| m.as_str());
                if unit == "ms" {
                    return val / 1000.0;
                } else {
                    return val;
                }
            }
        }
        0.0
    };

    let kernel = parse_time("kernel");
    let initrd = parse_time("initrd");
    let userspace = parse_time("userspace");

    let mut total = 0.0;
    if let Ok(re_total) = Regex::new(r"=\s*(\d+(?:\.\d+)?)(s|ms)") {
        if let Some(caps) = re_total.captures(&stdout) {
            let val = caps.get(1).map_or(0.0, |m| m.as_str().parse::<f32>().unwrap_or(0.0));
            let unit = caps.get(2).map_or("s", |m| m.as_str());
            total = if unit == "ms" { val / 1000.0 } else { val };
        }
    }

    if total == 0.0 {
        total = kernel + initrd + userspace;
    }

    if total > 0.0 {
        Some(BootMetric {
            kernel,
            initrd,
            userspace,
            total,
        })
    } else {
        None
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut terminal = ratatui::init();
    let mut app = App::new();

    while !app.should_quit {
        terminal.draw(|f| ui(f, &mut app))?;

        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    match key.code {
                        KeyCode::Char('q') | KeyCode::Char('Q') | KeyCode::Esc => {
                            app.should_quit = true;
                        }
                        KeyCode::Down | KeyCode::Char('j') => {
                            app.next();
                        }
                        KeyCode::Up | KeyCode::Char('k') => {
                            app.previous();
                        }
                        KeyCode::Char('r') | KeyCode::Char('R') => {
                            app.run_all_diagnostics();
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    ratatui::restore();
    Ok(())
}

fn ui(f: &mut Frame, app: &mut App) {
    let size = f.area();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(4),
            Constraint::Min(10),
            Constraint::Length(3),
        ])
        .split(size);

    let header_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(40),
            Constraint::Percentage(60),
        ])
        .split(chunks[0]);

    let title_widget = Paragraph::new(vec![
        Line::from(vec![
            Span::styled(" ARCHDOCTOR ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Span::styled("v0.1.2 ", Style::default().fg(Color::Magenta).add_modifier(Modifier::ITALIC)),
            Span::styled("| Diagnostic Panel", Style::default().fg(Color::DarkGray)),
        ]),
        Line::from(Span::styled("   Vibe-Coded minimalist terminal health check for Arch Linux", Style::default().fg(Color::Gray))),
    ])
    .block(Block::default().borders(Borders::NONE));

    f.render_widget(title_widget, header_chunks[0]);

    let gauge_color = match app.health_score {
        80..=100 => Color::Green,
        50..=79 => Color::Yellow,
        _ => Color::Red,
    };

    let status_text = match app.health_score {
        90..=100 => "PERFECT",
        80..=89 => "GOOD",
        60..=79 => "WARNINGS DETECTED",
        40..=59 => "NEEDS ATTENTION",
        _ => "CRITICAL STATE",
    };

    let health_gauge = LineGauge::default()
        .block(Block::bordered()
            .title(format!(" System Health: {}/100 [{}] ", app.health_score, status_text))
            .title_style(Style::default().fg(gauge_color).add_modifier(Modifier::BOLD))
            .border_style(Style::default().fg(Color::DarkGray)))
        .filled_symbol("█")
        .unfilled_symbol("░")
        .filled_style(Style::default().fg(gauge_color))
        .ratio(app.health_score as f64 / 100.0);

    f.render_widget(health_gauge, header_chunks[1]);

    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(35),
            Constraint::Percentage(65),
        ])
        .split(chunks[1]);

    let mut list_items = Vec::new();
    for r in &app.reports {
        let prefix = if r.passed { " [✓] " } else { " [!] " };
        let prefix_color = if r.passed { Color::Green } else { Color::Red };
        
        list_items.push(ListItem::new(Line::from(vec![
            Span::styled(prefix, Style::default().fg(prefix_color).add_modifier(Modifier::BOLD)),
            Span::styled(r.name, Style::default().fg(Color::White)),
        ])));
    }
    list_items.push(ListItem::new(Line::from(vec![
        Span::styled(" [i] ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Span::styled("Boot Performance Info", Style::default().fg(Color::White)),
    ])));

    let checks_list = List::new(list_items)
        .block(Block::bordered()
            .title(" System Checks ")
            .title_style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::DarkGray)))
        .highlight_style(Style::default().bg(Color::Rgb(30, 41, 59)).fg(Color::Cyan).add_modifier(Modifier::BOLD))
        .highlight_symbol(" ➜ ");

    f.render_stateful_widget(checks_list, main_chunks[0], &mut app.list_state);

    let selected_index = app.list_state.selected().unwrap_or(0);
    let mut detail_lines = Vec::new();

    if selected_index < app.reports.len() {
        let report = &app.reports[selected_index];
        
        detail_lines.push(Line::from(vec![
            Span::styled("Diagnostic Name: ", Style::default().fg(Color::DarkGray)),
            Span::styled(report.name, Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
        ]));

        let status_span = if report.passed {
            Span::styled("PASSED", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD))
        } else {
            Span::styled("WARNING/FAILED", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD))
        };

        detail_lines.push(Line::from(vec![
            Span::styled("Status:          ", Style::default().fg(Color::DarkGray)),
            status_span,
        ]));

        let (impact_text, impact_color) = if report.passed {
            ("0 Points".to_string(), Color::DarkGray)
        } else {
            (format!("-{} Points", report.score_deduction), Color::Magenta)
        };

        detail_lines.push(Line::from(vec![
            Span::styled("Score Impact:    ", Style::default().fg(Color::DarkGray)),
            Span::styled(impact_text, Style::default().fg(impact_color)),
        ]));

        detail_lines.push(Line::from(""));
        detail_lines.push(Line::from(Span::styled("Anomalies & Trace Logs Details:", Style::default().fg(Color::Cyan).add_modifier(Modifier::UNDERLINED))));
        
        for d in &report.details {
            detail_lines.push(Line::from(format!(" • {}", d)));
        }

        if let Some(ref suggestion) = report.suggestion {
            detail_lines.push(Line::from(""));
            detail_lines.push(Line::from(Span::styled("Recommended command solution:", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))));
            detail_lines.push(Line::from(vec![
                Span::styled("  $ ", Style::default().fg(Color::DarkGray)),
                Span::styled(suggestion, Style::default().fg(Color::Rgb(167, 243, 208)).add_modifier(Modifier::BOLD)),
            ]));
        } else if report.passed {
            detail_lines.push(Line::from(""));
            detail_lines.push(Line::from(Span::styled("Perfect status: No action required.", Style::default().fg(Color::Green))));
        }

    } else {
        detail_lines.push(Line::from(vec![
            Span::styled("Diagnostic Name: ", Style::default().fg(Color::DarkGray)),
            Span::styled("Boot Performance (systemd-analyze)", Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
        ]));
        detail_lines.push(Line::from(""));

        if let Some(ref m) = app.boot_metrics {
            detail_lines.push(Line::from(Span::styled("Active Systemd Boot breakdown metrics:", Style::default().fg(Color::Cyan).add_modifier(Modifier::UNDERLINED))));
            detail_lines.push(Line::from(""));

            let draw_bar = |val: f32, tot: f32, max_w: usize| -> String {
                if tot <= 0.0 { return String::new(); }
                let filled = ((val / tot) * max_w as f32).round() as usize;
                let empty = max_w.saturating_sub(filled);
                format!("{}[{}]", "█".repeat(filled), "░".repeat(empty))
            };

            let max_w = 20;

            detail_lines.push(Line::from(vec![
                Span::styled("  Kernel    : ", Style::default().fg(Color::White)),
                Span::styled(draw_bar(m.kernel, m.total, max_w), Style::default().fg(Color::Cyan)),
                Span::styled(format!(" {:.2}s ({:.1}%)", m.kernel, (m.kernel / m.total) * 100.0), Style::default().fg(Color::Gray)),
            ]));

            if m.initrd > 0.0 {
                detail_lines.push(Line::from(vec![
                    Span::styled("  Initrd    : ", Style::default().fg(Color::White)),
                    Span::styled(draw_bar(m.initrd, m.total, max_w), Style::default().fg(Color::Magenta)),
                    Span::styled(format!(" {:.2}s ({:.1}%)", m.initrd, (m.initrd / m.total) * 100.0), Style::default().fg(Color::Gray)),
                ]));
            }

            detail_lines.push(Line::from(vec![
                Span::styled("  Userspace : ", Style::default().fg(Color::White)),
                Span::styled(draw_bar(m.userspace, m.total, max_w), Style::default().fg(Color::Blue)),
                Span::styled(format!(" {:.2}s ({:.1}%)", m.userspace, (m.userspace / m.total) * 100.0), Style::default().fg(Color::Gray)),
            ]));

            detail_lines.push(Line::from(""));
            detail_lines.push(Line::from(vec![
                Span::styled("  Total Boot: ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                Span::styled(format!("{:.2}s", m.total), Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            ]));

        } else {
            detail_lines.push(Line::from(Span::styled("No metrics available. Make sure systemd-analyze is installed and callable.", Style::default().fg(Color::Red))));
        }
    }

    let detail_paragraph = Paragraph::new(detail_lines)
        .block(Block::bordered()
            .title(" Diagnostics Detail Pane ")
            .title_style(Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD))
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::DarkGray)))
        .scroll((0, 0));

    f.render_widget(detail_paragraph, main_chunks[1]);

    let help_menu = Paragraph::new(Line::from(vec![
        Span::styled(" [▲/▼/j/k] ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Span::styled("Navigate Panels", Style::default().fg(Color::White)),
        Span::styled("  |  ", Style::default().fg(Color::DarkGray)),
        Span::styled(" [R] ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        Span::styled("Re-run Diagnostics", Style::default().fg(Color::White)),
        Span::styled("  |  ", Style::default().fg(Color::DarkGray)),
        Span::styled(" [Q/ESC] ", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
        Span::styled("Quit App", Style::default().fg(Color::White)),
    ]))
    .block(Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::DarkGray)))
    .centered();

    f.render_widget(help_menu, chunks[2]);
}