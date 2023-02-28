//! Handles the communication with the Discord API.
//!

use crate::config::CONFIG;
use crate::controller::{self, CheckResponse, UnRegisterResponse, CONTROLLER_CHANNEL};
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
                autocomplete::AutocompleteInteraction,
                Interaction, InteractionResponseType,
            },
        },
        gateway::{GatewayIntents, Ready},
        id::GuildId,
        permissions::Permissions,
        prelude::{command::CommandOptionType, RoleId},
    },
    prelude::*,
    utils::MessageBuilder,
};
use std::str::FromStr;
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
            .create_application_command(make_get_command)
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
    if let Err(why) = Command::create_global_application_command(&http, make_get_command).await {
        error!("Error creating global slash command get: {:?}", why);
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
        info!("{}({}) is connected!", ready.user.name, ready.user.id);
    }
    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        debug!("Received interaction: {:?}", interaction);
        if let Interaction::ApplicationCommand(command) = interaction.clone() {
            let interaction_response = match command.data.name.as_str() {
                "gate" => gate_interaction_response(&command, &ctx).await,
                "get" => get_interaction_response(&command, &ctx).await,
                "list_gates" => list_gates_interaction_response(&command, &ctx).await,
                "check" => get_in_interaction_response(&command, &ctx).await,
                "checkout" => get_out_interaction_response(&command, &ctx).await,
                _ => unknown_interaction_response(&command, &ctx).await,
            };
            if let Err(why) = interaction_response {
                warn!("Error responding to interaction: {:?}", why);
            }
        }
        if let Interaction::Autocomplete(interaction) = interaction {
            if interaction.data.options.len() != 1 {
                warn!("Autocomplete interaction with more than one option");
                return;
            }
            let interaction_response = match (
                interaction.data.name.as_str(),
                interaction.data.options[0].name.as_str(),
            ) {
                ("gate", "add") => gate_add_autocomplete_interaction(&interaction, &ctx).await,
                _ => return,
            };
            if let Err(why) = interaction_response {
                warn!("Error responding to interaction: {:?}", why);
            }
        };
    }
}

async fn gate_add_autocomplete_interaction(
    interaction: &AutocompleteInteraction,
    ctx: &Context,
) -> Result<(), SerenityError> {
    debug!(
        "Received gate add autocomplete interaction: {:?}",
        interaction
    );
    if interaction.data.options[0].options.len() == 0 {
        warn!("Autocomplete interaction without suboptions");
        return Err(SerenityError::Other("Invalid interaction"));
    }
    let focused = if let Some(focused) = &interaction.data.options[0]
        .options
        .iter()
        .find(|o| o.focused)
    {
        *focused
    } else {
        warn!("Autocomplete interaction without focused option");
        return Err(SerenityError::Other("Invalid interaction"));
    };
    match focused.name.as_str() {
        "colony" => colony_autocomplete_response(&interaction, &ctx).await,
        "domain" => domain_autocomplete_response(&interaction, &ctx).await,
        _ => {
            warn!(
                "No autocompletion {:?}",
                interaction.data.options[0].options[0]
            );
            Ok(())
        }
    }
}

async fn colony_autocomplete_response(
    interaction: &AutocompleteInteraction,
    ctx: &Context,
) -> Result<(), SerenityError> {
    let option = interaction.data.options[0].options[0].clone();
    let typed = option.value.as_ref().unwrap().as_str().unwrap();
    interaction
        .create_autocomplete_response(ctx, |response| {
            if !typed.is_empty() {
                response.add_string_choice(&typed, &typed);
            }
            response
                .add_string_choice(
                    "MetaColonyAddress(0xcfd3aa1ebc6119d80ed47955a87a9d9c281a97b3)",
                    "0xcfd3aa1ebc6119d80ed47955a87a9d9c281a97b3",
                )
                .add_string_choice(
                    "DevColonyAddress(0x364b3153a24bb9eca28b8c7aceb15e3942eb4fc5)",
                    "0x364b3153a24bb9eca28b8c7aceb15e3942eb4fc5",
                )
        })
        .await
}

