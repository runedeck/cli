## Edge cases

### Requirement: The Validator Reports A Program Name

#### Edge case: Flag stands alone in a code span

- **WHEN** a requirement body holds an inline code span that starts with a dash and names no command before it
- **THEN** the validator reports the same finding as for a command

#### Edge case: Path stands alone in a code span

- **WHEN** a requirement body holds an inline code span that is a file path and names no command
- **THEN** the validator reports nothing for it
- **DECISION**: narrows Requirement Bodies Name Roles, which bans paths too. The validator reports commands and flags only, and the style checker's bare-command rule reports paths, so the two checks do not overlap.

#### Edge case: Command appears in a scenario step

- **WHEN** a scenario step, not the requirement body, holds an inline code span that names a command
- **THEN** the validator reports nothing for it
- **DECISION**: narrows the validator's finding to the requirement body. The house rule still covers scenario steps: a step describes observable behavior, and the style checker's bare-command rule reports a command there.

#### Edge case: Code span names a program outside the list

- **WHEN** a requirement body holds an inline code span that starts with no dash and whose first word is not in the repository's program list
- **THEN** the validator reports nothing, and the author extends the program list in the repository configuration when a program is missing
- **DECISION**: narrows The Validator Reports A Program Name. A command is a word in the repository's program list, so an unknown program passes until the author lists it.

#### Edge case: Command sits inside a fenced block

- **WHEN** a requirement body holds a fenced code block that shows a command
- **THEN** the validator reports nothing, because fenced code is an example and not prose

#### Edge case: Repository raises the level

- **WHEN** the repository configuration sets the program-name finding to an error after its specifications migrated
- **THEN** the validator reports an error and the change fails validation
