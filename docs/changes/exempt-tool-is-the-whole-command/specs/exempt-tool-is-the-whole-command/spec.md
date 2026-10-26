## ADDED Requirements

### Requirement: A sandbox-off request for an exempt command is refused

When the model asks to turn the sandbox off for a command line that the sandbox already lets run outside it, the sandbox guard MUST refuse the call and tell the model to run the same line without the request. The guard MUST let a sandbox-off request for any other line pass, because the owner decides those.

#### Scenario: Sandbox-off request for a jj command

- **WHEN** the model runs `jj -R /x log` and asks to turn the sandbox off
- **THEN** the call is refused
- **AND** the reason says `jj` already runs outside the sandbox and to run the line without the request

#### Scenario: Sandbox-off request for a build

- **WHEN** the model runs `cargo build --release` and asks to turn the sandbox off
- **THEN** the call passes

### Requirement: An exempt tool the sandbox would keep is refused with the command to run

When a line holds an exempt tool and something on the line keeps it inside the sandbox, the sandbox guard MUST refuse the call. That is an assignment or a wrapper word before it, a shell, parentheses, or a loop around it, a redirection to a file, a substitution, or another command that is not exempt. The reason MUST name what keeps it inside and give the command to run instead. A directory change before the tool MUST become the tool's own directory option where the tool has one. The guard MUST read a `$'…'` word as the shell decodes it.

#### Scenario: Tool name written in hex escapes

- **WHEN** the model runs `$'\x6a\x6a' log > out`
- **THEN** the guard reads the tool as `jj`, and the call is refused for the redirection

#### Scenario: Directory change before jj

- **WHEN** the model runs `cd /x && jj log`
- **THEN** the call is refused
- **AND** the reason gives `jj -R /x log`

#### Scenario: Pipe into head after gh

- **WHEN** the model runs `gh pr list | head -5`
- **THEN** the call is refused
- **AND** the reason names `head`, gives `gh pr list`, and points to the tool's own output filters

#### Scenario: Shell around rune run

- **WHEN** the model runs `bash -c 'rune run sol@claude hi'`
- **THEN** the call is refused
- **AND** the reason names the shell and gives `rune run sol@claude hi`

### Requirement: A substitution is refused with a way to pass the text

A substitution of any kind keeps an exempt tool inside the sandbox, even when the substitution itself runs an exempt tool. The sandbox guard MUST refuse such a line. The command it gives MUST mark the word that held the substitution, and the reason MUST say to write the text inline or to pass a file through the tool's own file option. A substitution in the operand of a `${…}` expansion MUST count, whatever quotes or `#` stand in the operand.

#### Scenario: Prompt read through a substitution

- **WHEN** the model runs `rune run sol@claude "$(cat brief.md)"`
- **THEN** the call is refused
- **AND** the reason names the substitution, gives `rune run sol@claude TEXT`, and points to `rune run --prompt-file`

#### Scenario: Substitution behind quotes in a default

- **WHEN** the model runs `jj log "${X:-'$(printf probe)'}"`
- **THEN** the call is refused, because inside double quotes the single quotes are plain characters and the substitution runs

### Requirement: Other tools are never refused and a chain gets advice

The sandbox guard MUST NOT refuse a line that holds no exempt tool. When a line joins commands with `&&`, `||`, `;`, or `&`, the guard MUST let it run and advise one command per call. A check before one action, a directory creation before entering it, a trailing ignored failure, a pipe, and `cd DIR && cargo …` get no advice: cargo reads its configuration from the current folder. When a cargo command names a manifest under a checkout whose configuration sets a target directory, the guard MUST advise running cargo from that checkout without the manifest option.

#### Scenario: Build after a directory change

- **WHEN** the model runs `cd /x && cargo build`
- **THEN** the call passes with no advice

#### Scenario: Two builds after a directory change

- **WHEN** the model runs `cd /x && cargo build && cargo test`
- **THEN** the call passes
- **AND** the model receives advice to split the line

#### Scenario: Make after a directory change

- **WHEN** the model runs `cd /x && make`
- **THEN** the call passes
- **AND** the model receives advice to split the line, which names cargo as the exception

#### Scenario: Manifest under a configured checkout

- **WHEN** the model runs `cargo build --manifest-path /c/ws/Cargo.toml` from `/x`, and `/c/.cargo/config.toml` sets `build.target-dir`
- **THEN** the call passes
- **AND** the advice names `/c/.cargo/config.toml` and says to run `cd /c/ws && cargo build` without `--manifest-path`

#### Scenario: Manifest with no configured target directory

- **WHEN** the model runs `cargo build --manifest-path /c/ws/Cargo.toml` and no `.cargo/config.toml` above the manifest sets `build.target-dir`
- **THEN** the call passes with no advice

#### Scenario: Test output through a pipe

- **WHEN** the model runs `cargo test | tail -40`
- **THEN** the call passes with no advice

### Requirement: The guard judges the line the harness runs

The sandbox guard MUST judge the tool input the harness will execute, after every rewriter that ran before it. When a rewriter changed the line and the guard refuses it, the reason MUST quote the line the model wrote beside the rewritten one.

#### Scenario: Rewritten line outside the exempt list

- **WHEN** a rewriter turns `jj log` into `rtk jj log` and only `jj *` is exempt
- **THEN** the call is refused
- **AND** the reason quotes `jj log` and `rtk jj log`

#### Scenario: Rewritten line inside the exempt list

- **WHEN** a rewriter turns `jj log` into `rtk jj log` and `rtk jj *` is exempt
- **THEN** the call passes

### Requirement: The exempt list is the harness's own

The sandbox guard MUST take the exempt patterns from its arguments when it has any. Without arguments, it MUST read them from the sandbox settings that the session's harness merges, and only while that sandbox is on. With no exempt pattern, the guard MUST refuse nothing.

#### Scenario: Patterns from the settings

- **WHEN** the harness settings turn the sandbox on and exempt `gh *`, and the model runs `cd /x && gh pr list`
- **THEN** the call is refused

#### Scenario: Sandbox turned off

- **WHEN** the harness settings turn the sandbox off, and the model runs `cd /x && gh pr list`
- **THEN** the call passes
