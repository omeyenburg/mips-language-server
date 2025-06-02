#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(unused_variables)]

use lsp_server::{
    Connection, ErrorCode, ExtractError, Message, Notification, Request, RequestId, Response,
    ResponseError,
};
use lsp_types::*;
use std::collections::HashMap;
use std::error::Error;

use crate::lsp::Workspace;

#[derive(Debug)]
struct Backend {
    connection: Connection,
    params: InitializeParams,
    workspace: Workspace,
}

impl Backend {
    pub fn new(connection: Connection) -> Self {

        let workspace = Workspace::new();

        Self {
            connection,
            params,
            workspace
        }
    }
}

pub fn serve() -> Result<(), Box<dyn Error + Sync + Send>> {
    // Initialize logging
    log_init!();
    log!("Starting MIPS language server");

    // Create the transport. Includes the stdio (stdin and stdout) versions but
    // this could also be implemented to use sockets or HTTP.
    let (connection, io_threads) = Connection::stdio();

    // Run the server and wait for the two threads to end (typically by trigger LSP Exit event).
    let server_capabilities = serde_json::to_value(&Workspace::get_server_capabilities()).unwrap();
    let initialization_params = connection.initialize(server_capabilities)?;
    let initialization_params: InitializeParams =
        serde_json::from_value(initialization_params).unwrap();

    main_loop(connection, initialization_params)?;
    io_threads.join()?;

    // Shut down gracefully.
    log!("Stopping MIPS language server");
    Ok(())
}

fn main_loop(
    connection: Connection,
    params: InitializeParams,
) -> Result<(), Box<dyn Error + Sync + Send>> {
    for msg in &connection.receiver {
        match msg {
            Message::Request(req) => {
                if connection.handle_shutdown(&req)? {
                    return Ok(());
                }

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
                    _ => log!("Unimplemented request: {}", req.method),
                }
            }
            Message::Response(res) => {
                log!("Got response: {:?}", res);
            }
            Message::Notification(not) => match not.method.as_str() {
                "textDocument/didChange" => {
                    handle_notification::<notification::DidChangeTextDocument, _, _>(
                        &not,
                        handle_notification_document_change,
                    )
                }
                "textDocument/didOpen" => (),
                "textDocument/didSave" => (),
                "workspace/didChangeConfiguration" => (),
                &_ => log!("Unimplemented request: {}", not.method),
            },
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
        log!("Failed to extract request.");
        send_response::<T>(&req.id, connection, Err("Failed to extract request."));
        return;
    };

    let Ok(result) = handler(id.clone(), params) else {
        log!("Failed to execute request.");
        send_response::<T>(&id, connection, Err("Failed to execute request."));
        return;
    };

    send_response::<T>(&id, connection, Ok(result))
}

fn handle_notification<R, T, F>(notification: &Notification, handler: F)
where
    R: lsp_types::notification::Notification,
    R::Params: serde::de::DeserializeOwned,
    T: serde::Serialize,
    F: FnOnce(R::Params) -> Result<T, Box<dyn Error + Send + Sync>>,
{
    let Ok(params) = notification.clone().extract(R::METHOD) else {
        log!("Failed to extract notification.");
        return;
    };

    handler(params);
}

fn handle_notification_document_change(
    params: DidChangeTextDocumentParams,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    log!("Handle doc change");
    Ok(())
}

fn send_response<T: serde::Serialize>(
    id: &RequestId,
    connection: &Connection,
    result: Result<T, &str>,
) {
    let (result, error) = match result {
        Ok(value) => match serde_json::to_value(value) {
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
        Err(err) => (
            None,
            Some(ResponseError {
                code: ErrorCode::RequestFailed as i32,
                message: err.to_string(),
                data: None,
            }),
        ),
    };

    if let Err(err) = connection.sender.send(Message::Response(Response {
        id: id.clone(),
        result,
        error,
    })) {
        log!("Failed to send response: {err}");
    }
}

fn handle_goto_definition(
    id: RequestId,
    params: GotoDefinitionParams,
) -> Result<GotoDefinitionResponse, Box<dyn Error + Send + Sync>> {
    log!("Handling GotoDefinition: {:?}", params);
    Ok(GotoDefinitionResponse::Array(Vec::new()))
}

fn handle_hover(id: RequestId, params: HoverParams) -> Result<Hover, Box<dyn Error + Send + Sync>> {
    log!("Handling Hover request: {:?}", params);
    Ok(Hover {
        contents: HoverContents::Scalar(MarkedString::String("Hover info".to_string())),
        range: None,
    })
}
