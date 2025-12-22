#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(unused_variables)]

#[macro_use]
mod logging;

mod server;
mod lsp;
mod version;
mod settings;

mod tree;

mod language_definitions;
mod lang;

mod completion;
mod hover;

#[tokio::main]
async fn main() {
    log!("Starting MIPS language server");
    log_init!();
    crate::server::serve().await
}
