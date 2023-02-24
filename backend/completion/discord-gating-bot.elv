
use builtin;
use str;

set edit:completion:arg-completer[discord-gating-bot] = {|@words|
    fn spaces {|n|
        builtin:repeat $n ' ' | str:join ''
    }
    fn cand {|text desc|
        edit:complex-candidate $text &display=$text' '(spaces (- 14 (wcswidth $text)))$desc
    }
    var command = 'discord-gating-bot'
    for word $words[1..-1] {
        if (str:has-prefix $word '-') {
            break
        }
        set command = $command';'$word
    }
    var completions = [
        &'discord-gating-bot'= {
            cand -c 'Sets a custom config file'
            cand --config-file 'Sets a custom config file'
            cand -t 'The discord bot token'
            cand --token 'The discord bot token'
            cand -i 'The discor bot invitation url'
            cand --invite-url 'The discor bot invitation url'
            cand -h 'The address to listen on'
            cand --host 'The address to listen on'
            cand -u 'The base url under which the server is reachable'
            cand --url 'The base url under which the server is reachable'
            cand -p 'The port to listen on'
            cand --port 'The port to listen on'
            cand -d 'The path where the persistent data is stored'
            cand --directory 'The path where the persistent data is stored'
            cand -s 'How to store data, on disk or in memory'
            cand --storage-type 'How to store data, on disk or in memory'
            cand -k 'The encryption_key used to encrypt the stored data'
            cand --key 'The encryption_key used to encrypt the stored data'
            cand --help 'Print help information'
            cand -V 'Print version information'
            cand --version 'Print version information'
            cand -v 'Define the verbosity of the application, repeat for more verbosity'
            cand --verbose 'Define the verbosity of the application, repeat for more verbosity'
            cand -q 'Supress all logging'
            cand --quiet 'Supress all logging'
            cand config 'Print the configuration and get a template file'
            cand storage 'Interact with the presistent storage and encryption'
            cand discord 'Interact with discord directly, e.g. register slash commands'
            cand help 'Print this message or the help of the given subcommand(s)'
        }
        &'discord-gating-bot;config'= {
            cand -h 'Print help information'
            cand --help 'Print help information'
            cand -v 'Define the verbosity of the application, repeat for more verbosity'
            cand --verbose 'Define the verbosity of the application, repeat for more verbosity'
            cand -q 'Supress all logging'
            cand --quiet 'Supress all logging'
            cand show 'Print the configuration sources and merged config'
            cand template 'Prints an example configuration template'
            cand help 'Print this message or the help of the given subcommand(s)'
        }
        &'discord-gating-bot;config;show'= {
            cand -h 'Print help information'
            cand --help 'Print help information'
            cand -v 'Define the verbosity of the application, repeat for more verbosity'
            cand --verbose 'Define the verbosity of the application, repeat for more verbosity'
            cand -q 'Supress all logging'
            cand --quiet 'Supress all logging'
        }
        &'discord-gating-bot;config;template'= {
            cand -h 'Print help information'
            cand --help 'Print help information'
            cand -v 'Define the verbosity of the application, repeat for more verbosity'
            cand --verbose 'Define the verbosity of the application, repeat for more verbosity'
            cand -q 'Supress all logging'
            cand --quiet 'Supress all logging'
        }
        &'discord-gating-bot;config;help'= {
            cand -v 'Define the verbosity of the application, repeat for more verbosity'
            cand --verbose 'Define the verbosity of the application, repeat for more verbosity'
            cand -q 'Supress all logging'
            cand --quiet 'Supress all logging'
        }
        &'discord-gating-bot;storage'= {
            cand -h 'Print help information'
            cand --help 'Print help information'
            cand -v 'Define the verbosity of the application, repeat for more verbosity'
            cand --verbose 'Define the verbosity of the application, repeat for more verbosity'
            cand -q 'Supress all logging'
            cand --quiet 'Supress all logging'
            cand generate 'Generates a new key than can be used for encryption at rest'
            cand guild 'List or delete discord guilds in the db'
            cand user 'List, add or delete discord users in the db'
            cand gate 'List, add or delete discord role gates in the db'
            cand help 'Print this message or the help of the given subcommand(s)'
        }
        &'discord-gating-bot;storage;generate'= {
            cand -h 'Print help information'
            cand --help 'Print help information'
            cand -v 'Define the verbosity of the application, repeat for more verbosity'
            cand --verbose 'Define the verbosity of the application, repeat for more verbosity'
            cand -q 'Supress all logging'
            cand --quiet 'Supress all logging'
        }
        &'discord-gating-bot;storage;guild'= {
            cand -h 'Print help information'
            cand --help 'Print help information'
            cand -v 'Define the verbosity of the application, repeat for more verbosity'
            cand --verbose 'Define the verbosity of the application, repeat for more verbosity'
            cand -q 'Supress all logging'
            cand --quiet 'Supress all logging'
            cand list 'List all guilds'
            cand remove 'Remove a guild'
            cand help 'Print this message or the help of the given subcommand(s)'
        }
        &'discord-gating-bot;storage;guild;list'= {
            cand -h 'Print help information'
            cand --help 'Print help information'
            cand -v 'Define the verbosity of the application, repeat for more verbosity'
            cand --verbose 'Define the verbosity of the application, repeat for more verbosity'
            cand -q 'Supress all logging'
            cand --quiet 'Supress all logging'
        }
        &'discord-gating-bot;storage;guild;remove'= {
            cand -h 'Print help information'
            cand --help 'Print help information'
            cand -v 'Define the verbosity of the application, repeat for more verbosity'
            cand --verbose 'Define the verbosity of the application, repeat for more verbosity'
            cand -q 'Supress all logging'
            cand --quiet 'Supress all logging'
        }
        &'discord-gating-bot;storage;guild;help'= {
            cand -v 'Define the verbosity of the application, repeat for more verbosity'
            cand --verbose 'Define the verbosity of the application, repeat for more verbosity'
            cand -q 'Supress all logging'
            cand --quiet 'Supress all logging'
        }
        &'discord-gating-bot;storage;user'= {
            cand -h 'Print help information'
            cand --help 'Print help information'
            cand -v 'Define the verbosity of the application, repeat for more verbosity'
            cand --verbose 'Define the verbosity of the application, repeat for more verbosity'
            cand -q 'Supress all logging'
            cand --quiet 'Supress all logging'
            cand list 'List all users'
            cand add 'Add a new user'
            cand remove 'Remove a user'
            cand help 'Print this message or the help of the given subcommand(s)'
        }
        &'discord-gating-bot;storage;user;list'= {
            cand -h 'Print help information'
            cand --help 'Print help information'
            cand -v 'Define the verbosity of the application, repeat for more verbosity'
            cand --verbose 'Define the verbosity of the application, repeat for more verbosity'
            cand -q 'Supress all logging'
            cand --quiet 'Supress all logging'
        }
        &'discord-gating-bot;storage;user;add'= {
            cand -h 'Print help information'
            cand --help 'Print help information'
            cand -v 'Define the verbosity of the application, repeat for more verbosity'
            cand --verbose 'Define the verbosity of the application, repeat for more verbosity'
            cand -q 'Supress all logging'
            cand --quiet 'Supress all logging'
        }
        &'discord-gating-bot;storage;user;remove'= {
            cand -h 'Print help information'
            cand --help 'Print help information'
            cand -v 'Define the verbosity of the application, repeat for more verbosity'
            cand --verbose 'Define the verbosity of the application, repeat for more verbosity'
            cand -q 'Supress all logging'
            cand --quiet 'Supress all logging'
        }
        &'discord-gating-bot;storage;user;help'= {
            cand -v 'Define the verbosity of the application, repeat for more verbosity'
            cand --verbose 'Define the verbosity of the application, repeat for more verbosity'
            cand -q 'Supress all logging'
            cand --quiet 'Supress all logging'
        }
        &'discord-gating-bot;storage;gate'= {
            cand -h 'Print help information'
            cand --help 'Print help information'
            cand -v 'Define the verbosity of the application, repeat for more verbosity'
            cand --verbose 'Define the verbosity of the application, repeat for more verbosity'
            cand -q 'Supress all logging'
            cand --quiet 'Supress all logging'
            cand list 'List all gates'
            cand add 'Add a new gate'
            cand remove 'Remove a gate'
            cand help 'Print this message or the help of the given subcommand(s)'
        }
        &'discord-gating-bot;storage;gate;list'= {
            cand -g 'The discord guild(server) id'
            cand --guild 'The discord guild(server) id'
            cand -a 'List gates in all guilds'
            cand --all-guilds 'List gates in all guilds'
            cand -h 'Print help information'
            cand --help 'Print help information'
            cand -v 'Define the verbosity of the application, repeat for more verbosity'
            cand --verbose 'Define the verbosity of the application, repeat for more verbosity'
            cand -q 'Supress all logging'
            cand --quiet 'Supress all logging'
        }
        &'discord-gating-bot;storage;gate;add'= {
            cand -h 'Print help information'
            cand --help 'Print help information'
            cand -v 'Define the verbosity of the application, repeat for more verbosity'
            cand --verbose 'Define the verbosity of the application, repeat for more verbosity'
            cand -q 'Supress all logging'
            cand --quiet 'Supress all logging'
        }
        &'discord-gating-bot;storage;gate;remove'= {
            cand -h 'Print help information'
            cand --help 'Print help information'
            cand -v 'Define the verbosity of the application, repeat for more verbosity'
            cand --verbose 'Define the verbosity of the application, repeat for more verbosity'
            cand -q 'Supress all logging'
            cand --quiet 'Supress all logging'
        }
        &'discord-gating-bot;storage;gate;help'= {
            cand -v 'Define the verbosity of the application, repeat for more verbosity'
            cand --verbose 'Define the verbosity of the application, repeat for more verbosity'
            cand -q 'Supress all logging'
            cand --quiet 'Supress all logging'
        }
        &'discord-gating-bot;storage;help'= {
            cand -v 'Define the verbosity of the application, repeat for more verbosity'
            cand --verbose 'Define the verbosity of the application, repeat for more verbosity'
            cand -q 'Supress all logging'
            cand --quiet 'Supress all logging'
        }
        &'discord-gating-bot;discord'= {
            cand -h 'Print help information'
            cand --help 'Print help information'
            cand -v 'Define the verbosity of the application, repeat for more verbosity'
            cand --verbose 'Define the verbosity of the application, repeat for more verbosity'
            cand -q 'Supress all logging'
            cand --quiet 'Supress all logging'
            cand register 'Register the global slash commands'
            cand delete 'Register the slash commands for a specific guild'
            cand help 'Print this message or the help of the given subcommand(s)'
        }
        &'discord-gating-bot;discord;register'= {
            cand -h 'Print help information'
            cand --help 'Print help information'
            cand -v 'Define the verbosity of the application, repeat for more verbosity'
            cand --verbose 'Define the verbosity of the application, repeat for more verbosity'
            cand -q 'Supress all logging'
            cand --quiet 'Supress all logging'
            cand global 'Register the global slash commands'
            cand guild 'Register the slash commands for a specific guild'
            cand help 'Print this message or the help of the given subcommand(s)'
        }
        &'discord-gating-bot;discord;register;global'= {
            cand -h 'Print help information'
            cand --help 'Print help information'
            cand -v 'Define the verbosity of the application, repeat for more verbosity'
            cand --verbose 'Define the verbosity of the application, repeat for more verbosity'
            cand -q 'Supress all logging'
            cand --quiet 'Supress all logging'
        }
        &'discord-gating-bot;discord;register;guild'= {
            cand -h 'Print help information'
            cand --help 'Print help information'
            cand -v 'Define the verbosity of the application, repeat for more verbosity'
            cand --verbose 'Define the verbosity of the application, repeat for more verbosity'
            cand -q 'Supress all logging'
            cand --quiet 'Supress all logging'
        }
        &'discord-gating-bot;discord;register;help'= {
            cand -v 'Define the verbosity of the application, repeat for more verbosity'
            cand --verbose 'Define the verbosity of the application, repeat for more verbosity'
            cand -q 'Supress all logging'
            cand --quiet 'Supress all logging'
        }
        &'discord-gating-bot;discord;delete'= {
            cand -h 'Print help information'
            cand --help 'Print help information'
            cand -v 'Define the verbosity of the application, repeat for more verbosity'
            cand --verbose 'Define the verbosity of the application, repeat for more verbosity'
            cand -q 'Supress all logging'
            cand --quiet 'Supress all logging'
            cand global 'Register the global slash commands'
            cand guild 'Register the slash commands for a specific guild'
            cand help 'Print this message or the help of the given subcommand(s)'
        }
        &'discord-gating-bot;discord;delete;global'= {
            cand -h 'Print help information'
            cand --help 'Print help information'
            cand -v 'Define the verbosity of the application, repeat for more verbosity'
            cand --verbose 'Define the verbosity of the application, repeat for more verbosity'
            cand -q 'Supress all logging'
            cand --quiet 'Supress all logging'
        }
        &'discord-gating-bot;discord;delete;guild'= {
            cand -h 'Print help information'
            cand --help 'Print help information'
            cand -v 'Define the verbosity of the application, repeat for more verbosity'
            cand --verbose 'Define the verbosity of the application, repeat for more verbosity'
            cand -q 'Supress all logging'
            cand --quiet 'Supress all logging'
        }
        &'discord-gating-bot;discord;delete;help'= {
            cand -v 'Define the verbosity of the application, repeat for more verbosity'
            cand --verbose 'Define the verbosity of the application, repeat for more verbosity'
            cand -q 'Supress all logging'
            cand --quiet 'Supress all logging'
        }
        &'discord-gating-bot;discord;help'= {
            cand -v 'Define the verbosity of the application, repeat for more verbosity'
            cand --verbose 'Define the verbosity of the application, repeat for more verbosity'
            cand -q 'Supress all logging'
            cand --quiet 'Supress all logging'
        }
        &'discord-gating-bot;help'= {
            cand -v 'Define the verbosity of the application, repeat for more verbosity'
            cand --verbose 'Define the verbosity of the application, repeat for more verbosity'
            cand -q 'Supress all logging'
            cand --quiet 'Supress all logging'
        }
    ]
    $completions[$command]
}
