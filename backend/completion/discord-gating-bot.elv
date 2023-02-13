
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
            cand -s 'The number of guild shards'
            cand --shards 'The number of guild shards'
            cand -h 'The address to listen on'
            cand --host 'The address to listen on'
            cand -p 'The port to listen on'
            cand --port 'The port to listen on'
            cand --cert 'The path to the certificate File'
            cand -k 'The path to the private key File'
            cand --key 'The path to the private key File'
            cand --acme-endpoint 'The address of the acme server to use'
            cand --acme-port 'The port to listen on'
            cand --directory 'The path to the directory where the certificates are stored'
            cand --staging-directory 'The path to the directory where the certificates are stored'
            cand --staging 'The path to the directory where the certificates are stored'
            cand --encryption-key 'The encryption key to use for the database and session tokens'
            cand --help 'Print help information'
            cand -V 'Print version information'
            cand --version 'Print version information'
            cand -v 'Define the verbosity of the application, repeat for more verbosity'
            cand --verbose 'Define the verbosity of the application, repeat for more verbosity'
            cand -q 'Supress all logging'
            cand --quiet 'Supress all logging'
            cand completion 'Generates completion scripts for the specified shell'
            cand config 'Print or edit the configuration'
            cand key 'Generate an encrypted key'
            cand discord 'Interact with discord directly'
            cand help 'Print this message or the help of the given subcommand(s)'
        }
        &'discord-gating-bot;completion'= {
            cand -h 'Print help information'
            cand --help 'Print help information'
            cand -v 'Define the verbosity of the application, repeat for more verbosity'
            cand --verbose 'Define the verbosity of the application, repeat for more verbosity'
            cand -q 'Supress all logging'
            cand --quiet 'Supress all logging'
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
        &'discord-gating-bot;key'= {
            cand -h 'Print help information'
            cand --help 'Print help information'
            cand -v 'Define the verbosity of the application, repeat for more verbosity'
            cand --verbose 'Define the verbosity of the application, repeat for more verbosity'
            cand -q 'Supress all logging'
            cand --quiet 'Supress all logging'
            cand generate 'Generates a new key than can be used for encryption at rest and for the sessions tokens'
            cand help 'Print this message or the help of the given subcommand(s)'
        }
        &'discord-gating-bot;key;generate'= {
            cand -h 'Print help information'
            cand --help 'Print help information'
            cand -v 'Define the verbosity of the application, repeat for more verbosity'
            cand --verbose 'Define the verbosity of the application, repeat for more verbosity'
            cand -q 'Supress all logging'
            cand --quiet 'Supress all logging'
        }
        &'discord-gating-bot;key;help'= {
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
            cand global 'Register the global slash commands'
            cand server 'Register the slash commands for a specific guild'
            cand help 'Print this message or the help of the given subcommand(s)'
        }
        &'discord-gating-bot;discord;global'= {
            cand -h 'Print help information'
            cand --help 'Print help information'
            cand -v 'Define the verbosity of the application, repeat for more verbosity'
            cand --verbose 'Define the verbosity of the application, repeat for more verbosity'
            cand -q 'Supress all logging'
            cand --quiet 'Supress all logging'
        }
        &'discord-gating-bot;discord;server'= {
            cand -h 'Print help information'
            cand --help 'Print help information'
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
