//! Starts the server that handles the http requests coming from the urls
//! sent to discord.
//!

use crate::config::CONFIG;
use crate::controller::{Message, RegisterResponse, Session, CONTROLLER_CHANNEL};
use actix_web::{get, post, web, App, HttpResponse, HttpServer, Responder};
use colony_rs::validate_signature;
use colony_rs::Signature;
use sailfish::TemplateOnce;
use serde::Deserialize;
use std::str::FromStr;
use tokio::sync::oneshot;
use tracing::{debug, error, info, warn};
use urlencoding;

static REGISTRATION_FORM: &'static str = include_str!("../static/registration.html");
static INDEX: &'static str = include_str!("../static/index.html");
static SUCCESS: &'static str = include_str!("../static/registration-success.html");
static EXPIRED: &'static str = include_str!("../static/registration-expired.html");
static FAVICON: &'static [u8] = include_bytes!("../static/favicon.ico");
static ALREADY_REGISTERED: &'static str = include_str!("../static/registered-already.html");
static INVALID_SESSION: &'static str = include_str!("../static/registration-invalid-session.html");
static ERROR: &'static str = include_str!("../static/error.html");
static SIGN_SCRIPT: &'static str = include_str!("../../frontend/dist/index.js");

const REGISTRATION_MESSAGE: &str = "PPlease sign this message to connect your Discord username {username} with your wallet address. Session ID: {session}";

#[post("/register/{username}/{session}")]
async fn register(path: web::Path<(String, String)>, data: web::Json<JsonData>) -> impl Responder {
    debug!("Received registration request for session {}", path.1);
    let (username_url, session_str) = path.into_inner();
    let session = match Session::from_str(&session_str) {
        Ok(session) => session,
        Err(_) => return HttpResponse::BadRequest().body(INVALID_SESSION),
    };
    if session.expired() {
        debug!("Session {} expired", session_str);
        return HttpResponse::BadRequest().body(EXPIRED);
    }
    let username = match urlencoding::decode(&username_url) {
        Ok(username) => username,
        Err(_) => {
            debug!("Failed to decode username {}", username_url);
            return HttpResponse::BadRequest().body("Decoding username failed");
        }
    };

    if username != session.username {
        warn!(
            "Username {} does not match session username {}",
            username, session.username
        );
        return HttpResponse::BadRequest().body(INVALID_SESSION);
    }

    let signature = match Signature::from_str(&data.signature) {
        Ok(signature) => signature,
        Err(_) => return HttpResponse::BadRequest().body(INVALID_SESSION),
    };

    let message = REGISTRATION_MESSAGE
        .replace("{username}", &session.username)
        .replace("{session}", &session_str);

    let wallet = match signature.recover(message.clone()) {
        Ok(wallet) => wallet,
        Err(_) => {
            warn!("Failed to recover wallet from signature");
            return HttpResponse::BadRequest().body(INVALID_SESSION);
        }
    };

    if validate_signature(&wallet, &message, &data.signature).is_err() {
        warn!("Invalid signature for session {}", session_str);
        return HttpResponse::BadRequest().body(INVALID_SESSION);
    }

    let (tx, rx) = oneshot::channel();

    let message = Message::Register {
        user_id: session.user_id,
        wallet: wallet.to_string(),
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

#[get("/register/{username}/{session}")]
async fn registration_form(path: web::Path<(String, String)>) -> impl Responder {
    debug!("Received registration form request for session {}", path.1);
    let (username_url, session_str) = path.into_inner();
    let session = match Session::from_str(&session_str) {
        Ok(session) => session,
        Err(_) => return HttpResponse::BadRequest().body(INVALID_SESSION),
    };
    if session.expired() {
        return HttpResponse::BadRequest().body(EXPIRED);
    }
    let username = match urlencoding::decode(&username_url) {
        Ok(username) => username,
        Err(_) => return HttpResponse::BadRequest().body("Decoding error"),
    };

    if username != session.username {
        error!(
            "Invalid username for session {} != {}",
            username, session.username
        );
        return HttpResponse::BadRequest().body("Invalid Username");
    }
    HttpResponse::Ok().body(REGISTRATION_FORM)
}

#[get("/index.js")]
async fn script() -> impl Responder {
    debug!("Received script request");
    HttpResponse::Ok().body(SIGN_SCRIPT)
}

#[get("/favicon.ico")]
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
            .service(script)
            .service(register)
            .service(registration_form)
    })
    .bind((host, port))?
    .run()
    .await
}

#[derive(Deserialize)]
struct JsonData {
    signature: String,
}

#[derive(TemplateOnce)]
#[template(path = "index.js")]
struct ScriptTemplate {
    signing_message: String,
}
