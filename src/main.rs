#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(unused_variables)]

#[macro_use]
mod logging;

mod json;
mod lsp;
mod parser;
mod tree;

#[cfg(feature = "sync")]
mod server_sync;

#[cfg(feature = "async")]
mod server_async;

fn init() {
    log_init!();
    log!("Starting MIPS language server");
}

#[cfg(feature = "sync")]
fn main() -> Result<(), Box<dyn std::error::Error + Sync + Send>> {
    init();
    crate::server_sync::serve()
}

#[tokio::main]
#[cfg(feature = "async")]
async fn main() {
    init();
    crate::server_async::serve().await
}

#[cfg(not(any(feature = "sync", feature = "async")))]
compile_error!(
    "Neither 'sync' nor 'async' feature is enabled. Please enable at least one of them."
);

#[cfg(all(feature = "sync", feature = "async"))]
compile_error!("Both 'sync' and 'async' features are enabled. Please enable only one of them.");

//fn main() -> Result<(), Box<dyn std::error::Error + Sync + Send>> {
//    let mut args = std::env::args();
//    match args.nth(1).as_deref() {
//        None => {
//        }
//        Some("--version") => {
//            println!("mips-language-server v0.1.0");
//        }
//        Some(_) => {
//            println!("Usage:\n  mips-language-server [options]\n\nOptions:\n  --version, -v         Version");
//        }
//    };
//    Ok(())
//}
