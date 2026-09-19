use super::SkillReadiness;
use crate::manifest::source_snapshot::{
    SOURCE_SNAPSHOT_PATH, SourceSnapshot, SourceSnapshotRecord,
};
use std::path::Path;

/// Bind inspected identities to independently computed current source inputs.
/// The caller validates the record's file and provider claims before this check.
pub fn verify_source_snapshot(
    report: &mut SkillReadiness,
    target: &Path,
    current: &SourceSnapshot,
    model: Option<&str>,
    record: &SourceSnapshotRecord,
) {
    match verify(report, target, current, model, record) {
        Ok(()) => report.source_verification = "verified".into(),
        Err(reason) => {
            report.source_verification = "unverified".into();
            report.finding(
                "CSI002_INCOMPLETE_IDENTITY",
                None,
                vec![target.join(SOURCE_SNAPSHOT_PATH).display().to_string()],
                reason,
            );
        }
    }
    report.refresh_identity();
}

fn verify(
    report: &SkillReadiness,
    target: &Path,
    current: &SourceSnapshot,
    model: Option<&str>,
    record: &SourceSnapshotRecord,
) -> Result<(), String> {
    record.validate()?;
    if &record.source != current || record.model_override.as_deref() != model {
        return Err("deployed source selection or build configuration is stale".into());
    }
    if record.selected_skills.is_empty() {
        return Err("source snapshot selects no skills".into());
    }
    for (relative, digest) in &record.selected_skills {
        let expected = target.join(relative).join("SKILL.md");
        let skill = report
            .skills
            .iter()
            .find(|skill| Path::new(&skill.path) == expected)
            .ok_or_else(|| format!("selected skill is missing from inventory: {relative}"))?;
        if skill.ownership != "managed"
            || skill.source.is_none()
            || skill.bundle.digest.as_ref() != Some(digest)
            || !skill.bundle.problems.is_empty()
        {
            return Err(format!(
                "selected bundle does not match source evidence: {relative}"
            ));
        }
    }
    Ok(())
}
