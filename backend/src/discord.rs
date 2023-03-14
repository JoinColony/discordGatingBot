//! Handles the communication with the Discord API.
//!

use crate::config::CONFIG;
use crate::controller::{
    self, BatchResponse, CheckResponse, UnRegisterResponse, CONTROLLER_CHANNEL,
};
use crate::gate::{Gate, GateOptionType, GateOptionValue, GateOptionValueType};
use crate::gates;
use anyhow::{anyhow, Result};
use futures::{stream, StreamExt};
use serenity::{
    async_trait,
    builder::CreateApplicationCommand,
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
use std::fmt::Display;
use std::{collections::HashMap, time::Duration};
use tokio::sync::oneshot;
use tracing::{debug, error, info, warn};

pub async fn start() {
    let token = &CONFIG.wait().discord.token.clone();
    let mut client = Client::builder(token, GatewayIntents::GUILD_MEMBERS)
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
            .create_application_command(make_get_command)
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
    if let Err(why) = Command::create_global_application_command(&http, make_get_command).await {
        error!("Error creating global slash command get: {:?}", why);
    }
    if let Err(why) =
        Command::create_global_application_command(&http, make_list_gates_command).await
    {
        error!("Error creating global slash command list gates: {:?}", why);
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
        info!("{}({}) is connected!", ready.user.name, ready.user.id);
    }
    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        debug!("Received interaction: {:?}", interaction);
        if let Interaction::ApplicationCommand(command) = interaction.clone() {
            let interaction_response = match command.data.name.as_str() {
                "gate" => gate_interaction_response(&command, &ctx).await,
                "get" => get_interaction_response(&command, &ctx).await,
                _ => {
                    error!("Unknown command: {}", command.data.name);
                    return;
                }
            };
            if let Err(why) = interaction_response {
                info!("Error responding to interaction: {:?}", why);
                let message = MessageBuilder::new()
                    .push("⚠️⚠️⚠️  An error happened while processing your command: ")
                    .push_mono(why.to_string())
                    .build();
                if let Err(why) = respond(&ctx, &command, message, true).await {
                    error!("Could not respond to discord {:?}", why);
                }
            }
        }
    }
}

async fn gate_interaction_response(
    interaction: &ApplicationCommandInteraction,
    ctx: &Context,
) -> Result<()> {
    debug!("Received gate interaction: {:?}", interaction);
    let option = &interaction.data.options[0];
    match option.name.as_str() {
        "add" => Ok(gate_add_interaction_response(&interaction, &ctx).await?),
        "list" => Ok(list_gates_interaction_response(&interaction, &ctx).await?),
        "enforce" => Ok(enforce_gates_interaction_response(&interaction, &ctx).await?),
        _ => Err(anyhow!("Unknown gate subcommand")),
    }
}

async fn get_interaction_response(
    interaction: &ApplicationCommandInteraction,
    ctx: &Context,
) -> Result<()> {
    debug!("Received get interaction: {:?}", interaction);
    let option = &interaction.data.options[0];
    match option.name.as_str() {
        "in" => Ok(get_in_interaction_response(&interaction, &ctx).await?),
        "out" => Ok(get_out_interaction_response(&interaction, &ctx).await?),
        _ => Err(anyhow!("Unknown get subcommand")),
    }
}

