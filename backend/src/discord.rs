//! Handles the communication with the Discord API.
//!

use std::str::FromStr;

use crate::config::CONFIG;
use crate::controller::{self, CheckResponse, UnRegisterResponse, CONTROLLER_CHANNEL};
use futures::{stream, StreamExt};
use serenity::builder::CreateApplicationCommand;
use serenity::model::prelude::RoleId;
use serenity::{
    async_trait,
    http::Http,
    model::{
        application::{
            command::Command,
            interaction::{
                application_command::{ApplicationCommandInteraction, CommandDataOptionValue},
                Interaction, InteractionResponseType,
            },
        },
        gateway::{GatewayIntents, Ready},
        id::GuildId,
        permissions::Permissions,
        prelude::command::CommandOptionType,
    },
    prelude::*,
    utils::MessageBuilder,
};
use std::time::Duration;
use tokio::sync::oneshot;
use tracing::{debug, error, info, warn};

pub async fn start() {
    let token = &CONFIG.wait().discord.token.clone();
    let mut client = Client::builder(token, GatewayIntents::empty())
        .event_handler(Handler)
        .await
        .expect("Error creating client");
    if let Err(why) = client.start().await {
        error!("Client error: {:?}", why);
    }
}

pub async fn register_guild_slash_commands(guild_id: u64) {
    let token = &CONFIG.wait().discord.token.clone();
    debug!("Registering slash commands for guild {}", guild_id);
    let guild_id = GuildId(guild_id);
    let http = Http::new(&token);
    let resp = http
        .get_current_application_info()
        .await
        .expect("Failed to get application info");
    let app_id = resp.id;
    http.set_application_id(app_id.into());
    let command_result = GuildId::set_application_commands(&guild_id, &http, |commands| {
        commands
            .create_application_command(make_gate_command)
            .create_application_command(make_list_gates_command)
            .create_application_command(make_check_command)
            .create_application_command(make_checkout_command)
    })
    .await;
    if let Err(why) = command_result {
        error!("Error registering guild slash commands: {:?}", why);
    }
}

pub async fn delete_guild_slash_commands(guild_id: u64) {
    let token = &CONFIG.wait().discord.token.clone();
    debug!("Deleting slash commands for guild {}", guild_id);
    let guild_id = GuildId(guild_id);
    let http = Http::new(&token);
    let resp = http
        .get_current_application_info()
        .await
        .expect("Failed to get application info");
    let app_id = resp.id;
    http.set_application_id(app_id.into());
    let commands = guild_id
        .get_application_commands(&http)
        .await
        .expect("Failed to get guild commands");
    for command in commands {
        if let Err(why) = guild_id.delete_application_command(&http, command.id).await {
            error!("Error deleting guild slash commands: {:?}", why);
        }
    }
}

pub async fn register_global_slash_commands() {
    let token = &CONFIG.wait().discord.token.clone();
    debug!("Registering slash commands globally");
    let http = Http::new(&token);
    let resp = http
        .get_current_application_info()
        .await
        .expect("Failed to get application info");
    let app_id = resp.id;
    http.set_application_id(app_id.into());
    if let Err(why) = Command::create_global_application_command(&http, make_gate_command).await {
        error!("Error creating global slash command gate: {:?}", why);
    }
    if let Err(why) =
        Command::create_global_application_command(&http, make_list_gates_command).await
    {
        error!("Error creating global slash command list gates: {:?}", why);
    }
    if let Err(why) = Command::create_global_application_command(&http, make_check_command).await {
        error!("Error creating global slash command check: {:?}", why);
    }
    if let Err(why) = Command::create_global_application_command(&http, make_checkout_command).await
    {
        error!("Error creating global slash command checkout: {:?}", why);
    }
}

pub async fn delete_global_slash_commands() {
    let token = &CONFIG.wait().discord.token.clone();
    debug!("Deleting slash commands globally");
    let http = Http::new(&token);
    let resp = http
        .get_current_application_info()
        .await
        .expect("Failed to get application info");
    let app_id = resp.id;
    http.set_application_id(app_id.into());
    let commands = Command::get_global_application_commands(&http)
        .await
        .expect("Failed to get global commands");
    for command in commands {
        if let Err(why) = Command::delete_global_application_command(&http, command.id).await {
            error!(
                "Error deleting global slash command {}: {:?}",
                command.id, why
            );
        }
    }
}

/// The handler for the Discord client.
struct Handler;

