//! Handles the communication with the Discord API.
//!
use crate::config::CONFIG;
use crate::controller::{
    self, BatchResponse, CheckResponse, RemoveUserResponse, UnRegisterResponse, CONTROLLER_CHANNEL,
};
use crate::gate::{Gate, GateOptionType, GateOptionValue, GateOptionValueType};
use crate::gates;
use anyhow::{anyhow, bail, Result};
use futures::{stream, StreamExt};
use secrecy::ExposeSecret;
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
use std::{collections::HashMap, time::Duration};
use tokio::sync::oneshot;
use tracing::{debug, error, info, info_span, instrument, warn, Instrument, Span};

#[instrument(level = "debug")]
pub async fn start() {
    info!("Starting discord bot");
    let token = &CONFIG.wait().discord.token.expose_secret();
    let mut client = Client::builder(token, GatewayIntents::GUILD_MEMBERS)
        .event_handler(Handler)
        .in_current_span()
        .await
        .expect("Error creating client");
    if let Err(why) = client.start().in_current_span().await {
        error!("Client error: {:?}", why);
    }
}

#[instrument(level = "debug")]
pub async fn start_maintenance_mode() {
    info!("Starting discord bot in maintenance mode");
    let token = &CONFIG.wait().discord.token.expose_secret();
    let mut client = Client::builder(token, GatewayIntents::GUILD_MEMBERS)
        .event_handler(MaintenanceHandler)
        .in_current_span()
        .await
        .expect("Error creating client");
    if let Err(why) = client.start().in_current_span().await {
        error!("Client error: {:?}", why);
    }
}

#[instrument]
pub async fn register_guild_slash_commands(guild_id: u64) {
    info!("Registering slash commands for guild");
    let token = &CONFIG.wait().discord.token.expose_secret();
    let guild_id = GuildId(guild_id);
    let http = Http::new(&token);
    let resp = http
        .get_current_application_info()
        .in_current_span()
        .await
        .expect("Failed to get application info");
    let app_id = resp.id;
    http.set_application_id(app_id.into());
    let command_result = GuildId::set_application_commands(&guild_id, &http, |commands| {
        commands
            .create_application_command(make_gate_command)
            .create_application_command(make_get_command)
    })
    .in_current_span()
    .await;
    info!("Done registering slash commands for guild");
    if let Err(why) = command_result {
        error!("Error registering guild slash commands: {:?}", why);
    }
}

#[instrument]
pub async fn delete_guild_slash_commands(guild_id: u64) {
    info!("Deleting slash commands for guild");
    let token = &CONFIG.wait().discord.token.expose_secret();
    let guild_id = GuildId(guild_id);
    let http = Http::new(&token);
    let resp = http
        .get_current_application_info()
        .in_current_span()
        .await
        .expect("Failed to get application info");
    let app_id = resp.id;
    http.set_application_id(app_id.into());
    let commands = guild_id
        .get_application_commands(&http)
        .in_current_span()
        .await
        .expect("Failed to get guild commands");
    for command in commands {
        if let Err(why) = guild_id
            .delete_application_command(&http, command.id)
            .in_current_span()
            .await
        {
            error!("Error deleting guild slash commands: {:?}", why);
        }
    }
    info!("Done deleting slash commands for guild");
}

#[instrument]
pub async fn register_global_slash_commands() {
    info!("Registering slash commands globally");
    let token = &CONFIG.wait().discord.token.expose_secret();
    let http = Http::new(&token);
    let resp = http
        .get_current_application_info()
        .in_current_span()
        .await
        .expect("Failed to get application info");
    let app_id = resp.id;
    http.set_application_id(app_id.into());
    if let Err(why) = Command::create_global_application_command(&http, make_gate_command)
        .in_current_span()
        .await
    {
        error!("Error creating global slash command gate: {:?}", why);
    }
    if let Err(why) = Command::create_global_application_command(&http, make_get_command)
        .in_current_span()
        .await
    {
        error!("Error creating global slash command get: {:?}", why);
    }
    info!("Done registering slash commands globally");
}

