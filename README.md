# battery-notify

battery-notify is a small, Linux-only program that sends notifications on
changes to system battery state.

## Installation

    cargo install battery-notify

## Features

- Small, easy to understand codebase
- Notifications on battery state change
- Works with multiple batteries
- Warnings on low/critical battery percentages
- Warnings when connected to a monitor but not mains power
- Ability to sleep the computer with a custom command on critical percentage

## Usage

Run `battery-notify`. You'll also need a notification daemon capable of
disabling [Desktop Notifications][], like
[dunst](https://github.com/dunst-project/dunst) or similar.

## Configuration

You can configure battery-notify at `~/.config/battery-notify/config.toml` --
on first run, this will be populated with a basic config if it doesn't exist.

The default config is:

```toml

# How often to check battery status, in seconds.
interval_secs = 30

# At what percentage of battery capacity to notify about low battery.
low_pct = 40

# At what percentage of battery capacity to notify and run sleep_command.
sleep_pct = 15

# The command to run when sleeping. Bear in mind that if you run as an
# unprivileged user, you'll likely need to consider elevation, either with
# NOPASSWD or things like polkit.
sleep_command = 'printf mem > /sys/class/power'

# If this many monitors are connected (that is, plugged in -- they can be off)
# and we are discharging, show a warning.
warn_on_mons_with_no_ac = 2
```

[Desktop Notifications]: https://specifications.freedesktop.org/notification-spec/latest/
