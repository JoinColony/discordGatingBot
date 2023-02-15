# discord-gating-bot

Running the bot without any sub command will start an http server,
connect to discord and listen for commands, all with the default configuration.

Running the bot for the first time, you probably want to generate an encryption
key and register the discord slash commands with the `storage` and `discord`
subcommands.

## The colony discord gating bot

By default the bot will store all data encrypted in an embedded database.
Most of the action will happen from slash commands in discord and the
following redirects to the http server.

The bot can be configured via a config file, environment variables or
command line arguments.

Other sub commands are used offline to help with certain
operations, e.g. key generation and most importantly the slash command
registration.

### First time usage
Before the bot can be used with discord, you need to setup a discord
application (and a bot) via the
[discord developer portal](https://discord.com/developers/applications).


When running the bot for the first time, no slash commands are
registered for the discord application, which makes the bot pretty useless.
With the `discord global/server` sub command, the bot will register all
slash commands either globally or for a specific guild. Global registration
may take some time to propagate, while guild registration is instant.

To get started just run and go from there
```bash
discord-gating-bot help
```
also man pages are genarated by the cargo build inside the man folder

Current version: 0.1.0

License: GPLv3