#[async_trait]
impl EventHandler for Handler {
    async fn ready(&self, _ctx: Context, ready: Ready) {
        info!("{} is connected!", ready.user.name);
    }
    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        if let Interaction::ApplicationCommand(command) = interaction {
            let interaction_response = match command.data.name.as_str() {
                "gate" => gate_interaction_response(&command, &ctx).await,
                "list_gates" => list_gates_interaction_response(&command, &ctx).await,
                "check" => check_interaction_response(&command, &ctx).await,
                "checkout" => checkout_interaction_response(&command, &ctx).await,
                _ => unknown_interaction_response(&command, &ctx).await,
            };
            if let Err(why) = interaction_response {
                warn!("Error responding to interaction: {:?}", why);
            }
        }
    }
}

async fn gate_interaction_response(
    command: &ApplicationCommandInteraction,
    ctx: &Context,
) -> Result<(), SerenityError> {
    debug!("Received gate interaction");
    let (colony, reputation, role_id, guild_id, domain) = extract_options(command)?;
    debug!(
        "Colony: {}, Reputation: {}, Role ID: {}, Guild ID: {}, Domain: {}",
        colony, reputation, role_id, guild_id, domain
    );
    if let Err(why) = validate_gate_input(ctx, command, &colony, domain, role_id, reputation).await
    {
        respond(ctx, command, &why).await?;
        info!("Invalid gate command: {}", why);
        return Err(SerenityError::Other("Failed to validate gate input"));
    }

    let message = controller::Message::Gate {
        colony,
        domain: domain as u64,
        reputation: reputation as u8,
        role_id,
        guild_id,
    };
    CONTROLLER_CHANNEL.wait().send(message).await.unwrap();
    let mut content = MessageBuilder::new();
    content.push("Your role: ");
    content.role(role_id);
    content.push(" is now being gated!");
    content.build();
    respond(ctx, command, content).await
}

async fn list_gates_interaction_response(
    command: &ApplicationCommandInteraction,
    ctx: &Context,
) -> Result<(), SerenityError> {
    debug!("Received list gates interaction");
    let guild_id = command.guild_id.unwrap().into();
    let (tx, rx) = oneshot::channel();
    let message = controller::Message::List {
        guild_id,
        response: tx,
    };
    if let Err(err) = CONTROLLER_CHANNEL.wait().send(message).await {
        error!("Error sending message to controller: {:?}", err);
    }

    let gates = rx.await.unwrap();
    debug!("Received response from controller: {:?}", gates);
    if gates.is_empty() {
        respond(ctx, command, "No gates found").await?;
    } else {
        respond(ctx, command, "Here are the gates on the server").await?;
    }

    stream::iter(gates)
        .for_each_concurrent(None, |gate| async move {
            let mut content = MessageBuilder::new();
            content.push("The role: ");
            content.role(gate.role_id);
            content.push_line(" is gated by the following criteria");

            let mut follow_up = command
                .create_followup_message(ctx, |message| {
                    message
                        .content(&content)
                        .embed(|e| {
                            e.field("Colony Address", gate.colony.clone(), true)
                                .field("Domain", gate.domain, true)
                                .field("Reputation", format!("{}%", gate.reputation), true)
                        })
                        .components(|c| {
                            c.create_action_row(|row| {
                                row.create_button(|button| {
                                    button
                                    .style(serenity::model::prelude::component::ButtonStyle::Danger)
                                    .label("Delete gate")
                                    .custom_id("delete_gate")
                                })
                            })
                        })
                })
                .await
                .unwrap();
            let mut reaction_stream = follow_up
                .await_component_interactions(&ctx)
                .timeout(Duration::from_secs(15))
                .build();

            while let Some(interaction) = reaction_stream.next().await {
                if interaction.user.id.as_u64() != command.user.id.as_u64() {
                    debug!(
                        "User {} is not the author {} of the message",
                        interaction.user.id, command.user.id
                    );
                    return;
                }
                if let Err(why) = interaction
                    .create_interaction_response(&ctx.http, |response| {
                        response.kind(InteractionResponseType::DeferredUpdateMessage)
                    })
                    .await
                {
                    error!("Error responding to interaction: {:?}", why);
                }
                let message = controller::Message::Delete {
                    guild_id,
                    colony: gate.colony.clone(),
                    domain: gate.domain,
                    reputation: gate.reputation,
                    role_id: gate.role_id,
                };
                follow_up
                    .edit(&ctx.http, |message| {
                        message
                            .content("Gate deleted")
                            .components(|c| c.set_action_rows(Vec::new()))
                    })
                    .await
                    .unwrap();
                if let Err(err) = CONTROLLER_CHANNEL.wait().send(message).await {
                    error!("Error sending message to controller: {:?}", err);
                }
            }
            if let Err(why) = follow_up
                .edit(ctx, |message| {
                    message.components(|c| c.set_action_rows(Vec::new()))
                })
                .await
            {
                error!("Error editing follow up message: {:?}", why);
            }
        })
        .await;

    Ok(())
}