async fn gate_add_interaction_response(
    command: &ApplicationCommandInteraction,
    ctx: &Context,
) -> Result<()> {
    debug!("Received get interaction: {:?}", command);
    let (name, role_id, role_position, guild_id, options) = extract_gate_add_options(command)?;
    if role_id == guild_id {
        return Err(anyhow!("Role cannot be @everyone"));
    }
    let gate = Gate::new(role_id, &name, &options)?;
    let message = controller::Message::Gate { guild_id, gate };
    CONTROLLER_CHANNEL.wait().send(message).await.unwrap();
    let mut content = MessageBuilder::new();
    content.push("Your role: ");
    content.role(role_id);
    content.push_line(" is now being gated!");
    if !is_below_bot_in_hierarchy(role_position, &ctx, guild_id, command.application_id.into())
        .await
    {
        content.push_line(
            "⚠️  The bot is currently below this role in the role hierarchy, \
                     so it will not be able to assign it to users. \
                     consider dragging the bot role above the gated role  ⚠️ ",
        );
    }
    content.build();
    Ok(respond(ctx, command, content, true).await?)
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
        respond(ctx, command, "No gates found", true).await?;
    } else {
        respond(ctx, command, "Here are the gates on the server", true).await?;
    }

    stream::iter(gates)
        .for_each_concurrent(None, |gate| async move {
            let mut content = MessageBuilder::new();
            content.push("The role: ");
            content.role(gate.role_id);
            content.push_line(" is gated by the following criteria");
            let follow_up = command
                .create_followup_message(ctx, |message| {
                    message
                        .ephemeral(true)
                        .content(&content)
                        .embed(|e| {
                            for field in gate.condition.fields() {
                                e.field(field.name, field.value, true);
                            }
                            e
                        })
                        .components(|c| {
                            c.create_action_row(|row| {
                                row.create_button(|button| {
                                    button
                                    .style(serenity::model::prelude::component::ButtonStyle::Danger)
                                    .label(format!("Delete gate (within {}s)", 15))
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
                let message = controller::Message::Delete {
                    guild_id,
                    gate: gate.clone(),
                };
                if let Err(err) = CONTROLLER_CHANNEL.wait().send(message).await {
                    error!("Error sending message to controller: {:?}", err);
                    return;
                }
                let content = MessageBuilder::new()
                    .push("❌The gate for the role: ")
                    .role(gate.role_id)
                    .push_line(" has been deleted")
                    .push_line("gated by the following criteria")
                    .build();
                if let Err(why) = interaction
                    .create_interaction_response(&ctx.http, |response| {
                        response.interaction_response_data(|message| {
                            message.content(content).ephemeral(true).embed(|e| {
                                for field in gate.condition.fields() {
                                    e.field(field.name, field.value, true);
                                }
                                e
                            })
                        });
                        response.kind(InteractionResponseType::ChannelMessageWithSource)
                    })
                    .await
                {
                    error!("Error responding to interaction: {:?}", why);
                }
            }
        })
        .await;
    Ok(())
}

async fn enforce_gates_interaction_response(
    command: &ApplicationCommandInteraction,
    ctx: &Context,
) -> Result<(), SerenityError> {
    debug!("Received enforce gates interaction");
    let guild_id = command.guild_id.unwrap();
    let (role_tx, role_rx) = tokio::sync::oneshot::channel();
    let message = controller::Message::Roles {
        guild_id: guild_id.into(),
        response: role_tx,
    };
    if let Err(err) = CONTROLLER_CHANNEL.wait().send(message).await {
        error!("Error sending message to controller: {:?}", err);
    }
    let managed_roles = role_rx.await.unwrap();
    let (tx, mut rx) = tokio::sync::mpsc::channel(100);
    let members = ctx
        .http
        .get_guild_members(guild_id.into(), None, None)
        .await?;
    let user_ids = members
        .iter()
        .map(|m| m.user.id.as_u64().clone())
        .collect::<Vec<_>>();
    let member_map = members
        .into_iter()
        .map(|m| {
            (
                m.user.id.as_u64().clone(),
                m.roles
                    .iter()
                    .filter_map(|&r| {
                        let id = u64::from(r);
                        if managed_roles.contains(&id) {
                            Some(id)
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<_>>(),
            )
        })
        .collect::<HashMap<_, _>>();
    let message = controller::Message::Batch {
        guild_id: guild_id.into(),
        user_ids,
        response_tx: tx,
    };
    eprintln!("{:#?}", member_map);
    if let Err(err) = CONTROLLER_CHANNEL.wait().send(message).await {
        error!("Error sending message to controller: {:?}", err);
    }
    let mut message = MessageBuilder::new();
    message.push("Enforcing gates for all server members and the following roles");
    for role in managed_roles.iter() {
        message.role(*role);
    }
    respond(ctx, command, message, true).await?;
    while let Some(response) = rx.recv().await {
        match response {
            BatchResponse::Grant { user_id, roles } => {
                let gained_roles = roles
                    .iter()
                    .filter(|&r| !member_map[&user_id].contains(r))
                    .collect::<Vec<_>>();
                let lost_roles = member_map[&user_id]
                    .iter()
                    .filter(|&r| !roles.contains(r))
                    .collect::<Vec<_>>();
                let mut message = MessageBuilder::new();
                if gained_roles.is_empty() && lost_roles.is_empty() {
                    continue;
                }
                let mut failed_grants = Vec::new();
                let mut failed_losses = Vec::new();

                for role in gained_roles.clone() {
                    if let Err(why) = ctx
                        .http
                        .add_member_role(guild_id.into(), user_id.into(), *role, None)
                        .await
                    {
                        info!("Error granting role: {:?}", why);
                        failed_grants.push(role);
                    }
                }
                for role in lost_roles.clone() {
                    if let Err(why) = ctx
                        .http
                        .remove_member_role(guild_id.into(), user_id.into(), *role, None)
                        .await
                    {
                        info!("Error removing role: {:?}", why);
                        failed_losses.push(role);
                    }
                }
                message.user(user_id);
                message.push_line(" there was a role update for you!");

                message.push_line(
                    "You can always use the `/get in` command to \
                                  check if new roles are available to you.",
                );
                message.push_line("");
                if !gained_roles.is_empty() {
                    message.push("You have been granted the following roles: ");
                    for role in gained_roles {
                        message.role(*role);
                    }
                    message.push_line("");
                }
                if !lost_roles.is_empty() {
                    message.push("You lost the following roles: ");
                    for role in lost_roles {
                        message.role(*role);
                    }
                }
                if !failed_grants.is_empty() {
                    message.push_line("");
                    message.push("there were problems however granting you the roles: ");
                    for role in failed_grants {
                        message.role(*role);
                    }
                }
                if !failed_losses.is_empty() {
                    message.push_line("");
                    message
                        .push("luckily for you, I couldn't remove the following roles from you: ");
                    for role in failed_losses {
                        message.role(*role);
                    }
                }
                message.build();
                follow_up(&ctx, command, message, false).await?;
            }
            BatchResponse::Done => break,
        }
    }
    follow_up(&ctx, command, "Finished enforcement of gates", true).await
}

async fn get_in_interaction_response(
    command: &ApplicationCommandInteraction,
    ctx: &Context,
) -> Result<(), SerenityError> {
    debug!("Received get in interaction");
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
    command
        .create_interaction_response(&ctx, |response| {
            response.interaction_response_data(|message| message.ephemeral(true));
            response.kind(InteractionResponseType::DeferredChannelMessageWithSource)
        })
        .await?;
    follow_up(
        &ctx,
        command,
        "Checking your reputation in the colonies,\
              this might take a while...",
        true,
    )
    .await?;
    let response = match rx.await {
        Ok(repsonse) => repsonse,
        Err(err) => {
            error!("Error receiving response from controller: {:?}", err);
            return Err(SerenityError::Other(
                "Error receiving response from controller",
            ));
        }
    };
    match response {
        CheckResponse::Grant(roles) => grant_roles(ctx, command, &roles).await,
        CheckResponse::Register(url) => register_user(ctx, command, &url).await,
    }
}

async fn get_out_interaction_response(
    command: &ApplicationCommandInteraction,
    ctx: &Context,
) -> Result<(), SerenityError> {
    debug!("Received get out interaction");
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
        UnRegisterResponse::NotRegistered => {
            respond(ctx, command, "You are not registered", true).await
        }
        UnRegisterResponse::Unregister(url) => unregister_user(ctx, command, &url).await,
    }
}

async fn unknown_interaction_response(
    command: &ApplicationCommandInteraction,
    ctx: &Context,
) -> Result<(), SerenityError> {
    warn!("Unknown interaction: {:?}", command);
    respond(ctx, command, "Unknown command, Try /gate or /check", true).await
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
    follow_up(ctx, command, message, true).await
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
    respond(ctx, command, message, true).await
}

async fn grant_roles(
    ctx: &Context,
    command: &ApplicationCommandInteraction,
    roles: &Vec<u64>,
) -> Result<(), SerenityError> {
    debug!("Granting roles: {:?}", roles);
    let mut granted_roles = Vec::new();
    let mut failed_roles = Vec::new();
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
            warn!("Error adding role: {} {:?}", role, why);
            failed_roles.push(*role);
        } else {
            debug!("Role granted: {}", role);
            granted_roles.push(*role);
        }
    }

    let mut content = MessageBuilder::new();
    if granted_roles.is_empty() {
        content.push_line("using the `/get in` command sadly, didn't give you any roles yet  😢");
    } else {
        content.push("using the `/get in` command got you the following roles: ");
        for role in granted_roles.iter() {
            content.role(*role);
        }
        content.push_line("  🎉");
    };
    if !failed_roles.is_empty() {
        content.push("Got error while granting roles: ");
        for role in failed_roles.iter() {
            content.role(*role);
        }
        content.push_line("");
        content.push("Maybe your admin should check the role hierarchy!  🤔");
    }
    content.build();

    let ephemeral = match (granted_roles.is_empty(), failed_roles.is_empty()) {
        (false, false) => false,
        (true, true) => true,
        (true, false) => false,
        (false, true) => false,
    };
    match follow_up(ctx, command, &content, ephemeral).await {
        Ok(_) => Ok(()),
        Err(why) => {
            error!("Error sending follow up message: {:?}", why);
            Err(why)
        }
    }
}

fn make_list_gates_command(
    command: &mut CreateApplicationCommand,
) -> &mut CreateApplicationCommand {
    debug!("Creating list gates slash command");
    command
        .name("list_gates")
        .description("Lists gates that are currently active for this server.")
        .default_member_permissions(Permissions::MANAGE_GUILD)
}

fn make_gate_command(command: &mut CreateApplicationCommand) -> &mut CreateApplicationCommand {
    debug!("Creating gate slash command");
    let options = gates!(options);
    let descriptions = gates!(descriptions);
    command
        .name("gate")
        .description("Create a new gate for a role on this server")
        .create_option(|option| {
            for (gate_name, gate_option) in options.into_iter() {
                option.create_sub_option(|sub_option| {
                    sub_option
                        .name(gate_name)
                        .kind(CommandOptionType::SubCommand)
                        .description(descriptions.get(gate_name).expect(
                            "Did not find description, in the gates! \
                                    macro generated map. This should not happen",
                        ));
                    for o in gate_option.into_iter() {
                        sub_option.create_sub_option(|sub_sub_option| {
                            sub_sub_option
                                .name(o.name)
                                .description(o.description)
                                .required(o.required);
                            match o.option_type {
                                GateOptionType::String {
                                    min_length,
                                    max_length,
                                } => {
                                    sub_sub_option.kind(CommandOptionType::String);
                                    if let Some(min_length) = min_length {
                                        sub_sub_option.min_length(min_length);
                                    }
                                    if let Some(max_length) = max_length {
                                        sub_sub_option.max_length(max_length);
                                    }
                                }
                                GateOptionType::I64 { min, max } => {
                                    sub_sub_option.kind(CommandOptionType::Integer);
                                    if let Some(min) = min {
                                        sub_sub_option.min_int_value(min);
                                    }
                                    if let Some(max) = max {
                                        sub_sub_option.max_int_value(max);
                                    }
                                }
                                GateOptionType::F64 { min, max } => {
                                    sub_sub_option.kind(CommandOptionType::Number);
                                    if let Some(min) = min {
                                        sub_sub_option.min_number_value(min);
                                    }
                                    if let Some(max) = max {
                                        sub_sub_option.max_number_value(max);
                                    }
                                }
                            };
                            sub_sub_option
                        });
                    }
                    sub_option.create_sub_option(|sub_option| {
                        sub_option
                            .name("role")
                            .description("The role to be gated")
                            .kind(CommandOptionType::Role)
                            .required(true)
                    });
                    sub_option
                });
            }
            option
                .name("add")
                .description("Add a new gate to protect a role on the server")
                .kind(CommandOptionType::SubCommandGroup)
        })
        .create_option(|option| {
            option
                .name("list")
                .description("Lists gates that are currently active for this server.")
                .kind(CommandOptionType::SubCommand)
        })
        .create_option(|option| {
            option
                .name("enforce")
                .description("Enforce the active gates on all members of the server")
                .kind(CommandOptionType::SubCommand)
        })
        .default_member_permissions(Permissions::MANAGE_GUILD)
}

fn make_get_command(command: &mut CreateApplicationCommand) -> &mut CreateApplicationCommand {
    debug!("Creating get slash command");
    command
        .name("get")
        .description("Get in or out of gated roles")
        .create_option(|option| {
            option
                .name("in")
                .description("Get roles granted that are gated by the gating bot")
                .kind(CommandOptionType::SubCommand)
        })
        .create_option(|option| {
            option
                .name("out")
                .description("Deregister your discord user and wallet address from the gating bot")
                .kind(CommandOptionType::SubCommand)
        })
}

async fn respond(
    ctx: &Context,
    command: &ApplicationCommandInteraction,
    message: impl ToString,
    ephemeral: bool,
) -> Result<(), SerenityError> {
    command
        .create_interaction_response(&ctx.http, |response| {
            response
                .kind(InteractionResponseType::ChannelMessageWithSource)
                .interaction_response_data(|m| m.content(message).ephemeral(ephemeral))
        })
        .await
}

async fn follow_up(
    ctx: &Context,
    command: &ApplicationCommandInteraction,
    message: impl ToString,
    ephemeral: bool,
) -> Result<(), SerenityError> {
    command
        .create_followup_message(&ctx.http, |m| m.content(message).ephemeral(ephemeral))
        .await
        .map(|_| ())
}

async fn is_below_bot_in_hierarchy(
    position: u64,
    ctx: &Context,
    guild_id: u64,
    bot_user_id: u64,
) -> bool {
    let bot_member = ctx.http.get_member(guild_id, bot_user_id).await.unwrap();
    let bot_roles = bot_member.roles;
    let guild_roles = ctx.http.get_guild_roles(guild_id).await.unwrap();
    if let Some(max) = guild_roles
        .iter()
        .filter(|r| bot_roles.iter().any(|&br| br == r.id))
        .map(|r| r.position)
        .max()
    {
        position < max as u64
    } else {
        error!("No bot roles found");
        false
    }
}

fn extract_gate_add_options(
    command: &ApplicationCommandInteraction,
) -> Result<(String, u64, u64, u64, Vec<GateOptionValue>)> {
    let mut role_id: Option<u64> = None;
    let mut role_position: u64 = 0;
    let mut guild_id: u64 = 0;
    let add_option = command
        .data
        .options
        .iter()
        .find(|o| o.name.as_str() == "add")
        .ok_or(anyhow!("No add option found"))?;
    if add_option.options.is_empty() {
        return Err(anyhow!("No options found on add found"));
    }
    let sub_option = &add_option.options[0];
    let name = sub_option.name.clone();
    let options = sub_option
        .options
        .iter()
        .filter_map(|sub_sub_option| match sub_sub_option.name.as_str() {
            "role" => {
                if let Some(CommandDataOptionValue::Role(role)) = sub_sub_option.resolved.as_ref() {
                    role_id = Some(role.id.into());
                    role_position = role.position as u64;
                    guild_id = role.guild_id.into();
                } else {
                    error!("Role field did not hold a role type");
                }
                None
            }
            _ => {
                let value = match sub_sub_option.resolved.as_ref() {
                    Some(CommandDataOptionValue::String(s)) => {
                        GateOptionValueType::String(s.clone())
                    }
                    Some(CommandDataOptionValue::Integer(i)) => GateOptionValueType::I64(*i),
                    Some(CommandDataOptionValue::Number(n)) => GateOptionValueType::F64(*n),
                    _ => {
                        error!("Unknown option type");
                        return None;
                    }
                };

                Some(GateOptionValue {
                    name: sub_sub_option.name.clone(),
                    value,
                })
            }
        })
        .collect();
    if let Some(role_id) = role_id {
        Ok((name, role_id, role_position, guild_id, options))
    } else {
        Err(anyhow!("Role id missing"))
    }
}
