//! Starts the server that handles the http requests coming from the urls
//! sent to discord.
//!

use crate::config::CONFIG;
use crate::controller::{Message, RegisterResponse, Session, CONTROLLER_CHANNEL};
use actix_web::{get, post, web, App, HttpResponse, HttpServer, Responder};
use serde::Deserialize;
use std::str::FromStr;
use tokio::sync::oneshot;
use tracing::{debug, info};

static REGISTRATION_FORM: &'static str = include_str!("../static/registration.html");
static INDEX: &'static str = include_str!("../static/index.html");
static SUCCESS: &'static str = include_str!("../static/registration-success.html");
static EXPIRED: &'static str = include_str!("../static/registration-expired.html");
static FAVICON: &'static [u8] = include_bytes!("../static/favicon.ico");
static ALREADY_REGISTERED: &'static str = include_str!("../static/registered-already.html");
static INVALID_SESSION: &'static str = include_str!("../static/registration-invalid-session.html");
static ERROR: &'static str = include_str!("../static/error.html");

#[post("/session/{session}")]
async fn register(session_str: web::Path<String>, data: web::Form<FormData>) -> impl Responder {
    debug!("Received registration request for session {}", session_str);
    let session = match Session::from_str(&session_str) {
        Ok(session) => session,
        Err(_) => return HttpResponse::BadRequest().body(INVALID_SESSION),
    };
    if session.expired() {
        return HttpResponse::BadRequest().body(EXPIRED);
    }

    let (tx, rx) = oneshot::channel();

    let message = Message::Register {
        user_id: session.user_id,
        wallet: data.wallet.clone(),
        response_tx: tx,
    };

    CONTROLLER_CHANNEL
        .get()
        .unwrap()
        .send(message)
        .await
        .unwrap();

    if let Ok(response) = rx.await {
        match response {
            RegisterResponse::Success => HttpResponse::Ok().body(SUCCESS),
            RegisterResponse::AlreadyRegistered => {
                HttpResponse::BadRequest().body(ALREADY_REGISTERED)
            }
        }
    } else {
        HttpResponse::InternalServerError().body(ERROR)
    }
}

#[get("/session/{session}")]
async fn registration_form(session_str: web::Path<String>) -> impl Responder {
    debug!(
        "Received registration form request for session {}",
        session_str
    );
    let session = match Session::from_str(&session_str) {
        Ok(session) => session,
        Err(_) => return HttpResponse::BadRequest().body(INVALID_SESSION),
    };
    if session.expired() {
        return HttpResponse::BadRequest().body(EXPIRED);
    }
    HttpResponse::Ok().body(REGISTRATION_FORM)
}

#[get("/{favicon.ico}")]
async fn favicon() -> impl Responder {
    debug!("Received favicon request");
    HttpResponse::Ok().body(FAVICON)
}

#[get("/")]
async fn index() -> impl Responder {
    debug!("Received index request");
    HttpResponse::Ok().body(INDEX)
}

pub async fn start() -> std::io::Result<()> {
    let host = CONFIG.wait().server.host.clone();
    let port = CONFIG.wait().server.port;
    info!("Starting server on {}:{}", host, port);
    HttpServer::new(|| {
        App::new()
            .service(index)
            .service(favicon)
            .service(register)
            .service(registration_form)
    })
    .bind((host, port))?
    .run()
    .await
}

#[derive(Deserialize)]
struct FormData {
    wallet: String,
}
