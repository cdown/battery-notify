use anyhow::Result;

#[cfg(feature = "mons")]
pub fn get_nr_connected() -> Result<usize> {
    use once_cell::sync::Lazy;
    use x11rb::{connection::Connection, protocol::randr, rust_connection::RustConnection};

    static CONN_AND_ROOT: Lazy<(RustConnection, u32)> = Lazy::new(|| {
        x11rb::connect(None)
            .map(|(c, screen)| {
                let root = c.setup().roots[screen].root;
                (c, root)
            })
            .unwrap()
    });

    let (conn, root) = Lazy::force(&CONN_AND_ROOT);
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
