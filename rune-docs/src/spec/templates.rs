//! Embedded scaffolding templates and validation schemas. A repository can
//! override any of them with a file of the same relative path; the
//! embedded copy is the fallback.

pub(super) const PROPOSAL_TEMPLATE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/templates/spec/proposal.md"
));
pub(super) const TASKS_TEMPLATE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/templates/spec/tasks.md"
));
pub(super) const DELTA_SPEC_TEMPLATE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/templates/spec/delta-spec.md"
));
pub(super) const DESIGN_TEMPLATE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/templates/spec/design.md"
));
pub(super) const SPEC_MDSCHEMA: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/schemas/spec.mdschema"
));
pub(super) const DELTA_SPEC_MDSCHEMA: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/schemas/delta-spec.mdschema"
));
