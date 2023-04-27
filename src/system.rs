use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::ffi::OsStr;
use std::fs;
use std::path::Path;

#[derive(Debug, Eq, PartialEq, Copy, Clone, Serialize, Deserialize)]
pub enum BatteryState {
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
pub struct Battery {
    pub state: BatteryState,
    now_uwh: u64,
    full_uwh: u64,
}

impl Battery {
    pub const fn level(&self) -> u8 {
        let mut level = (self.now_uwh * 100) / self.full_uwh;
        if level > 100 {
            level = 100;
        }
        level as _
    }
}

pub fn read_battery_file(dir: &Path, file: impl AsRef<str>) -> Result<String> {
    let mut content = fs::read_to_string(dir.join(file.as_ref()))?;
    if let Some(idx) = content.find('\n') {
        content.truncate(idx);
    }
    Ok(content)
}

pub fn name_to_battery_state(name: &str) -> BatteryState {
    serde_plain::from_str(name).unwrap()
}

pub fn battery_state_to_name(state: BatteryState) -> String {
    serde_plain::to_string(&state).unwrap()
}

/// Some drivers expose µAh (charge), some drivers expose µWh (energy), some drivers expose both.
pub fn read_battery_file_energy_or_charge(dir: &Path, partial_file: &str) -> Result<u64> {
    let uwh = read_battery_file(dir, "energy_".to_string() + partial_file);
    if uwh.is_ok() {
        return Ok(uwh?.parse()?);
    }

    let voltage: u64 = read_battery_file(dir, "voltage_now")?.parse()?;
    let uah: u64 = read_battery_file(dir, "charge_".to_string() + partial_file)?.parse()?;
    Ok((uah * voltage) / 1000)
}

pub fn read_battery_dir(dir: impl AsRef<Path>) -> Result<Battery> {
    let dir = dir.as_ref();

    Ok(Battery {
        state: name_to_battery_state(&read_battery_file(dir, "status")?),
        now_uwh: read_battery_file_energy_or_charge(dir, "now")?,
        full_uwh: read_battery_file_energy_or_charge(dir, "full")?,
    })
}

pub fn get_batteries() -> Result<Vec<Battery>> {
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

pub fn get_global_battery(batteries: &[Battery]) -> Battery {
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
        BatteryState::AtThreshold
    } else {
        BatteryState::Discharging
    };

    Battery {
        state,
        now_uwh: batteries.iter().map(|b| b.now_uwh).sum(),
        full_uwh: batteries.iter().map(|b| b.full_uwh).sum(),
    }
}