#[instrument]
pub async fn delete_global_slash_commands() {
    info!("Deleting slash commands globally");
    let token = &CONFIG.wait().discord.token.expose_secret();
    let http = Http::new(&token);
    let resp = http
        .get_current_application_info()
        .in_current_span()
        .await
        .expect("Failed to get application info");
    let app_id = resp.id;
    http.set_application_id(app_id.into());
    let commands = Command::get_global_application_commands(&http)
        .in_current_span()
        .await
        .expect("Failed to get global commands");
    for command in commands {
        if let Err(why) = Command::delete_global_application_command(&http, command.id)
            .in_current_span()
            .await
        {
            error!(
                "Error deleting global slash command {}: {:?}",
                command.id, why
            );
        }
    }
    info!("Done deleting slash commands globally");
}
struct MaintenanceHandler;

#[async_trait]
impl EventHandler for MaintenanceHandler {
    async fn ready(&self, _ctx: Context, ready: Ready) {
        info!(
            "{}({}) is connected in maintenance mode!",
            ready.user.name, ready.user.id
        );
    }
    #[instrument(
        name = "handling_interaction_in_maintenance_mode",
        level = "info",
        skip(self, ctx, interaction),
        fields(guild_id, interaction_id, username, user_id, command)
    )]
    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        match &interaction {
            Interaction::ApplicationCommand(command) => {
                let command_name = command.data.name.as_str();
                let user_name = command.user.name.as_str();
                let user_id = command.user.id;
                let guild_id = command.guild_id.unwrap_or(0.into());
                let interaction_id = command.id;
                Span::current().record("guild_id", &guild_id.as_u64());
                Span::current().record("username", &user_name);
                Span::current().record("user_id", &user_id.as_u64());
                Span::current().record("command", &command_name);
                Span::current().record("interaction_id", &interaction_id.as_u64());
                debug!("Start handling command interaction");
                if let Err(why) = respond(
                    &ctx,
                    &command,
                    "⚠️⚠️⚠️  The bot is currently in maintenance mode, and will be back soon",
                    true,
                )
                .in_current_span()
                .await
                {
                    error!("Could not respond to discord {:?}", why);
                }
            }
            _ => info!("Received non-command interaction in maintenance mode, do nothing"),
        }
    }
}

/// The handler for the Discord client.
struct Handler;

#[async_trait]
impl EventHandler for Handler {
    #[instrument(level = "trace", skip(self, _ctx))]
    async fn ready(&self, _ctx: Context, ready: Ready) {
        info!("{}({}) is connected!", ready.user.name, ready.user.id);
    }
    #[instrument(
        name = "handling_interaction",
        level = "info",
        skip(self, ctx, interaction),
        fields(guild_id, interaction_id, username, user_id, command)
    )]
    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        match &interaction {
            Interaction::ApplicationCommand(command) => {
                let command_name = command.data.name.as_str();
                let user_name = command.user.name.as_str();
                let user_id = command.user.id;
                let guild_id = command.guild_id.unwrap_or(0.into());
                let interaction_id = command.id;
                Span::current().record("guild_id", &guild_id.as_u64());
                Span::current().record("username", &user_name);
                Span::current().record("user_id", &user_id.as_u64());
                Span::current().record("command", &command_name);
                Span::current().record("interaction_id", &interaction_id.as_u64());
                debug!("Start handling command interaction");
                let interaction_response = match command_name {
                    "gate" => gate_interaction(&command, &ctx).in_current_span().await,
                    "get" => get_interaction(&command, &ctx).in_current_span().await,
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
                    if let Err(why) = respond(&ctx, &command, message, true)
                        .in_current_span()
                        .await
                    {
                        error!("Could not respond to discord {:?}", why);
                    }
                }
            }

            Interaction::MessageComponent(interaction) => {
                let interaction_id = interaction.id;
                Span::current().record("interaction_id", &interaction_id.as_u64());
                debug!("Got message component interaction");
            }
            Interaction::Ping(interaction) => {
                let interaction_id = interaction.id;
                Span::current().record("interaction_id", &interaction_id.as_u64());
                debug!("Got ping interaction");
            }
            Interaction::Autocomplete(interaction) => {
                let interaction_id = interaction.id;
                Span::current().record("interaction_id", &interaction_id.as_u64());
                debug!("Got autocomplete interaction");
            }
            Interaction::ModalSubmit(interaction) => {
                let interaction_id = interaction.id;
                Span::current().record("interaction_id", &interaction_id.as_u64());
                debug!("Got modal submit interaction");
            }
        }
        debug!("Done handling interaction");
    }
}

