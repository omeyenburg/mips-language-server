#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(unused_variables)]

#[macro_use]
mod logging;

mod language_definitions;
mod server;
mod lsp;
mod settings;
mod tree;
mod completion;
mod lang;
mod version;
mod hover;

#[tokio::main]
async fn main() {
    log!("Starting MIPS language server");
    log_init!();
    crate::server::serve().await
}
