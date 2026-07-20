use anyhow::{bail, Context, Result};
use log::warn;
use serde::de::{self, Unexpected, Visitor};
use serde::{Deserialize, Deserializer, Serialize};
use std::fmt;
use std::time::Duration;

#[derive(Debug, Serialize, PartialEq, Eq)]
pub enum Notification {
    #[serde(rename = "persistent")]
    Persistent,
    #[serde(rename = "server-default")]
    ServerDefault,
    #[serde(rename = "disabled")]
    Disabled,
    Int(i32),
}

impl Notification {
    pub fn to_i32(&self) -> i32 {
        match self {
            Notification::Persistent => 0,
            Notification::ServerDefault => -1,
            Notification::Disabled => -1, // unreachable: callers check Disabled before calling to_i32
            Notification::Int(value) => *value,
        }
    }
}

impl<'de> Deserialize<'de> for Notification {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct NotificationVisitor;

        impl<'de> Visitor<'de> for NotificationVisitor {
            type Value = Notification;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str(
                    "a non-zero positive integer or a string: 'persistent', 'server-default', or 'disabled'",
                )
            }

            fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                if value > 0 && value <= i32::MAX as i64 {
                    Ok(Notification::Int(value as i32))
                } else {
                    Err(E::invalid_value(Unexpected::Signed(value), &self))
                }
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                match value {
                    "persistent" => Ok(Notification::Persistent),
                    "server-default" => Ok(Notification::ServerDefault),
                    "disabled" => Ok(Notification::Disabled),
                    _ => Err(E::invalid_value(Unexpected::Str(value), &self)),
                }
            }
        }

        deserializer.deserialize_any(NotificationVisitor)
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Notifications {
    pub low: Notification,
    pub sleep: Notification,
    pub bluetooth_low: Notification,
    pub monitors_with_no_ac: Notification,
    pub charging: Notification,
    pub discharging: Notification,
    pub not_charging: Notification,
    pub full: Notification,
    pub unknown: Notification,
    pub at_threshold: Notification,
}

impl Default for Notifications {
    fn default() -> Self {
        Self {
            low: Notification::Persistent,
            sleep: Notification::Persistent,
            bluetooth_low: Notification::Persistent,
            monitors_with_no_ac: Notification::Persistent,
            charging: Notification::ServerDefault,
            discharging: Notification::ServerDefault,
            not_charging: Notification::Persistent,
            full: Notification::Persistent,
            unknown: Notification::Persistent,
            at_threshold: Notification::Persistent,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct CustomCommands {
    pub low: Option<String>,
    pub sleep: Option<String>,
    pub charging: Option<String>,
    pub discharging: Option<String>,
    pub not_charging: Option<String>,
    pub full: Option<String>,
    pub unknown: Option<String>,
    pub at_threshold: Option<String>,
}

impl Default for CustomCommands {
    fn default() -> Self {
        Self {
            low: None,
            sleep: Some("systemctl suspend".to_string()),
            charging: None,
            discharging: None,
            not_charging: None,
            full: None,
            unknown: None,
            at_threshold: None,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
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
    pub events: CustomCommands,
    pub notifications: Notifications,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sleep_command: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            interval_secs: None,
            interval: Some("30s".to_string()),
            sleep_pct: 15,
            low_pct: 40,
            monitors_with_no_ac: Some(2),
            warn_on_mons_with_no_ac: None,
            bluetooth_low_pct: 40,
            events: CustomCommands::default(),
            notifications: Notifications::default(),
            sleep_command: None,
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

        // --- sleep_command ---
        if let Some(old_cmd) = self.sleep_command.take() {
            if self.events.sleep.is_some() {
                bail!(
                    "cannot specify both 'sleep_command' and 'events.sleep'. \
                     Use 'events.sleep' only"
                )
            }
            warn!("'sleep_command' is deprecated, use 'events.sleep' instead");
            self.events.sleep = if old_cmd.is_empty() {
                None
            } else {
                Some(old_cmd)
            };
        }

        Ok((interval, monitors))
    }
}
