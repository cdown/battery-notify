use anyhow::{bail, Context, Result};
use hashbrown::HashMap;
use log::{error, info};
use notify_rust::Urgency;
use serde::{Deserialize, Serialize};

use std::io;

use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

mod bluetooth;
mod monitors;
mod notification;
mod system;

use notification::SingleNotification;

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

fn run_sleep_command(cmd: &str) {
    if let Err(err) = Command::new("sh").args(["-c", cmd]).status() {
        error!("Failed to run sleep command '{cmd}': {err}");
    }
}

fn main() -> Result<()> {
    let cfg: Config = confy::load("battery-notify", "config")?;
    let interval = Duration::from_secs(cfg.interval_secs);
    let mut state_notif = SingleNotification::default();
    let mut low_notif = SingleNotification::default();
    let mut mon_notif = SingleNotification::default();
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

    let mut next_wake = Instant::now() + interval;

    sd_notify::notify(
        false,
        &[
            sd_notify::NotifyState::Ready,
            // Grace period in case interval takes too long
            sd_notify::NotifyState::WatchdogUsec((interval * 2).as_micros().try_into()?),
        ],
    )?;

    while !should_term.load(Ordering::Relaxed) {
        sd_notify::notify(false, &[sd_notify::NotifyState::Watchdog])?;
        let start = Instant::now();
        let batteries = system::get_batteries().context("failed to get list of batteries")?;

        if batteries.is_empty() {
            bail!("no batteries detected");
        }

        info!("Battery status: {:?}", &batteries);

        let global = system::get_global_battery(&batteries);
        info!("Global status: {:?}", &global);
        state_notif.show(
            format!(
                "Battery now {}",
                system::battery_state_to_name(global.state).to_lowercase()
            ),
            Urgency::Normal,
        );

        let level = global.level();

        if global.state == system::BatteryState::Charging || level > cfg.low_pct {
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

        if cfg.warn_on_mons_with_no_ac > 0 && global.state == system::BatteryState::Discharging {
            let conn = monitors::get_nr_connected().unwrap_or_else(|err| {
                error!("{err}");
                0
            });
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
            let bbats = bluetooth::get_battery_levels().unwrap_or_else(|err| {
                error!("{err}");
                Vec::new()
            });
            info!("Bluetooth battery status: {:?}", bbats);
            for bbat in &bbats {
                let (_, notif) = bbat_notifs
                    .raw_entry_mut()
                    .from_key(&bbat.name)
                    .or_insert_with(|| (bbat.name.clone(), SingleNotification::default()));
                if bbat.level <= cfg.bluetooth_low_pct {
                    notif.show(format!("{} battery low", bbat.name), Urgency::Critical);
                } else {
                    notif.close();
                }
            }

            // Get rid of any non-present devices and close the notification through Drop
            bbat_notifs.retain(|key, _| bbats.iter().any(|b| b.name == *key));
        }

        let now = Instant::now();
        if now < next_wake {
            match timer.sleep(next_wake - now) {
                Err(err) if err.kind() != io::ErrorKind::Interrupted => Err(err),
                _ => Ok(()),
            }?;
            next_wake += interval;
        } else {
            // Avoid spamming with more runs
            next_wake = now + interval;
        }
    }

    Ok(())
}
