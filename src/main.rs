use anyhow::Result;
use std::cell::Cell;
use std::ffi::OsStr;
use std::fs;
use std::path::Path;

thread_local! {
    static LAST_BATTERY_STATE: Cell<BatteryState> = Cell::new(BatteryState::Unknown);
}

#[derive(Debug, Eq, PartialEq, Copy, Clone)]
enum BatteryState {
    Discharging,
    Charging,
    NotCharging,
    Full,
    Unknown,
}

#[derive(Debug)]
struct Battery {
    state: BatteryState,

    // Unitless, may either come from charge_* or energy_* since we just use it for percentage
    full: u64,
    now: u64,
}

fn set_last_state(state: BatteryState) {
    LAST_BATTERY_STATE.with(|lbs| lbs.set(state));
}

fn get_last_state() -> BatteryState {
    LAST_BATTERY_STATE.with(Cell::get)
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

fn main() {
    dbg!(get_batteries().unwrap());
}
