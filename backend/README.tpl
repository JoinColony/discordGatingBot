# {{crate}}

Running the bot binary without any sub command will start an http server,
connect to discord and listen for commands. 

By default the bot will store all data encrypted in an embedded database. 
Most of the action will happens via the slash commands from discord and the 
following redirects to the http server. 

The bot can be configured via a config file, environment variables or 
command line arguments. 

Other sub commands are used offline to help with certain
operations, e.g. key generation and most importantly the slash command 
registration. 

When running the bot for the first time, no slash commands are 
registered for the discord application, which makes the bot pretty useless.
With the `register` sub command, the bot will register all slash commands either
globally or for a specific guild. Global registration may take some time to 
propagate, while guild registration is instant. 

## usage of {{crate}}
{{readme}}

Current version: {{version}}

License: {{license}}
