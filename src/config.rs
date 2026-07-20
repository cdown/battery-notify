use anyhow::{bail, Context, Result};
use log::warn;
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub sleep_command: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub interval_secs: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub interval: Option<String>,
    pub sleep_pct: u8,
    pub low_pct: u8,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub monitors_with_no_ac: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub warn_on_mons_with_no_ac: Option<usize>,
    pub bluetooth_low_pct: u8,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            sleep_command: "systemctl suspend".to_string(),
            interval_secs: None,
            interval: Some("30s".to_string()),
            sleep_pct: 15,
            low_pct: 40,
            monitors_with_no_ac: Some(2),
            warn_on_mons_with_no_ac: None,
            bluetooth_low_pct: 40,
        }
    }
}

impl Config {
    /// Validate configuration, apply backward compatibility deprecations, and return the
    /// resolved `(interval, monitors_with_no_ac)`.
    pub fn validate(&mut self) -> Result<(Duration, usize)> {
        // --- interval ---
        let interval = match (self.interval_secs, &self.interval) {
            (Some(_), Some(_)) => {
                bail!(
                    "cannot specify both 'interval_secs' and 'interval'. \
                     Use 'interval' only (e.g. interval = \"30s\")"
                )
            }
            (Some(secs), None) => {
                warn!(
                    "'interval_secs' is deprecated, use 'interval' instead \
                     (e.g. interval = \"30s\")"
                );
                Duration::from_secs(secs)
            }
            (None, Some(s)) => humantime::parse_duration(s)
                .with_context(|| format!("invalid value for 'interval': '{s}'"))?,
            (None, None) => Duration::from_secs(30),
        };

        // --- monitors_with_no_ac ---
        let monitors = match (self.warn_on_mons_with_no_ac, self.monitors_with_no_ac) {
            (Some(_), Some(_)) => {
                bail!(
                    "cannot specify both 'warn_on_mons_with_no_ac' and 'monitors_with_no_ac'. \
                     Use 'monitors_with_no_ac' only"
                )
            }
            (Some(n), None) => {
                warn!(
                    "'warn_on_mons_with_no_ac' is deprecated, use 'monitors_with_no_ac' instead"
                );
                n
            }
            (None, Some(n)) => n,
            (None, None) => 2,
        };

        Ok((interval, monitors))
    }
}
