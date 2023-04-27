# battery-notify | [![Tests](https://img.shields.io/github/actions/workflow/status/cdown/battery-notify/ci.yml?branch=master)](https://github.com/cdown/battery-notify/actions?query=branch%3Amaster)

battery-notify is a small, Linux-only program that sends notifications on
changes to system or Bluetooth battery state.

## Features

- Small, easy to understand codebase
- Notifications on battery state change
- Bluetooth battery support
- Works with multiple system batteries
- Warnings on low/critical battery percentages
- Warnings when connected to an external monitor but not mains power (X11 only)
- Ability to sleep the computer with a custom command on critical percentage

## Installation

    cargo install battery-notify

Default features:

- `mons`: Support `warn_on_mons_with_no_ac`. Adds a dependency on the x11rb
  crate.
- `bluetooth`: Support `bluetooth_low_pct`. Adds a dependency on the zbus
  crate. You will also need to run `bluetoothd` with the `--experimental` flag
  to expose battery information.

If you don't want to use some subset of these features, you can pass
`--no-default-features` and select the ones you do want with `--feature`.

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
# unprivileged user, you may need to consider elevation, either with NOPASSWD
# or things like polkit.
sleep_command = 'systemctl suspend'

# If this many monitors are connected (that is, plugged in -- they can be off)
# and we are discharging, show a warning. Intended to avoid cases where power
# is inadvertently disconnected at a desk.
#
# Set to 0 to disable.
warn_on_mons_with_no_ac = 2

# If a bluetooth device is below this percentage, notify about low battery.
# Note that you need to run bluetoothd with --experimental in order for it to
# expose battery information.
#
# Set to 0 to disable.
bluetooth_low_pct = 40
```

## Output

If you don't like the output, you can disable logging with `RUST_LOG=none`.

[Desktop Notifications]: https://specifications.freedesktop.org/notification-spec/latest/
