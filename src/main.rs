#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(unused_variables)]

#[macro_use]
mod logging;

mod json;
mod lsp;
mod parser;
mod server;
mod tree;

#[tokio::main]
async fn main() {
    log!("Starting MIPS language server");
    log_init!();
    crate::server::serve().await
}

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
