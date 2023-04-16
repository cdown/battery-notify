use anyhow::{bail, Context, Result};
use notify_rust::{Notification, NotificationHandle};
use serde::{Deserialize, Serialize};
use std::ffi::OsStr;
use std::fs;
use std::path::Path;
use std::thread::sleep;
use std::time::{Duration, Instant};

#[derive(Debug, Eq, PartialEq, Copy, Clone, Serialize, Deserialize)]
enum BatteryState {
    Discharging,
    Charging,
    #[serde(rename = "Not charging")]
    NotCharging,
    Full,
    Unknown,
    Invalid,
}

#[derive(Debug)]
struct Battery {
    state: BatteryState,
    capacity_pct: u8,
}

struct SingleNotification {
    hnd: Option<NotificationHandle>,
}

impl SingleNotification {
    fn new() -> Self {
        Self { hnd: None }
    }

    fn show(&mut self, summary: &str) {
        self.close();
        self.hnd = Notification::new()
            .summary(summary)
            .show()
            .map_err(|err| eprintln!("error showing notification: {}", err))
            .ok();
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

fn main() -> Result<()> {
    let interval = Duration::from_millis(500);
    let mut last_state = BatteryState::Invalid;
    let mut notif = SingleNotification::new();

    loop {
        let start = Instant::now();
        let batteries = get_batteries().context("failed to get list of batteries")?;

        if batteries.is_empty() {
            bail!("no batteries detected");
        }

        let global = get_global_battery(&batteries);
        if global.state != last_state {
            println!("State transition: {last_state:?} -> {:?}", global.state);
            notif.show(&format!(
                "Battery now {}",
                battery_state_to_name(global.state).to_lowercase()
            ));
            last_state = global.state;
        }
        if global.capacity_pct <= 85 {
            println!("Would warn for percentage {}", global.capacity_pct);
        }

        let elapsed = start.elapsed();

        if elapsed < interval {
            sleep(interval - elapsed);
        }
    }
}
