use std::process::Command;
use std::io::{self, stdout};
use regex::Regex;
use tui::{
    backend::{CrosstermBackend},
    Terminal,
    layout::{Constraint, Direction, Layout},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    style::{Color, Modifier, Style},
    text::{Span, Spans}
};
use crossterm::{
    execute,
    terminal::{enable_raw_mode, disable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    event::{self, Event, KeyCode}
};

struct MonitorManagerApp {
    connected_monitors: Vec<String>,
    selected_index: usize,
    current_selection: Option<String>,
}

impl MonitorManagerApp {
    fn new() -> Self {
        let connected_monitors = Self::detect_connected_monitors();

        Self {
            connected_monitors,
            selected_index: 0,
            current_selection: None,
        }
    }

    fn detect_connected_monitors() -> Vec<String> {
        let output = Command::new("xrandr")
            .output()
            .expect("Failed to execute xrandr command");
        let output_str = String::from_utf8_lossy(&output.stdout);
        output_str.lines()
            .filter_map(|line| {
                if line.contains(" connected") {
                    line.split_whitespace().next().map(String::from)
                } else {
                    None
                }
            })
            .collect()
    }

    fn get_resolutions(monitor: &str) -> Vec<String> {
        let output = Command::new("xrandr")
            .arg("--verbose")
            .output()
            .expect("Failed to execute xrandr command");
        let output_str = String::from_utf8_lossy(&output.stdout);
        let re = Regex::new(&format!(r"{} connected.*?\n((?:\s+\d+x\d+.*?\n)+)", monitor)).unwrap();
        re.captures_iter(&output_str)
            .flat_map(|cap| {
                cap.get(1)
                    .map(|m| {
                        m.as_str()
                            .lines()
                            .map(|line| line.trim().split_whitespace().next().unwrap_or("").to_string())
                            .collect::<Vec<String>>()
                    })
                    .unwrap_or_default()
            })
            .collect::<Vec<String>>()
    }

    fn set_resolution(&self, monitor: &str, resolution: &str) {
        Command::new("xrandr")
            .arg("--output")
            .arg(monitor)
            .arg("--mode")
            .arg(resolution)
            .status()
            .expect("Failed to set resolution");
    }

    fn duplicate_displays(&self) {
        let primary = &self.connected_monitors[0];
        for monitor in &self.connected_monitors[1..] {
            Command::new("xrandr")
                .arg("--output")
                .arg(monitor)
                .arg("--same-as")
                .arg(primary)
                .status()
                .expect("Failed to duplicate displays");
        }
    }

    fn extend_displays(&self) {
        let mut prev_monitor = None;
        for monitor in &self.connected_monitors {
            if let Some(prev) = prev_monitor {
                Command::new("xrandr")
                    .arg("--output")
                    .arg(monitor)
                    .arg("--right-of")
                    .arg(prev)
                    .status()
                    .expect("Failed to extend displays");
            }
            prev_monitor = Some(monitor.clone());
        }
    }

    fn auto_detect_displays(&self) {
        Command::new("xrandr")
            .arg("--auto")
            .status()
            .expect("Failed to auto detect displays");

        let common_resolutions = self.get_common_resolutions(&self.connected_monitors);
        if let Some(resolution) = common_resolutions.last() {
            println!("Setting all monitors to {}", resolution);
            for monitor in &self.connected_monitors {
                self.set_resolution(monitor, resolution);
            }
        } else {
            println!("No common resolution found.");
        }
    }

    fn disable_monitor(&self, monitor: &str) {
        Command::new("xrandr")
            .arg("--output")
            .arg(monitor)
            .arg("--off")
            .status()
            .expect("Failed to disable monitor");
    }

    fn enable_monitor(&self, monitor: &str) {
        Command::new("xrandr")
            .arg("--output")
            .arg(monitor)
            .arg("--auto")
            .status()
            .expect("Failed to enable monitor");
    }

    fn draw<B: tui::backend::Backend>(&self, f: &mut tui::Frame<B>) {
        let size = f.size();
    
        // Define layout with two vertical chunks
        let layout_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage(95), // Monitor Control list
                Constraint::Percentage(5), // Instructions
            ].as_ref())
            .split(size);
    
        // Combine monitors, actions, and controls into a single list
        let mut items: Vec<ListItem> = Vec::new();
    
        // Add title for monitors section with light blue color
        items.push(ListItem::new(Spans::from(vec![Span::styled(
            "Monitors:",
            Style::default().fg(Color::LightBlue).add_modifier(Modifier::BOLD),
        )])));
    
        // Add monitors to the list
        items.extend(self.connected_monitors.iter().enumerate().map(|(i, m)| {
            let is_selected = self.selected_index == i;
            let resolution = self.get_current_resolution(m);
            let style = if is_selected {
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            ListItem::new(Spans::from(vec![
                Span::styled(format!("{}: ", m), style),
                Span::raw(resolution)
            ]))
        }));
    
        // Add title for actions section with light blue color
        items.push(ListItem::new(Spans::from(vec![Span::styled(
            "Actions:",
            Style::default().fg(Color::LightBlue).add_modifier(Modifier::BOLD),
        )])));
    
        // Add actions to the list
        let actions = vec![
            "Duplicate Displays (d)",
            "Extend Displays (e)",
            "Auto Detect Displays (a)"
        ];
        items.extend(actions.iter().enumerate().map(|(i, &item)| {
            let is_selected = self.selected_index == self.connected_monitors.len() + i;
            let style = if is_selected {
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            ListItem::new(Spans::from(Span::styled(item, style)))
        }));
    
        // Add title for controls section with light blue color
        items.push(ListItem::new(Spans::from(vec![Span::styled(
            "Controls:",
            Style::default().fg(Color::LightBlue).add_modifier(Modifier::BOLD),
        )])));
    
        // Add controls to the list
        let controls = vec![
            "Enable Primary Monitor (P)",
            "Enable Secondary Monitor (O)",
            "Disable Primary Monitor (p)",
            "Disable Secondary Monitor (o)"
        ];
        items.extend(controls.iter().enumerate().map(|(i, &item)| {
            let is_selected = self.selected_index == self.connected_monitors.len() + actions.len() + i;
            let style = if is_selected {
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            ListItem::new(Spans::from(Span::styled(item, style)))
        }));
    
        // Create the Monitor Control list widget
        let monitor_control_list = List::new(items)
            .block(Block::default().borders(Borders::ALL).title("Monitor Control"))
            .highlight_style(Style::default().add_modifier(Modifier::BOLD));
    
        // Render the Monitor Control list
        f.render_widget(monitor_control_list, layout_chunks[0]);
    
        // Instructions
        let instructions = Paragraph::new("Use Arrow Keys to navigate, Enter to select action, q to quit")
            .style(Style::default().fg(Color::LightCyan))
            .block(Block::default().borders(Borders::ALL).title("Instructions"));
    
        // Render the instructions
        f.render_widget(instructions, layout_chunks[1]);
    }
    
    
    fn get_current_resolution(&self, monitor: &str) -> String {
        let output = Command::new("xrandr")
            .output()
            .expect("Failed to execute xrandr command");
        let output_str = String::from_utf8_lossy(&output.stdout);
        let re = Regex::new(&format!(r"{} connected (primary )?(\d+x\d+)", monitor)).unwrap();
        re.captures(&output_str)
            .and_then(|cap| cap.get(2))
            .map(|m| m.as_str().to_string())
            .unwrap_or_else(|| "Disabled".into())
    }

    fn get_common_resolutions(&self, monitors: &[String]) -> Vec<String> {
        if monitors.is_empty() {
            return vec![];
        }

        let mut common_resolutions = Self::get_resolutions(&monitors[0]);
        for monitor in &monitors[1..] {
            let resolutions = Self::get_resolutions(monitor);
            common_resolutions.retain(|res| resolutions.contains(res));
        }

        common_resolutions.sort_by_key(|res| {
            let mut parts = res.split('x');
            let width = parts.next().unwrap_or("0").parse::<usize>().unwrap_or(0);
            let height = parts.next().unwrap_or("0").parse::<usize>().unwrap_or(0);
            (width, height)
        });

        common_resolutions
    }

    fn handle_input(&mut self, event: Event) -> bool {
        match event {
            Event::Key(key) => match key.code {
                KeyCode::Char('q') => return true, // Quit the application
                
                KeyCode::Up => {
                    if self.selected_index > 0 {
                        self.selected_index -= 1;
                    }
                }
                KeyCode::Down => {
                    if self.selected_index < self.get_total_items() - 1 {
                        self.selected_index += 1;
                    }
                }
                
                KeyCode::Enter => {
                    let index = self.selected_index;
                    if index < self.connected_monitors.len() {
                        // No action on Enter for monitor items
                        self.current_selection = Some(self.connected_monitors[index].clone());
                    } else if index < self.connected_monitors.len() + 3 {
                        // Actions
                        match index {
                            i if i == self.connected_monitors.len() => self.duplicate_displays(), // Duplicate Displays
                            i if i == self.connected_monitors.len() + 1 => self.extend_displays(), // Extend Displays
                            i if i == self.connected_monitors.len() + 2 => self.auto_detect_displays(), // Auto Detect Displays
                            _ => {}
                        }
                    } else if index < self.connected_monitors.len() + 3 + 4 {
                        // Controls
                        match index {
                            i if i == self.connected_monitors.len() + 3 => {
                                if let Some(monitor) = self.connected_monitors.get(0) {
                                    self.enable_monitor(monitor);
                                }
                            }
                            i if i == self.connected_monitors.len() + 4 => {
                                if let Some(monitor) = self.connected_monitors.get(1) {
                                    self.enable_monitor(monitor);
                                }
                            }
                            i if i == self.connected_monitors.len() + 5 => {
                                if let Some(monitor) = self.connected_monitors.get(0) {
                                    self.disable_monitor(monitor);
                                }
                            }
                            i if i == self.connected_monitors.len() + 6 => {
                                if let Some(monitor) = self.connected_monitors.get(1) {
                                    self.disable_monitor(monitor);
                                }
                            }
                            _ => {}
                        }
                    }
                }
    
                _ => {}
            },
            _ => {}
        }
        false
    }
    
    fn get_total_items(&self) -> usize {
        self.connected_monitors.len() + 3 + 4 // monitors + actions + controls
    }
    
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    enable_raw_mode()?;
    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = MonitorManagerApp::new();
    let res = run_app(&mut terminal, &mut app);

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        println!("{:?}", err)
    }

    Ok(())
}

fn run_app<B: tui::backend::Backend>(terminal: &mut Terminal<B>, app: &mut MonitorManagerApp) -> io::Result<()> {
    loop {
        terminal.draw(|f| app.draw(f))?;
        if let Event::Key(key) = event::read()? {
            if app.handle_input(Event::Key(key)) {
                return Ok(());
            }
        }
    }
}
