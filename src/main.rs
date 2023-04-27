use anyhow::{bail, Context, Result};
use hashbrown::HashMap;
use log::{error, info, trace};
use notify_rust::{Notification, NotificationHandle, Urgency};
use serde::{Deserialize, Serialize};
use std::ffi::OsStr;
use std::fs;
use std::io;
use std::path::Path;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

#[derive(Debug, Eq, PartialEq, Copy, Clone, Serialize, Deserialize)]
enum BatteryState {
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
struct Battery {
    state: BatteryState,
    now_uwh: u64,
    full_uwh: u64,
}

impl Battery {
    const fn level(&self) -> u8 {
        let mut level = (self.now_uwh * 100) / self.full_uwh;
        if level > 100 {
            level = 100;
        }
        level as _
    }
}

#[derive(Debug)]
struct BluetoothBattery {
    name: String,
    level: u8,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(default)]
struct Config {
    sleep_command: String,
    interval_secs: u64,
    sleep_pct: u8,
    low_pct: u8,
    warn_on_mons_with_no_ac: usize,
    bluetooth_low_pct: u8,
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

struct SingleNotification {
    hnd: Option<NotificationHandle>,
    summary: String,
}

impl SingleNotification {
    const fn new() -> Self {
        Self {
            hnd: None,
            summary: String::new(),
        }
    }

    fn show(&mut self, summary: String, urgency: Urgency) {
        if self.summary != summary {
            self.close();
            self.summary = summary;
            trace!("Creating notification for {}", self.summary);
            self.hnd = Notification::new()
                .summary(&self.summary)
                .urgency(urgency)
                .show()
                .map_err(|err| error!("error showing notification: {err}"))
                .ok();
        }
    }