async fn check_interaction_response(
    command: &ApplicationCommandInteraction,
    ctx: &Context,
) -> Result<(), SerenityError> {
    debug!("Received check interaction");
    let (tx, rx) = oneshot::channel();
    let message = controller::Message::Check {
        user_id: command.user.id.into(),
        username: command.user.name.clone(),
        guild_id: command.guild_id.unwrap().into(),
        response_tx: tx,
    };
    if let Err(err) = CONTROLLER_CHANNEL.wait().send(message).await {
        error!("Error sending message to controller: {:?}", err);
    }
    let response = rx.await.unwrap();
    match response {
        CheckResponse::Grant(roles) => grant_roles(ctx, command, &roles).await,
        CheckResponse::Register(url) => register_user(ctx, command, &url).await,
    }
}

async fn checkout_interaction_response(
    command: &ApplicationCommandInteraction,
    ctx: &Context,
) -> Result<(), SerenityError> {
    debug!("Received checkout interaction");
    let (tx, rx) = oneshot::channel();
    let message = controller::Message::Unregister {
        user_id: command.user.id.into(),
        username: command.user.name.clone(),
        response_tx: tx,
    };
    if let Err(err) = CONTROLLER_CHANNEL.wait().send(message).await {
        error!("Error sending message to controller: {:?}", err);
    }
    let response = rx.await.unwrap();
    match response {
        UnRegisterResponse::NotRegistered => respond(ctx, command, "You are not registered").await,
        UnRegisterResponse::Unregister(url) => unregister_user(ctx, command, &url).await,
    }
}

async fn unknown_interaction_response(
    command: &ApplicationCommandInteraction,
    ctx: &Context,
) -> Result<(), SerenityError> {
    warn!("Unknown interaction: {:?}", command);
    respond(ctx, command, "Unknown command, Try /gate or /check").await
}

async fn register_user(
    ctx: &Context,
    command: &ApplicationCommandInteraction,
    url: &str,
) -> Result<(), SerenityError> {
    debug!("Registering user with URL: {}", url);
    let message = format!(
        "You need to register your wallet address with your discord user to get \
        gated roles. Please go to {} and follow the instructions.",
        url
    );
    command
        .user
        .direct_message(&ctx.http, |m| m.content(message))
        .await
        .unwrap();
    respond(ctx, command, "You need to register first, check your DMs").await
}

async fn unregister_user(
    ctx: &Context,
    command: &ApplicationCommandInteraction,
    url: &str,
) -> Result<(), SerenityError> {
    debug!("Unregistering user with URL: {}", url);
    let message = format!(
        "☠️ ☠️ ☠️  To unregister your wallet from your discord user follow this link \
        {} and follow the instructions. ☠️ ☠️ ☠️",
        url
    );
    command
        .user
        .direct_message(&ctx.http, |m| m.content(message))
        .await
        .unwrap();
    respond(ctx, command, "You need to register first, check your DMs").await
}

async fn grant_roles(
    ctx: &Context,
    command: &ApplicationCommandInteraction,
    roles: &Vec<u64>,
) -> Result<(), SerenityError> {
    debug!("Granting roles: {:?}", roles);
    for role in roles.iter() {
        if let Err(why) = ctx
            .http
            .add_member_role(
                command.guild_id.unwrap().into(),
                command.user.id.into(),
                *role,
                None,
            )
            .await
        {
            warn!("Error adding roles: {:?}", why);
            let mut content = MessageBuilder::new();
            content.push("Got error while granting roles: ");
            for role in roles.iter() {
                content.role(*role);
            }
            content.build();
            content.push(" Maybe your admin should check the role hierarchy!");

            return respond(ctx, command, content).await;
        }
    }
    let mut content = MessageBuilder::new();
    if roles.is_empty() {
        content.push("Sadly, you didn't receive any role yet   😢");
    } else {
        content.push("You have been granted the following roles: ");
        for role in roles.iter() {
            content.role(*role);
        }
    };
    content.build();
    respond(ctx, command, &content).await
}

fn make_gate_command(command: &mut CreateApplicationCommand) -> &mut CreateApplicationCommand {
    debug!("Creating gate slash command");
    command
        .name("gate")
        .description("Make a role gated by the reputation in a colony")
        .create_option(|o| {
            o.name("colony")
                .description("The colony in which the reputation guards the role")
                .kind(CommandOptionType::String)
                .required(true)
                .min_length(42)
                .max_length(42)
            // .add_string_choice("meta colony", "0xCFD3aa1EbC6119D80Ed47955a87A9d9C281A97B3")
        })
        .create_option(|o| {
            o.name("domain")
                .description("The domain of the colony in which the reputation guards the role")
                .kind(CommandOptionType::Integer)
                .required(true)
                .min_int_value(1)
        })
        .create_option(|o| {
            o.name("reputation")
                .description("The percentage of reputation in the domain, required to get the role")
                .kind(CommandOptionType::Integer)
                .required(true)
                .min_int_value(0)
                .max_int_value(100)
        })
        .create_option(|o| {
            o.name("role")
                .description("The role to be gated by reputation")
                .kind(CommandOptionType::Role)
                .required(true)
        })
        .default_member_permissions(Permissions::ADMINISTRATOR)
}

