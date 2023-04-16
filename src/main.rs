use anyhow::{bail, Context, Result};
use notify_rust::{Notification, NotificationHandle, Urgency};
use serde::{Deserialize, Serialize};
use std::ffi::OsStr;
use std::fs;
use std::path::Path;
use std::process::Command;
use std::thread::sleep;
use std::time::{Duration, Instant};
use x11rb::connection::Connection;
use x11rb::protocol::randr;

#[derive(Debug, Eq, PartialEq, Copy, Clone, Serialize, Deserialize)]
enum BatteryState {
    Discharging,
    Charging,
    #[serde(rename = "Not charging")]
    NotCharging,
    Full,
    Unknown,

    // These are internal values -- they never come from sysfs
    #[serde(rename = "At threshold")]
    AtThreshold,
    Invalid,
}

#[derive(Debug)]
struct Battery {
    state: BatteryState,
    capacity_pct: u8,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(default)]
struct Config {
    sleep_command: String,
    interval_secs: u64,
    sleep_pct: u8,
    low_pct: u8,
    warn_on_mons_with_no_ac: usize,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            sleep_command: "printf mem > /sys/class/power".to_string(),
            interval_secs: 30,
            sleep_pct: 15,
            low_pct: 40,
            warn_on_mons_with_no_ac: 2,
        }
    }
}

struct SingleNotification {
    hnd: Option<NotificationHandle>,
    summary: String,
}

impl SingleNotification {
    const fn new() -> Self {
        Self {
            hnd: None,
            summary: String::new(),
        }
    }

    fn show(&mut self, summary: String, urgency: Urgency) {
        if self.summary != summary {
            self.close();
            self.summary = summary;
            self.hnd = Notification::new()
                .summary(&self.summary)
                .urgency(urgency)
                .show()
                .map_err(|err| eprintln!("error showing notification: {err}"))
                .ok();
        }
    }

    fn close(&mut self) {
        if let Some(hnd) = self.hnd.take() {
            hnd.close();
        }
    }
}

fn read_battery_file(dir: &Path, file: impl AsRef<str>) -> Result<String> {
    let mut content = fs::read_to_string(dir.join(file.as_ref()))?;
    if let Some(idx) = content.find('\n') {
        content.truncate(idx);
    }
    Ok(content)
}

fn name_to_battery_state(name: &str) -> BatteryState {
    serde_plain::from_str(name).unwrap()
}

fn battery_state_to_name(state: BatteryState) -> String {
    serde_plain::to_string(&state).unwrap()
}

fn read_battery_dir(dir: impl AsRef<Path>) -> Result<Battery> {
    let dir = dir.as_ref();
    Ok(Battery {
        state: name_to_battery_state(&read_battery_file(dir, "status")?),
        capacity_pct: read_battery_file(dir, "capacity")?.parse()?,
    })
}

fn get_nr_connected_monitors() -> Result<usize> {
    let (conn, screen_num) = x11rb::connect(None)?;
    let root = conn.setup().roots[screen_num].root;
    let resources = randr::get_screen_resources(&conn, root)?;

    let mut nr_connected_monitors = 0;
    for output in resources.reply()?.outputs {
        let output_info = randr::get_output_info(&conn, output, 0)?.reply()?;
        if output_info.connection == randr::Connection::CONNECTED {
            nr_connected_monitors += 1;
        }
    }
    Ok(nr_connected_monitors)
}

fn get_batteries() -> Result<Vec<Battery>> {
    Ok(fs::read_dir("/sys/class/power_supply")?
        .filter_map(std::result::Result::ok)
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(OsStr::to_str)
                .unwrap_or("")
                .starts_with("BAT")
        })
        .map(read_battery_dir)
        .filter_map(std::result::Result::ok)
        .collect::<Vec<Battery>>())
}

fn get_global_battery(batteries: &[Battery]) -> Battery {
    let state = if batteries.iter().any(|b| b.state == BatteryState::Charging) {
        BatteryState::Charging
    } else if batteries
        .iter()
        .any(|b| b.state == BatteryState::Discharging)
    {
        BatteryState::Discharging
    } else if batteries.iter().all(|b| b.state == BatteryState::Full) {
        BatteryState::Full
    } else if batteries.iter().all(|b| {
        b.state == BatteryState::Unknown
            || b.state == BatteryState::NotCharging
            || b.state == BatteryState::Full
    }) {
        // Confusingly some laptops set "Unknown" instead of "Not charging" when at threshold
        BatteryState::NotCharging
    } else {
        BatteryState::Discharging
    };

    Battery {
        state,
        capacity_pct: batteries.iter().map(|b| b.capacity_pct).sum(),
    }
}

fn mem_sleep(cmd: &str) {
    if let Err(err) = Command::new("sh").args(["-c", cmd]).status() {
        eprintln!("Failed to run sleep command '{cmd}': {err}");
    }
}

fn main() -> Result<()> {
    let cfg: Config = confy::load("battery-notify", "config")?;
    let interval = Duration::from_secs(cfg.interval_secs);
    let mut last_state = BatteryState::Invalid;
    let mut state_notif = SingleNotification::new();
    let mut low_notif = SingleNotification::new();
    let mut mon_notif = SingleNotification::new();
    let sleep_backoff = Duration::from_secs(60);
    let mut last_sleep_epoch = Instant::now() - sleep_backoff;

    println!(
        "Config (configurable at {}):\n\n{:#?}\n",
        confy::get_configuration_file_path("battery-notify", "config")?.display(),
        cfg
    );

    loop {
        let start = Instant::now();
        let batteries = get_batteries().context("failed to get list of batteries")?;

        if batteries.is_empty() {
            bail!("no batteries detected");
        }

        let global = get_global_battery(&batteries);
        if global.state != last_state {
            let state = if global.state == BatteryState::NotCharging {
                // "not charging" is somewhat confusing, it just means we hit charging thresh
                BatteryState::AtThreshold
            } else {
                global.state
            };
            state_notif.show(
                format!(
                    "Battery now {}",
                    battery_state_to_name(state).to_lowercase()
                ),
                Urgency::Normal,
            );
            last_state = global.state;
        }

        if global.state == BatteryState::Charging {
            low_notif.close();
        } else if global.capacity_pct <= cfg.sleep_pct {
            low_notif.show("Battery critical".to_string(), Urgency::Critical);
            // Just in case we've gone loco, don't do this more than once a minute
            if last_sleep_epoch < start - sleep_backoff {
                last_sleep_epoch = start;
                mem_sleep(&cfg.sleep_command);
            }
        } else if global.capacity_pct <= cfg.low_pct {
            low_notif.show("Battery low".to_string(), Urgency::Critical);
        }

        if cfg.warn_on_mons_with_no_ac > 0
            && global.state == BatteryState::Discharging
            && get_nr_connected_monitors().unwrap_or(0) >= cfg.warn_on_mons_with_no_ac
        {
            mon_notif.show(
                format!(
                    "Connected to {} monitors but not AC",
                    cfg.warn_on_mons_with_no_ac
                ),
                Urgency::Critical,
            );
        } else {
            mon_notif.close();
        }

        let elapsed = start.elapsed();

        if elapsed < interval {
            sleep(interval - elapsed);
        }
    }
}
