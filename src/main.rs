use anyhow::Result;
use std::cell::Cell;
use std::ffi::OsStr;
use std::fs;
use std::path::PathBuf;

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
}

fn set_last_state(state: BatteryState) {
    LAST_BATTERY_STATE.with(|lbs| lbs.set(state));
}

fn get_last_state() -> BatteryState {
    LAST_BATTERY_STATE.with(Cell::get)
}

fn read_battery_file(dir: PathBuf, file: &str) -> Result<String> {
    let mut content = fs::read_to_string(dir.join(file))?;
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

fn read_battery_dir(dir: PathBuf) -> Result<Battery> {
    Ok(Battery {
        state: name_to_battery_state(&read_battery_file(dir, "status")?),
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
