## ADDED Requirements

### Requirement: A caller asks a question set about one piece of state

A caller MUST be able to ask a judging model a question set about one piece of state and read typed answers in a stable order. The text the caller gives, as an argument, a file, or a stream, MUST become the state's text. A caller that has a state object MUST be able to give the whole object instead. A question set MUST be named by file or by a name looked up in the folders the configuration lists, in order. A name no folder holds MUST be refused with every folder searched named.

#### Scenario: Text becomes the state

- **WHEN** a pipeline step gives a text and names a question set
- **THEN** the request carries the text as the state's text and the set's questions
- **AND** the answers print one per line, sorted by question key

#### Scenario: Text from a file or a stream

- **WHEN** a step gives the text as a file, or on a noninteractive stream
- **THEN** the plan names that file, or the stream, as the state source

#### Scenario: A state object of the caller's own

- **WHEN** a caller gives a state object and no text
- **THEN** the request's state is that object, and no text is read

#### Scenario: Unknown question set

- **WHEN** no configured folder holds a set of the given name
- **THEN** the command exits 125 with a configuration error that names the name and every folder searched

### Requirement: Only the questions leave the machine

The command MUST send the question-set file's questions object alone. The file's signal block, and any other member, MUST NOT leave the machine. A file without a questions object MUST be refused as a configuration error. The plan and the check MUST send no state.

#### Scenario: Signal stays home

- **WHEN** the question-set file holds questions and a signal block
- **THEN** the request body carries the questions and no signal member at any depth

#### Scenario: Plan and check send no state

- **WHEN** the owner asks for the plan, or checks the route
- **THEN** no request carries the state, and the plan names the endpoint, the model, the question-set path, and the state source

### Requirement: The judging model is a route, not a harness

The judging model MUST be known by a built-in route with a model id, an endpoint, and a key variable, usable without configuration. The configuration MUST be able to override any field of that route, and to add any route that speaks the same protocol, each with its own id, endpoint, and key variable. The caller MUST be able to pick a route, or a model id on the default route. The launcher and the runner MUST refuse the judging model by name with one line that names the command that asks it.

#### Scenario: Built-in route without configuration

- **WHEN** the configuration names no route
- **THEN** the plan names the built-in model, the hosted endpoint, and the built-in key variable

#### Scenario: Proxy by configuration alone

- **WHEN** the configuration gives the built-in route another endpoint and another key variable
- **THEN** the plan names that endpoint and that key variable, and the built-in model id

#### Scenario: Another route

- **WHEN** the configuration adds a route with an endpoint and no key variable, and the caller picks it
- **THEN** the request goes to that endpoint without an authorization header

#### Scenario: Harness commands refuse the judging model

- **WHEN** someone names the judging model as a tool to run or to launch
- **THEN** the command fails with one line that calls it a judging model and names the command that asks it, and starts nothing

### Requirement: The key is read at call time and never printed

The command MUST read the route's key from the variable the route names, from the environment first, then the env file. It MUST NOT read a key value from the configuration file. A missing key MUST be a configuration error that names the variable, and nothing MUST be sent. The key value MUST read as redacted in every report, a refusal that echoes it included.

#### Scenario: Missing key

- **WHEN** the route's key variable is unset in the environment and absent from the env file
- **THEN** the command exits 125 with a configuration error that names the variable, and no request is sent

#### Scenario: Refusal echoes the key

- **WHEN** the endpoint answers 401 with a body that quotes the key
- **THEN** the command exits 1 with an endpoint error, status 401, and a message in which the key reads as redacted

### Requirement: The reply is the result, with the runner's exit codes

The command MUST post the model, the state, and the questions in one request and take the reply's answers object as the result. The machine-readable result MUST carry the answers, the model, the usage, and whether it succeeded, with its kind and details. A reply with a status other than 200, a transport failure, or a reply without answers MUST exit 1. A request past its deadline MUST exit 124. A configuration or usage error MUST exit 125.

#### Scenario: Answers as a machine-readable result

- **WHEN** the endpoint answers 200 with an answers object, a usage object, and a model name
- **THEN** the result is one object with ok true, kind success, the answers, the usage, the model, and exit code 0

#### Scenario: Endpoint failure

- **WHEN** the endpoint answers 503
- **THEN** the command exits 1 with kind endpoint error and status 503 in the details

#### Scenario: Deadline passed

- **WHEN** no reply is whole within the deadline
- **THEN** the command exits 124 with kind timeout and the deadline in the details
