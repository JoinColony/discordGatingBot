//! Handles the communication with the Discord API.
//!

use crate::config::CONFIG;
use crate::controller::{self, CheckResponse, CONTROLLER_CHANNEL};
use serenity::builder::{CreateApplicationCommand, CreateInteractionResponse};
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
            .create_application_command(make_check_command)
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
    if let Err(why) = Command::create_global_application_command(&http, make_check_command).await {
        error!("Error creating global slash command check: {:?}", why);
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
                "check" => check_interaction_response(&command, &ctx).await,
                _ => unknown_interaction_response(&command, &ctx).await,
            };
            if let Err(why) = interaction_response {
                error!("Error responding to interaction: {:?}", why);
            }
        }
    }
}

async fn unknown_interaction_response(
    command: &ApplicationCommandInteraction,
    ctx: &Context,
) -> Result<(), SerenityError> {
    warn!("Unknown interaction: {:?}", command);
    command
        .create_interaction_response(&ctx.http, |response| {
            response
                .kind(InteractionResponseType::ChannelMessageWithSource)
                .interaction_response_data(|message| {
                    message.content("Unknown command. Try /gate or /check")
                })
        })
        .await
}

async fn channel_response<C: ToString>(
    repsonse: &mut CreateInteractionResponse<'_>,
    content: C,
) -> Result<(), SerenityError> {
    Ok(())
}

async fn role_adding_failure_response(
    command: &ApplicationCommandInteraction,
    ctx: &Context,
    roles: &Vec<u64>,
    error: &SerenityError,
) {
    warn!("Error adding roles: {:?}", error);
    if let Err(why) = command.create_interaction_response(&ctx.http, |response| {
        response
            .kind(InteractionResponseType::ChannelMessageWithSource)
            .interaction_response_data(|message| {
                message.content(format!(
                        "Got error while granting roles: {:?}: error: {}, maybe your admin should check the role hierarchy",
                        roles, error
                ))
            })
    })
    .await {
        error!("Error responding to interaction: {:?}", why);
    }
}

async fn role_adding_success_response(
    command: &ApplicationCommandInteraction,
    ctx: &Context,
    roles: &Vec<u64>,
) -> Result<(), SerenityError> {
    if let Err(why) = command
        .create_interaction_response(&ctx.http, |response| {
            response
                .kind(InteractionResponseType::ChannelMessageWithSource)
                .interaction_response_data(|message| {
                    message.content({
                        let mut builder = MessageBuilder::new();
                        builder.push("You have been granted the following roles: ");
                        for role in roles.iter() {
                            builder.role(*role);
                        }
                        builder.build()
                    })
                })
        })
        .await
    {
        error!("Error responding to interaction: {:?}", why);
        return Err(why);
    }
    Ok(())
}

async fn grant_roles(
    ctx: &Context,
    command: &ApplicationCommandInteraction,
    roles: &Vec<u64>,
) -> Result<(), SerenityError> {
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
            role_adding_failure_response(command, ctx, roles, &why).await;
            return Err(why);
        }
    }
    role_adding_success_response(command, ctx, roles).await
}

async fn check_interaction_response(
    command: &ApplicationCommandInteraction,
    ctx: &Context,
) -> Result<(), SerenityError> {
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
        CheckResponse::Register(url) => {
            command.user .direct_message(&ctx.http, |m| {
                m.content(format!(
                    "You need to register your wallet address with your discord user to get gated roles. Please go to {} and follow the instructions.",
                    url
                ))
            })
            .await
            .unwrap();
            command
                .create_interaction_response(&ctx.http, |response| {
                    response
                        .kind(InteractionResponseType::ChannelMessageWithSource)
                        .interaction_response_data(|message| {
                            message.content("You need to register first, check your DMs")
                        })
                })
                .await
        }
    }
}
async fn gate_interaction_response(
    command: &ApplicationCommandInteraction,
    ctx: &Context,
) -> Result<(), SerenityError> {
    let mut colony: String = String::new();
    let mut reputation: u32 = 0;
    let mut role_id: u64 = 0;
    let mut guild_id: u64 = 0;
    let mut domain: u64 = 0;
    for option in command.data.options.iter() {
        match option.name.as_str() {
            "colony" => {
                if let CommandDataOptionValue::String(colony_value) =
                    option.resolved.as_ref().unwrap()
                {
                    colony = colony_value.into();
                }
            }
            "domain" => {
                if let CommandDataOptionValue::Integer(domain_value) =
                    option.resolved.as_ref().unwrap()
                {
                    domain = *domain_value as u64;
                }
            }
            "reputation" => {
                if let CommandDataOptionValue::Integer(reputation_value) =
                    option.resolved.as_ref().unwrap()
                {
                    reputation = *reputation_value as u32;
                }
            }
            "role" => {
                if let CommandDataOptionValue::Role(role) = option.resolved.as_ref().unwrap() {
                    role_id = role.id.into();
                    guild_id = role.guild_id.into();
                }
            }
            _ => error!("Unknown option {}", option.name),
        }
    }
    let message = controller::Message::Gate {
        colony,
        domain,
        reputation,
        role_id,
        guild_id,
    };
    CONTROLLER_CHANNEL.wait().send(message).await.unwrap();
    command
        .create_interaction_response(&ctx.http, |response| {
            response
                .kind(InteractionResponseType::ChannelMessageWithSource)
                .interaction_response_data(|message| message.content("Gates up"))
        })
        .await
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
        })
        .create_option(|o| {
            o.name("domain")
                .description("The domain of the colony in which the reputation guards the role")
                .kind(CommandOptionType::Integer)
                .required(true)
        })
        .create_option(|o| {
            o.name("reputation")
                .description("The percentage of reputation in the domain, required to get the role")
                .kind(CommandOptionType::Integer)
                .required(true)
        })
        .create_option(|o| {
            o.name("role")
                .description("The role to be gated by reputation")
                .kind(CommandOptionType::Role)
                .required(true)
        })
        .default_member_permissions(Permissions::ADMINISTRATOR)
}

fn make_check_command(command: &mut CreateApplicationCommand) -> &mut CreateApplicationCommand {
    debug!("Creating check slash command");
    command
        .name("check")
        .description("Check the reputation of a colony and get the gated roles")
}
