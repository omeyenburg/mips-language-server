fn main() {
    let sync_enabled = std::env::var("CARGO_FEATURE_SYNC").is_ok();
    let async_enabled = std::env::var("CARGO_FEATURE_ASYNC").is_ok();

    if async_enabled && sync_enabled {
        println!("cargo:rustc-cfg=feature=\"async\"");
        println!("cargo:warning=Both 'sync' and 'async' were enabled. 'sync' has been disabled.");
    } else if sync_enabled {
        println!("cargo:rustc-cfg=feature=\"sync\"");
    } else if async_enabled {
        println!("cargo:rustc-cfg=feature=\"async\"");
    } else {
        println!("cargo:rustc-cfg=feature=\"sync\""); // Default to sync if neither is set
    }
}