#[instrument(level = "info", skip(ctx, interaction), fields(option))]
async fn gate_interaction(
    interaction: &ApplicationCommandInteraction,
    ctx: &Context,
) -> Result<()> {
    let option = &interaction.data.options[0];
    Span::current().record("option", &option.name.as_str());
    debug!("Handling gate command");
    match option.name.as_str() {
        "add" => Ok(add_gate(&interaction, &ctx).in_current_span().await?),
        "list" => Ok(list_gates(&interaction, &ctx).in_current_span().await?),
        "enforce" => Ok(enforce_gates(&interaction, &ctx).in_current_span().await?),
        _ => Err(anyhow!("Unknown gate subcommand")),
    }
}

#[instrument(level = "info", skip(ctx, interaction), fields(option))]
async fn get_interaction(interaction: &ApplicationCommandInteraction, ctx: &Context) -> Result<()> {
    let option = &interaction.data.options[0];
    Span::current().record("option", &option.name.as_str());
    debug!("Handling get command");
    match option.name.as_str() {
        "in" => get_in_check(&interaction, &ctx).in_current_span().await,
        "out" => get_out_request(&interaction, &ctx).in_current_span().await,
        _ => Err(anyhow!("Unknown get subcommand")),
    }
}

#[instrument(level = "info", skip(ctx, interaction))]
async fn add_gate(interaction: &ApplicationCommandInteraction, ctx: &Context) -> Result<()> {
    debug!("Received gate add interaction");
    let (name, role_id, role_position, guild_id, options) = extract_gate_add_options(interaction)?;
    debug!(
        name,
        role_id,
        role_position,
        guild_id,
        ?options,
        "Extracted options",
    );
    if role_id == guild_id {
        return Err(anyhow!("Role cannot be @everyone"));
    }
    let gate = Gate::new(role_id, &name, &options)
        .in_current_span()
        .await?;
    let span = info_span!("controller");
    let message = controller::Message::Gate {
        guild_id,
        gate,
        span,
    };
    if let Err(why) = CONTROLLER_CHANNEL
        .wait()
        .send(message)
        .in_current_span()
        .await
    {
        error!("Error sending gate message: {:?}", why);
    }
    let mut content = MessageBuilder::new();
    content.push("Your role: ");
    content.role(role_id);
    content.push_line(" is now being gated!");
    if !is_below_bot_in_hierarchy(
        role_position,
        &ctx,
        guild_id,
        interaction.application_id.into(),
    )
    .in_current_span()
    .await
    .unwrap_or(true)
    {
        content.push_line(
            "⚠️  The bot is currently below this role in the role hierarchy, \
                     so it will not be able to assign it to users. Drag the bot \
                     role above the gated role under `Server Settings -> Roles ⚠️ ",
        );
    }
    content.build();
    respond(ctx, interaction, content, true)
        .in_current_span()
        .await
}

