use anyhow::{bail, Context, Result};
use hashbrown::HashMap;
use log::{error, info};
use notify_rust::Urgency;
use serde::{Deserialize, Serialize};

use std::env;
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
    battery_state_charging_command: String,
    battery_state_discharging_command: String,
    battery_state_not_charging_command: String,
    battery_state_full_command: String,
    battery_state_unknown_command: String,
    battery_state_at_threshold_command: String,
    interval: u64,
    sleep_pct: u8,
    low_pct: u8,
    warn_on_mons_with_no_ac: usize,
    bluetooth_low_pct: u8,
    state_notif_enabled: bool,
    sleep_pct_notif_timeout: i32,
    low_pct_notif_timeout: i32,
    state_notif_timeout: i32,
    warn_on_mons_with_no_ac_notif_timeout: i32,
    bluetooth_low_pct_notif_timeout: i32,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            sleep_command: "systemctl suspend".to_string(),
            battery_state_charging_command: "true".to_string(),
            battery_state_discharging_command: "true".to_string(),
            battery_state_not_charging_command: "true".to_string(),
            battery_state_full_command: "true".to_string(),
            battery_state_unknown_command: "true".to_string(),
            battery_state_at_threshold_command: "true".to_string(),
            interval: 30000,
            sleep_pct: 15,
            low_pct: 40,
            warn_on_mons_with_no_ac: 2,
            bluetooth_low_pct: 40,
            state_notif_enabled: true,
            sleep_pct_notif_timeout: 0,
            low_pct_notif_timeout: 0,
            state_notif_timeout: -1,
            warn_on_mons_with_no_ac_notif_timeout: 0,
            bluetooth_low_pct_notif_timeout: 0,
        }
    }
}

fn run_command(cmd: &str) {
    let shell = env::var("SHELL").unwrap_or("sh".to_string());

    // all the major shells including sh, bash, zsh, ksh, dash, fish seem to support the -c option
    if let Err(err) = Command::new(shell).args(["-c", cmd]).status() {
        error!("Failed to run command '{cmd}': {err}");
    }
}

fn main() -> Result<()> {
    let cfg: Config = confy::load("battery-notify", "config")?;
    let interval = Duration::from_millis(cfg.interval);
    let mut last_battery_state_name = String::default();
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
        let battery_state_name = system::battery_state_to_name(global.state);

        if battery_state_name != last_battery_state_name {
            last_battery_state_name = battery_state_name.clone();

            match battery_state_name.as_str() {
                "Charging" => run_command(&cfg.battery_state_charging_command),
                "Discharging" => run_command(&cfg.battery_state_discharging_command),
                "Not charging" => run_command(&cfg.battery_state_not_charging_command),
                "Full" => run_command(&cfg.battery_state_full_command),
                "Unknown" => run_command(&cfg.battery_state_unknown_command),
                "At threshold" => run_command(&cfg.battery_state_at_threshold_command),
                _ => {}
            }
        }

        info!("Global status: {:?}", &global);
        if cfg.state_notif_enabled {
            state_notif.show(
                format!("Battery now {}", battery_state_name.to_lowercase()),
                Urgency::Normal,
                cfg.state_notif_timeout,
            );
        }

        let level = global.level();

        if global.state == system::BatteryState::Charging || level > cfg.low_pct {
            low_notif.close();
        } else if level <= cfg.sleep_pct {
            low_notif.show(
                "Battery critical".to_string(),
                Urgency::Critical,
                cfg.sleep_pct_notif_timeout,
            );
            // Just in case we've gone loco, don't do this more than once a minute
            if start > next_sleep_epoch {
                next_sleep_epoch = start + sleep_backoff;
                run_command(&cfg.sleep_command);
            }
        } else if level <= cfg.low_pct {
            low_notif.show(
                "Battery low".to_string(),
                Urgency::Critical,
                cfg.low_pct_notif_timeout,
            );
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
                    cfg.warn_on_mons_with_no_ac_notif_timeout,
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
                    notif.show(
                        format!("{} battery low", bbat.name),
                        Urgency::Critical,
                        cfg.bluetooth_low_pct_notif_timeout,
                    );
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
