
using namespace System.Management.Automation
using namespace System.Management.Automation.Language

Register-ArgumentCompleter -Native -CommandName 'discord-gating-bot' -ScriptBlock {
    param($wordToComplete, $commandAst, $cursorPosition)

    $commandElements = $commandAst.CommandElements
    $command = @(
        'discord-gating-bot'
        for ($i = 1; $i -lt $commandElements.Count; $i++) {
            $element = $commandElements[$i]
            if ($element -isnot [StringConstantExpressionAst] -or
                $element.StringConstantType -ne [StringConstantType]::BareWord -or
                $element.Value.StartsWith('-') -or
                $element.Value -eq $wordToComplete) {
                break
        }
        $element.Value
    }) -join ';'

    $completions = @(switch ($command) {
        'discord-gating-bot' {
            [CompletionResult]::new('-c', 'c', [CompletionResultType]::ParameterName, 'Sets a custom config file')
            [CompletionResult]::new('--config-file', 'config-file', [CompletionResultType]::ParameterName, 'Sets a custom config file')
            [CompletionResult]::new('-t', 't', [CompletionResultType]::ParameterName, 'The discord bot token')
            [CompletionResult]::new('--token', 'token', [CompletionResultType]::ParameterName, 'The discord bot token')
            [CompletionResult]::new('-h', 'h', [CompletionResultType]::ParameterName, 'The address to listen on')
            [CompletionResult]::new('--host', 'host', [CompletionResultType]::ParameterName, 'The address to listen on')
            [CompletionResult]::new('-u', 'u', [CompletionResultType]::ParameterName, 'The base url under which the server is reachable')
            [CompletionResult]::new('--url', 'url', [CompletionResultType]::ParameterName, 'The base url under which the server is reachable')
            [CompletionResult]::new('-p', 'p', [CompletionResultType]::ParameterName, 'The port to listen on')
            [CompletionResult]::new('--port', 'port', [CompletionResultType]::ParameterName, 'The port to listen on')
            [CompletionResult]::new('-d', 'd', [CompletionResultType]::ParameterName, 'The path where the persistent data is stored')
            [CompletionResult]::new('--directory', 'directory', [CompletionResultType]::ParameterName, 'The path where the persistent data is stored')
            [CompletionResult]::new('-s', 's', [CompletionResultType]::ParameterName, 'How to store data, on disk or in memory')
            [CompletionResult]::new('--storage-type', 'storage-type', [CompletionResultType]::ParameterName, 'How to store data, on disk or in memory')
            [CompletionResult]::new('-k', 'k', [CompletionResultType]::ParameterName, 'The encryption_key used to encrypt the stored data')
            [CompletionResult]::new('--key', 'key', [CompletionResultType]::ParameterName, 'The encryption_key used to encrypt the stored data')
            [CompletionResult]::new('--help', 'help', [CompletionResultType]::ParameterName, 'Print help information')
            [CompletionResult]::new('-V', 'V', [CompletionResultType]::ParameterName, 'Print version information')
            [CompletionResult]::new('--version', 'version', [CompletionResultType]::ParameterName, 'Print version information')
            [CompletionResult]::new('-v', 'v', [CompletionResultType]::ParameterName, 'Define the verbosity of the application, repeat for more verbosity')
            [CompletionResult]::new('--verbose', 'verbose', [CompletionResultType]::ParameterName, 'Define the verbosity of the application, repeat for more verbosity')
            [CompletionResult]::new('-q', 'q', [CompletionResultType]::ParameterName, 'Supress all logging')
            [CompletionResult]::new('--quiet', 'quiet', [CompletionResultType]::ParameterName, 'Supress all logging')
            [CompletionResult]::new('config', 'config', [CompletionResultType]::ParameterValue, 'Print the configuration and get a template file')
            [CompletionResult]::new('storage', 'storage', [CompletionResultType]::ParameterValue, 'Interact with the presistent storage and encryption')
            [CompletionResult]::new('discord', 'discord', [CompletionResultType]::ParameterValue, 'Interact with discord directly, e.g. register slash commands')
            [CompletionResult]::new('help', 'help', [CompletionResultType]::ParameterValue, 'Print this message or the help of the given subcommand(s)')
            break
        }
        'discord-gating-bot;config' {
            [CompletionResult]::new('-h', 'h', [CompletionResultType]::ParameterName, 'Print help information')
            [CompletionResult]::new('--help', 'help', [CompletionResultType]::ParameterName, 'Print help information')
            [CompletionResult]::new('-v', 'v', [CompletionResultType]::ParameterName, 'Define the verbosity of the application, repeat for more verbosity')
            [CompletionResult]::new('--verbose', 'verbose', [CompletionResultType]::ParameterName, 'Define the verbosity of the application, repeat for more verbosity')
            [CompletionResult]::new('-q', 'q', [CompletionResultType]::ParameterName, 'Supress all logging')
            [CompletionResult]::new('--quiet', 'quiet', [CompletionResultType]::ParameterName, 'Supress all logging')
            [CompletionResult]::new('show', 'show', [CompletionResultType]::ParameterValue, 'Print the configuration sources and merged config')
            [CompletionResult]::new('template', 'template', [CompletionResultType]::ParameterValue, 'Prints an example configuration template')
            [CompletionResult]::new('help', 'help', [CompletionResultType]::ParameterValue, 'Print this message or the help of the given subcommand(s)')
            break
        }
        'discord-gating-bot;config;show' {
            [CompletionResult]::new('-h', 'h', [CompletionResultType]::ParameterName, 'Print help information')
            [CompletionResult]::new('--help', 'help', [CompletionResultType]::ParameterName, 'Print help information')
            [CompletionResult]::new('-v', 'v', [CompletionResultType]::ParameterName, 'Define the verbosity of the application, repeat for more verbosity')
            [CompletionResult]::new('--verbose', 'verbose', [CompletionResultType]::ParameterName, 'Define the verbosity of the application, repeat for more verbosity')
            [CompletionResult]::new('-q', 'q', [CompletionResultType]::ParameterName, 'Supress all logging')
            [CompletionResult]::new('--quiet', 'quiet', [CompletionResultType]::ParameterName, 'Supress all logging')
            break
        }
        'discord-gating-bot;config;template' {
            [CompletionResult]::new('-h', 'h', [CompletionResultType]::ParameterName, 'Print help information')
            [CompletionResult]::new('--help', 'help', [CompletionResultType]::ParameterName, 'Print help information')
            [CompletionResult]::new('-v', 'v', [CompletionResultType]::ParameterName, 'Define the verbosity of the application, repeat for more verbosity')
            [CompletionResult]::new('--verbose', 'verbose', [CompletionResultType]::ParameterName, 'Define the verbosity of the application, repeat for more verbosity')
            [CompletionResult]::new('-q', 'q', [CompletionResultType]::ParameterName, 'Supress all logging')
            [CompletionResult]::new('--quiet', 'quiet', [CompletionResultType]::ParameterName, 'Supress all logging')
            break
        }
        'discord-gating-bot;config;help' {
            [CompletionResult]::new('-v', 'v', [CompletionResultType]::ParameterName, 'Define the verbosity of the application, repeat for more verbosity')
            [CompletionResult]::new('--verbose', 'verbose', [CompletionResultType]::ParameterName, 'Define the verbosity of the application, repeat for more verbosity')
            [CompletionResult]::new('-q', 'q', [CompletionResultType]::ParameterName, 'Supress all logging')
            [CompletionResult]::new('--quiet', 'quiet', [CompletionResultType]::ParameterName, 'Supress all logging')
            break
        }
        'discord-gating-bot;storage' {
            [CompletionResult]::new('-h', 'h', [CompletionResultType]::ParameterName, 'Print help information')
            [CompletionResult]::new('--help', 'help', [CompletionResultType]::ParameterName, 'Print help information')
            [CompletionResult]::new('-v', 'v', [CompletionResultType]::ParameterName, 'Define the verbosity of the application, repeat for more verbosity')
            [CompletionResult]::new('--verbose', 'verbose', [CompletionResultType]::ParameterName, 'Define the verbosity of the application, repeat for more verbosity')
            [CompletionResult]::new('-q', 'q', [CompletionResultType]::ParameterName, 'Supress all logging')
            [CompletionResult]::new('--quiet', 'quiet', [CompletionResultType]::ParameterName, 'Supress all logging')
            [CompletionResult]::new('generate', 'generate', [CompletionResultType]::ParameterValue, 'Generates a new key than can be used for encryption at rest')
            [CompletionResult]::new('help', 'help', [CompletionResultType]::ParameterValue, 'Print this message or the help of the given subcommand(s)')
            break
        }
        'discord-gating-bot;storage;generate' {
            [CompletionResult]::new('-h', 'h', [CompletionResultType]::ParameterName, 'Print help information')
            [CompletionResult]::new('--help', 'help', [CompletionResultType]::ParameterName, 'Print help information')
            [CompletionResult]::new('-v', 'v', [CompletionResultType]::ParameterName, 'Define the verbosity of the application, repeat for more verbosity')
            [CompletionResult]::new('--verbose', 'verbose', [CompletionResultType]::ParameterName, 'Define the verbosity of the application, repeat for more verbosity')
            [CompletionResult]::new('-q', 'q', [CompletionResultType]::ParameterName, 'Supress all logging')
            [CompletionResult]::new('--quiet', 'quiet', [CompletionResultType]::ParameterName, 'Supress all logging')
            break
        }
        'discord-gating-bot;storage;help' {
            [CompletionResult]::new('-v', 'v', [CompletionResultType]::ParameterName, 'Define the verbosity of the application, repeat for more verbosity')
            [CompletionResult]::new('--verbose', 'verbose', [CompletionResultType]::ParameterName, 'Define the verbosity of the application, repeat for more verbosity')
            [CompletionResult]::new('-q', 'q', [CompletionResultType]::ParameterName, 'Supress all logging')
            [CompletionResult]::new('--quiet', 'quiet', [CompletionResultType]::ParameterName, 'Supress all logging')
            break
        }
        'discord-gating-bot;discord' {
            [CompletionResult]::new('-h', 'h', [CompletionResultType]::ParameterName, 'Print help information')
            [CompletionResult]::new('--help', 'help', [CompletionResultType]::ParameterName, 'Print help information')
            [CompletionResult]::new('-v', 'v', [CompletionResultType]::ParameterName, 'Define the verbosity of the application, repeat for more verbosity')
            [CompletionResult]::new('--verbose', 'verbose', [CompletionResultType]::ParameterName, 'Define the verbosity of the application, repeat for more verbosity')
            [CompletionResult]::new('-q', 'q', [CompletionResultType]::ParameterName, 'Supress all logging')
            [CompletionResult]::new('--quiet', 'quiet', [CompletionResultType]::ParameterName, 'Supress all logging')
            [CompletionResult]::new('global', 'global', [CompletionResultType]::ParameterValue, 'Register the global slash commands')
            [CompletionResult]::new('server', 'server', [CompletionResultType]::ParameterValue, 'Register the slash commands for a specific guild')
            [CompletionResult]::new('help', 'help', [CompletionResultType]::ParameterValue, 'Print this message or the help of the given subcommand(s)')
            break
        }
        'discord-gating-bot;discord;global' {
            [CompletionResult]::new('-h', 'h', [CompletionResultType]::ParameterName, 'Print help information')
            [CompletionResult]::new('--help', 'help', [CompletionResultType]::ParameterName, 'Print help information')
            [CompletionResult]::new('-v', 'v', [CompletionResultType]::ParameterName, 'Define the verbosity of the application, repeat for more verbosity')
            [CompletionResult]::new('--verbose', 'verbose', [CompletionResultType]::ParameterName, 'Define the verbosity of the application, repeat for more verbosity')
            [CompletionResult]::new('-q', 'q', [CompletionResultType]::ParameterName, 'Supress all logging')
            [CompletionResult]::new('--quiet', 'quiet', [CompletionResultType]::ParameterName, 'Supress all logging')
            break
        }
        'discord-gating-bot;discord;server' {
            [CompletionResult]::new('-h', 'h', [CompletionResultType]::ParameterName, 'Print help information')
            [CompletionResult]::new('--help', 'help', [CompletionResultType]::ParameterName, 'Print help information')
            [CompletionResult]::new('-v', 'v', [CompletionResultType]::ParameterName, 'Define the verbosity of the application, repeat for more verbosity')
            [CompletionResult]::new('--verbose', 'verbose', [CompletionResultType]::ParameterName, 'Define the verbosity of the application, repeat for more verbosity')
            [CompletionResult]::new('-q', 'q', [CompletionResultType]::ParameterName, 'Supress all logging')
            [CompletionResult]::new('--quiet', 'quiet', [CompletionResultType]::ParameterName, 'Supress all logging')
            break
        }
        'discord-gating-bot;discord;help' {
            [CompletionResult]::new('-v', 'v', [CompletionResultType]::ParameterName, 'Define the verbosity of the application, repeat for more verbosity')
            [CompletionResult]::new('--verbose', 'verbose', [CompletionResultType]::ParameterName, 'Define the verbosity of the application, repeat for more verbosity')
            [CompletionResult]::new('-q', 'q', [CompletionResultType]::ParameterName, 'Supress all logging')
            [CompletionResult]::new('--quiet', 'quiet', [CompletionResultType]::ParameterName, 'Supress all logging')
            break
        }
        'discord-gating-bot;help' {
            [CompletionResult]::new('-v', 'v', [CompletionResultType]::ParameterName, 'Define the verbosity of the application, repeat for more verbosity')
            [CompletionResult]::new('--verbose', 'verbose', [CompletionResultType]::ParameterName, 'Define the verbosity of the application, repeat for more verbosity')
            [CompletionResult]::new('-q', 'q', [CompletionResultType]::ParameterName, 'Supress all logging')
            [CompletionResult]::new('--quiet', 'quiet', [CompletionResultType]::ParameterName, 'Supress all logging')
            break
        }
    })

    $completions.Where{ $_.CompletionText -like "$wordToComplete*" } |
        Sort-Object -Property ListItemText
}
