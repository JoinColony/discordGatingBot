use crate::config::CONFIG;
use crate::controller::{self, CheckResponse, CONTROLLER_CHANNEL};
use serenity::async_trait;
use serenity::http::Http;
use serenity::model::application::command::Command;
use serenity::model::application::interaction::{
    application_command::{ApplicationCommandInteraction, CommandDataOptionValue},
    Interaction, InteractionResponseType,
};

use serenity::model::gateway::{GatewayIntents, Ready};
use serenity::model::id::GuildId;
use serenity::model::permissions::Permissions;
use serenity::model::prelude::command::CommandOptionType;
use serenity::prelude::*;
use serenity::utils::MessageBuilder;

use tokio::sync::oneshot;
use tracing::{debug, error, info, warn};

pub async fn start() {
    // Configure the client with your Discord bot token in the environment.
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
            .create_application_command(|c| {
                debug!("Creating command gate");
                c.name("gate")
                    .description("Make a role gated by the reputation in a colony")
                    .create_option(|o| {
                        o.name("colony")
                            .description("The colony in which the reputation guards the role")
                            .kind(CommandOptionType::String)
                            .required(true)
                    })
                    .create_option(|o| {
                        o.name("reputation")
                            .description("The reputation required to get the role")
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
            })
            .create_application_command(|c| {
                debug!("Creating command check");
                c.name("check")
                    .description("Check the reputation of a colony and get the gated roles")
            })
    })
    .await;
    if let Err(why) = command_result {
        error!("Error creating slash commands: {:?}", why);
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
    if let Err(why) = Command::create_global_application_command(&http, |c| {
        debug!("Creating global command gate");
        c.name("gate")
            .description("Make a role gated by the reputation in a colony")
            .create_option(|o| {
                o.name("colony")
                    .description("The colony in which the reputation guards the role")
                    .kind(CommandOptionType::String)
                    .required(true)
            })
            .create_option(|o| {
                o.name("reputation")
                    .description("The reputation required to get the role")
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
    })
    .await
    {
        error!("Error creating global slash command gate: {:?}", why);
    }
    if let Err(why) = Command::create_global_application_command(&http, |c| {
        debug!("Creating global command check");
        c.name("check")
            .description("Check the reputation of a colony and get the gated roles")
    })
    .await
    {
        error!("Error creating global slash command check: {:?}", why);
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
                _ => {
                    command
                        .create_interaction_response(&ctx.http, |response| {
                            response
                                .kind(InteractionResponseType::ChannelMessageWithSource)
                                .interaction_response_data(|message| {
                                    message.content("There are no gated roles on this server")
                                })
                        })
                        .await
                }
            };
            if let Err(why) = interaction_response {
                error!("Error responding to interaction: {:?}", why);
            }
        }
    }
}

async fn check_interaction_response(
    command: &ApplicationCommandInteraction,
    ctx: &Context,
) -> Result<(), SerenityError> {
    let (tx, rx) = oneshot::channel();
    let message = controller::Message::Check {
        user_id: command.user.id.into(),
        guild_id: command.guild_id.unwrap().into(),
        response_tx: tx,
    };
    if let Err(err) = CONTROLLER_CHANNEL.wait().send(message).await {
        error!("Error sending message to controller: {:?}", err);
    }
    let response = rx.await.unwrap();
    match response {
        CheckResponse::Grant(roles) => {
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
                    if let Err(why) = command
                        .create_interaction_response(&ctx.http, |response| {
                            response
                                .kind(InteractionResponseType::ChannelMessageWithSource)
                                .interaction_response_data(|message| {
                                    message.content(format!(
                                        "Got error while granting roles: {:?}: error: {}, maybe your admin should check the role hierarchy",
                                        roles, why
                                    ))
                                })
                        })
                        .await {
                        warn!("Error responding to interaction: {:?}", why);
                        }
                    return Err(why);
                }
            }
            command
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
                            // message.content(format!(
                            //     "Your reputation in the colonies is good enough to get these roles: {:?}",
                            // roles
                            // ))
                        })
                })
                .await
        }
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
        CheckResponse::NoGates => {
            command
                .create_interaction_response(&ctx.http, |response| {
                    response
                        .kind(InteractionResponseType::ChannelMessageWithSource)
                        .interaction_response_data(|message| {
                            message.content("Here are no roles gated by colony reputation")
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
    for option in command.data.options.iter() {
        match option.name.as_str() {
            "colony" => {
                if let CommandDataOptionValue::String(colony_value) =
                    option.resolved.as_ref().unwrap()
                {
                    colony = colony_value.into();
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
