use anyhow::Result;

#[derive(Debug)]
pub struct BluetoothBattery {
    pub name: String,
    pub level: u8,
}

#[cfg(feature = "bluetooth")]
pub fn get_battery_levels() -> Result<Vec<BluetoothBattery>> {
    use once_cell::sync::Lazy;
    use std::collections::HashMap;
    use zbus::blocking::Connection;
    use zbus::zvariant::{ObjectPath, Value};

    type ManagedObjects<'a> = HashMap<ObjectPath<'a>, HashMap<String, HashMap<String, Value<'a>>>>;

    static CONN: Lazy<Connection> = Lazy::new(|| Connection::system().unwrap());

    let ret = CONN.call_method(
        Some("org.bluez"),
        "/",
        Some("org.freedesktop.DBus.ObjectManager"),
        "GetManagedObjects",
        &(),
    )?;
    let body = ret.body();
    let (devices,): (ManagedObjects<'_>,) = body.deserialize()?;

    Ok(devices
        .iter()
        .filter_map(|(_, ifs)| {
            let bat = ifs.get("org.bluez.Battery1")?;
            let level = bat
                .get("Percentage")
                .and_then(|p| p.clone().downcast::<u8>().ok())?;
            let dev = ifs.get("org.bluez.Device1")?;
            let name = dev
                .get("Name")
                .and_then(|n| n.clone().downcast::<String>().ok())?;
            Some(BluetoothBattery { name, level })
        })
        .collect::<Vec<_>>())
}

#[cfg(not(feature = "bluetooth"))]
pub fn get_battery_levels() -> Result<Vec<BluetoothBattery>> {
    Ok(Vec::new())
}
