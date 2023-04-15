use std::cell::Cell;

thread_local! {
    static LAST_BATTERY_STATE: Cell<BatteryState> = Cell::new(BatteryState::Unknown);
}

#[derive(Debug, Eq, PartialEq, Copy, Clone)]
enum BatteryState {
    Discharging,
    Charging,
    NotCharging,
    Full,
    Unknown,
}

fn set_last_state(state: BatteryState) {
    LAST_BATTERY_STATE.with(|lbs| lbs.set(state));
}

fn get_last_state() -> BatteryState {
    LAST_BATTERY_STATE.with(Cell::get)
}

fn main() {
    set_last_state(BatteryState::Charging);
    dbg!(get_last_state());
}