async fn domain_autocomplete_response(
    interaction: &AutocompleteInteraction,
    ctx: &Context,
) -> Result<(), SerenityError> {
    eprintln!("Domain autocomplete response for {:#?}", interaction);
    let option = interaction.data.options[0].options[1].clone();
    eprintln!("Option: {:#?}", option);
    interaction
        .create_autocomplete_response(ctx, |response| {
            if let Some(typed_raw) = option.value.as_ref() {
                eprintln!("Typed: {:#?}", typed_raw);
                if let Some(typed_str) = typed_raw.as_str() {
                    eprintln!("Typed: {:#?}", typed_str);
                    if let Ok(typed) = typed_str.parse::<i64>() {
                        eprintln!("Typed: {}", typed);
                        response.add_int_choice(typed.to_string(), typed);
                    }
                }
            }
            response
                .add_int_choice("1", 1)
                .add_int_choice("2", 2)
                .add_int_choice("3", 3)
                .add_int_choice("4", 4)
                .add_int_choice("5", 5)
                .add_int_choice("6", 6)
                .add_int_choice("7", 7)
                .add_int_choice("8", 8)
                .add_int_choice("9", 9)
                .add_int_choice("10", 10)
        })
        .await
}

async fn gate_interaction_response(
    interaction: &ApplicationCommandInteraction,
    ctx: &Context,
) -> Result<(), SerenityError> {
    debug!("Received gate interaction: {:?}", interaction);
    let option = &interaction.data.options[0];
    match option.name.as_str() {
        "add" => gate_add_interaction_response(&interaction, &ctx).await,
        "list" => list_gates_interaction_response(&interaction, &ctx).await,
        _ => unknown_interaction_response(&interaction, &ctx).await,
    }
}

