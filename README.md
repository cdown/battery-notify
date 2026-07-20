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
- Custom commands on any battery state change

## Installation

    cargo install battery-notify

Default features:

- `mons`: Support `monitors_with_no_ac`. Adds a dependency on the x11rb
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
# How often to check battery status. Uses humantime format: "30s", "500ms", "1min", etc.
interval = "30s"

# At what percentage of battery capacity to notify about low battery. Set to 0 to disable.
low_pct = 40

# At what percentage of battery capacity to notify and run the sleep event. Set to 0 to disable.
sleep_pct = 15

# If a bluetooth device is below this percentage, notify about low battery.
# Note that you need to run bluetoothd with --experimental in order for it to
# expose battery information.
#
# Set to 0 to disable.
bluetooth_low_pct = 40

# If this many monitors are connected (that is, plugged in -- they can be off)
# and we are discharging, show a warning. Intended to avoid cases where power
# is inadvertently disconnected at a desk.
#
# Set to 0 to disable.
monitors_with_no_ac = 2

# Custom commands to be run once per battery state change or threshold crossing.
# Commands are run with $SHELL -c (falls back to sh). Set to "" or omit to disable.
[events]
sleep = "systemctl suspend"

# Set an expiry timeout for notifications or disable them.
#
# Allowed values:
#   persistent: The notification will never expire
#   server-default: Use your notification server's default timeout
#   disabled: Disables the particular notification
#   non-zero positive integers: Timeout in milliseconds
[notifications]
low = "persistent"
sleep = "persistent"
bluetooth_low = "persistent"
monitors_with_no_ac = "persistent"
charging = "server-default"
discharging = "server-default"
not_charging = "persistent"
full = "persistent"
unknown = "persistent"
at_threshold = "persistent"
```

### Migrating from older versions

The following config keys are deprecated and will be removed in a future
release:

| Old key | New key |
|---|---|
| `interval_secs` | `interval = "30s"` (humantime format) |
| `sleep_command` | `events.sleep` |
| `warn_on_mons_with_no_ac` | `monitors_with_no_ac` |

If both old and new keys are present, battery-notify will refuse to start.

## Output

If you don't like the output, you can disable logging with `RUST_LOG=none`.

[Desktop Notifications]: https://specifications.freedesktop.org/notification-spec/latest/
