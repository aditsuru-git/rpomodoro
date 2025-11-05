# rpomodoro

A terminal-based Pomodoro timer written in Rust.

## Installation

Clone the repository and build from source:

```bash
git clone <repository-url>
cd rpomodoro
cargo build --release
```

## Usage

Run the timer:

```bash
cargo run --release
```

Or install globally:

```bash
cargo install --path .
rpomodoro
```

## Controls

- `space` - Start/pause timer
- `r` - Reset session
- `s` - Skip to next phase
- `c` - Open configuration
- `q` - Quit

## Configuration

Configuration is stored in `~/.config/rpomodoro/config.json` (Linux/macOS) or `%APPDATA%\rpomodoro\config.json` (Windows).

Available settings:
- Theme (blue, purple, green, red, orange, cyan)
- Work duration (minutes)
- Short break duration (minutes)
- Long break duration (minutes)
- Number of cycles before long break

Navigate with `j/k`, change values with `h/l`, save with `q` or `esc`.

## Requirements

- Rust 1.70 or higher
- A terminal with Unicode support