#[instrument(level = "info", skip(ctx, interaction))]
async fn list_gates(interaction: &ApplicationCommandInteraction, ctx: &Context) -> Result<()> {
    debug!("Listing gates");
    let guild_id = interaction
        .guild_id
        .ok_or(anyhow!("Error getting guild id from command"))?
        .into();
    let (tx, rx) = oneshot::channel();
    let span = info_span!("controller");
    let message = controller::Message::List {
        guild_id,
        response: tx,
        span,
    };
    if let Err(err) = CONTROLLER_CHANNEL
        .wait()
        .send(message)
        .in_current_span()
        .await
    {
        error!("Error sending message to controller: {:?}", err);
    }

    let gates = rx.in_current_span().await?;
    debug!(?gates, "Received response from controller");
    if gates.is_empty() {
        respond(ctx, interaction, "No gates found", true)
            .in_current_span()
            .await?;
    } else {
        respond(ctx, interaction, "Here are the gates on the server", true)
            .in_current_span()
            .await?;
    }

    stream::iter(gates)
        .for_each_concurrent(None, |gate| async move {
            let mut content = MessageBuilder::new();
            content.push("The role: ");
            content.role(gate.role_id);
            content.push_line(" is gated by the following criteria");
            let follow_up = match interaction
                .create_followup_message(ctx, |message| {
                    message
                        .ephemeral(true)
                        .content(&content)
                        .embed(|e| {
                            for field in gate.fields() {
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
                .in_current_span()
                .await
            {
                Ok(follow_up) => follow_up,
                Err(why) => {
                    error!("Error sending follow up message: {:?}", why);
                    return;
                }
            };
            let mut reaction_stream = follow_up
                .await_component_interactions(&ctx)
                .timeout(Duration::from_secs(15))
                .build();
            while let Some(interaction) = reaction_stream.next().in_current_span().await {
                if interaction.user.id.as_u64() != interaction.user.id.as_u64() {
                    debug!(
                        "User {} is not the author {} of the message",
                        interaction.user.id, interaction.user.id
                    );
                    return;
                }
                let span = info_span!("controller");
                let message = controller::Message::Delete {
                    guild_id,
                    gate: gate.clone(),
                    span,
                };
                if let Err(err) = CONTROLLER_CHANNEL
                    .wait()
                    .send(message)
                    .in_current_span()
                    .await
                {
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
                    .in_current_span()
                    .await
                {
                    error!("Error responding to interaction: {:?}", why);
                }
            }
        })
        .in_current_span()
        .await;
    Ok(())
}

#[instrument(level = "info", skip(ctx, interaction))]
async fn enforce_gates(interaction: &ApplicationCommandInteraction, ctx: &Context) -> Result<()> {
    debug!("Enforcing gates");
    let guild_id = interaction
        .guild_id
        .ok_or(anyhow!("Error getting guild id from command"))?;
    let (role_tx, role_rx) = tokio::sync::oneshot::channel();
    let span = info_span!("controller");
    let message = controller::Message::Roles {
        guild_id: guild_id.into(),
        response: role_tx,
        span,
    };
    if let Err(err) = CONTROLLER_CHANNEL
        .wait()
        .send(message)
        .in_current_span()
        .await
    {
        error!("Error sending message to controller: {:?}", err);
    }
    let managed_roles = role_rx.in_current_span().await?;
    debug!(?managed_roles, "Received response from controller");
    let (tx, mut rx) = tokio::sync::mpsc::channel(100);
    let members = ctx
        .http
        .get_guild_members(guild_id.into(), None, None)
        .in_current_span()
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
    let span = info_span!("controller");
    let message = controller::Message::Batch {
        guild_id: guild_id.into(),
        user_ids,
        response_tx: tx,
        span,
    };
    if let Err(err) = CONTROLLER_CHANNEL
        .wait()
        .send(message)
        .in_current_span()
        .await
    {
        error!("Error sending message to controller: {:?}", err);
    }
    let mut message = MessageBuilder::new();
    message.push("Enforcing gates for all server members and the following roles");
    for role in managed_roles.iter() {
        message.role(*role);
    }
    respond(ctx, interaction, message, true)
        .in_current_span()
        .await?;
    while let Some(response) = rx.recv().in_current_span().await {
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
                debug!(
                    user_id,
                    ?gained_roles,
                    ?lost_roles,
                    "Roles to grant or remove for user"
                );
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
                        .in_current_span()
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
                        .in_current_span()
                        .await
                    {
                        info!("Could not remove role: {:?}", why);
                        failed_losses.push(role);
                    }
                }
                message.user(user_id);
                message.push_line("");
                if !gained_roles.is_empty() {
                    message.push("has been granted the following roles: ");
                    for role in gained_roles {
                        message.role(*role);
                    }
                    message.push_line("");
                }
                if !lost_roles.is_empty() {
                    message.push("lost the following roles: ");
                    for role in lost_roles {
                        message.role(*role);
                    }
                }
                if !failed_grants.is_empty() {
                    message.push_line("");
                    message.push("there were problems granting the roles: ");
                    for role in failed_grants {
                        message.role(*role);
                    }
                }
                if !failed_losses.is_empty() {
                    message.push_line("");
                    message.push("couldn't remove the following roles: ");
                    for role in failed_losses {
                        message.role(*role);
                    }
                }
                message.build();
                follow_up(&ctx, interaction, message, true)
                    .in_current_span()
                    .await?;
            }
            BatchResponse::Done => break,
        }
    }
    follow_up(&ctx, interaction, "Finished enforcement of gates", true)
        .in_current_span()
        .await
}

#[instrument(level = "info", skip(ctx, interaction))]
async fn get_in_check(interaction: &ApplicationCommandInteraction, ctx: &Context) -> Result<()> {
    debug!("checking `get in` request");
    let (tx, rx) = oneshot::channel();
    let span = info_span!("controller");
    let message = controller::Message::Check {
        user_id: interaction.user.id.into(),
        username: interaction.user.name.clone(),
        guild_id: interaction
            .guild_id
            .ok_or(anyhow!("Error getting guild id from command"))?
            .into(),
        response_tx: tx,
        span,
    };
    if let Err(err) = CONTROLLER_CHANNEL
        .wait()
        .send(message)
        .in_current_span()
        .await
    {
        error!("Error sending message to controller: {:?}", err);
    }
    interaction
        .create_interaction_response(&ctx, |response| {
            response.interaction_response_data(|message| message.ephemeral(true));
            response.kind(InteractionResponseType::DeferredChannelMessageWithSource)
        })
        .in_current_span()
        .await?;
    follow_up(
        &ctx,
        interaction,
        "Checking your reputation in the colonies,\
              this might take a while...",
        true,
    )
    .in_current_span()
    .await?;
    let response = match rx.in_current_span().await {
        Ok(repsonse) => repsonse,
        Err(why) => {
            error!("Error receiving response from controller: {:?}", why);
            bail!("Error receiving response from controller: {:?}", why);
        }
    };
    match response {
        CheckResponse::Grant(roles) => {
            grant_roles(ctx, interaction, &roles)
                .in_current_span()
                .await
        }
        CheckResponse::Register(url) => {
            register_user(ctx, interaction, &url)
                .in_current_span()
                .await
        }
        CheckResponse::Error(why) => bail!("Error checking your reputation: {}", why),
    }
}

#[instrument(level = "info", skip(ctx, interaction))]
async fn get_out_request(interaction: &ApplicationCommandInteraction, ctx: &Context) -> Result<()> {
    debug!("checking `get out` request");
    let (tx, rx) = oneshot::channel();
    let (role_tx, role_rx) = oneshot::channel();
    let (removed_tx, removed_rx) = oneshot::channel();
    let guild_id = interaction
        .guild_id
        .ok_or(anyhow!("Error getting guild id from command"))?;
    let user_id = interaction.user.id;
    let span = info_span!("controller");
    let message = controller::Message::Roles {
        guild_id: guild_id.into(),
        response: role_tx,
        span,
    };
    if let Err(err) = CONTROLLER_CHANNEL
        .wait()
        .send(message)
        .in_current_span()
        .await
    {
        error!("Error sending message to controller: {:?}", err);
    }
    let roles = role_rx.in_current_span().await?;
    let span = info_span!("controller");
    let message = controller::Message::Unregister {
        user_id: user_id.into(),
        username: interaction.user.name.clone(),
        response_tx: tx,
        removed_tx,
        span,
    };
    if let Err(err) = CONTROLLER_CHANNEL
        .wait()
        .send(message)
        .in_current_span()
        .await
    {
        error!("Error sending message to controller: {:?}", err);
    }
    let response = rx.in_current_span().await?;
    match response {
        UnRegisterResponse::NotRegistered => {
            respond(ctx, interaction, "You are not registered", true)
                .in_current_span()
                .await?
        }
        UnRegisterResponse::Unregister(url) => {
            unregister_user(ctx, interaction, &url)
                .in_current_span()
                .await?
        }
        UnRegisterResponse::Error(why) => bail!("Error unregistering: {}", why),
    };
    match removed_rx.in_current_span().await? {
        RemoveUserResponse::Success => {
            let mut message = MessageBuilder::new();
            message.push("You have been removed from the following roles: ");
            for role in roles.iter() {
                if let Err(why) = ctx
                    .http
                    .remove_member_role(guild_id.into(), user_id.into(), *role, None)
                    .in_current_span()
                    .await
                {
                    info!("Could not remove role: {:?}", why);
                }

                message.role(*role);
            }
            message.build();
            follow_up(&ctx, interaction, message, true)
                .in_current_span()
                .await
        }
        RemoveUserResponse::Error(why) => {
            info!("Error while removing user: {}", why);
            Ok(())
        }
    }
}

#[instrument(level = "info", skip(ctx, interaction))]
async fn register_user(
    ctx: &Context,
    interaction: &ApplicationCommandInteraction,
    url: &str,
) -> Result<()> {
    debug!("Registering user");
    let message = format!(
        "You need to register your wallet address with your discord user to get \
        gated roles. Please go to {} and follow the instructions.",
        url
    );
    follow_up(ctx, interaction, message, true)
        .in_current_span()
        .await
}

#[instrument(level = "info", skip(ctx, interaction))]
async fn unregister_user(
    ctx: &Context,
    interaction: &ApplicationCommandInteraction,
    url: &str,
) -> Result<()> {
    debug!("Unregistering user");
    let message = format!(
        "☠️ ☠️ ☠️  To unregister your wallet from your discord user follow this link \
        {} and follow the instructions. ☠️ ☠️ ☠️",
        url
    );
    respond(ctx, interaction, message, true)
        .in_current_span()
        .await
}

#[instrument(level = "info", skip(ctx, interaction))]
async fn grant_roles(
    ctx: &Context,
    interaction: &ApplicationCommandInteraction,
    roles: &Vec<u64>,
) -> Result<()> {
    debug!(?roles, "Granting roles in discord");
    let mut granted_roles = Vec::new();
    let mut failed_roles = Vec::new();
    for role in roles.iter() {
        if let Err(why) = ctx
            .http
            .add_member_role(
                interaction
                    .guild_id
                    .ok_or(anyhow!("Error getting guild id from command"))?
                    .into(),
                interaction.user.id.into(),
                *role,
                None,
            )
            .in_current_span()
            .await
        {
            warn!(role, "Error adding role: {:?}", why);
            failed_roles.push(*role);
        } else {
            debug!(role, "Role added");
            granted_roles.push(*role);
        }
    }

    let mut content = MessageBuilder::new();
    content.user(&interaction.user);
    if granted_roles.is_empty() {
        content.push_line("used the `/get in` but sadly, didn't get any roles yet 😢");
    } else {
        content.push("used the `/get in` command and got the following roles: ");
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
    if ephemeral {
        follow_up(ctx, interaction, &content, ephemeral)
            .in_current_span()
            .await
    } else {
        interaction
            .channel_id
            .say(&ctx.http, &content)
            .in_current_span()
            .await?;
        Ok(())
    }
}

#[instrument(level = "info")]
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

#[instrument(level = "info")]
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

#[instrument(level = "info", skip(ctx, interaction))]
async fn respond(
    ctx: &Context,
    interaction: &ApplicationCommandInteraction,
    message: impl ToString + std::fmt::Debug,
    ephemeral: bool,
) -> Result<()> {
    debug!("Responding to interaction");
    Ok(interaction
        .create_interaction_response(&ctx.http, |response| {
            response
                .kind(InteractionResponseType::ChannelMessageWithSource)
                .interaction_response_data(|m| m.content(message).ephemeral(ephemeral))
        })
        .in_current_span()
        .await?)
}

#[instrument(level = "info", skip(ctx, interaction))]
async fn follow_up(
    ctx: &Context,
    interaction: &ApplicationCommandInteraction,
    message: impl ToString + std::fmt::Debug,
    ephemeral: bool,
) -> Result<()> {
    debug!("Following up with interaction");
    Ok(interaction
        .create_followup_message(&ctx.http, |m| m.content(message).ephemeral(ephemeral))
        .in_current_span()
        .await
        .map(|_| ())?)
}

#[instrument(level = "info", skip(ctx))]
async fn is_below_bot_in_hierarchy(
    position: u64,
    ctx: &Context,
    guild_id: u64,
    bot_user_id: u64,
) -> Result<bool> {
    let bot_member = ctx
        .http
        .get_member(guild_id, bot_user_id)
        .in_current_span()
        .await?;
    let bot_roles = bot_member.roles;
    let guild_roles = ctx.http.get_guild_roles(guild_id).in_current_span().await?;
    if let Some(max) = guild_roles
        .iter()
        .filter(|r| bot_roles.iter().any(|&br| br == r.id))
        .map(|r| r.position)
        .max()
    {
        Ok(position < max as u64)
    } else {
        error!("No bot roles found");
        anyhow::bail!("No bot roles found");
    }
}

#[instrument(level = "info", skip(interaction))]
fn extract_gate_add_options(
    interaction: &ApplicationCommandInteraction,
) -> Result<(String, u64, u64, u64, Vec<GateOptionValue>)> {
    let mut role_id: Option<u64> = None;
    let mut role_position: u64 = 0;
    let mut guild_id: u64 = 0;
    let add_option = interaction
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
