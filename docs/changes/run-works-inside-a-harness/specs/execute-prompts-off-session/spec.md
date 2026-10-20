## ADDED Requirements

### Requirement: The child harness starts free of the parent harness's session

The runner and the launcher MUST remove the parent harness's session variables from the child's environment before they
apply the route's environment. The route's own values MUST win over anything inherited. When a route sets an Anthropic
or OpenAI endpoint and sets no credential for it, the runner MUST NOT pass an inherited credential of that family to
the endpoint.

#### Scenario: Desktop session starts a proxied claude route

- **WHEN** the runner starts `sol@claude` from a process that carries `CLAUDECODE`, `CLAUDE_CODE_ENTRYPOINT`, and
  `CLAUDE_CODE_SUBPROCESS_ENV_SCRUB`
- **THEN** the child claude sees none of those variables and sees the route's endpoint, model, and credential

#### Scenario: Codex session starts another harness

- **WHEN** the runner starts a harness from a process that carries codex's sandbox variables
- **THEN** the child sees none of them

#### Scenario: Route endpoint without a credential

- **WHEN** a route sets an Anthropic endpoint and no Anthropic credential, and the parent carries one
- **THEN** the child receives no Anthropic credential and the dry run says that the inherited one was withheld

#### Scenario: Native claude without a route endpoint

- **WHEN** the runner starts plain `claude` and the parent carries an Anthropic endpoint and credential
- **THEN** the child keeps that endpoint and credential as one pair

### Requirement: A failed run shows what the harness said

A run that exits nonzero, times out, or returns no answer MUST report the last lines of both the harness's stdout and
its stderr. The text error MUST show them, and the JSON result MUST carry them as separate fields.

#### Scenario: Claude prints its error on stdout

- **WHEN** a claude route exits 1 with `API Error: 400` on stdout and a warning on stderr
- **THEN** the error names the exit code and shows both the API error and the warning

#### Scenario: Run times out

- **WHEN** a run reaches its deadline after the harness wrote partial output
- **THEN** the timeout result carries the tail of the output the harness wrote before the deadline

### Requirement: JSON output is one object for every outcome

With `--json`, the runner MUST print exactly one JSON object on stdout for every outcome: success, failure, timeout,
configuration error, dry run, and check.

#### Scenario: Dry run as JSON

- **WHEN** the user asks for a dry run with `--json`
- **THEN** stdout is one JSON object that holds the plan

#### Scenario: Unknown profile as JSON

- **WHEN** the route names a profile that does not exist and `--json` is set
- **THEN** stdout is one JSON object with `ok: false` and the list of known profiles

### Requirement: The dry run shows what would start

The dry run MUST print the full argument list the harness would receive, with the prompt replaced by a placeholder, the
model the run sends, and the endpoint the child would reach, whether a profile key or the environment sets it. It MUST
list the environment keys the run sets or removes and MUST hide every credential value.

#### Scenario: Endpoint set through the environment

- **WHEN** a profile sets the endpoint only through its environment
- **THEN** the dry run names that endpoint instead of reporting none

#### Scenario: Credential in the plan

- **WHEN** the plan carries a credential variable
- **THEN** the dry run prints the variable's name and a placeholder, never its value

### Requirement: Clean state bridges only what exists

A clean run MUST start when the harness's authentication file is absent and the route authenticates another way. It MUST
copy the file when the file exists.

#### Scenario: Grok route authenticates through the environment

- **WHEN** a clean grok run starts on a machine with no Grok authentication file
- **THEN** the run starts and the harness authenticates through its environment

### Requirement: Claude workspace-write runs shell commands in a sandbox

In workspace-write mode the runner MUST let claude edit files and run shell commands, with the shell confined by Claude
Code's own sandbox. Every claude run MUST name its allowed tools explicitly, so a hardening setting that resets the
permission mode cannot remove them.

#### Scenario: Implementation run builds its work

- **WHEN** a claude route runs in workspace-write mode and the prompt asks for a shell command in the repository
- **THEN** the command runs inside the sandbox without a permission prompt

#### Scenario: User settings set the subprocess scrub

- **WHEN** the user's Claude settings set the subprocess scrub and the run is not clean
- **THEN** the run still names its allowed tools and the harness keeps them

### Requirement: A failure caused by a sandbox says so

When a failed run's output carries a known sandbox denial, such as `Operation not permitted` or the certificate error
`-26276`, the runner MUST add a hint that an outer sandbox blocked the harness.

#### Scenario: Proxy certificate fails inside a sandbox

- **WHEN** a run fails and its output carries `-26276`
- **THEN** the error carries a hint that an outer sandbox blocked the certificate check

### Requirement: A profile model the run passes is not reported as dropped

When the runner drops a profile's model key because it passes the model itself, and the two values match, the runner MUST
NOT warn about the drop. It MUST still warn when the values differ.

#### Scenario: Codex profile names the routed model

- **WHEN** `sol@codex` runs and its profile sets `model="gpt-5.6-sol"` while the run passes `gpt-5.6-sol`
- **THEN** the run prints no drop warning
