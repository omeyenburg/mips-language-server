#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(unused_variables)]

#[macro_use]
mod logging;

mod server;
mod version;
mod settings;

mod tree;
mod lang;

mod diagnostic;
mod completion;
mod hover;

#[tokio::main]
async fn main() {
    log!("Starting MIPS language server");
    log_init!();
    crate::server::serve().await
}
