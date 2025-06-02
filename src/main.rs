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

fn init() {
    log_init!();
    log!("Starting MIPS language server");
}

fn main() -> Result<(), Box<dyn std::error::Error + Sync + Send>> {
    init();
    crate::server::serve()
}
