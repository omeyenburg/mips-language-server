#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(unused_variables)]

use std::error::Error;
use streaming_iterator::StreamingIterator;
use tracing::info;

use lsp_server::{
    Connection, ErrorCode, ExtractError, Message, Request, RequestId, Response, ResponseError,
};
use lsp_types::*;

fn main() -> Result<(), Box<dyn Error + Sync + Send>> {
    // TODO: Note that  we must have our logging only write out to stderr.
    // eprintln!("...")

    // Set up log file
    let log_file =
        std::fs::File::create("/home/oskar/git/mips-language-server/lsp.log").expect("Create file");
    let log_file = std::io::BufWriter::new(log_file);
    let (non_blocking, _guard) = tracing_appender::non_blocking(log_file);
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(tracing::level_filters::LevelFilter::DEBUG)
        .with_writer(non_blocking)
        .without_time() // Compact log messages
        .with_level(false)
        .with_target(false)
        .finish();
    tracing::subscriber::set_global_default(subscriber).expect("Could not set default subscriber");
    info!("starting generic LSP server");

    // Create the transport. Includes the stdio (stdin and stdout) versions but this could
    // also be implemented to use sockets or HTTP.
    let (connection, io_threads) = Connection::stdio();

    // Run the server and wait for the two threads to end (typically by trigger LSP Exit event).
    let server_capabilities = serde_json::to_value(&ServerCapabilities {
        inlay_hint_provider: Some(OneOf::Left(true)),
        hover_provider: Some(HoverProviderCapability::Simple(true)),
        definition_provider: Some(OneOf::Left(true)),
        references_provider: Some(OneOf::Left(true)),
        diagnostic_provider: Some(DiagnosticServerCapabilities::Options({
            DiagnosticOptions {
                identifier: None,
                inter_file_dependencies: true,
                workspace_diagnostics: true,
                work_done_progress_options: WorkDoneProgressOptions {
                    work_done_progress: None,
                },
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
    info!("shutting down server");
    Ok(())
}

fn main_loop(
    connection: Connection,
    params: serde_json::Value,
) -> Result<(), Box<dyn Error + Sync + Send>> {
    let params: InitializeParams = serde_json::from_value(params).unwrap();
    for msg in &connection.receiver {
        match msg {
            Message::Request(req) => {
                if connection.handle_shutdown(&req)? {
                    return Ok(());
                }
                info!("Got request: {:?}", req);

                match req.method.as_str() {
                    "textDocument/definition" => {
                        handle_request::<request::GotoDefinition, _, _>(
                            &req,
                            &connection,
                            handle_goto_definition,
                        );
                    }
                    "textDocument/hover" => {
                        handle_request::<request::HoverRequest, _, _>(
                            &req,
                            &connection,
                            handle_hover,
                        );
                    }
                    _ => info!("Unimplemented request: {}", req.method),
                }
            }
            Message::Response(resp) => {
                info!("Got response: {:?}", resp);
            }
            Message::Notification(not) => {
                info!("Got notification: {:?}", not);
            }
        }
    }
    Ok(())
}

fn handle_request<R, T, F>(req: &Request, connection: &Connection, handler: F)
where
    R: lsp_types::request::Request,
    R::Params: serde::de::DeserializeOwned,
    T: serde::Serialize,
    F: FnOnce(RequestId, R::Params) -> Result<T, Box<dyn Error + Send + Sync>>,
{
    let Ok((id, params)) = req.clone().extract(R::METHOD) else {
        info!("Failed to extract request.");
        send_response::<T>(
            &req.id,
            connection,
            None,
            Some("Failed to extract request."),
        );
        return;
    };

    let Ok(result) = handler(id.clone(), params) else {
        info!("Failed to execute request.");
        send_response::<T>(&id, connection, None, Some("Failed to execute request."));
        return;
    };

    send_response::<T>(&id, connection, Some(result), None)
}

fn send_response<T: serde::Serialize>(
    id: &RequestId,
    connection: &Connection,
    result: Option<T>,
    error: Option<&str>,
) {
    let (result, error) = match (result, error) {
        (Some(value), _) => match serde_json::to_value(value) {
            Ok(serialized) => (Some(serialized), None),
            Err(err) => (
                None,
                Some(ResponseError {
                    code: ErrorCode::ParseError as i32,
                    message: String::from("Failed to serialize response."),
                    data: None,
                }),
            ),
        },
        (None, Some(err)) => (
            None,
            Some(ResponseError {
                code: ErrorCode::RequestFailed as i32,
                message: err.to_string(),
                data: None,
            }),
        ),
        (None, None) => (
            None,
            Some(ResponseError {
                code: ErrorCode::InternalError as i32,
                message: "Unexpected missing result and error.".to_string(),
                data: None,
            }),
        ),
    };

    if let Err(err) = connection.sender.send(Message::Response(Response {
        id: id.clone(),
        result,
        error,
    })) {
        info!("Failed to send response: {err}");
    }
}

fn handle_goto_definition(
    id: RequestId,
    params: GotoDefinitionParams,
) -> Result<GotoDefinitionResponse, Box<dyn Error + Send + Sync>> {
    info!("Handling GotoDefinition: {:?}", params);
    Ok(GotoDefinitionResponse::Array(Vec::new()))
}

fn handle_hover(id: RequestId, params: HoverParams) -> Result<Hover, Box<dyn Error + Send + Sync>> {
    info!("Handling Hover request: {:?}", params);
    Ok(Hover {
        contents: HoverContents::Scalar(MarkedString::String("Hover info".to_string())),
        range: None,
    })
}
