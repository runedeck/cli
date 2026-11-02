## ADDED Requirements

### Requirement: The deployer points the machine's clients at the hub

The deployer MUST apply the hub client step from the launch profiles: the hub URL and key for Claude Code,
and a hub model provider for Codex that uses the responses wire API and websockets. It MUST back up each file
before the first change. When the owner keeps the normal install, it MUST write a second configuration
directory instead and leave the normal one unchanged.

#### Scenario: Owner keeps the normal Claude install

- **WHEN** the owner enables T3 Code with the second directory option
- **THEN** the normal Claude settings read the same as before
- **AND** the second directory holds the hub URL and key reference

### Requirement: The key enters a file only as a secret reference

The deployer MUST write the hub key into a client file as a reference that the client resolves at start,
never as a literal value. The same holds for the hub management key in a T3 usage source.

#### Scenario: Claude settings after the client step

- **WHEN** the deployer finishes the client step for Claude Code
- **THEN** the settings file contains no key value

### Requirement: The deployer sets the cache lifetime on hub-routed Claude

On a machine whose Claude Code routes through the hub, the deployer MUST set a one-hour prompt cache for the
main conversation, and for subagents when the owner asks. It MUST refuse with the required version when
Claude Code is older than the version that reads the setting.

#### Scenario: Claude Code is too old

- **WHEN** Claude Code reports a version older than the one that reads the cache setting
- **THEN** the deployer names the required version and writes no cache setting

### Requirement: The deployer leaves the entrypoint variable alone

The deployer MUST NOT write the variable that tells Claude Code which program started it. T3 Code sets it,
and a value in Claude's settings would override T3's value.

#### Scenario: T3 Code starts Claude after the client step

- **WHEN** T3 Code starts Claude Code on a machine the deployer configured
- **THEN** Claude Code receives the entrypoint value that T3 Code set

### Requirement: The deployer offers routed models in T3 Code

The deployer MUST add the model names the launch profiles route through the hub to the custom model list of
T3's Claude entry. It MUST keep every other T3 setting unchanged, write only through the settings file T3
reloads, and read the file again after the write. When the owner uses a second configuration directory, the
entry MUST name that directory as its home. The deployer MUST NOT write T3's thread store.

#### Scenario: T3 settings hold other entries

- **WHEN** the owner enables T3 Code and its settings hold favorites and other provider entries
- **THEN** the Claude entry lists the routed models
- **AND** every other setting reads the same as before

### Requirement: Multica needs no entry by default

The deployer MUST leave Multica's runtimes unchanged when they run the machine's own Claude Code and Codex. It
MUST register a launch wrapper only for a profile that needs arguments or middleware an interface cannot
express.

#### Scenario: Profile with extra arguments

- **WHEN** a profile adds arguments that no interface setting can carry
- **THEN** the deployer registers one wrapper for that profile and none for the others

### Requirement: Removal removes only what the deployer wrote

Disabling an interface MUST remove the keys, entries, and wrappers the deployer recorded as its own, and
nothing else. A key the owner changed after the deployer wrote it MUST be reported, not reverted.

#### Scenario: Owner edited the model list by hand

- **WHEN** the owner added a model to T3's list and then disables T3 Code
- **THEN** the routed models rune added are gone
- **AND** the owner's model stays

### Requirement: Secret values stay out of output

A dry run and structured output MUST show the keys the deployer would add, change, or remove, and MUST NOT
show a secret value.

#### Scenario: Dry run against files that hold keys

- **WHEN** the owner runs a dry run and T3 settings hold a usage source key
- **THEN** the key does not appear in the output

### Requirement: The validator checks the hub's model list

The validator MUST ask the hub's models endpoint with the key and MUST report each routed model the hub does
not serve.

#### Scenario: Hub does not serve a routed model

- **WHEN** a profile routes a model the hub's list does not contain
- **THEN** the validator reports that model as missing

### Requirement: The validator checks the cache lifetime

The validator MUST report a hub-routed Claude Code whose main conversation cache is not one hour, including
when a variable forces five minutes.

#### Scenario: Force variable is set

- **WHEN** the variable that forces five-minute caching is set for Claude Code
- **THEN** the validator reports the cache lifetime as five minutes

### Requirement: The validator checks session affinity

When the hub configuration is readable, the validator MUST report session affinity that is off. When it is
not readable, the validator MUST report the check as unknown, not as passed. It MUST NOT change the hub file.

#### Scenario: Hub configuration is not readable

- **WHEN** the validator cannot read the hub configuration
- **THEN** it reports session affinity as unknown

### Requirement: The validator checks Codex websockets

The validator MUST report a Codex hub provider that does not enable websockets.

#### Scenario: Provider without websockets

- **WHEN** the Codex hub provider omits websockets support
- **THEN** the validator reports it with the setting that enables it

### Requirement: The validator warns about Claude models outside Claude Code

The validator MUST warn when a profile or interface sends a Claude subscription model through the hub to a
harness other than Claude Code.

#### Scenario: Codex profile routes a Claude model

- **WHEN** a Codex profile names a Claude model and routes through the hub
- **THEN** the validator warns and names the profile

### Requirement: The validator reports stale ACP entries

The validator MUST report a T3 Code entry or a Multica runtime profile that runs rune through ACP, and MUST
name the step that removes it.

#### Scenario: Stale entry in T3 Code

- **WHEN** T3 settings hold an ACP registry entry whose command is rune
- **THEN** the validator reports it as stale with the removal step

### Requirement: The first slice reads and never writes

The deployer MUST list the known interfaces with whether each is present. It MUST print the provider instances of
an interface with the models that the launch profiles would own, and MUST print the plan of an installation as
a dry run. In this slice it MUST NOT open the settings file of an interface for writing. It MUST refuse an
installation without the dry run, and MUST say that this release writes nothing.

#### Scenario: Dry run leaves the settings file as it was

- **WHEN** the owner asks for the dry run of T3 Code
- **THEN** the output names the profiles, the provider instance of each, the keys rune would own, and each field
  that T3 Code cannot represent
- **AND** the settings file has the same bytes and the same modification time as before

#### Scenario: Installation without the dry run

- **WHEN** the owner asks to enable T3 Code without the dry run
- **THEN** the deployer refuses and says that this release writes nothing
- **AND** the settings file is as it was

### Requirement: A proxy-backed profile gets an instance beside the native one

The deployer MUST plan the model of a profile without environment values into the native provider instance of
its tool. It MUST plan a profile with environment values into a provider instance of its own beside the native
one, and MUST NOT plan a change to the environment of the native instance. Profiles of one tool with the same
environment MUST share one instance. A model that the instance already lists MUST NOT become a key that rune
owns.

#### Scenario: Profile with a base URL

- **WHEN** a profile names a base URL
- **THEN** the plan holds an instance beside the native one, with the profile's model
- **AND** the plan holds no key of the native instance

### Requirement: The planner rejects what the interface cannot carry

The deployer MUST plan from profile data alone. It MUST NOT resolve a secret and MUST NOT run middleware. When
the interface cannot represent a profile, the deployer MUST reject the profile whole and give every reason. For
an accepted profile it MUST name each field that the interface loses. A secret reference MUST stay a reference
in every output.

#### Scenario: Profile with middleware

- **WHEN** a profile names middleware
- **THEN** the plan lists the profile as rejected with the reason
- **AND** the plan holds no instance for it
