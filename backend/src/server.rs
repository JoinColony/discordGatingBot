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

const REGISTRATION_MESSAGE: &str = "Please sign this message to connect your Discord username {username} with your wallet address. Session ID: {session}";

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
    HttpResponse::Ok().body(index.render_once().unwrap())
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

#[get("/register/{username}/{session}")]
async fn registration_form(path: web::Path<(String, String)>) -> impl Responder {
    debug!("Received registration form request for session {}", path.1);
    let (username_url, session_str) = path.into_inner();
    let session = match Session::from_str(&session_str) {
        Ok(session) => session,
        Err(_) => {
            warn!("Invalid session {}", session_str);
            let invalid_session = Skeleton::invalid_session();
            return HttpResponse::BadRequest().body(invalid_session.render_once().unwrap());
        }
    };
    if session.expired() {
        debug!("Session {} expired", session_str);
        let expired_session = Skeleton::expired_session();
        return HttpResponse::BadRequest().body(expired_session.render_once().unwrap());
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
        return HttpResponse::BadRequest().body(invalid_username.render_once().unwrap());
    }
    let registration_form = Skeleton::registration_form();
    HttpResponse::Ok().body(registration_form.render_once().unwrap())
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
            return HttpResponse::BadRequest().body(invalid_session.render_once().unwrap());
        }
    };
    if session.expired() {
        debug!("Session {} expired", session_str);
        let expired_session = Skeleton::expired_session();
        return HttpResponse::BadRequest().body(expired_session.render_once().unwrap());
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
        return HttpResponse::BadRequest().body(invalid_username.render_once().unwrap());
    }

    let signature = match Signature::from_str(&data.signature) {
        Ok(signature) => signature,
        Err(_) => {
            warn!("Invalid signature {}", data.signature);
            let invalid_signature = Skeleton::invalid_signature();
            return HttpResponse::BadRequest().body(invalid_signature.render_once().unwrap());
        }
    };

    let message = REGISTRATION_MESSAGE
        .replace("{username}", &session.username)
        .replace("{session}", &session_str);

    let wallet = match signature.recover(message.clone()) {
        Ok(wallet) => wallet,
        Err(_) => {
            warn!("Failed to recover wallet from signature");
            let invalid_signature = Skeleton::invalid_signature();
            return HttpResponse::BadRequest().body(invalid_signature.render_once().unwrap());
        }
    };
    debug!(
        "Recovered wallet {:?} from signature: {:?} and message: {}",
        wallet, &data.signature, message
    );

    if signature.verify(message.clone(), wallet).is_err() {
        warn!(
            "Invalid signature {} for message {}",
            data.signature, message
        );
        let invalid_signature = Skeleton::invalid_signature();
        return HttpResponse::BadRequest().body(invalid_signature.render_once().unwrap());
    }

    let (tx, rx) = oneshot::channel();

    let message = Message::Register {
        user_id: session.user_id,
        wallet: format!("{:?}", wallet),
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
            RegisterResponse::Success => {
                debug!("Registration successful");
                let registration_success = Skeleton::register_success();
                HttpResponse::Ok().body(registration_success.render_once().unwrap())
            }
            RegisterResponse::AlreadyRegistered => {
                debug!("User already registered");
                let already_registered = Skeleton::already_registered();
                HttpResponse::BadRequest().body(already_registered.render_once().unwrap())
            }
        }
    } else {
        error!("Failed to receive response from controller");
        let internal_error = Skeleton::internal_error();
        HttpResponse::InternalServerError().body(internal_error.render_once().unwrap())
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
            return HttpResponse::BadRequest().body(invalid_session.render_once().unwrap());
        }
    };
    if session.expired() {
        debug!("Session {} expired", session_str);
        let expired_session = Skeleton::expired_session();
        return HttpResponse::BadRequest().body(expired_session.render_once().unwrap());
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
    HttpResponse::Ok().body(unregistration_form.render_once().unwrap())
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
            return HttpResponse::BadRequest().body(invalid_session.render_once().unwrap());
        }
    };
    if session.expired() {
        debug!("Session {} expired", session_str);
        let expired_session = Skeleton::expired_session();
        return HttpResponse::BadRequest().body(expired_session.render_once().unwrap());
    }
    let username = match urlencoding::decode(&username_url) {
        Ok(username) => username,
        Err(_) => {
            debug!("Failed to decode username {}", username_url);
            let invalid_username = Skeleton::invalid_username();
            return HttpResponse::BadRequest().body(invalid_username.render_once().unwrap());
        }
    };

    if username != session.username {
        warn!(
            "Username {} does not match session username {}",
            username, session.username
        );
        let invalid_username = Skeleton::invalid_username();
        return HttpResponse::BadRequest().body(invalid_username.render_once().unwrap());
    }

    let message = Message::RemovUser {
        user_id: session.user_id,
    };

    CONTROLLER_CHANNEL
        .get()
        .unwrap()
        .send(message)
        .await
        .unwrap();
    let unregistration_success = Skeleton::unregister_success();
    HttpResponse::Ok().body(unregistration_success.render_once().unwrap())
}

#[derive(Deserialize)]
struct JsonData {
    signature: String,
}

#[derive(TemplateOnce)]
#[template(path = "skeleton.stpl", escape = false)]
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
                link: link,
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
            paragraph_text: "Welcome to the registration to the colony gating bot.\
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
            Here you can unregister your wallet address with metamask <br />"
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