async fn get_interaction_response(
    interaction: &ApplicationCommandInteraction,
    ctx: &Context,
) -> Result<(), SerenityError> {
    debug!("Received get interaction: {:?}", interaction);
    let option = &interaction.data.options[0];
    match option.name.as_str() {
        "in" => get_in_interaction_response(&interaction, &ctx).await,
        "out" => get_out_interaction_response(&interaction, &ctx).await,
        _ => unknown_interaction_response(&interaction, &ctx).await,
    }
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

async fn gate_add_interaction_response(
    command: &ApplicationCommandInteraction,
    ctx: &Context,
) -> Result<(), SerenityError> {
    debug!("Received gate add interaction");
    let (colony, reputation, role_id, role_position, guild_id, domain) = extract_options(command)?;
    debug!(
        "Colony: {}, Reputation: {}, Role ID: {}, Guild ID: {}, Domain: {}",
        colony, reputation, role_id, guild_id, domain
    );
    if let Err(why) = validate_gate_input(ctx, command, &colony, domain, role_id, reputation).await
    {
        respond(ctx, command, &why, true).await?;
        info!("Invalid gate command: {}", why);
        return Err(SerenityError::Other("Failed to validate gate input"));
    }

    let message = controller::Message::Gate {
        colony,
        domain: domain as u64,
        reputation: reputation as u32,
        role_id,
        guild_id,
    };
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
    respond(ctx, command, content, true).await
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
    let precision = CONFIG.wait().precision;
    let factor = 10.0f64.powi(-(precision as i32));

    stream::iter(gates)
        .for_each_concurrent(None, |gate| async move {
            let mut content = MessageBuilder::new();
            content.push("The role: ");
            content.role(gate.role_id);
            content.push_line(" is gated by the following criteria");
            let reputation = gate.reputation as f64 * factor;

            let follow_up = command
                .create_followup_message(ctx, |message| {
                    message
                        .ephemeral(true)
                        .content(&content)
                        .embed(|e| {
                            e.field("Colony Address", gate.colony.clone(), true)
                                .field("Domain", gate.domain, true)
                                .field("Reputation", format!("{}%", reputation), true)
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
                    colony: gate.colony.clone(),
                    domain: gate.domain,
                    reputation: gate.reputation,
                    role_id: gate.role_id,
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
                                e.field("Colony Address", gate.colony.clone(), true)
                                    .field("Domain", gate.domain, true)
                                    .field("Reputation", format!("{}%", reputation), true)
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
              this might take a while",
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
    command
        .name("gate")
        .description("Make a role gated by the reputation in a colony")
        .create_option(|option| {
            option
                .name("add")
                .description("Add a new gate to protect a role on the server")
                .kind(CommandOptionType::SubCommand)
                .create_sub_option(|sub_option| {
                    sub_option
                        .name("colony")
                        .description("The colony in which the reputation guards the role")
                        .kind(CommandOptionType::String)
                        .required(true)
                        .set_autocomplete(true)
                })
                .create_sub_option(|sub_option| {
                    sub_option
                        .name("domain")
                        .description(
                            "The domain of the colony in which the reputation guards the role",
                        )
                        .kind(CommandOptionType::Integer)
                        .required(true)
                        .min_int_value(1)
                        .set_autocomplete(true)
                })
                .create_sub_option(|sub_option| {
                    sub_option
                        .name("reputation")
                        .description(
                            "The percentage of reputation in the domain, required to get the role",
                        )
                        .kind(CommandOptionType::Number)
                        .min_number_value(0.0)
                        .max_number_value(100.0)
                        .required(true)
                })
                .create_sub_option(|sub_option| {
                    sub_option
                        .name("role")
                        .description("The role to be gated by reputation")
                        .kind(CommandOptionType::Role)
                        .required(true)
                })
        })
        .create_option(|option| {
            option
                .name("list")
                .description("Lists gates that are currently active for this server.")
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
        return Err("Colony address cannot be empty".to_string());
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
    let precision = CONFIG.wait().precision;
    let factor = 10u64.pow(precision as u32);
    if reputation > 100 * factor as i64 {
        return Err("Reputation must be 100 or less".to_string());
    }

    Ok(())
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

fn extract_options(
    command: &ApplicationCommandInteraction,
) -> Result<(String, i64, u64, u64, u64, i64), SerenityError> {
    let mut colony: String = String::new();
    let mut reputation: i64 = 0;
    let mut role_id: u64 = 0;
    let mut role_position: u64 = 0;
    let mut guild_id: u64 = 0;
    let mut domain: i64 = 0;
    for option in command.data.options.iter() {
        match option.name.as_str() {
            "add" => {
                for sub_option in option.options.iter() {
                    match sub_option.name.as_str() {
                        "colony" => {
                            debug!("colony suboption {:?}", sub_option);
                            if let Some(CommandDataOptionValue::String(colony_value)) =
                                sub_option.resolved.as_ref()
                            {
                                colony = colony_value.to_lowercase();
                            }
                        }
                        "domain" => {
                            debug!("domain suboption {:?}", sub_option);
                            if let Some(CommandDataOptionValue::Integer(domain_value)) =
                                sub_option.resolved.as_ref()
                            {
                                domain = *domain_value as i64;
                            }
                        }
                        "reputation" => {
                            debug!("reputation suboption {:?}", sub_option);
                            if let Some(CommandDataOptionValue::Number(reputation_value)) =
                                sub_option.resolved.as_ref()
                            {
                                let precision = CONFIG.wait().precision;
                                reputation =
                                    (*reputation_value * 10.0_f64.powi(precision as i32)) as i64;
                            }
                        }
                        "role" => {
                            debug!("role suboption {:?}", sub_option);
                            if let Some(CommandDataOptionValue::Role(role)) =
                                sub_option.resolved.as_ref()
                            {
                                role_id = role.id.into();
                                role_position = role.position as u64;
                                guild_id = role.guild_id.into();
                            }
                        }
                        _ => {
                            error!("Unknown suboption {}", sub_option.name);
                            return Err(SerenityError::Other("Unknown option"));
                        }
                    }
                }
            }
            "colony" => {
                debug!("colony option {:?}", option);
                if let CommandDataOptionValue::String(colony_value) =
                    option.resolved.as_ref().unwrap()
                {
                    colony = colony_value.to_lowercase();
                }
            }
            "domain" => {
                debug!("domain option {:?}", option);
                if let CommandDataOptionValue::Integer(domain_value) =
                    option.resolved.as_ref().unwrap()
                {
                    domain = *domain_value as i64;
                }
            }
            "reputation" => {
                debug!("reputation option {:?}", option);
                if let CommandDataOptionValue::Number(reputation_value) =
                    option.resolved.as_ref().unwrap()
                {
                    let precision = CONFIG.wait().precision;
                    reputation = (*reputation_value * 10.0_f64.powi(precision as i32)) as i64;
                }
            }
            "role" => {
                debug!("role option {:?}", option);
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
    Ok((colony, reputation, role_id, role_position, guild_id, domain))
}
