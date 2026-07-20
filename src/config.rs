use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub sleep_command: String,
    pub interval_secs: u64,
    pub sleep_pct: u8,
    pub low_pct: u8,
    pub warn_on_mons_with_no_ac: usize,
    pub bluetooth_low_pct: u8,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            sleep_command: "systemctl suspend".to_string(),
            interval_secs: 30,
            sleep_pct: 15,
            low_pct: 40,
            warn_on_mons_with_no_ac: 2,
            bluetooth_low_pct: 40,
        }
    }
}
