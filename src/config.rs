use serde::de::{self, Unexpected, Visitor};
use serde::{Deserialize, Deserializer, Serialize};
use std::fmt;

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
            Notification::Disabled => -2, // not used anywhere
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
    pub low: String,
    pub sleep: String,
    pub charging: String,
    pub discharging: String,
    pub not_charging: String,
    pub full: String,
    pub unknown: String,
    pub at_threshold: String,
}

impl Default for CustomCommands {
    fn default() -> Self {
        Self {
            low: "".to_string(),
            sleep: "systemctl suspend".to_string(),
            charging: "".to_string(),
            discharging: "".to_string(),
            not_charging: "".to_string(),
            full: "".to_string(),
            unknown: "".to_string(),
            at_threshold: "".to_string(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub interval: u64,
    pub sleep_pct: u8,
    pub low_pct: u8,
    pub monitors_with_no_ac: usize,
    pub bluetooth_low_pct: u8,
    pub events: CustomCommands,
    pub notifications: Notifications,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            interval: 30000,
            low_pct: 40,
            sleep_pct: 15,
            bluetooth_low_pct: 40,
            monitors_with_no_ac: 2,
            events: CustomCommands::default(),
            notifications: Notifications::default(),
        }
    }
}
