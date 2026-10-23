use std::time::{SystemTime, UNIX_EPOCH};

fn main() {
    // The framework reads resource 0x101 for the Windows window/taskbar icon.
    // Embedding it also gives the portable executable its own Explorer icon.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        embed_resource::compile_for(
            "assets/stepx_companion.rc",
            &["stepx-companion"],
            embed_resource::NONE,
        )
        .manifest_required()
        .expect("compile StepX Companion Windows icon");
    }
    // Track application sources/assets so the timestamp changes with the binary,
    // while an unchanged build keeps its original timestamp. No host `date` tool
    // or host timezone is needed, including when cross-compiling.
    for path in ["build.rs", "Cargo.toml", "Cargo.lock", "src", "assets"] {
        println!("cargo:rerun-if-changed={path}");
    }
    println!("cargo:rerun-if-env-changed=SOURCE_DATE_EPOCH");
    let seconds = match std::env::var("SOURCE_DATE_EPOCH") {
        Ok(value) => value
            .parse::<u64>()
            .expect("SOURCE_DATE_EPOCH must be a non-negative Unix timestamp in seconds"),
        Err(std::env::VarError::NotPresent) => SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("build clock must be after the Unix epoch")
            .as_secs(),
        Err(error) => panic!("cannot read SOURCE_DATE_EPOCH: {error}"),
    };
    assert!(
        seconds <= 253_402_271_999,
        "build timestamp exceeds year 9999 in UTC+8"
    );
    println!("cargo:rustc-env=STEPX_BUILD_UNIX_SECONDS={seconds}");
}
