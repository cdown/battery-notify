use anyhow::{bail, Context, Result};
use hashbrown::HashMap;
use log::{error, info};
use notify_rust::Urgency;

use std::env;
use std::io;

use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

mod bluetooth;
mod config;
mod monitors;
mod notification;
mod system;

use notification::SingleNotification;

fn run_command(cmd: &str) {
    let shell = env::var("SHELL").unwrap_or("sh".to_string());

    // all the major shells including sh, bash, zsh, ksh, dash, fish seem to support the -c option
    if let Err(err) = Command::new(shell).args(["-c", cmd]).status() {
        error!("Failed to run command '{cmd}': {err}");
    }
}

fn main() -> Result<()> {
    let cfg: config::Config = confy::load("battery-notify", "config")?;
    log::debug!("{cfg:?}");
    let interval = Duration::from_millis(cfg.interval);
    let mut last_global_state = system::BatteryState::Invalid;
    let mut run_low_commmand = true;
    let mut state_notif = SingleNotification::default();
    let mut low_notif = SingleNotification::default();
    let mut mon_notif = SingleNotification::default();
    let mut bluetooth_bat_notifs = HashMap::new();
    let sleep_backoff = Duration::from_secs(60);
    let mut next_sleep_epoch = Instant::now();
    let should_term = Arc::new(AtomicBool::new(false));
    let st_for_hnd = should_term.clone();
    let (mut timer, canceller) = cancellable_timer::Timer::new2()?;

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

        if global.state != last_global_state {
            match global.state {
                system::BatteryState::Charging => {
                    run_command(&cfg.events.charging);

                    if cfg.notifications.charging != config::Notification::Disabled {
                        state_notif.show(
                            format!(
                                "Battery now {}",
                                system::battery_state_to_name(global.state).to_lowercase()
                            ),
                            Urgency::Normal,
                            cfg.notifications.charging.to_i32(),
                        );
                    }
                }
                system::BatteryState::Discharging => {
                    run_command(&cfg.events.discharging);

                    if cfg.notifications.discharging != config::Notification::Disabled {
                        state_notif.show(
                            format!(
                                "Battery now {}",
                                system::battery_state_to_name(global.state).to_lowercase()
                            ),
                            Urgency::Normal,
                            cfg.notifications.discharging.to_i32(),
                        );
                    }
                }
                system::BatteryState::NotCharging => {
                    run_command(&cfg.events.not_charging);

                    if cfg.notifications.not_charging != config::Notification::Disabled {
                        state_notif.show(
                            format!(
                                "Battery now {}",
                                system::battery_state_to_name(global.state).to_lowercase()
                            ),
                            Urgency::Normal,
                            cfg.notifications.not_charging.to_i32(),
                        );
                    }
                }
                system::BatteryState::Full => {
                    run_command(&cfg.events.full);

                    if cfg.notifications.full != config::Notification::Disabled {
                        state_notif.show(
                            format!(
                                "Battery now {}",
                                system::battery_state_to_name(global.state).to_lowercase()
                            ),
                            Urgency::Normal,
                            cfg.notifications.full.to_i32(),
                        );
                    }
                }
                system::BatteryState::Unknown => {
                    run_command(&cfg.events.unknown);

                    if cfg.notifications.unknown != config::Notification::Disabled {
                        state_notif.show(
                            format!(
                                "Battery now {}",
                                system::battery_state_to_name(global.state).to_lowercase()
                            ),
                            Urgency::Normal,
                            cfg.notifications.unknown.to_i32(),
                        );
                    }
                }
                system::BatteryState::AtThreshold => {
                    run_command(&cfg.events.at_threshold);

                    if cfg.notifications.at_threshold != config::Notification::Disabled {
                        state_notif.show(
                            format!(
                                "Battery now {}",
                                system::battery_state_to_name(global.state).to_lowercase()
                            ),
                            Urgency::Normal,
                            cfg.notifications.at_threshold.to_i32(),
                        );
                    }
                }
                _ => {}
            }

            last_global_state = global.state;
        }

        let level = global.level();

        if global.state == system::BatteryState::Charging || level > cfg.low_pct {
            low_notif.close();
            run_low_commmand = true;
        } else if level <= cfg.sleep_pct {
            if cfg.notifications.sleep != config::Notification::Disabled {
                low_notif.show(
                    "Battery critical".to_string(),
                    Urgency::Critical,
                    cfg.notifications.sleep.to_i32(),
                );
            }
            // Just in case we've gone loco, don't do this more than once a minute
            if start > next_sleep_epoch {
                next_sleep_epoch = start + sleep_backoff;
                run_command(&cfg.events.sleep);
            }
        } else if level <= cfg.low_pct {
            if cfg.notifications.low != config::Notification::Disabled {
                low_notif.show(
                    "Battery low".to_string(),
                    Urgency::Critical,
                    cfg.notifications.low.to_i32(),
                );
            }

            if run_low_commmand {
                run_command(&cfg.events.low);
                run_low_commmand = false;
            }
        }

        if cfg.monitors_with_no_ac > 0 && global.state == system::BatteryState::Discharging {
            let conn = monitors::get_nr_connected().unwrap_or_else(|err| {
                error!("{err}");
                0
            });
            info!("Current connected monitors: {conn}");
            if conn >= cfg.monitors_with_no_ac {
                if cfg.notifications.monitors_with_no_ac != config::Notification::Disabled {
                    mon_notif.show(
                        format!("Connected to {} monitors but not AC", conn),
                        Urgency::Critical,
                        cfg.notifications.monitors_with_no_ac.to_i32(),
                    );
                }
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
                let (_, notif) = bluetooth_bat_notifs
                    .raw_entry_mut()
                    .from_key(&bbat.name)
                    .or_insert_with(|| (bbat.name.clone(), SingleNotification::default()));
                if bbat.level <= cfg.bluetooth_low_pct {
                    if cfg.notifications.bluetooth_low != config::Notification::Disabled {
                        notif.show(
                            format!("{} battery low", bbat.name),
                            Urgency::Critical,
                            cfg.notifications.bluetooth_low.to_i32(),
                        );
                    }
                } else {
                    notif.close();
                }
            }

            // Get rid of any non-present devices and close the notification through Drop
            bluetooth_bat_notifs.retain(|key, _| bbats.iter().any(|b| b.name == *key));
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
