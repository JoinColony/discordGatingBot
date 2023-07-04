//! Starts the server that handles the http requests coming from the urls
//! sent to discord.
//!

use crate::config::CONFIG;
use crate::controller::{
    Message, RegisterResponse, RemoveUserResponse, Session, CONTROLLER_CHANNEL,
};
use actix_web::{get, post, web, App, HttpResponse, HttpResponseBuilder, HttpServer, Responder};
use anyhow::{bail, Result};
use colony_rs::Signature;
use sailfish::TemplateOnce;
use secrecy::{ExposeSecret, SecretString};
use serde::Deserialize;
use std::str::FromStr;
use tokio::sync::oneshot;
use tracing::{debug, debug_span, error, info, instrument, warn};
use tracing_actix_web::TracingLogger;

static SIGN_SCRIPT: &str = include_str!("../../frontend/dist/index.js");
static FAVICON: &[u8] = include_bytes!("../static/favicon.ico");

const REGISTRATION_MESSAGE: &str = "Please sign this message to connect your \
                                    Discord username {username} with your wallet \
                                    address. Session ID: {session}";

pub async fn start() -> std::io::Result<()> {
    let host = CONFIG.wait().server.host.clone();
    let port = CONFIG.wait().server.port;
    info!("Starting server on {}:{}", &host, port);
    HttpServer::new(|| {
        App::new()
            .wrap(TracingLogger::default())
            .service(index)
            .service(favicon)
            .service(script)
            .service(registration_page)
            .service(register)
            .service(unregistration_page)
            .service(unregister)
    })
    .bind((host, port))?
    .run()
    .await
}

#[instrument]
#[get("/")]
async fn index() -> impl Responder {
    debug!("Received index request");
    Skeleton::index()
}

#[instrument]
#[get("/index.js")]
async fn script() -> impl Responder {
    debug!("Received script request");
    HttpResponse::Ok()
        .content_type("application/javascript")
        .body(SIGN_SCRIPT)
}

#[instrument]
#[get("/favicon.ico")]
async fn favicon() -> impl Responder {
    debug!("Received favicon request");
    HttpResponse::Ok()
        .content_type("image/x-icon")
        .body(FAVICON)
}

#[instrument]
#[get("/register/{username}/{session}")]
async fn registration_page(path: web::Path<(String, String)>) -> impl Responder {
    debug!("Received registration request");
    let (username_url, session_str) = path.into_inner();
    let session = match validate_session(&username_url, &session_str) {
        Ok(session) => session,
        Err(why) => {
            warn!("Invalid session: {}", why);
            return Skeleton::invalid_session(&why.to_string());
        }
    };
    debug!(?session, "Valid session");
    Skeleton::registration_page()
}

#[post("/register/{username}/{session}")]
#[instrument]
async fn register(path: web::Path<(String, String)>, data: web::Json<JsonData>) -> impl Responder {
    debug!("Received acknowledged registration request");
    let (username_url, session_str) = path.into_inner();
    let session = match validate_session(&username_url, &session_str) {
        Ok(session) => session,
        Err(why) => {
            warn!("Invalid session: {}", why);
            return Skeleton::invalid_session(&why.to_string());
        }
    };
    debug!(?session, "Valid session");
    let wallet = match validate_signature(&data, &session, &session_str) {
        Ok(wallet) => wallet,
        Err(why) => {
            warn!("Invalid signature: {}", why);
            return Skeleton::invalid_signature(&why.to_string());
        }
    };
    debug!(?wallet, "Valid signature");
    let (response_tx, rx) = oneshot::channel();
    let span = debug_span!("server_register", %session.username, %session.user_id);
    let message = Message::Register {
        user_id: session.user_id,
        wallet,
        response_tx,
        span,
    };
    if let Err(why) = CONTROLLER_CHANNEL.wait().send(message).await {
        error!("Error sending message to controller: {}", why);
        return Skeleton::internal_error();
    }
    if let Ok(response) = rx.await {
        match response {
            RegisterResponse::Success => {
                debug!("Registration successful");
                Skeleton::register_success()
            }
            RegisterResponse::AlreadyRegistered => {
                debug!("User already registered");
                Skeleton::already_registered()
            }
            RegisterResponse::Error(why) => {
                warn!("Internal registration error: {}", why);
                Skeleton::internal_error()
            }
        }
    } else {
        error!("Failed to receive response from controller");
        Skeleton::internal_error()
    }
}