fn make_list_gates_command(
    command: &mut CreateApplicationCommand,
) -> &mut CreateApplicationCommand {
    debug!("Creating list gates slash command");
    command
        .name("list_gates")
        .description(
            "Lists gates. ⚠️ REVEALS GATES TO THE CHANNEL. IF YOU DON'T WANT THAT, USE IT IN A PRIVATE CHANNEL ⚠️",
        )
        .default_member_permissions(Permissions::ADMINISTRATOR)
}

fn make_checkout_command(command: &mut CreateApplicationCommand) -> &mut CreateApplicationCommand {
    debug!("Creating check slash command");
    command
        .name("checkout")
        .description("Deregister your wallet address from your discord user")
}

fn make_check_command(command: &mut CreateApplicationCommand) -> &mut CreateApplicationCommand {
    debug!("Creating check slash command");
    command
        .name("check")
        .description("Check the reputation of a colony and get the gated roles")
}

async fn validate_gate_input(
    ctx: &Context,
    command: &ApplicationCommandInteraction,
    colony: &str,
    domain: i64,
    role_id: u64,
    reputation: i64,
) -> Result<(), String> {
    if colony.is_empty() {
        return Err("Colony name cannot be empty".to_string());
    }
    if let Err(err) = colony_rs::Address::from_str(colony) {
        return Err(format!("Invalid colony address: {}", err));
    }
    if domain < 1 {
        return Err("Domain must be greater than 0".to_string());
    }
    let guild_id = match command.guild_id {
        Some(guild_id) => guild_id,
        None => {
            error!("No guild ID found for interaction");
            return Err("Guild ID not found".to_string());
        }
    };
    if guild_id.0 == role_id {
        return Err("⚠️  Role cannot be @everyone  ⚠️".to_string());
    }
    let roles = match ctx.http.get_guild_roles(guild_id.into()).await {
        Ok(roles) => roles,
        Err(why) => {
            error!("Error getting guild roles: {:?}", why);
            return Err(format!("Error getting guild roles: {:?}", why));
        }
    };
    if !roles
        .iter()
        .any(|r| r.id == <u64 as Into<RoleId>>::into(role_id))
    {
        return Err("Role not found".to_string());
    }
    if reputation < 0 {
        return Err("Reputation must be 0 or greater".to_string());
    }
    if reputation > 100 {
        return Err("Reputation must be 100 or less".to_string());
    }

    Ok(())
}

async fn respond(
    ctx: &Context,
    command: &ApplicationCommandInteraction,
    message: impl ToString,
) -> Result<(), SerenityError> {
    command
        .create_interaction_response(&ctx.http, |response| {
            response
                .kind(InteractionResponseType::ChannelMessageWithSource)
                .interaction_response_data(|m| m.content(message))
        })
        .await
}

fn extract_options(
    command: &ApplicationCommandInteraction,
) -> Result<(String, i64, u64, u64, i64), SerenityError> {
    let mut colony: String = String::new();
    let mut reputation: i64 = 0;
    let mut role_id: u64 = 0;
    let mut guild_id: u64 = 0;
    let mut domain: i64 = 0;
    for option in command.data.options.iter() {
        match option.name.as_str() {
            "colony" => {
                if let CommandDataOptionValue::String(colony_value) =
                    option.resolved.as_ref().unwrap()
                {
                    colony = colony_value.to_lowercase();
                }
            }
            "domain" => {
                if let CommandDataOptionValue::Integer(domain_value) =
                    option.resolved.as_ref().unwrap()
                {
                    domain = *domain_value as i64;
                }
            }
            "reputation" => {
                if let CommandDataOptionValue::Integer(reputation_value) =
                    option.resolved.as_ref().unwrap()
                {
                    reputation = *reputation_value as i64;
                }
            }
            "role" => {
                if let CommandDataOptionValue::Role(role) = option.resolved.as_ref().unwrap() {
                    role_id = role.id.into();
                    guild_id = role.guild_id.into();
                }
            }
            _ => {
                error!("Unknown option {}", option.name);
                return Err(SerenityError::Other("Unknown option"));
            }
        }
    }
    Ok((colony, reputation, role_id, guild_id, domain))
}
