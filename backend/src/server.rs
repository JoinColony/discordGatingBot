//! Starts the server that handles the http requests coming from the urls
//! sent to discord.
//!

use crate::config::CONFIG;
use crate::controller::{Message, RegisterResponse, Session, CONTROLLER_CHANNEL};
use actix_web::{get, post, web, App, HttpResponse, HttpServer, Responder};
use colony_rs::Signature;
use sailfish::TemplateOnce;
use serde::Deserialize;
use std::str::FromStr;
use tokio::sync::oneshot;
use tracing::{debug, error, info, warn};
use urlencoding;

static SIGN_SCRIPT: &'static str = include_str!("../../frontend/dist/index.js");
static FAVICON: &'static [u8] = include_bytes!("../static/favicon.ico");

const REGISTRATION_MESSAGE: &str = "Please sign this message to connect your \
                                    Discord username {username} with your wallet \
                                    address. Session ID: {session}";

pub async fn start() -> std::io::Result<()> {
    let host = CONFIG.wait().server.host.clone();
    let port = CONFIG.wait().server.port;
    info!("Starting server on {}:{}", host, port);
    HttpServer::new(|| {
        App::new()
            .service(index)
            .service(favicon)
            .service(script)
            .service(registration_form)
            .service(register)
            .service(unregistration_form)
            .service(unregister)
    })
    .bind((host, port))?
    .run()
    .await
}

#[get("/")]
async fn index() -> impl Responder {
    debug!("Received index request");
    let index = Skeleton::index();
    match index.render_once() {
        Ok(html) => HttpResponse::Ok().content_type("text/html").body(html),
        Err(why) => {
            error!("Error rendering index: {}", why);
            HttpResponse::InternalServerError().finish()
        }
    }
}

#[get("/index.js")]
async fn script() -> impl Responder {
    debug!("Received script request");
    HttpResponse::Ok()
        .content_type("application/javascript")
        .body(SIGN_SCRIPT)
}

#[get("/favicon.ico")]
async fn favicon() -> impl Responder {
    debug!("Received favicon request");
    HttpResponse::Ok()
        .content_type("image/x-icon")
        .body(FAVICON)
}

#[get("/register/{username}/{session}")]
async fn registration_form(path: web::Path<(String, String)>) -> impl Responder {
    debug!("Received registration form request for session {}", path.1);
    let (username_url, session_str) = path.into_inner();
    let session = match Session::from_str(&session_str) {
        Ok(session) => session,
        Err(_) => {
            warn!("Invalid session {}", session_str);
            let invalid_session = Skeleton::invalid_session();
            match invalid_session.render_once() {
                Ok(html) => {
                    return HttpResponse::BadRequest()
                        .content_type("text/html")
                        .body(html)
                }
                Err(why) => {
                    error!("Error rendering invalid session: {}", why);
                    return HttpResponse::InternalServerError().finish();
                }
            }
        }
    };
    if session.expired() {
        debug!("Session {} expired", session_str);
        let expired_session = Skeleton::expired_session();
        match expired_session.render_once() {
            Ok(html) => {
                return HttpResponse::BadRequest()
                    .content_type("text/html")
                    .body(html)
            }
            Err(why) => {
                error!("Error rendering expired session: {}", why);
                return HttpResponse::InternalServerError().finish();
            }
        }
    }
    let username = match urlencoding::decode(&username_url) {
        Ok(username) => username,
        Err(_) => {
            warn!("Invalid username {}", username_url);
            return HttpResponse::BadRequest().body("Decoding error");
        }
    };

    if username != session.username {
        error!(
            "Invalid username for session {} != {}",
            username, session.username
        );
        let invalid_username = Skeleton::invalid_username();
        match invalid_username.render_once() {
            Ok(html) => {
                return HttpResponse::BadRequest()
                    .content_type("text/html")
                    .body(html)
            }
            Err(why) => {
                error!("Error rendering invalid username: {}", why);
                return HttpResponse::InternalServerError().finish();
            }
        }
    }
    let registration_form = Skeleton::registration_form();
    match registration_form.render_once() {
        Ok(html) => HttpResponse::Ok().content_type("text/html").body(html),
        Err(why) => {
            error!("Error rendering registration form: {}", why);
            HttpResponse::InternalServerError().finish()
        }
    }
}

