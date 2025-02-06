#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(unused_variables)]

use std::error::Error;

//use serde::de::value;
//use tracing::info;
//use tree_sitter::{InputEdit, Query, QueryCursor};

//use lsp_server::{Connection, ExtractError, Message, Request, RequestId, Response};

use streaming_iterator::StreamingIterator;

//mod json;
//mod parser;
//mod tree;

//use lsp_types::OneOf;
//use lsp_types::{
//    request::GotoDefinition, GotoDefinitionResponse, InitializeParams, ServerCapabilities,
//};
use lsp_types::*;
use lsp_types::request::GotoDefinition;

use lsp_server::{Connection, ExtractError, Message, Request, RequestId, Response};

fn main() -> Result<(), Box<dyn Error + Sync + Send>> {
    // Note that  we must have our logging only write out to stderr.
    eprintln!("starting generic LSP server");

    // Create the transport. Includes the stdio (stdin and stdout) versions but this could
    // also be implemented to use sockets or HTTP.
    let (connection, io_threads) = Connection::stdio();

    // Run the server and wait for the two threads to end (typically by trigger LSP Exit event).
    let server_capabilities = serde_json::to_value(&ServerCapabilities {
        inlay_hint_provider: Some(OneOf::Left(true)),
        hover_provider: Some(HoverProviderCapability::Simple(true)),
        definition_provider: Some(OneOf::Left(true)),
        references_provider: Some(OneOf::Left(true)),
        diagnostic_provider: Some(DiagnosticServerCapabilities::Options ({
            DiagnosticOptions {
                identifier: None,
                inter_file_dependencies: true,
                workspace_diagnostics: true,
                work_done_progress_options: WorkDoneProgressOptions {
                    work_done_progress: None
                }
            }
        })),
        completion_provider: Some(CompletionOptions {
            resolve_provider: Some(false),
            trigger_characters: Some(vec![".".to_string(), "$".to_string()]),
            work_done_progress_options: Default::default(),
            all_commit_characters: None,
            completion_item: None,
        }),
        text_document_sync: Some(TextDocumentSyncCapability::Options(
            TextDocumentSyncOptions {
                open_close: Some(true),
                //change: Some(TextDocumentSyncKind::FULL),
                change: Some(TextDocumentSyncKind::INCREMENTAL),
                save: Some(TextDocumentSyncSaveOptions::SaveOptions(SaveOptions {
                    include_text: Some(true),
                })),
                ..Default::default()
            },
        )),
        ..Default::default()
    })
    .unwrap();
    let initialization_params = connection.initialize(server_capabilities)?;
    main_loop(connection, initialization_params)?;
    io_threads.join()?;

    // Shut down gracefully.
    eprintln!("shutting down server");
    Ok(())
}

fn main_loop(
    connection: Connection,
    params: serde_json::Value,
) -> Result<(), Box<dyn Error + Sync + Send>> {
    let params: InitializeParams = serde_json::from_value(params).unwrap();
    eprintln!("starting example main loop");
    for msg in &connection.receiver {
        eprintln!("got msg: {:?}", msg);
    match msg {
        Message::Request(req) => {
            if connection.handle_shutdown(&req)? {
                return Ok(());
            }
            eprintln!("got request: {:?}", req);
            match cast::<GotoDefinition>(req.clone()) {
                Ok((id, params)) => {
                    eprintln!("got gotoDefinition request #{}: {:?}", id, params);
                    let result = Some(GotoDefinitionResponse::Array(Vec::new()));
                    let result = serde_json::to_value(&result).unwrap();
                    let resp = Response { id, result: Some(result), error: None };
                    connection.sender.send(Message::Response(resp))?;
                    continue;
                }
                Err(err @ ExtractError::JsonError { .. }) => panic!("{:?}", err),
                Err(ExtractError::MethodMismatch(req)) => req,
            };
            // ...
        }
        Message::Response(resp) => {
            eprintln!("got response: {:?}", resp);
        }
        Message::Notification(not) => {
            eprintln!("got notification: {:?}", not);
        }
    }
    }
    Ok(())
}

//fn handle_message(msg: &Message, params: &InitializeParams, connection: &Connection) -> Result<(), Box<dyn std::error::Error>> {
//}

fn cast<R>(req: Request) -> Result<(RequestId, R::Params), ExtractError<Request>>
where
    R: lsp_types::request::Request,
    R::Params: serde::de::DeserializeOwned,
{
    req.extract(R::METHOD)
}
