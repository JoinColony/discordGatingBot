complete -c discord-gating-bot -n "__fish_use_subcommand" -s c -l config-file -d 'Sets a custom config file' -r -F
complete -c discord-gating-bot -n "__fish_use_subcommand" -s t -l token -d 'The discord bot token' -r
complete -c discord-gating-bot -n "__fish_use_subcommand" -s s -l shards -d 'The number of guild shards' -r
complete -c discord-gating-bot -n "__fish_use_subcommand" -s h -l host -d 'The address to listen on' -r
complete -c discord-gating-bot -n "__fish_use_subcommand" -s p -l port -d 'The port to listen on' -r
complete -c discord-gating-bot -n "__fish_use_subcommand" -l cert -d 'The path to the certificate File' -r -F
complete -c discord-gating-bot -n "__fish_use_subcommand" -s k -l key -d 'The path to the private key File' -r -F
complete -c discord-gating-bot -n "__fish_use_subcommand" -l acme-endpoint -d 'The address of the acme server to use' -r
complete -c discord-gating-bot -n "__fish_use_subcommand" -l acme-port -d 'The port to listen on' -r
complete -c discord-gating-bot -n "__fish_use_subcommand" -l directory -d 'The path to the directory where the certificates are stored' -r -f -a "(__fish_complete_directories)"
complete -c discord-gating-bot -n "__fish_use_subcommand" -l staging-directory -d 'The path to the directory where the certificates are stored' -r -f -a "(__fish_complete_directories)"
complete -c discord-gating-bot -n "__fish_use_subcommand" -l staging -d 'The path to the directory where the certificates are stored' -r -f -a "(__fish_complete_directories)"
complete -c discord-gating-bot -n "__fish_use_subcommand" -l encryption-key -d 'The encryption key to use for the database and session tokens' -r
complete -c discord-gating-bot -n "__fish_use_subcommand" -l help -d 'Print help information'
complete -c discord-gating-bot -n "__fish_use_subcommand" -s V -l version -d 'Print version information'
complete -c discord-gating-bot -n "__fish_use_subcommand" -s v -l verbose -d 'Define the verbosity of the application, repeat for more verbosity'
complete -c discord-gating-bot -n "__fish_use_subcommand" -s q -l quiet -d 'Supress all logging'
complete -c discord-gating-bot -n "__fish_use_subcommand" -f -a "completion" -d 'Generates completion scripts for the specified shell'
complete -c discord-gating-bot -n "__fish_use_subcommand" -f -a "config" -d 'Print or edit the configuration'
complete -c discord-gating-bot -n "__fish_use_subcommand" -f -a "key" -d 'Generate an encrypted key'
complete -c discord-gating-bot -n "__fish_use_subcommand" -f -a "discord" -d 'Interact with discord directly'
complete -c discord-gating-bot -n "__fish_use_subcommand" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c discord-gating-bot -n "__fish_seen_subcommand_from completion" -s h -l help -d 'Print help information'
complete -c discord-gating-bot -n "__fish_seen_subcommand_from completion" -s v -l verbose -d 'Define the verbosity of the application, repeat for more verbosity'
complete -c discord-gating-bot -n "__fish_seen_subcommand_from completion" -s q -l quiet -d 'Supress all logging'
complete -c discord-gating-bot -n "__fish_seen_subcommand_from config; and not __fish_seen_subcommand_from show; and not __fish_seen_subcommand_from template; and not __fish_seen_subcommand_from help" -s h -l help -d 'Print help information'
complete -c discord-gating-bot -n "__fish_seen_subcommand_from config; and not __fish_seen_subcommand_from show; and not __fish_seen_subcommand_from template; and not __fish_seen_subcommand_from help" -s v -l verbose -d 'Define the verbosity of the application, repeat for more verbosity'
complete -c discord-gating-bot -n "__fish_seen_subcommand_from config; and not __fish_seen_subcommand_from show; and not __fish_seen_subcommand_from template; and not __fish_seen_subcommand_from help" -s q -l quiet -d 'Supress all logging'
complete -c discord-gating-bot -n "__fish_seen_subcommand_from config; and not __fish_seen_subcommand_from show; and not __fish_seen_subcommand_from template; and not __fish_seen_subcommand_from help" -f -a "show" -d 'Print the configuration sources and merged config'
complete -c discord-gating-bot -n "__fish_seen_subcommand_from config; and not __fish_seen_subcommand_from show; and not __fish_seen_subcommand_from template; and not __fish_seen_subcommand_from help" -f -a "template" -d 'Prints an example configuration template'
complete -c discord-gating-bot -n "__fish_seen_subcommand_from config; and not __fish_seen_subcommand_from show; and not __fish_seen_subcommand_from template; and not __fish_seen_subcommand_from help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c discord-gating-bot -n "__fish_seen_subcommand_from config; and __fish_seen_subcommand_from show" -s h -l help -d 'Print help information'
complete -c discord-gating-bot -n "__fish_seen_subcommand_from config; and __fish_seen_subcommand_from show" -s v -l verbose -d 'Define the verbosity of the application, repeat for more verbosity'
complete -c discord-gating-bot -n "__fish_seen_subcommand_from config; and __fish_seen_subcommand_from show" -s q -l quiet -d 'Supress all logging'
complete -c discord-gating-bot -n "__fish_seen_subcommand_from config; and __fish_seen_subcommand_from template" -s h -l help -d 'Print help information'
complete -c discord-gating-bot -n "__fish_seen_subcommand_from config; and __fish_seen_subcommand_from template" -s v -l verbose -d 'Define the verbosity of the application, repeat for more verbosity'
complete -c discord-gating-bot -n "__fish_seen_subcommand_from config; and __fish_seen_subcommand_from template" -s q -l quiet -d 'Supress all logging'
complete -c discord-gating-bot -n "__fish_seen_subcommand_from config; and __fish_seen_subcommand_from help" -s v -l verbose -d 'Define the verbosity of the application, repeat for more verbosity'
complete -c discord-gating-bot -n "__fish_seen_subcommand_from config; and __fish_seen_subcommand_from help" -s q -l quiet -d 'Supress all logging'
complete -c discord-gating-bot -n "__fish_seen_subcommand_from key; and not __fish_seen_subcommand_from generate; and not __fish_seen_subcommand_from help" -s h -l help -d 'Print help information'
complete -c discord-gating-bot -n "__fish_seen_subcommand_from key; and not __fish_seen_subcommand_from generate; and not __fish_seen_subcommand_from help" -s v -l verbose -d 'Define the verbosity of the application, repeat for more verbosity'
complete -c discord-gating-bot -n "__fish_seen_subcommand_from key; and not __fish_seen_subcommand_from generate; and not __fish_seen_subcommand_from help" -s q -l quiet -d 'Supress all logging'
complete -c discord-gating-bot -n "__fish_seen_subcommand_from key; and not __fish_seen_subcommand_from generate; and not __fish_seen_subcommand_from help" -f -a "generate" -d 'Generates a new key than can be used for encryption at rest and for the sessions tokens'
complete -c discord-gating-bot -n "__fish_seen_subcommand_from key; and not __fish_seen_subcommand_from generate; and not __fish_seen_subcommand_from help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c discord-gating-bot -n "__fish_seen_subcommand_from key; and __fish_seen_subcommand_from generate" -s h -l help -d 'Print help information'
complete -c discord-gating-bot -n "__fish_seen_subcommand_from key; and __fish_seen_subcommand_from generate" -s v -l verbose -d 'Define the verbosity of the application, repeat for more verbosity'
complete -c discord-gating-bot -n "__fish_seen_subcommand_from key; and __fish_seen_subcommand_from generate" -s q -l quiet -d 'Supress all logging'
complete -c discord-gating-bot -n "__fish_seen_subcommand_from key; and __fish_seen_subcommand_from help" -s v -l verbose -d 'Define the verbosity of the application, repeat for more verbosity'
complete -c discord-gating-bot -n "__fish_seen_subcommand_from key; and __fish_seen_subcommand_from help" -s q -l quiet -d 'Supress all logging'
complete -c discord-gating-bot -n "__fish_seen_subcommand_from discord; and not __fish_seen_subcommand_from global; and not __fish_seen_subcommand_from server; and not __fish_seen_subcommand_from help" -s h -l help -d 'Print help information'
complete -c discord-gating-bot -n "__fish_seen_subcommand_from discord; and not __fish_seen_subcommand_from global; and not __fish_seen_subcommand_from server; and not __fish_seen_subcommand_from help" -s v -l verbose -d 'Define the verbosity of the application, repeat for more verbosity'
complete -c discord-gating-bot -n "__fish_seen_subcommand_from discord; and not __fish_seen_subcommand_from global; and not __fish_seen_subcommand_from server; and not __fish_seen_subcommand_from help" -s q -l quiet -d 'Supress all logging'
complete -c discord-gating-bot -n "__fish_seen_subcommand_from discord; and not __fish_seen_subcommand_from global; and not __fish_seen_subcommand_from server; and not __fish_seen_subcommand_from help" -f -a "global" -d 'Register the global slash commands'
complete -c discord-gating-bot -n "__fish_seen_subcommand_from discord; and not __fish_seen_subcommand_from global; and not __fish_seen_subcommand_from server; and not __fish_seen_subcommand_from help" -f -a "server" -d 'Register the slash commands for a specific guild'
complete -c discord-gating-bot -n "__fish_seen_subcommand_from discord; and not __fish_seen_subcommand_from global; and not __fish_seen_subcommand_from server; and not __fish_seen_subcommand_from help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c discord-gating-bot -n "__fish_seen_subcommand_from discord; and __fish_seen_subcommand_from global" -s h -l help -d 'Print help information'
complete -c discord-gating-bot -n "__fish_seen_subcommand_from discord; and __fish_seen_subcommand_from global" -s v -l verbose -d 'Define the verbosity of the application, repeat for more verbosity'
complete -c discord-gating-bot -n "__fish_seen_subcommand_from discord; and __fish_seen_subcommand_from global" -s q -l quiet -d 'Supress all logging'
complete -c discord-gating-bot -n "__fish_seen_subcommand_from discord; and __fish_seen_subcommand_from server" -s h -l help -d 'Print help information'
complete -c discord-gating-bot -n "__fish_seen_subcommand_from discord; and __fish_seen_subcommand_from server" -s v -l verbose -d 'Define the verbosity of the application, repeat for more verbosity'
complete -c discord-gating-bot -n "__fish_seen_subcommand_from discord; and __fish_seen_subcommand_from server" -s q -l quiet -d 'Supress all logging'
complete -c discord-gating-bot -n "__fish_seen_subcommand_from discord; and __fish_seen_subcommand_from help" -s v -l verbose -d 'Define the verbosity of the application, repeat for more verbosity'
complete -c discord-gating-bot -n "__fish_seen_subcommand_from discord; and __fish_seen_subcommand_from help" -s q -l quiet -d 'Supress all logging'
complete -c discord-gating-bot -n "__fish_seen_subcommand_from help" -s v -l verbose -d 'Define the verbosity of the application, repeat for more verbosity'
complete -c discord-gating-bot -n "__fish_seen_subcommand_from help" -s q -l quiet -d 'Supress all logging'