#[post("/register/{username}/{session}")]
async fn register(path: web::Path<(String, String)>, data: web::Json<JsonData>) -> impl Responder {
    debug!(
        "Received registration request for session: {}, payload: {}",
        path.1, data.signature
    );
    let (username_url, session_str) = path.into_inner();
    let session = match Session::from_str(&session_str) {
        Ok(session) => session,
        Err(_) => {
            warn!("Invalid session {}", session_str);
            let invalid_session = Skeleton::invalid_session();
            match invalid_session.render_once() {
                Ok(html) => {
                    return HttpResponse::BadRequest()
                        .content_type("text/html")
                        .body(html)
                }
                Err(why) => {
                    error!("Error rendering invalid session: {}", why);
                    return HttpResponse::InternalServerError().finish();
                }
            }
        }
    };
    if session.expired() {
        debug!("Session {} expired", session_str);
        let expired_session = Skeleton::expired_session();
        match expired_session.render_once() {
            Ok(html) => {
                return HttpResponse::BadRequest()
                    .content_type("text/html")
                    .body(html)
            }
            Err(why) => {
                error!("Error rendering expired session: {}", why);
                return HttpResponse::InternalServerError().finish();
            }
        }
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
        let invalid_username = Skeleton::invalid_username();
        match invalid_username.render_once() {
            Ok(html) => {
                return HttpResponse::BadRequest()
                    .content_type("text/html")
                    .body(html)
            }
            Err(why) => {
                error!("Error rendering invalid username: {}", why);
                return HttpResponse::InternalServerError().finish();
            }
        }
    }

    let signature = match Signature::from_str(&data.signature) {
        Ok(signature) => signature,
        Err(_) => {
            warn!("Invalid signature {}", data.signature);
            let invalid_signature = Skeleton::invalid_signature();
            match invalid_signature.render_once() {
                Ok(html) => {
                    return HttpResponse::BadRequest()
                        .content_type("text/html")
                        .body(html)
                }
                Err(why) => {
                    error!("Error rendering invalid signature: {}", why);
                    return HttpResponse::InternalServerError().finish();
                }
            }
        }
    };

    let message = REGISTRATION_MESSAGE
        .replace("{username}", &session.username)
        .replace("{session}", &session_str);
    let wallet = match colony_rs::Address::from_str(&data.address) {
        Ok(wallet) => wallet,
        Err(_) => {
            warn!("Invalid wallet {}", data.address);
            let invalid_wallet = Skeleton::invalid_address();
            match invalid_wallet.render_once() {
                Ok(html) => {
                    return HttpResponse::BadRequest()
                        .content_type("text/html")
                        .body(html)
                }
                Err(why) => {
                    error!("Error rendering invalid wallet: {}", why);
                    return HttpResponse::InternalServerError().finish();
                }
            }
        }
    };

    if signature.verify(message.clone(), wallet).is_err() {
        warn!(
            "Invalid signature {} for message {}",
            data.signature, message
        );
        let invalid_signature = Skeleton::invalid_signature();
        match invalid_signature.render_once() {
            Ok(html) => {
                return HttpResponse::BadRequest()
                    .content_type("text/html")
                    .body(html)
            }
            Err(why) => {
                error!("Error rendering invalid signature: {}", why);
                return HttpResponse::InternalServerError().finish();
            }
        }
    }

    let (tx, rx) = oneshot::channel();

    let message = Message::Register {
        user_id: session.user_id,
        wallet: format!("{:?}", wallet),
        response_tx: tx,
    };

    if let Err(why) = CONTROLLER_CHANNEL.wait().send(message).await {
        error!("Error sending message to controller: {}", why);
        return HttpResponse::InternalServerError().finish();
    }

    if let Ok(response) = rx.await {
        match response {
            RegisterResponse::Success => {
                debug!("Registration successful");
                let registration_success = Skeleton::register_success();
                match registration_success.render_once() {
                    Ok(html) => return HttpResponse::Ok().content_type("text/html").body(html),
                    Err(why) => {
                        error!("Error rendering registration success: {}", why);
                        return HttpResponse::InternalServerError().finish();
                    }
                }
            }
            RegisterResponse::AlreadyRegistered => {
                debug!("User already registered");
                let already_registered = Skeleton::already_registered();
                match already_registered.render_once() {
                    Ok(html) => {
                        return HttpResponse::BadRequest()
                            .content_type("text/html")
                            .body(html)
                    }
                    Err(why) => {
                        error!("Error rendering already registered: {}", why);
                        return HttpResponse::InternalServerError().finish();
                    }
                }
            }
            RegisterResponse::Error(why) => {
                warn!("Internal registration error: {}", why);
                let registration_error = Skeleton::internal_error();
                match registration_error.render_once() {
                    Ok(html) => {
                        return HttpResponse::InternalServerError()
                            .content_type("text/html")
                            .body(html)
                    }
                    Err(why) => {
                        error!("Error rendering internal error: {}", why);
                        return HttpResponse::InternalServerError().finish();
                    }
                }
            }
        }
    } else {
        error!("Failed to receive response from controller");
        let internal_error = Skeleton::internal_error();
        match internal_error.render_once() {
            Ok(html) => {
                return HttpResponse::InternalServerError()
                    .content_type("text/html")
                    .body(html)
            }
            Err(why) => {
                error!("Error rendering internal error: {}", why);
                return HttpResponse::InternalServerError().finish();
            }
        }
    }
}

