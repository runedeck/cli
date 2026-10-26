## ADDED Requirements

### Requirement: Cursor launches the agent CLI

The launcher MUST know Cursor as a tool and MUST start Cursor's agent command line for it when the configuration names no binary, because the editor command of the same name opens the editor on a Mac. When the configuration names a binary, the launcher MUST start that one. The tool list MUST look up the same binary the launch starts.

#### Scenario: Cursor launch without a configured binary

- **WHEN** the config has no `tools.cursor.binary` and the caller runs `rune launch cursor --dry-run`
- **THEN** the printed command starts with `cursor-agent`
- **AND** `rune launch` without a tool lists `cursor` as installed when `cursor-agent` is on `PATH`

#### Scenario: Cursor launch with a configured binary

- **WHEN** `tools.cursor.binary` is `/opt/cursor/agent` and the caller runs `rune launch cursor --dry-run`
- **THEN** the printed command starts with `/opt/cursor/agent`

### Requirement: Cursor has a built-in Fable profile

The launcher MUST supply a Fable profile for Cursor without configuration, selecting a built-in route whose id is Cursor's name for Fable at high effort, because Cursor names each effort level as its own id and no tool-neutral route serves it. The launcher MUST pass a route's model to Cursor on the command line in front of the profile arguments. A configured profile or route of the same name MUST replace the built-in one, and the missing-profile error and the tool list MUST name built-in and configured profiles together.

#### Scenario: Built-in Fable profile for Cursor

- **WHEN** the config defines no `cursor` profile named `fable` and the caller runs `rune launch fable@cursor --dry-run`
- **THEN** the printed command is `cursor-agent --model claude-fable-5-1-high`
- **AND** `rune run fable@cursor` starts `cursor-agent` with `--model claude-fable-5-1-high` once

#### Scenario: Configured profile replaces the built-in one

- **WHEN** `launch.profiles.cursor.fable` sets `args: [--endpoint, https://own.example.test]` and no model
- **THEN** `rune launch fable@cursor --dry-run` prints `cursor-agent --endpoint https://own.example.test` and no `--model`

#### Scenario: Unknown profile lists the built-in one

- **WHEN** the caller runs `rune launch nope@cursor` with no cursor profiles configured
- **THEN** the error reads `no launch profile 'nope' for cursor (profiles: fable)`

### Requirement: Cursor commands run outside the harness sandbox

The launcher and the runner MUST work for Cursor when rune starts as a bare command outside the harness sandbox, because Cursor reads its login from the macOS Keychain. Neither MUST try to detect or leave the sandbox itself. The harness configuration decides where rune starts.

#### Scenario: Bare rune command from an agent session

- **WHEN** an agent session runs `rune run fable@cursor "Reply with pong"` as a bare command and its harness excludes `rune` from the sandbox
- **THEN** `cursor-agent` reads the Keychain login and the run prints the reply

### Requirement: URL credentials are refused as a WHATWG parser reads them

The launcher MUST find every URL candidate by its scheme, in any case, in each argument, profile option, and route setting a launch or a run passes, read as the tool decodes it. A WHATWG parser MUST decide what a candidate carries, without a slash rule of rune's own: a backslash, one slash, three, or none after the scheme, and a tab before the `@`, still count, as a harness reads them. A URL that carries a password MUST refuse the launch or the run before anything starts, and the refusal MUST NOT repeat the URL.

#### Scenario: Endpoint with a password is refused by name of the key variable

- **WHEN** a Cursor profile carries `-ehttps://user:pa'ss-SECRET@example.test`, or a Codex profile carries `-c model_providers.p.base_url="https://TOKEN\U00000040example.test"`, which Codex decodes to an `@`
- **THEN** the run exits 125 before the tool starts, the message names `CURSOR_API_KEY` or `OPENAI_API_KEY`, and neither the password, the token, nor the host is printed

#### Scenario: Credential forms a slash rule missed are refused

- **WHEN** a launch passes `https:///u:SECRET@h/v1`, `https:\\u:SECRET@h`, `https:/u:SECRET@h`, `https:u:SECRET@h`, or a tab before the `@` in an endpoint flag, a base URL variable, or a Codex `-c` value
- **THEN** `rune run` and `rune launch --dry-run` exit 125 with `kind: configuration_error` before the tool starts
- **AND** neither `SECRET` nor the host is printed

### Requirement: User-only URLs refuse only where the tool reads an endpoint

A URL that carries a user name and no password MUST refuse only where the tool reads an endpoint: an argument, a decoded argument value, the plan's base URL, and the tool's base URL variable. In any other route variable it MUST pass, because a name such as `postgres` is not a secret. A proxy variable, `HTTPS_PROXY`, `HTTP_PROXY`, or `ALL_PROXY` in either case, MUST pass whole with its user and password, because the child needs them. A refusal MUST name the variable or key file the tool reads its credential from.

#### Scenario: Database variable with a user name starts the tool

- **WHEN** a Cursor profile sets `DATABASE_URL: postgres://postgres@localhost/app` and `HTTPS_PROXY: http://user:pa'ss@proxy.test:3128` beside `-ehttps://api.example.test`
- **THEN** the launch resolves and the proxy URL reaches the child whole
- **AND** a Claude profile that sets `ANTHROPIC_BASE_URL: https://TOKEN@proxy.test/v1`, or any profile with a password in `DATABASE_URL`, is refused by name of the credential variable

### Requirement: Secrets are hidden as the tool decodes them

The redaction set MUST hold only the credential-named values, the URL variables the route reads, and each proxy credential, as written and percent-decoded, so a user-only URL in an unrelated variable hides no word. The dry run, the reply, the JSON result, and the error text MUST pass one redaction function. It MUST read a Codex `-c` value as TOML before it hides a URL, and MUST NOT end a URL at a quote inside it. It MUST hide a query value whose parameter name, split on `_`, `-`, and camelCase boundaries, holds `key`, `token`, `secret`, `password`, or `sig`.

#### Scenario: Escaped Codex query secret is hidden in the dry run

- **WHEN** a Codex profile carries `-c model_providers.p.base_url="https://h/v1?\u0074oken=\u0046AKE-SECRET"`, whose name and value are TOML escapes
- **THEN** `rune launch --dry-run` prints `base_url="https://h/v1?token=<redacted>"`
- **AND** a reply of `about postgres, anonymous and documentation` beside inherited `DATABASE_URL=postgres://postgres@localhost/app` and `SEARCH_URL=https://example.test/search?keyword=documentation` prints as written

#### Scenario: Proxy password reaches the child and never the plan

- **WHEN** a profile sets `HTTPS_PROXY: http://user:pa'ss-SECRET@proxy.test:3128`
- **THEN** the child environment carries the URL whole
- **AND** the dry run prints `http://user:<redacted>@proxy.test:3128` and `pa'ss-SECRET` nowhere