#[get("/unregister/{username}/{session}")]
#[instrument]
async fn unregistration_page(path: web::Path<(String, String)>) -> impl Responder {
    debug!("Received unregister request");
    let (username_url, session_str) = path.into_inner();
    let session = match validate_session(&username_url, &session_str) {
        Ok(session) => session,
        Err(why) => {
            warn!("Invalid session");
            return Skeleton::invalid_session(&why.to_string());
        }
    };
    debug!(?session, "Valid session");
    Skeleton::unregistration_page()
}

#[post("/unregister/{username}/{session}")]
#[instrument]
async fn unregister(path: web::Path<(String, String)>) -> impl Responder {
    debug!("Received acknowledged unregistration request");
    let (username_url, session_str) = path.into_inner();
    let session = match validate_session(&username_url, &session_str) {
        Ok(session) => session,
        Err(why) => {
            warn!("Invalid session");
            return Skeleton::invalid_session(&why.to_string());
        }
    };
    let span = debug_span!("unregister", %session.username, %session.user_id);
    let (tx, rx) = oneshot::channel();
    let message = Message::RemovUser {
        session: session_str,
        response_tx: tx,
        span,
    };
    if let Err(why) = CONTROLLER_CHANNEL.wait().send(message).await {
        error!("Error sending message to controller: {}", why);
        return Skeleton::internal_error();
    }
    if let Ok(response) = rx.await {
        match response {
            RemoveUserResponse::Success => {
                debug!("Unregistration successful");
                Skeleton::unregister_success()
            }
            RemoveUserResponse::Error(why) => {
                error!("Error removing user: {}", why);
                Skeleton::internal_error()
            }
        }
    } else {
        error!("Controller hung up");
        Skeleton::internal_error()
    }
}

#[instrument(skip(data))]
fn validate_signature(
    data: &JsonData,
    session: &Session,
    session_str: &str,
) -> Result<SecretString> {
    let signature = Signature::from_str(data.signature.expose_secret())?;
    let message = REGISTRATION_MESSAGE
        .replace("{username}", &session.username)
        .replace("{session}", session_str);
    debug!(?message, "Message to verify");
    let wallet = colony_rs::Address::from_str(data.address.expose_secret())?;
    if let Err(why) = signature.verify(message, wallet) {
        warn!("Invalid message: {}", why);
        bail!("Invalid message");
    }
    Ok(data.address.clone())
}

#[instrument]
fn validate_session(username_url: &str, session_str: &str) -> Result<Session> {
    let session = Session::from_str(session_str)?;
    if session.expired() {
        debug!("Session expired");
        bail!("Session expired");
    }
    let username = urlencoding::decode(username_url)?;

    if username != session.username {
        warn!(
            "Username {} does not match session username {}",
            username, session.username
        );
        bail!("Invalid username");
    }
    Ok(session)
}

#[derive(Debug, Deserialize)]
struct JsonData {
    signature: SecretString,
    address: SecretString,
}

#[derive(Debug)]
struct Button {
    text: &'static str,
    link: String,
}

#[derive(Debug)]
struct FormInput {
    title: &'static str,
    method: &'static str,
    action: &'static str,
}

#[derive(Debug, TemplateOnce)]
#[template(path = "skeleton.stpl")]
struct Skeleton {
    index_script: Option<&'static str>,
    paragraph_text: String,
    button: Option<Button>,
    form_input: Option<FormInput>,
}

impl Skeleton {
    #[instrument(skip(response))]
    fn render_response(self, name: &str, mut response: HttpResponseBuilder) -> HttpResponse {
        match self.render_once() {
            Ok(html) => response.content_type("text/html").body(html),
            Err(why) => {
                error!("Error rendering {}: {}", name, why);
                HttpResponse::InternalServerError().finish()
            }
        }
    }

    #[instrument]
    fn index() -> HttpResponse {
        let link = CONFIG.wait().discord.invite_url.clone();
        Skeleton {
            index_script: None,
            paragraph_text: r#"
This is the <a href="https://colony.io">colony</a> discord bot. You can invite the bot to your discord server and then use the <b>/get in</b> and <b>/gate</b> commands there. After the bot joined, you must reorder the created bot role in the role hierarchy to be above all roles the bot should manage.
            "#.to_string(),
            button: Some(Button {
                text: "Invite Bot",
                link,
            }),
            form_input: None,
        }
        .render_response("index", HttpResponse::Ok())
    }