    fn close(&mut self) {
        if let Some(hnd) = self.hnd.take() {
            trace!("Closing notification for {}", self.summary);
            hnd.close();
        }
    }
}

impl Drop for SingleNotification {
    fn drop(&mut self) {
        self.close();
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

#[cfg(feature = "bluetooth")]
fn get_bluetooth_battery_levels() -> Result<Vec<BluetoothBattery>> {
    use std::collections::HashMap as SHashMap;
    use zbus::blocking::Connection;
    use zbus::zvariant::{ObjectPath, Value};

    type ManagedObjects<'a> =
        SHashMap<ObjectPath<'a>, SHashMap<String, SHashMap<String, Value<'a>>>>;

    let conn = Connection::system().map_err(|err| {
        error!(
            "Failed to connect to dbus, will not be able to retrieve bluetooth information: {err}"
        );
        err
    })?;

    let ret = conn.call_method(
        Some("org.bluez"),
        "/",
        Some("org.freedesktop.DBus.ObjectManager"),
        "GetManagedObjects",
        &(),
    )?;
    let (devices,): (ManagedObjects<'_>,) = ret.body()?;

    Ok(devices
        .iter()
        .filter_map(|(_, ifs)| {
            let bat = ifs.get("org.bluez.Battery1")?;
            let level = bat
                .get("Percentage")
                .and_then(|p| p.clone().downcast::<u8>())?;
            let dev = ifs.get("org.bluez.Device1")?;
            let name = dev
                .get("Name")
                .and_then(|n| n.clone().downcast::<String>())?;
            Some(BluetoothBattery { name, level })
        })
        .collect::<Vec<_>>())
}

#[cfg(not(feature = "bluetooth"))]
fn get_bluetooth_battery_levels() -> Result<Vec<BluetoothBattery>> {
    Ok(Vec::new())
}

/// Some drivers expose µAh (charge), some drivers expose µWh (energy), some drivers expose both.
fn read_battery_file_energy_or_charge(dir: &Path, partial_file: &str) -> Result<u64> {
    let uwh = read_battery_file(dir, "energy_".to_string() + partial_file);
    if uwh.is_ok() {
        return Ok(uwh?.parse()?);
    }

    let voltage: u64 = read_battery_file(dir, "voltage_now")?.parse()?;
    let uah: u64 = read_battery_file(dir, "charge_".to_string() + partial_file)?.parse()?;
    Ok((uah * voltage) / 1000)
}

fn read_battery_dir(dir: impl AsRef<Path>) -> Result<Battery> {
    let dir = dir.as_ref();

    Ok(Battery {
        state: name_to_battery_state(&read_battery_file(dir, "status")?),
        now_uwh: read_battery_file_energy_or_charge(dir, "now")?,
        full_uwh: read_battery_file_energy_or_charge(dir, "full")?,
    })
}

#[cfg(feature = "mons")]
fn get_nr_connected_monitors() -> Result<usize> {
    use once_cell::sync::Lazy;
    use x11rb::{connection::Connection, protocol::randr, rust_connection::RustConnection};

    static CONN_AND_ROOT: Lazy<Option<(RustConnection, u32)>> = Lazy::new(|| {
        let conn_and_root = x11rb::connect(None).map(|(c, screen)| {
            let root = c.setup().roots[screen].root;
            (c, root)
        });
        match conn_and_root {
            Ok((conn, root)) => Some((conn, root)),
            Err(err) => {
                error!("Failed to connect to X, will not be able to retrieve monitor information: {err}");
                None
            }
        }
    });

    let (conn, root) = match Lazy::force(&CONN_AND_ROOT) {
        Some((conn, root)) => (conn, root),
        None => bail!("No X connection"),
    };

    let resources = randr::get_screen_resources(conn, *root)?;

    let mut nr_connected_monitors = 0;
    for output in resources.reply()?.outputs {
        let output_info = randr::get_output_info(conn, output, 0)?.reply()?;
        if output_info.connection == randr::Connection::CONNECTED {
            nr_connected_monitors += 1;
        }
    }
    Ok(nr_connected_monitors)
}

#[cfg(not(feature = "mons"))]
fn get_nr_connected_monitors() -> Result<usize> {
    Ok(0)
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

fn run_sleep_command(cmd: &str) {
    if let Err(err) = Command::new("sh").args(["-c", cmd]).status() {
        error!("Failed to run sleep command '{cmd}': {err}");
    }
}

fn main() -> Result<()> {
    let cfg: Config = confy::load("battery-notify", "config")?;
    let interval = Duration::from_secs(cfg.interval_secs);
    let mut state_notif = SingleNotification::new();
    let mut low_notif = SingleNotification::new();
    let mut mon_notif = SingleNotification::new();
    let sleep_backoff = Duration::from_secs(60);
    let mut next_sleep_epoch = Instant::now();
    let should_term = Arc::new(AtomicBool::new(false));
    let st_for_hnd = should_term.clone();
    let (mut timer, canceller) = cancellable_timer::Timer::new2()?;
    let mut bbat_notifs = HashMap::new();

    env_logger::init_from_env(env_logger::Env::default().default_filter_or("info"));

    ctrlc::set_handler(move || {
        st_for_hnd.store(true, Ordering::Relaxed);
        // If we fail to cancel, we'll just do it at the next start of the loop
        let _ = canceller.cancel();
    })
    .expect("Failed to set signal handler");

    info!(
        "Config (configurable at {}):\n\n{:#?}\n",
        confy::get_configuration_file_path("battery-notify", "config")?.display(),
        cfg
    );

    while !should_term.load(Ordering::Relaxed) {
        let start = Instant::now();
        let batteries = get_batteries().context("failed to get list of batteries")?;

        if batteries.is_empty() {
            bail!("no batteries detected");
        }

        info!("Battery status: {:?}", &batteries);

        let global = get_global_battery(&batteries);
        info!("Global status: {:?}", &global);
        state_notif.show(
            format!(
                "Battery now {}",
                battery_state_to_name(global.state).to_lowercase()
            ),
            Urgency::Normal,
        );

        let level = global.level();

        if global.state == BatteryState::Charging {
            low_notif.close();
        } else if level <= cfg.sleep_pct {
            low_notif.show("Battery critical".to_string(), Urgency::Critical);
            // Just in case we've gone loco, don't do this more than once a minute
            if start > next_sleep_epoch {
                next_sleep_epoch = start + sleep_backoff;
                run_sleep_command(&cfg.sleep_command);
            }
        } else if level <= cfg.low_pct {
            low_notif.show("Battery low".to_string(), Urgency::Critical);
        }

        if cfg.warn_on_mons_with_no_ac > 0 && global.state == BatteryState::Discharging {
            let conn = get_nr_connected_monitors().unwrap_or(0);
            info!("Current connected monitors: {conn}");
            if conn >= cfg.warn_on_mons_with_no_ac {
                mon_notif.show(
                    format!("Connected to {} monitors but not AC", conn),
                    Urgency::Critical,
                );
            } else {
                mon_notif.close()
            }
        } else {
            mon_notif.close();
        }

        if cfg.bluetooth_low_pct != 0 {
            let bbats = get_bluetooth_battery_levels().unwrap_or_else(|_| Vec::new());
            info!("Bluetooth battery status: {:?}", bbats);
            for bbat in &bbats {
                let (_, notif) = bbat_notifs
                    .raw_entry_mut()
                    .from_key(&bbat.name)
                    .or_insert_with(|| (bbat.name.clone(), SingleNotification::new()));
                if bbat.level <= cfg.bluetooth_low_pct {
                    notif.show(format!("{} battery low", bbat.name), Urgency::Critical);
                } else {
                    notif.close();
                }
            }

            // Get rid of any non-present devices and close the notification through Drop
            bbat_notifs.retain(|key, _| bbats.iter().any(|b| b.name == *key));
        }

        let elapsed = start.elapsed();

        if elapsed < interval {
            match timer.sleep(interval - elapsed) {
                Err(err) if err.kind() != io::ErrorKind::Interrupted => Err(err),
                _ => Ok(()),
            }?;
        }
    }

    Ok(())
}
