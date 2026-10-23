//! Pair without putting the code in arguments or logs.
use anyhow::{Context, Result};
use stepx_companion::{adb::Adb, pairing};

fn main() -> Result<()> {
    let endpoint = std::env::args()
        .nth(1)
        .context("usage: pair_phone <IP:pairing-port>; supply pairing code on stdin")?;
    let mut code = String::new();
    std::io::stdin().read_line(&mut code)?;
    let adb = Adb::discover()?;
    println!("{}", pairing::pair_code(&adb, &endpoint, code.trim())?);
    Ok(())
}
