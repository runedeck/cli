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

When a line holds an exempt tool and something on the line keeps it inside the sandbox, the sandbox guard MUST refuse the call. That is an assignment or a wrapper word before it, a shell that runs it, parentheses or a loop around it, a redirection to a file, a substitution, or another command that is not exempt. The reason MUST name what keeps it inside and give the command to run instead. A directory change before the tool MUST become the tool's own directory option where the tool has one.

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

A substitution of any kind keeps an exempt tool inside the sandbox, even when the substitution itself runs an exempt tool. The sandbox guard MUST refuse such a line. The command it gives MUST mark the word that held the substitution, and the reason MUST say to write the text inline or to pass a file through the tool's own file option.

#### Scenario: Prompt read through a substitution

- **WHEN** the model runs `rune run sol@claude "$(cat brief.md)"`
- **THEN** the call is refused
- **AND** the reason names the substitution, gives `rune run sol@claude TEXT`, and points to `rune run --prompt-file`

### Requirement: Other tools are never refused and a chain gets advice

The sandbox guard MUST NOT refuse a line that holds no exempt tool. When a line joins separate commands with `&&`, `||`, `;`, or `&`, the guard MUST let it run and add advice to split it into one command per call. A check before one action, a directory creation before entering it, and an ignored failure at the end get no advice. A pipe gets no advice.

#### Scenario: Build after a directory change

- **WHEN** the model runs `cd /x && cargo build`
- **THEN** the call passes
- **AND** the model receives advice to split the line

#### Scenario: Test output through a pipe

- **WHEN** the model runs `cargo test | tail -40`
- **THEN** the call passes with no advice

### Requirement: The exempt list is the harness's own

The sandbox guard MUST take the exempt patterns from its arguments when it has any. Without arguments, it MUST read them from the sandbox settings that the session's harness merges, and only while that sandbox is on. With no exempt pattern, the guard MUST refuse nothing.

#### Scenario: Patterns from the settings

- **WHEN** the harness settings turn the sandbox on and exempt `gh *`, and the model runs `cd /x && gh pr list`
- **THEN** the call is refused

#### Scenario: Sandbox turned off

- **WHEN** the harness settings turn the sandbox off, and the model runs `cd /x && gh pr list`
- **THEN** the call passes
