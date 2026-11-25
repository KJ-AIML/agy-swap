//! Embeds the AGY Swap application icon and version metadata into `agswitch-desktop.exe` on
//! Windows (Explorer, Start Menu shortcut, Task Manager show "AGY Swap"). No-op elsewhere.

fn main() -> std::io::Result<()> {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=assets/agy-swap.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        embed_windows_resources()?;
    }
    Ok(())
}

#[cfg(windows)]
fn embed_windows_resources() -> std::io::Result<()> {
    let mut res = winresource::WindowsResource::new();
    res.set_icon("assets/agy-swap.ico")
        .set("FileDescription", "AGY Swap")
        .set("ProductName", "AGY Swap")
        .set("InternalName", "agswitch-desktop")
        .set("OriginalFilename", "agswitch-desktop.exe")
        .set("LegalCopyright", "MIT License");
    res.compile()
}

#[cfg(not(windows))]
fn embed_windows_resources() -> std::io::Result<()> {
    Ok(())
}
