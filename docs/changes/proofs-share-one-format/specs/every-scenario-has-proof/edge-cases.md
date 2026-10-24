## Edge cases

### Requirement: Scenes Run In One Chain

#### Edge case: Block fails in the middle of the chain

- **WHEN** a block of the second of four scenes fails
- **THEN** the second scene is failed, the third and fourth are reported as not reached, and Teardown runs

#### Edge case: Author records one scene

- **WHEN** an author asks the proof runner to record only the third scene
- **THEN** it replays Setup and the first two scenes without writing their evidence, then records the third scene

#### Edge case: Instruction scene in the chain

- **WHEN** a proof holds an instruction scene between two check scenes
- **THEN** the instruction scene starts from Setup alone, and the check scene after it sees the state the first check scene left
- **DECISION**: narrows Scenes Run In One Chain. A model prompt is independent of the story, and replaying it would cost a model call for nothing.

### Requirement: Scenes Declare What They Need

#### Edge case: Later scene depends on a skipped scene

- **WHEN** a skipped scene would have written a file that the next scene reads
- **THEN** the next scene fails on the missing file, and the author moves the scene with a need to the end of the chain
- **DECISION**: narrows Scenes Declare What They Need. The proof runner does not track which scene depends on which, so the order is the author's job.

### Requirement: Sections Match The Scenarios

#### Edge case: Two scenarios share a slug

- **WHEN** two requirements of one change each hold a scenario with the same slug
- **THEN** both scene folders take the requirement slug as a prefix, and neither overwrites the other

#### Edge case: Live scene has no screen steps

- **WHEN** a scene of kind live holds no subheading with screen steps
- **THEN** the validator refuses the proof and names the scene

### Requirement: A Run Uses A Checkout Of Head

#### Edge case: Repository has no jj

- **WHEN** the repository is a plain git repository
- **THEN** the proof runner makes the checkout as a git worktree of head and records no change id