    #[instrument]
    fn invalid_session(reason: &str) -> HttpResponse {
        Skeleton {
            index_script: None,
            paragraph_text: format!("Invalid session: {}", reason),
            button: None,
            form_input: None,
        }
        .render_response("invalid session", HttpResponse::BadRequest())
    }

    #[instrument]
    fn registration_page() -> HttpResponse {
        Skeleton {
            index_script: Some("/index.js"),
            paragraph_text: "Welcome to the registration to the colony gating bot. \
            Here you can register your wallet address with metamask <br />"
                .to_string(),
            button: None,
            form_input: None,
        }
        .render_response("registration page", HttpResponse::Ok())
    }

    #[instrument]
    fn invalid_signature(reason: &str) -> HttpResponse {
        Skeleton {
            index_script: None,
            paragraph_text: format!("Invalid signature: {}", reason),
            button: None,
            form_input: None,
        }
        .render_response("invalid signature", HttpResponse::BadRequest())
    }

    #[instrument]
    fn register_success() -> HttpResponse {
        Skeleton {
            index_script: None,
            paragraph_text: "Registration successful".to_string(),
            button: None,
            form_input: None,
        }
        .render_response("register success", HttpResponse::Ok())
    }

    #[instrument]
    fn already_registered() -> HttpResponse {
        Skeleton {
            index_script: None,
            paragraph_text: "You are already registered".to_string(),
            button: None,
            form_input: None,
        }
        .render_response("already registered", HttpResponse::BadRequest())
    }

    #[instrument]
    fn internal_error() -> HttpResponse {
        Skeleton {
            index_script: None,
            paragraph_text: "Internal error".to_string(),
            button: None,
            form_input: None,
        }
        .render_response("internal error", HttpResponse::InternalServerError())
    }

    #[instrument]
    fn unregistration_page() -> HttpResponse {
        Skeleton {
            index_script: None,
            paragraph_text: "Welcome to the unregistration from the colony gating bot.\
            Here you can unregister your wallet address <br />"
                .to_string(),

            button: None,
            form_input: Some(FormInput {
                title: "Unregister",
                method: "POST",
                action: "",
            }),
        }
        .render_response("unregistration page", HttpResponse::Ok())
    }

    fn unregister_success() -> HttpResponse {
        Skeleton {
            index_script: None,
            paragraph_text: "Deregistration successful! Hope to see you again \
            soon! Use <b>/get in</b> to register again."
                .to_string(),
            button: None,
            form_input: None,
        }
        .render_response("unregister success", HttpResponse::Ok())
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn recover_from_ok_signature() {
        let address =
            colony_rs::Address::from_str("0xcB313f361847e245954FD338Cb21b5F4225b17d1").unwrap();
        let message = "Please sign this message to connect your Discord username hmuendel with your wallet address. Session ID: b2a76f67b6c1bdf61cea3b2c.046c5bfeea4351a17b8be03a516380a13ebd1396d69a57ff306a3249fc6d0763d3071171cda9d1f6250e7a3b82344fccd85c7ca92da0";
        let signature_str = "0x092e15f49b64ae802fa4d5e8d2439e92a174b23dabe99650191f1028377d4e7711952f199bf84f5e49868b9db68ef2ce1f7ab5dbeb34afa6393d517afc42cd251c";
        let signature = Signature::from_str(signature_str).unwrap();
        let recovered_address = signature.recover(message).unwrap();
        assert_eq!(address, recovered_address);
    }

    #[test]
    fn recover_from_bad_signature() {
        let address =
            colony_rs::Address::from_str("0xcB313f361847e245954FD338Cb21b5F4225b17d1").unwrap();
        let message = "obviously wrong message";
        let signature_str = "0x092e15f49b64ae802fa4d5e8d2439e92a174b23dabe99650191f1028377d4e7711952f199bf84f5e49868b9db68ef2ce1f7ab5dbeb34afa6393d517afc42cd251c";
        let signature = Signature::from_str(signature_str).unwrap();
        let recovered_address = signature.recover(message).unwrap();
        assert_ne!(address, recovered_address);
    }
}
