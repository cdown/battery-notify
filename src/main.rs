use anyhow::{bail, Context, Result};
use std::ffi::OsStr;
use std::fs;
use std::path::Path;
use std::thread::sleep;
use std::time::Duration;

#[derive(Debug, Eq, PartialEq, Copy, Clone)]
enum BatteryState {
    Discharging,
    Charging,
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

fn read_battery_file(dir: &Path, file: impl AsRef<str>) -> Result<String> {
    let mut content = fs::read_to_string(dir.join(file.as_ref()))?;
    if let Some(idx) = content.find('\n') {
        content.truncate(idx);
    }
    Ok(content)
}

fn name_to_battery_state(name: &str) -> BatteryState {
    match name {
        "Charging" => BatteryState::Charging,
        "Discharging" => BatteryState::Discharging,
        "Not charging" => BatteryState::NotCharging,
        "Full" => BatteryState::Full,
        _ => BatteryState::Unknown,
    }
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
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(OsStr::to_str)
                .unwrap_or("")
                .starts_with("BAT")
        })
        .map(read_battery_dir)
        .filter_map(|b| b.ok())
        .collect::<Vec<Battery>>())
}

fn get_global_battery(batteries: Vec<Battery>) -> Battery {
    let mut state = BatteryState::Discharging;
    if batteries.iter().any(|b| b.state == BatteryState::Charging) {
        state = BatteryState::Charging;
    }
    if batteries
        .iter()
        .any(|b| b.state == BatteryState::Discharging)
    {
        state = BatteryState::Discharging;
    }
    if batteries.iter().all(|b| b.state == BatteryState::Full) {
        state = BatteryState::Full;
    }
    if batteries.iter().all(|b| {
        b.state == BatteryState::Unknown
            || b.state == BatteryState::NotCharging
            || b.state == BatteryState::Full
    }) {
        // Confusingly some laptops set "Unknown" instead of "Not charging" when at threshold
        state = BatteryState::NotCharging;
    }

    Battery {
        state,
        capacity_pct: batteries.iter().map(|b| b.capacity_pct).sum(),
    }
}

fn main() -> Result<()> {
    let mut last_state = BatteryState::Invalid;

    loop {
        let batteries = get_batteries().context("failed to get list of batteries")?;
        if batteries.is_empty() {
            bail!("no batteries detected");
        }
        let global = get_global_battery(batteries);

        if global.state != last_state {
            println!("State transition: {last_state:?} -> {:?}", global.state);
            last_state = global.state;
        }

        if global.capacity_pct <= 15 {
            println!("Would warn for percentage {}", global.capacity_pct);
        }

        sleep(Duration::from_millis(500));
    }
}
