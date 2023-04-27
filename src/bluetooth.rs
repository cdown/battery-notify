use anyhow::Result;

#[derive(Debug)]
pub struct BluetoothBattery {
    pub name: String,
    pub level: u8,
}

#[cfg(feature = "bluetooth")]
pub fn get_battery_levels() -> Result<Vec<BluetoothBattery>> {
    use log::error;
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
pub fn get_battery_levels() -> Result<Vec<BluetoothBattery>> {
    Ok(Vec::new())
}
