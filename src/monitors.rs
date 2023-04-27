use anyhow::Result;

#[cfg(feature = "mons")]
pub fn get_nr_connected() -> Result<usize> {
    use anyhow::bail;
    use log::error;
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

    let mut nr_connected = 0;
    for output in resources.reply()?.outputs {
        let output_info = randr::get_output_info(conn, output, 0)?.reply()?;
        if output_info.connection == randr::Connection::CONNECTED {
            nr_connected += 1;
        }
    }
    Ok(nr_connected)
}

#[cfg(not(feature = "mons"))]
pub fn get_nr_connected() -> Result<usize> {
    Ok(0)
}
