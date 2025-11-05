use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    execute,
    style::{Color, Print, ResetColor, SetForegroundColor},
    terminal::{self, Clear, ClearType},
};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Config {
    theme: String,
    work_duration: u32,
    short_break: u32,
    long_break: u32,
    cycles_before_long: u32,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            theme: "blue".to_string(),
            work_duration: 25,
            short_break: 5,
            long_break: 15,
            cycles_before_long: 4,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum PomodoroState {
    Work,
    ShortBreak,
    LongBreak,
}

#[derive(Debug, Clone, Copy)]
struct Theme {
    primary: Color,
    dim: Color,
}

impl Theme {
    fn from_name(name: &str) -> Self {
        match name {
            "blue" => Theme {
                primary: Color::Rgb { r: 96, g: 165, b: 250 },
                dim: Color::Rgb { r: 147, g: 197, b: 253 },
            },
            "purple" => Theme {
                primary: Color::Rgb { r: 192, g: 132, b: 252 },
                dim: Color::Rgb { r: 233, g: 213, b: 255 },
            },
            "green" => Theme {
                primary: Color::Rgb { r: 74, g: 222, b: 128 },
                dim: Color::Rgb { r: 134, g: 239, b: 172 },
            },
            "red" => Theme {
                primary: Color::Rgb { r: 248, g: 113, b: 113 },
                dim: Color::Rgb { r: 254, g: 202, b: 202 },
            },
            "orange" => Theme {
                primary: Color::Rgb { r: 251, g: 191, b: 36 },
                dim: Color::Rgb { r: 253, g: 224, b: 71 },
            },
            "cyan" => Theme {
                primary: Color::Rgb { r: 34, g: 211, b: 238 },
                dim: Color::Rgb { r: 103, g: 232, b: 249 },
            },
            _ => Theme::from_name("blue"),
        }
    }
}

// tty-clock style: 3x5 matrix, each cell is 2 chars wide
// This matches the exact tty-clock implementation
const DIGITS: [[[bool; 3]; 5]; 10] = [
    // 0
    [[true, true, true], [true, false, true], [true, false, true], [true, false, true], [true, true, true]],
    // 1
    [[false, false, true], [false, false, true], [false, false, true], [false, false, true], [false, false, true]],
    // 2
    [[true, true, true], [false, false, true], [true, true, true], [true, false, false], [true, true, true]],
    // 3
    [[true, true, true], [false, false, true], [true, true, true], [false, false, true], [true, true, true]],
    // 4
    [[true, false, true], [true, false, true], [true, true, true], [false, false, true], [false, false, true]],
    // 5
    [[true, true, true], [true, false, false], [true, true, true], [false, false, true], [true, true, true]],
    // 6
    [[true, true, true], [true, false, false], [true, true, true], [true, false, true], [true, true, true]],
    // 7
    [[true, true, true], [false, false, true], [false, false, true], [false, false, true], [false, false, true]],
    // 8
    [[true, true, true], [true, false, true], [true, true, true], [true, false, true], [true, true, true]],
    // 9
    [[true, true, true], [true, false, true], [true, true, true], [false, false, true], [true, true, true]],
];

struct App {
    config: Config,
    config_path: PathBuf,
    state: PomodoroState,
    cycle_count: u32,
    time_remaining: Duration,
    last_tick: Instant,
    paused: bool,
    theme: Theme,
    width: u16,
    height: u16,
    config_mode: bool,
    config_cursor: usize,
}

impl App {
    fn new() -> io::Result<Self> {
        let config_dir = dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("rpomodoro");
        
        fs::create_dir_all(&config_dir)?;
        let config_path = config_dir.join("config.json");
        
        let config = if config_path.exists() {
            let content = fs::read_to_string(&config_path)?;
            serde_json::from_str(&content).unwrap_or_default()
        } else {
            let default = Config::default();
            let json = serde_json::to_string_pretty(&default)?;
            fs::write(&config_path, json)?;
            default
        };

        let theme = Theme::from_name(&config.theme);
        let (width, height) = terminal::size()?;
        
        Ok(App {
            time_remaining: Duration::from_secs(config.work_duration as u64 * 60),
            config,
            config_path,
            state: PomodoroState::Work,
            cycle_count: 0,
            last_tick: Instant::now(),
            paused: true,
            theme,
            width,
            height,
            config_mode: false,
            config_cursor: 0,
        })
    }

    fn save_config(&self) -> io::Result<()> {
        let json = serde_json::to_string_pretty(&self.config)?;
        fs::write(&self.config_path, json)?;
        Ok(())
    }

    fn update(&mut self) {
        if !self.paused {
            let now = Instant::now();
            let elapsed = now.duration_since(self.last_tick);
            self.last_tick = now;

            if let Some(new_remaining) = self.time_remaining.checked_sub(elapsed) {
                self.time_remaining = new_remaining;
            } else {
                self.time_remaining = Duration::ZERO;
                self.advance_state();
            }
        }
    }

    fn advance_state(&mut self) {
        match self.state {
            PomodoroState::Work => {
                self.cycle_count += 1;
                if self.cycle_count >= self.config.cycles_before_long {
                    self.state = PomodoroState::LongBreak;
                    self.time_remaining = Duration::from_secs(self.config.long_break as u64 * 60);
                    self.cycle_count = 0;
                } else {
                    self.state = PomodoroState::ShortBreak;
                    self.time_remaining = Duration::from_secs(self.config.short_break as u64 * 60);
                }
            }
            PomodoroState::ShortBreak | PomodoroState::LongBreak => {
                self.state = PomodoroState::Work;
                self.time_remaining = Duration::from_secs(self.config.work_duration as u64 * 60);
            }
        }
        self.paused = true;
    }

    fn draw(&self) -> io::Result<()> {
        let mut stdout = io::stdout();
        execute!(stdout, Clear(ClearType::All))?;

        let center_x = self.width / 2;
        let center_y = self.height / 2;

        // Draw large clock
        self.draw_clock(center_x, center_y.saturating_sub(3))?;

        // Draw minimal status bar at bottom
        self.draw_statusline()?;

        stdout.flush()?;
        Ok(())
    }

    fn draw_digit(&self, digit: usize, x: u16, y: u16) -> io::Result<()> {
        let mut stdout = io::stdout();
        
        for row in 0..5 {
            execute!(stdout, cursor::MoveTo(x, y + row as u16))?;
            for col in 0..3 {
                if DIGITS[digit][row as usize][col] {
                    execute!(stdout, SetForegroundColor(self.theme.primary))?;
                    print!("██");
                } else {
                    print!("  ");
                }
            }
        }
        
        execute!(stdout, ResetColor)?;
        Ok(())
    }

    fn draw_colon(&self, x: u16, y: u16) -> io::Result<()> {
        let mut stdout = io::stdout();
        
        execute!(stdout, SetForegroundColor(self.theme.primary))?;
        
        execute!(stdout, cursor::MoveTo(x, y + 1))?;
        print!("██");
        execute!(stdout, cursor::MoveTo(x, y + 3))?;
        print!("██");
        
        execute!(stdout, ResetColor)?;
        Ok(())
    }

    fn draw_clock(&self, center_x: u16, y: u16) -> io::Result<()> {
        let total_secs = self.time_remaining.as_secs();
        let mins = total_secs / 60;
        let secs = total_secs % 60;

        let digit1 = (mins / 10) as usize;
        let digit2 = (mins % 10) as usize;
        let digit3 = (secs / 10) as usize;
        let digit4 = (secs % 10) as usize;

        // Each digit is 6 chars wide (3 cols * 2 chars)
        // Add 2 char spacing between digit pairs = 2 chars
        // Colon is 2 chars, with 2 char spacing on each side = 6 chars total
        // Total: 6 + 2 + 6 + 6 + 6 + 2 + 6 = 34 chars
        let total_width = 34;
        let start_x = center_x.saturating_sub(total_width / 2);

        // Draw minutes
        self.draw_digit(digit1, start_x, y)?;
        self.draw_digit(digit2, start_x + 8, y)?;  // 6 + 2 spacing
        
        // Draw colon
        self.draw_colon(start_x + 16, y)?;
        
        // Draw seconds
        self.draw_digit(digit3, start_x + 20, y)?;
        self.draw_digit(digit4, start_x + 28, y)?;  // 6 + 2 spacing

        Ok(())
    }

    fn draw_statusline(&self) -> io::Result<()> {
        let mut stdout = io::stdout();
        let y = self.height - 1;

        // Clear the line first
        execute!(stdout, cursor::MoveTo(0, y))?;
        print!("{}", " ".repeat(self.width as usize));

        // Left side - mode indicator (lowercase, clean)
        let mode = match self.state {
            PomodoroState::Work => "work",
            PomodoroState::ShortBreak => "break",
            PomodoroState::LongBreak => "long break",
        };
        
        let status = if self.paused { "paused" } else { "running" };
        let left_side = format!(" {} | {} ", mode, status);

        // Center - cycle info
        let cycles = format!("cycles: {}/{}", self.cycle_count, self.config.cycles_before_long);

        // Right side - keybindings (lowercase, vim-style)
        let right_side = " space:start/pause  r:reset  s:skip  c:config  q:quit ";

        execute!(
            stdout,
            cursor::MoveTo(0, y),
            SetForegroundColor(self.theme.primary)
        )?;
        print!("{}", left_side);

        let center_x = (self.width / 2).saturating_sub((cycles.len() / 2) as u16);
        execute!(stdout, cursor::MoveTo(center_x, y), SetForegroundColor(self.theme.dim))?;
        print!("{}", cycles);

        let right_x = self.width.saturating_sub(right_side.len() as u16);
        execute!(stdout, cursor::MoveTo(right_x, y), SetForegroundColor(self.theme.dim))?;
        print!("{}", right_side);

        execute!(stdout, ResetColor)?;
        Ok(())
    }

    fn draw_config(&self) -> io::Result<()> {
        let mut stdout = io::stdout();
        execute!(stdout, Clear(ClearType::All))?;

        let center_x = self.width / 2;
        let start_y = self.height / 2 - 10;

        let configs = [
            ("theme", self.config.theme.clone()),
            ("work_duration", format!("{}", self.config.work_duration)),
            ("short_break", format!("{}", self.config.short_break)),
            ("long_break", format!("{}", self.config.long_break)),
            ("cycles_before_long", format!("{}", self.config.cycles_before_long)),
        ];

        for (i, (label, value)) in configs.iter().enumerate() {
            let y = start_y + i as u16 * 2;
            let is_selected = i == self.config_cursor;
            
            let color = if is_selected { self.theme.primary } else { self.theme.dim };
            let pointer = if is_selected { "> " } else { "  " };
            
            let line = format!("{}{}: {}", pointer, label, value);
            let x = center_x.saturating_sub((line.len() / 2) as u16);
            
            execute!(
                stdout,
                cursor::MoveTo(x, y),
                SetForegroundColor(color),
                Print(&line),
                ResetColor
            )?;
        }

        // Statusline for config mode
        let y = self.height - 1;
        execute!(stdout, cursor::MoveTo(0, y))?;
        print!("{}", " ".repeat(self.width as usize));
        
        let help = " config | j/k:navigate  h/l:change  q/esc:save&exit ";
        let help_x = (self.width / 2).saturating_sub((help.len() / 2) as u16);
        execute!(
            stdout,
            cursor::MoveTo(help_x, y),
            SetForegroundColor(self.theme.primary),
            Print(help),
            ResetColor
        )?;

        stdout.flush()?;
        Ok(())
    }

    fn handle_config_input(&mut self, key: KeyEvent) -> io::Result<()> {
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => {
                self.config_mode = false;
                self.save_config()?;
                self.theme = Theme::from_name(&self.config.theme);
            }
            KeyCode::Char('j') | KeyCode::Down => {
                self.config_cursor = (self.config_cursor + 1).min(4);
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.config_cursor = self.config_cursor.saturating_sub(1);
            }
            KeyCode::Char('h') | KeyCode::Left => {
                match self.config_cursor {
                    0 => {
                        let themes = ["blue", "purple", "green", "red", "orange", "cyan"];
                        if let Some(pos) = themes.iter().position(|&t| t == self.config.theme) {
                            let new_pos = if pos == 0 { themes.len() - 1 } else { pos - 1 };
                            self.config.theme = themes[new_pos].to_string();
                            self.theme = Theme::from_name(&self.config.theme);
                        }
                    }
                    1 => self.config.work_duration = self.config.work_duration.saturating_sub(1).max(1),
                    2 => self.config.short_break = self.config.short_break.saturating_sub(1).max(1),
                    3 => self.config.long_break = self.config.long_break.saturating_sub(1).max(1),
                    4 => self.config.cycles_before_long = self.config.cycles_before_long.saturating_sub(1).max(1),
                    _ => {}
                }
            }
            KeyCode::Char('l') | KeyCode::Right => {
                match self.config_cursor {
                    0 => {
                        let themes = ["blue", "purple", "green", "red", "orange", "cyan"];
                        if let Some(pos) = themes.iter().position(|&t| t == self.config.theme) {
                            let new_pos = (pos + 1) % themes.len();
                            self.config.theme = themes[new_pos].to_string();
                            self.theme = Theme::from_name(&self.config.theme);
                        }
                    }
                    1 => self.config.work_duration = (self.config.work_duration + 1).min(120),
                    2 => self.config.short_break = (self.config.short_break + 1).min(60),
                    3 => self.config.long_break = (self.config.long_break + 1).min(120),
                    4 => self.config.cycles_before_long = (self.config.cycles_before_long + 1).min(10),
                    _ => {}
                }
            }
            _ => {}
        }
        Ok(())
    }
}

