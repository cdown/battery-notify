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

    // Unitless, may either come from charge_* or energy_* since we just use it for percentage
    full: u64,
    now: u64,
}

fn read_battery_file(dir: &Path, file: impl AsRef<str>) -> Result<String> {
    let mut content = fs::read_to_string(dir.join(file.as_ref()))?;
    if let Some(idx) = content.find('\n') {
        content.truncate(idx);
    }
    Ok(content)
}

/// Some drivers expose coloumb counter (charge), some drivers expose µWh (energy), some drivers
/// expose both. We only care about the percentage.
fn read_battery_file_energy_or_charge(dir: &Path, partial_file: &str) -> Result<String> {
    let energy = read_battery_file(dir, "energy".to_owned() + partial_file);
    if energy.is_ok() {
        return energy;
    }
    read_battery_file(dir, "charge".to_owned() + partial_file)
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
        full: read_battery_file_energy_or_charge(dir, "_full")?.parse()?,
        now: read_battery_file_energy_or_charge(dir, "_now")?.parse()?,
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

fn get_state(batteries: Vec<Battery>) -> BatteryState {
    if batteries.iter().any(|b| b.state == BatteryState::Charging) {
        return BatteryState::Charging;
    }
    if batteries
        .iter()
        .any(|b| b.state == BatteryState::Discharging)
    {
        return BatteryState::Discharging;
    }
    if batteries.iter().all(|b| b.state == BatteryState::Full) {
        return BatteryState::Full;
    }
    if batteries.iter().all(|b| {
        b.state == BatteryState::Unknown
            || b.state == BatteryState::NotCharging
            || b.state == BatteryState::Full
    }) {
        // Confusingly some laptops set "Unknown" instead of "Not charging" when at threshold
        return BatteryState::NotCharging;
    }
    BatteryState::Discharging
}

fn main() -> Result<()> {
    let mut last_state = BatteryState::Invalid;

    loop {
        let batteries = get_batteries().context("failed to get list of batteries")?;
        if batteries.is_empty() {
            bail!("no batteries detected");
        }
        let state = get_state(batteries);

        if state != last_state {
            println!("State transition: {last_state:?} -> {state:?}");
            last_state = state;
        }

        sleep(Duration::from_millis(500));
    }
}
