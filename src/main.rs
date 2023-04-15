use num_enum::{IntoPrimitive, TryFromPrimitive};
use std::convert::TryFrom;
use std::sync::atomic::{AtomicU8, Ordering};

static LAST_BATTERY_STATE: AtomicU8 = AtomicU8::new(0);

#[derive(Debug, Eq, PartialEq, IntoPrimitive, TryFromPrimitive)]
#[repr(u8)]
enum BatteryState {
    Discharging,
    Charging,
    NotCharging,
    Full,
    Unknown,
}

fn set_last_state(state: BatteryState) {
    LAST_BATTERY_STATE.store(state.into(), Ordering::Release);
}

fn get_last_state() -> BatteryState {
    BatteryState::try_from(LAST_BATTERY_STATE.load(Ordering::Acquire))
        .expect("LAST_BATTERY_STATE is corrupt")
}

fn main() {
    set_last_state(BatteryState::Charging);
    dbg!(get_last_state());
}
