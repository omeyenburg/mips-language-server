#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(unused_variables)]

#[macro_use]
mod logging;

mod server;
mod settings;
mod version;

mod lang;
mod tree;

mod completion;
mod diagnostic;
mod hover;

#[tokio::main]
async fn main() {
    log!("Starting MIPS language server");
    log_init!();
    crate::server::serve().await
}