fn main() -> io::Result<()> {
    let mut app = App::new()?;
    
    terminal::enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, terminal::EnterAlternateScreen, cursor::Hide)?;

    let result = run_app(&mut app);

    execute!(stdout, terminal::LeaveAlternateScreen, cursor::Show)?;
    terminal::disable_raw_mode()?;

    result
}

fn run_app(app: &mut App) -> io::Result<()> {
    loop {
        if app.config_mode {
            app.draw_config()?;
        } else {
            app.update();
            app.draw()?;
        }

        if event::poll(Duration::from_millis(50))? {
            match event::read()? {
                Event::Key(key) => {
                    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
                        break;
                    }

                    if app.config_mode {
                        app.handle_config_input(key)?;
                    } else {
                        match key.code {
                            KeyCode::Char('q') | KeyCode::Char('Q') => break,
                            KeyCode::Char(' ') => {
                                app.paused = !app.paused;
                                if !app.paused {
                                    app.last_tick = Instant::now();
                                }
                            }
                            KeyCode::Char('r') | KeyCode::Char('R') => {
                                app.paused = true;
                                app.cycle_count = 0;
                                app.state = PomodoroState::Work;
                                app.time_remaining = Duration::from_secs(app.config.work_duration as u64 * 60);
                            }
                            KeyCode::Char('s') | KeyCode::Char('S') => {
                                app.advance_state();
                            }
                            KeyCode::Char('c') | KeyCode::Char('C') => {
                                app.config_mode = true;
                            }
                            _ => {}
                        }
                    }
                }
                Event::Resize(w, h) => {
                    app.width = w;
                    app.height = h;
                }
                _ => {}
            }
        }
    }

    Ok(())
}