#[get("/unregister/{username}/{session}")]
async fn unregistration_form(path: web::Path<(String, String)>) -> impl Responder {
    debug!("Received unregister form request for session {}", path.1);
    let (username_url, session_str) = path.into_inner();
    let session = match Session::from_str(&session_str) {
        Ok(session) => session,
        Err(_) => {
            warn!("Invalid session {}", session_str);
            let invalid_session = Skeleton::invalid_session();
            match invalid_session.render_once() {
                Ok(html) => {
                    return HttpResponse::BadRequest()
                        .content_type("text/html")
                        .body(html)
                }
                Err(why) => {
                    error!("Error rendering invalid session: {}", why);
                    return HttpResponse::InternalServerError().finish();
                }
            }
        }
    };
    if session.expired() {
        debug!("Session {} expired", session_str);
        let expired_session = Skeleton::expired_session();
        match expired_session.render_once() {
            Ok(html) => {
                return HttpResponse::BadRequest()
                    .content_type("text/html")
                    .body(html)
            }
            Err(why) => {
                error!("Error rendering expired session: {}", why);
                return HttpResponse::InternalServerError().finish();
            }
        }
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
    let unregistration_form = Skeleton::unregistration_form();
    match unregistration_form.render_once() {
        Ok(html) => HttpResponse::Ok().content_type("text/html").body(html),
        Err(why) => {
            error!("Error rendering unregistration form: {}", why);
            HttpResponse::InternalServerError().finish()
        }
    }
}

#[post("/unregister/{username}/{session}")]
async fn unregister(path: web::Path<(String, String)>) -> impl Responder {
    debug!("Received unregistration request for session: {}", path.1);
    let (username_url, session_str) = path.into_inner();
    let session = match Session::from_str(&session_str) {
        Ok(session) => session,
        Err(_) => {
            warn!("Invalid session {}", session_str);
            let invalid_session = Skeleton::invalid_session();
            match invalid_session.render_once() {
                Ok(html) => {
                    return HttpResponse::BadRequest()
                        .content_type("text/html")
                        .body(html)
                }
                Err(why) => {
                    error!("Error rendering invalid session: {}", why);
                    return HttpResponse::InternalServerError().finish();
                }
            }
        }
    };
    if session.expired() {
        debug!("Session {} expired", session_str);
        let expired_session = Skeleton::expired_session();
        match expired_session.render_once() {
            Ok(html) => {
                return HttpResponse::BadRequest()
                    .content_type("text/html")
                    .body(html)
            }
            Err(why) => {
                error!("Error rendering expired session: {}", why);
                return HttpResponse::InternalServerError().finish();
            }
        }
    }
    let username = match urlencoding::decode(&username_url) {
        Ok(username) => username,
        Err(_) => {
            debug!("Failed to decode username {}", username_url);
            let invalid_username = Skeleton::invalid_username();
            match invalid_username.render_once() {
                Ok(html) => {
                    return HttpResponse::BadRequest()
                        .content_type("text/html")
                        .body(html)
                }
                Err(why) => {
                    error!("Error rendering invalid username: {}", why);
                    return HttpResponse::InternalServerError().finish();
                }
            }
        }
    };

    if username != session.username {
        warn!(
            "Username {} does not match session username {}",
            username, session.username
        );
        let invalid_username = Skeleton::invalid_username();
        match invalid_username.render_once() {
            Ok(html) => {
                return HttpResponse::BadRequest()
                    .content_type("text/html")
                    .body(html)
            }
            Err(why) => {
                error!("Error rendering invalid username: {}", why);
                return HttpResponse::InternalServerError().finish();
            }
        }
    }

    let message = Message::RemovUser {
        user_id: session.user_id,
    };

    if let Err(why) = CONTROLLER_CHANNEL.wait().send(message).await {
        error!("Failed to send message to controller: {}", why);
        return HttpResponse::InternalServerError().finish();
    }

    let unregistration_success = Skeleton::unregister_success();
    match unregistration_success.render_once() {
        Ok(html) => HttpResponse::Ok().content_type("text/html").body(html),
        Err(why) => {
            error!("Error rendering unregistration success: {}", why);
            HttpResponse::InternalServerError().finish()
        }
    }
}

#[derive(Deserialize)]
struct JsonData {
    signature: String,
    address: String,
}

#[derive(TemplateOnce)]
#[template(path = "skeleton.stpl")]
struct Skeleton {
    index_script: Option<String>,
    paragraph_text: String,
    button: Option<Button>,
    form_input: Option<FormInput>,
}

struct Button {
    text: String,
    link: String,
}

struct FormInput {
    title: String,
    method: String,
    action: String,
}

impl Skeleton {
    fn index() -> Self {
        let link = CONFIG.wait().discord.invite_url.clone();
        Skeleton {
            index_script: None,
            paragraph_text: r#"
This is the <a href="https://colony.io">colony</a> discord bot. You can invite the bot to your discord server and then use the <b>/check</b> and <b>/gate</b> commands there. After the bot joined, you must reorder the created bot role in the role hierarchy to be above all roles the bot should manage.
            "#.to_string(),
            button: Some(Button {
                text: "Invite Bot".to_string(),
                link,
            }),
            form_input: None,
        }
    }

    fn invalid_session() -> Self {
        Skeleton {
            index_script: None,
            paragraph_text: "Invalid session".to_string(),
            button: None,
            form_input: None,
        }
    }

    fn expired_session() -> Self {
        Skeleton {
            index_script: None,
            paragraph_text: "Session expired".to_string(),
            button: None,
            form_input: None,
        }
    }

    fn invalid_username() -> Self {
        Skeleton {
            index_script: None,
            paragraph_text: "Invalid username".to_string(),
            button: None,
            form_input: None,
        }
    }

    fn registration_form() -> Self {
        Skeleton {
            index_script: Some("/index.js".to_string()),
            paragraph_text: "Welcome to the registration to the colony gating bot. \
            Here you can register your wallet address with metamask <br />"
                .to_string(),
            button: None,
            form_input: None,
        }
    }

    fn invalid_signature() -> Self {
        Skeleton {
            index_script: None,
            paragraph_text: "Invalid signature".to_string(),
            button: None,
            form_input: None,
        }
    }

    fn invalid_address() -> Self {
        Skeleton {
            index_script: None,
            paragraph_text: "Invalid address".to_string(),
            button: None,
            form_input: None,
        }
    }

    fn register_success() -> Self {
        Skeleton {
            index_script: None,
            paragraph_text: "Registration successful".to_string(),
            button: None,
            form_input: None,
        }
    }

    fn already_registered() -> Self {
        Skeleton {
            index_script: None,
            paragraph_text: "You are already registered".to_string(),
            button: None,
            form_input: None,
        }
    }

    fn internal_error() -> Self {
        Skeleton {
            index_script: None,
            paragraph_text: "Internal error".to_string(),
            button: None,
            form_input: None,
        }
    }

    fn unregistration_form() -> Self {
        Skeleton {
            index_script: None,
            paragraph_text: "Welcome to the unregistration to the colony gating bot.\
            Here you can unregister your wallet address <br />"
                .to_string(),
            button: None,
            form_input: Some(FormInput {
                title: "Unregister".to_string(),
                method: "POST".to_string(),
                action: "".to_string(),
            }),
        }
    }

    fn unregister_success() -> Self {
        Skeleton {
            index_script: None,
            paragraph_text: "Deregistration successful! Hope to see you again \
            soon! Use <b>/check</b> to register again."
                .to_string(),
            button: None,
            form_input: None,
        }
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
