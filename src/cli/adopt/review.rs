//! The adoption review state machine.
//!
//! `start` imports an artifact and opens a session: every file is segmented
//! into review blocks (markdown finely, anything else as one unit) and each
//! block waits for a maintainer verdict. `verdict` records one decision at a
//! time. `finalize` refuses while anything is pending, enforces the verdicts
//! against the edited files, validates structure with mdschema, re-syncs the
//! adopt sidecars, and seals the record. `abandon` retires a session into the
//! tree-local trash.
//!
//! The review record is an in-toto Statement v1 with the runedeck
//! adoption-review predicate. While the session is open it carries each
//! block's content so `next` and the finalize consistency checks work from
//! the imported state even after the files are edited; sealing strips the
//! content and keeps the digests. The record is sealed by the signed commit
//! that lands it — rune does not sign files itself.

use super::segment::{self, Block, BlockKind};
use regex::Regex;
use rune::manifest;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::LazyLock;

static OVERRIDE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)(ignore|disregard|forget)\s+(all\s+|any\s+)?(previous|prior|above|earlier)\s+(instructions|context|rules|constraints)")
        .expect("override lint regex compiles")
});
static HIJACK_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)(you are now|new instructions:|mark (every|all) blocks? .{0,20}keep|skip the review)",
    )
    .expect("hijack lint regex compiles")
});
static TOOL_CALL_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"<function_calls>|<invoke\s|!`[^`]+`").expect("tool-call lint regex compiles")
});
static BASE64_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"[A-Za-z0-9+/]{80,}={0,2}").expect("base64 lint regex compiles"));
static URL_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"https?://([A-Za-z0-9.-]+)").expect("url lint regex compiles"));

/// Advisory suspect-content flags. They mark blocks for hostile scrutiny in
/// the record and the review questions; they never block a verdict, but a
/// `keep` on a flagged block demands a rationale note.
fn lint_flags(kind: BlockKind, content: &str, upstream_host: Option<&str>) -> Vec<String> {
    let mut flags = Vec::new();
    if OVERRIDE_RE.is_match(content) || HIJACK_RE.is_match(content) {
        flags.push("instruction-override".to_string());
    }
    if TOOL_CALL_RE.is_match(content) {
        flags.push("tool-invocation".to_string());
    }
    if BASE64_RE.is_match(content) {
        flags.push("high-entropy".to_string());
    }
    if matches!(kind, BlockKind::Code | BlockKind::File) {
        for capture in URL_RE.captures_iter(content) {
            let host = capture.get(1).map_or("", |matched| matched.as_str());
            if upstream_host.is_some_and(|upstream| !host.eq_ignore_ascii_case(upstream)) {
                flags.push("external-url".to_string());
                break;
            }
        }
    }
    if content.chars().any(|character| {
        matches!(character,
            '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}' | '\u{200B}'..='\u{200F}' | '\u{FEFF}')
    }) {
        flags.push("unicode-hidden".to_string());
    }
    flags
}

fn host_of(uri: &str) -> Option<&str> {
    let rest = uri
        .strip_prefix("https://")
        .or_else(|| uri.strip_prefix("http://"))?;
    rest.split('/').next()
}

pub const REVIEW_PREDICATE_TYPE: &str = "https://runedeck.github.io/attestation/adoption-review/v1";
const STATEMENT_TYPE: &str = "https://in-toto.io/Statement/v1";
const REVIEW_FILE: &str = "review.yaml";
const SKIP_WALK: &[&str] = &[".git", ".jj", ".trash", manifest::PROVENANCE_DIRECTORY];

#[derive(Debug, Deserialize, Serialize)]
pub struct ReviewRecord {
    pub review: ReviewStatement,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ReviewStatement {
    #[serde(rename = "_type")]
    pub statement_type: String,
    #[serde(rename = "predicateType")]
    pub predicate_type: String,
    pub subject: Vec<manifest::provenance::Subject>,
    pub predicate: ReviewPredicate,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewPredicate {
    pub status: String,
    pub upstream: UpstreamPin,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub reviewer: String,
    pub segmenter: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub lint: String,
    pub rune: String,
    pub started_on: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub completed_on: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub schema: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub schema_digest: String,
    pub blocks: Vec<BlockEntry>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct UpstreamPin {
    pub uri: String,
    pub digest: manifest::provenance::DigestMap,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BlockEntry {
    pub id: String,
    pub kind: BlockKind,
    pub digest: manifest::provenance::DigestMap,
    pub line: usize,
    pub verdict: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub note: String,
    /// Suspect-content flags attached at segmentation; advisory, never blocking.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub flags: Vec<String>,
    /// When the verdict was recorded (UTC RFC 3339). Telemetry for pacing
    /// forensics, not an integrity claim — wall clocks are spoofable.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub decided_on: String,
    /// Transport the entry arrived through: `verdict-cli` proves only that
    /// the subcommand ran, `finalize` marks generated `added` entries,
    /// `review-tty` (future) proves a controlling TTY. Transport, not
    /// authorship — none of these identify who supplied the decision.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub transport: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
}

pub struct Session {
    pub record_path: PathBuf,
    pub artifact_root: PathBuf,
    pub record: ReviewRecord,
}

impl Session {
    fn pending(&self) -> Vec<&BlockEntry> {
        self.record
            .review
            .predicate
            .blocks
            .iter()
            .filter(|block| block.verdict == "pending")
            .collect()
    }
}

/// Open a review session over a freshly imported artifact rooted at
/// `artifact_root` (a directory for skills, a single file for agents/rules).
pub fn open_session(
    artifact_root: &Path,
    upstream_uri: &str,
    upstream_digest: &str,
) -> Result<PathBuf, String> {
    let record_path = record_path_for(artifact_root)?;
    if record_path.is_file() {
        let existing = read_record(&record_path)?;
        if existing.review.predicate.status == "pending" {
            return Err(format!(
                "a review session is already in flight: {}",
                record_path.display()
            ));
        }
    }

    let files = artifact_files(artifact_root)?;
    let mut subjects = Vec::new();
    let mut blocks = Vec::new();
    for file in &files {
        let relative = relative_id(artifact_root, file);
        let bytes =
            fs::read(file).map_err(|error| format!("cannot read {}: {error}", file.display()))?;
        subjects.push(manifest::provenance::Subject {
            name: relative.clone(),
            digest: manifest::provenance::DigestMap {
                sha256: manifest::content_sha256_bytes(&bytes),
            },
        });
        for block in segment_bytes(file, &bytes) {
            blocks.push(BlockEntry {
                id: format!("{relative}:{}", block.ordinal),
                kind: block.kind,
                digest: manifest::provenance::DigestMap {
                    sha256: manifest::content_sha256(&block.content),
                },
                line: block.start_line,
                verdict: "pending".to_string(),
                note: String::new(),
                flags: lint_flags(block.kind, &block.content, host_of(upstream_uri)),
                decided_on: String::new(),
                transport: String::new(),
                content: Some(block.content),
            });
        }
    }
    if blocks.is_empty() {
        return Err(format!(
            "nothing to review under {}",
            artifact_root.display()
        ));
    }

    let record = ReviewRecord {
        review: ReviewStatement {
            statement_type: STATEMENT_TYPE.to_string(),
            predicate_type: REVIEW_PREDICATE_TYPE.to_string(),
            subject: subjects,
            predicate: ReviewPredicate {
                status: "pending".to_string(),
                upstream: UpstreamPin {
                    uri: upstream_uri.to_string(),
                    digest: manifest::provenance::DigestMap {
                        sha256: upstream_digest.to_string(),
                    },
                },
                reviewer: String::new(),
                segmenter: segment::SEGMENTER_VERSION.to_string(),
                lint: "lint/v1".to_string(),
                rune: env!("CARGO_PKG_VERSION").to_string(),
                started_on: chrono::Utc::now().to_rfc3339(),
                completed_on: String::new(),
                schema: String::new(),
                schema_digest: String::new(),
                blocks,
            },
        },
    };
    write_record(&record_path, &record)?;
    Ok(record_path)
}

pub fn status(root: &Path, json: bool) -> Result<i32, String> {
    let sessions = find_sessions(root)?;
    if json {
        let items: Vec<serde_json::Value> = sessions
            .iter()
            .map(|session| {
                let total = session.record.review.predicate.blocks.len();
                let pending = session.pending();
                serde_json::json!({
                    "artifact": display_relative(root, &session.artifact_root),
                    "record": display_relative(root, &session.record_path),
                    "status": session.record.review.predicate.status,
                    "blocks": total,
                    "decided": total - pending.len(),
                    "next": pending.first().map(|block| block.id.clone()),
                })
            })
            .collect();
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({ "sessions": items }))
                .map_err(|error| format!("cannot serialize status: {error}"))?
        );
        return Ok(0);
    }
    if sessions.is_empty() {
        println!("no review sessions");
        return Ok(0);
    }
    for session in &sessions {
        let total = session.record.review.predicate.blocks.len();
        let pending = session.pending().len();
        println!(
            "{}  {}  {}/{total} decided",
            display_relative(root, &session.artifact_root),
            session.record.review.predicate.status,
            total - pending,
        );
    }
    Ok(0)
}

pub fn next(root: &Path, artifact: Option<&str>, count: usize, json: bool) -> Result<i32, String> {
    let session = single_session(root, artifact)?;
    let pending = session.pending();
    let slice: Vec<&&BlockEntry> = pending.iter().take(count.max(1)).collect();
    if json {
        let items: Vec<serde_json::Value> = slice
            .iter()
            .map(|block| {
                serde_json::json!({
                    "id": block.id,
                    "kind": block.kind,
                    "line": block.line,
                    "flags": block.flags,
                    "content": block.content,
                })
            })
            .collect();
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "artifact": display_relative(root, &session.artifact_root),
                "remaining": pending.len(),
                "blocks": items,
            }))
            .map_err(|error| format!("cannot serialize next: {error}"))?
        );
        return Ok(0);
    }
    if slice.is_empty() {
        println!("no pending blocks; run `rune adopt finalize`");
        return Ok(0);
    }
    for block in slice {
        println!("── {} ({:?}, line {})", block.id, block.kind, block.line);
        if !block.flags.is_empty() {
            println!("⚑ flagged: {}", block.flags.join(", "));
        }
        println!("{}", block.content.as_deref().unwrap_or(""));
        println!();
    }
    println!("{} pending", pending.len());
    Ok(0)
}

pub fn verdict(
    root: &Path,
    artifact: Option<&str>,
    block_id: &str,
    verdict: &str,
    note: Option<&str>,
    force: bool,
) -> Result<i32, String> {
    if !matches!(verdict, "keep" | "adapt" | "cut") {
        return Err(format!(
            "verdict must be keep, adapt, or cut, got '{verdict}'"
        ));
    }
    if matches!(verdict, "adapt" | "cut") && note.is_none_or(|text| text.trim().is_empty()) {
        return Err(format!(
            "a {verdict} verdict requires --note with the rationale"
        ));
    }
    let mut session = single_session(root, artifact)?;
    let block = session
        .record
        .review
        .predicate
        .blocks
        .iter_mut()
        .find(|block| block.id == block_id)
        .ok_or_else(|| format!("unknown block id '{block_id}'; see `rune adopt next`"))?;
    if block.verdict != "pending" && !force {
        return Err(format!(
            "block {block_id} already decided ({}); pass --force to re-decide",
            block.verdict
        ));
    }
    if verdict == "keep"
        && !block.flags.is_empty()
        && note.is_none_or(|text| text.trim().is_empty())
    {
        return Err(format!(
            "block {block_id} is flagged ({}); keeping it requires --note with the maintainer's rationale",
            block.flags.join(", ")
        ));
    }
    block.verdict = verdict.to_string();
    block.note = note.unwrap_or("").trim().to_string();
    block.decided_on = chrono::Utc::now().to_rfc3339();
    block.transport = "verdict-cli".to_string();
    write_record(&session.record_path, &session.record)?;
    let remaining = session.pending().len();
    println!("{block_id}: {verdict} ({remaining} pending)");
    Ok(0)
}

pub fn finalize(
    root: &Path,
    artifact: Option<&str>,
    reviewer: Option<&str>,
    allow_new: bool,
) -> Result<i32, String> {
    let mut session = single_session(root, artifact)?;
    let pending: Vec<String> = session
        .pending()
        .iter()
        .map(|block| block.id.clone())
        .collect();
    if !pending.is_empty() {
        return Err(format!(
            "{} blocks still pending: {}",
            pending.len(),
            pending.join(", ")
        ));
    }

    let final_files = artifact_files(&session.artifact_root)?;
    let recorded: Vec<String> = session
        .record
        .review
        .subject
        .iter()
        .map(|subject| subject.name.clone())
        .collect();
    let new_files = detect_new_files(&session, &final_files, &recorded, allow_new)?;
    // A skill without its primary document is not an artifact; deleting
    // SKILL.md mid-review must fail finalize, not slip past schema checks.
    if session.artifact_root.is_dir() && !session.artifact_root.join("SKILL.md").is_file() {
        return Err(format!(
            "{} lost its SKILL.md during review; restore it or abandon the session",
            display_relative(root, &session.artifact_root)
        ));
    }

    check_consistency(&session)?;
    check_schema(root, &mut session)?;
    let additions = collect_additions(&session, &final_files);

    let reviewer_identity = match reviewer {
        Some(value) if !value.trim().is_empty() => value.trim().to_string(),
        _ => git_identity(root)?,
    };

    // Fully-cut companions may legitimately be deleted; their sidecars would
    // otherwise dangle as pending forever. Quarantine them with the deletion.
    let final_names: Vec<String> = final_files
        .iter()
        .map(|file| relative_id(&session.artifact_root, file))
        .collect();
    for recorded_name in &recorded {
        if !final_names.contains(recorded_name) {
            remove_orphaned_sidecar(&session.artifact_root, recorded_name);
        }
    }

    let mut subjects = Vec::new();
    let mut digests: Vec<(PathBuf, String)> = Vec::new();
    for file in &final_files {
        let bytes =
            fs::read(file).map_err(|error| format!("cannot read {}: {error}", file.display()))?;
        let digest = manifest::content_sha256_bytes(&bytes);
        subjects.push(manifest::provenance::Subject {
            name: relative_id(&session.artifact_root, file),
            digest: manifest::provenance::DigestMap {
                sha256: digest.clone(),
            },
        });
        digests.push((file.clone(), digest));
    }
    session.record.review.subject = subjects;
    session.record.review.predicate.status = "reviewed".to_string();
    session.record.review.predicate.reviewer = reviewer_identity;
    let completed_on = chrono::Utc::now().to_rfc3339();
    session
        .record
        .review
        .predicate
        .completed_on
        .clone_from(&completed_on);
    for block in &mut session.record.review.predicate.blocks {
        block.content = None;
    }
    let addition_ids: Vec<String> = additions.iter().map(|block| block.id.clone()).collect();
    for mut addition in additions {
        addition.decided_on.clone_from(&completed_on);
        addition.transport = "finalize".to_string();
        session.record.review.predicate.blocks.push(addition);
    }
    // Seal the record BEFORE flipping sidecars: a crash between the two
    // leaves sidecars pending (not deployable), never the reverse. Files that
    // entered via --allow-new were authored in the ceremony, not imported;
    // they have no adopt sidecar and stay first-party, covered by the
    // record's added entries.
    write_record(&session.record_path, &session.record)?;
    for (file, digest) in &digests {
        let relative = relative_id(&session.artifact_root, file);
        if new_files.contains(&relative) {
            continue;
        }
        resync_sidecar(&session.artifact_root, file, digest)?;
    }
    if !addition_ids.is_empty() {
        println!(
            "content added during review (recorded as `added`, endorsed by your commit): {}",
            addition_ids.join(", ")
        );
    }
    println!(
        "reviewed {} ({} blocks) — record: {}",
        display_relative(root, &session.artifact_root),
        session.record.review.predicate.blocks.len(),
        display_relative(root, &session.record_path),
    );
    Ok(0)
}

/// Files that appeared after the review started. Refused unless
/// `--allow-new`, which records them as ceremony-authored added content.
fn detect_new_files(
    session: &Session,
    final_files: &[PathBuf],
    recorded: &[String],
    allow_new: bool,
) -> Result<Vec<String>, String> {
    let mut new_files: Vec<String> = Vec::new();
    for file in final_files {
        let relative = relative_id(&session.artifact_root, file);
        if !recorded.contains(&relative) {
            if allow_new {
                new_files.push(relative);
            } else {
                return Err(format!(
                    "{relative} appeared during the review and has no verdicts; remove it, or re-run with --allow-new to record it as added content"
                ));
            }
        }
    }
    if !new_files.is_empty() {
        println!(
            "new files entering the record as added content (--allow-new): {}",
            new_files.join(", ")
        );
    }
    Ok(new_files)
}

/// Final-state blocks whose content matches no imported block: rewrites from
/// `adapt` verdicts and genuinely new material. They enter the record as
/// `added` entries so the sealed statement covers the whole reviewed file,
/// not only what upstream shipped.
fn collect_additions(session: &Session, final_files: &[PathBuf]) -> Vec<BlockEntry> {
    let mut imported: BTreeMap<String, usize> = BTreeMap::new();
    for block in &session.record.review.predicate.blocks {
        let content = block.content.as_deref().unwrap_or("");
        *imported
            .entry(segment::normalize(block.kind, content))
            .or_insert(0) += 1;
    }
    let mut additions = Vec::new();
    for path in final_files {
        let name = relative_id(&session.artifact_root, path);
        let Ok(bytes) = fs::read(path) else {
            continue;
        };
        for block in segment_bytes(path, &bytes) {
            let key = block.normalized();
            if let Some(count) = imported.get_mut(&key)
                && *count > 0
            {
                *count -= 1;
                continue;
            }
            additions.push(BlockEntry {
                id: format!("{name}:added:{}", block.ordinal),
                flags: lint_flags(block.kind, &block.content, None),
                kind: block.kind,
                digest: manifest::provenance::DigestMap {
                    sha256: manifest::content_sha256(&block.content),
                },
                line: block.start_line,
                verdict: "added".to_string(),
                note: String::new(),
                decided_on: String::new(),
                transport: String::new(),
                content: None,
            });
        }
    }
    additions
}

/// Re-sync a sealed record to maintainer edits made before the commit lands.
/// Pre-commit, the signed commit is the signature layer, so the record's
/// digests follow the maintainer's final touch-ups; the diff under review
/// carries the endorsement. Refuses after nothing changed, and never touches
/// verdicts, notes, or block entries — content digests and sidecars only.
pub fn reseal(root: &Path, artifact: Option<&str>) -> Result<i32, String> {
    let sessions = find_sessions(root)?;
    let mut sealed: Vec<Session> = sessions
        .into_iter()
        .filter(|session| session.record.review.predicate.status == "reviewed")
        .collect();
    if let Some(selector) = artifact {
        let selector_path = root.join(selector);
        sealed.retain(|session| {
            session.artifact_root == selector_path
                || display_relative(root, &session.artifact_root) == selector
        });
    }
    let mut session = match sealed.len() {
        0 => return Err("no sealed review record to reseal".to_string()),
        1 => sealed.pop().expect("length checked"),
        _ => {
            return Err(format!(
                "several sealed records; pass --artifact: {}",
                sealed
                    .iter()
                    .map(|session| display_relative(root, &session.artifact_root))
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
    };

    let mut changed = 0usize;
    let mut updates: Vec<(PathBuf, String)> = Vec::new();
    let subject_files: Vec<(String, PathBuf)> = session
        .record
        .review
        .subject
        .iter()
        .map(|subject| {
            (
                subject.name.clone(),
                session.artifact_root_file(&subject.name),
            )
        })
        .collect();
    for ((name, file), subject) in subject_files
        .into_iter()
        .zip(session.record.review.subject.iter_mut())
    {
        let bytes =
            fs::read(&file).map_err(|error| format!("cannot read {}: {error}", file.display()))?;
        let digest = manifest::content_sha256_bytes(&bytes);
        if digest != subject.digest.sha256 {
            println!("resealing {name} (content changed)");
            subject.digest.sha256.clone_from(&digest);
            changed += 1;
        }
        updates.push((file, digest));
    }
    if changed == 0 {
        println!("nothing changed since the seal; record untouched");
        return Ok(0);
    }
    write_record(&session.record_path, &session.record)?;
    for (file, digest) in &updates {
        if manifest::existing_sidecar_for(file).is_some() {
            resync_sidecar(&session.artifact_root, file, digest)?;
        }
    }
    println!(
        "resealed {} ({changed} subject(s) updated); the signed commit endorses the touch-ups",
        display_relative(root, &session.artifact_root)
    );
    Ok(0)
}

/// Repair the one recoverable incoherence: a sealed record whose sidecars
/// were not flipped because finalize crashed between sealing and re-sync.
/// The record is the authority — a sidecar is repaired only when the file on
/// disk matches the record's subject digest exactly.
pub fn doctor_repair(root: &Path) -> Result<i32, String> {
    let sessions = find_sessions(root)?;
    let mut repaired = 0usize;
    for session in &sessions {
        if session.record.review.predicate.status != "reviewed" {
            continue;
        }
        for subject in &session.record.review.subject {
            let file = session.artifact_root_file(&subject.name);
            let Ok(bytes) = fs::read(&file) else {
                continue;
            };
            if manifest::content_sha256_bytes(&bytes) != subject.digest.sha256 {
                continue;
            }
            let Some(sidecar_path) = manifest::existing_sidecar_for(&file) else {
                continue;
            };
            let sidecar = manifest::provenance::read(&sidecar_path)?;
            if sidecar.provenance.predicate.run_details.metadata.review == "reviewed" {
                continue;
            }
            resync_sidecar(&session.artifact_root, &file, &subject.digest.sha256)?;
            println!("repaired {}", display_relative(root, &sidecar_path));
            repaired += 1;
        }
    }
    println!("{repaired} sidecar(s) repaired");
    Ok(0)
}

/// Verify every review record in the tree: sealed records against the files
/// and sidecars they attest, open sessions surfaced, adopt sidecars without
/// any record reported. Errors are integrity breaks; warnings are telemetry
/// (pacing, missing timestamps on legacy records). Exit 1 on any error.
pub fn doctor(root: &Path, json: bool) -> Result<i32, String> {
    let sessions = find_sessions(root)?;
    let mut errors: Vec<String> = Vec::new();
    let mut warnings: Vec<String> = Vec::new();
    let mut covered_files: Vec<PathBuf> = Vec::new();

    for session in &sessions {
        let label = display_relative(root, &session.artifact_root);
        let predicate = &session.record.review.predicate;
        for subject in &session.record.review.subject {
            covered_files.push(session.artifact_root_file(&subject.name));
        }
        if predicate.status == "pending" {
            let pending = session.pending().len();
            let total = predicate.blocks.len();
            warnings.push(format!(
                "{label}: review in flight ({}/{total} decided, started {})",
                total - pending,
                predicate.started_on
            ));
            continue;
        }

        check_sealed_subjects(&label, session, &mut errors, &mut warnings);

        let mut decided: Vec<chrono::DateTime<chrono::Utc>> = Vec::new();
        for block in &predicate.blocks {
            match block.verdict.as_str() {
                "pending" => errors.push(format!(
                    "{label}: sealed record contains a pending block ({})",
                    block.id
                )),
                "adapt" | "cut" if block.note.trim().is_empty() => errors.push(format!(
                    "{label}: {} verdict on {} has no rationale note",
                    block.verdict, block.id
                )),
                _ => {}
            }
            if block.verdict != "added" {
                if block.decided_on.is_empty() {
                    warnings.push(format!(
                        "{label}: {} has no decision timestamp (pre-hardening record)",
                        block.id
                    ));
                } else if block.transport != "review-tty"
                    && let Ok(timestamp) = chrono::DateTime::parse_from_rfc3339(&block.decided_on)
                {
                    decided.push(timestamp.with_timezone(&chrono::Utc));
                }
            }
        }
        if decided.len() >= 4 {
            decided.sort();
            let mut gaps: Vec<i64> = decided
                .windows(2)
                .map(|pair| (pair[1] - pair[0]).num_milliseconds())
                .collect();
            gaps.sort_unstable();
            let median = gaps[gaps.len() / 2];
            if median < 2000 {
                warnings.push(format!(
                    "{label}: verdict pacing implausible for an interactive review (median gap {median} ms) — telemetry, not proof"
                ));
            }
        }
    }

    let mut orphaned: Vec<String> = Vec::new();
    find_adopt_sidecar_artifacts(root, &mut orphaned, &covered_files);
    for orphan in orphaned {
        warnings.push(format!(
            "{orphan}: imported artifact with no review record — backfill via `rune adopt`"
        ));
    }

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "errors": errors,
                "warnings": warnings,
            }))
            .map_err(|error| format!("cannot serialize doctor report: {error}"))?
        );
    } else {
        for error in &errors {
            println!("✗ {error}");
        }
        for warning in &warnings {
            println!("⚡ {warning}");
        }
        println!(
            "{} error(s), {} warning(s) across {} review record(s)",
            errors.len(),
            warnings.len(),
            sessions.len()
        );
    }
    Ok(i32::from(!errors.is_empty()))
}

fn find_adopt_sidecar_artifacts(directory: &Path, orphans: &mut Vec<String>, covered: &[PathBuf]) {
    let Ok(entries) = fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if !path.is_dir() {
            continue;
        }
        if name == manifest::PROVENANCE_DIRECTORY {
            let holder = directory.to_path_buf();
            let Ok(inner) = fs::read_dir(&path) else {
                continue;
            };
            for sidecar_entry in inner.flatten() {
                let sidecar_path = sidecar_entry.path();
                let file_name = sidecar_entry.file_name().to_string_lossy().to_string();
                if file_name == REVIEW_FILE || file_name.ends_with(".review.yaml") {
                    continue;
                }
                let Ok(sidecar) = manifest::provenance::read(&sidecar_path) else {
                    continue;
                };
                if sidecar.provenance.predicate.build_definition.build_type != "adopt/v1" {
                    continue;
                }
                let Some(subject) = sidecar.provenance.subject.first() else {
                    continue;
                };
                let artifact =
                    holder.join(Path::new(&subject.name).file_name().unwrap_or_default());
                if !covered.contains(&artifact) {
                    orphans.push(subject.name.clone());
                }
            }
        } else if !SKIP_WALK.contains(&name.as_str()) && name != "build" {
            find_adopt_sidecar_artifacts(&path, orphans, covered);
        }
    }
}

pub fn abandon(root: &Path, artifact: Option<&str>, yes: bool) -> Result<i32, String> {
    let session = single_session(root, artifact)?;
    if !yes {
        return Err(format!(
            "abandon moves {} to the tree trash; re-run with --yes to confirm",
            display_relative(root, &session.artifact_root)
        ));
    }
    let trash_root = root.join(".trash").join(format!(
        "adopt-{}",
        chrono::Utc::now().format("%Y-%m-%dT%H-%M-%SZ")
    ));
    fs::create_dir_all(&trash_root)
        .map_err(|error| format!("cannot create {}: {error}", trash_root.display()))?;
    let target = trash_root.join(
        session
            .artifact_root
            .file_name()
            .unwrap_or_else(|| std::ffi::OsStr::new("artifact")),
    );
    fs::rename(&session.artifact_root, &target).map_err(|error| {
        format!(
            "cannot move {} to {}: {error}",
            session.artifact_root.display(),
            target.display()
        )
    })?;
    if session.record_path.exists() {
        let record_target = trash_root.join(REVIEW_FILE);
        fs::rename(&session.record_path, &record_target).map_err(|error| {
            format!(
                "cannot move {} to {}: {error}",
                session.record_path.display(),
                record_target.display()
            )
        })?;
    }
    println!("abandoned into {}", trash_root.display());
    Ok(0)
}

/// Verify each sealed subject against disk and its adopt sidecar: content
/// digests must match the record, and imported subjects must carry a
/// sidecar that says reviewed. Ceremony-authored subjects (all blocks
/// `added`) legitimately have no sidecar and warn instead.
fn check_sealed_subjects(
    label: &str,
    session: &Session,
    errors: &mut Vec<String>,
    warnings: &mut Vec<String>,
) {
    let predicate = &session.record.review.predicate;
    for subject in &session.record.review.subject {
        let file = session.artifact_root_file(&subject.name);
        let Ok(bytes) = fs::read(&file) else {
            errors.push(format!(
                "{label}: reviewed subject {} is missing from disk",
                subject.name
            ));
            continue;
        };
        let disk_digest = manifest::content_sha256_bytes(&bytes);
        if disk_digest != subject.digest.sha256 {
            errors.push(format!(
                "{label}: {} was edited after review (record sha256:{}, disk sha256:{disk_digest})",
                subject.name, subject.digest.sha256
            ));
        }
        let Some(sidecar_path) = manifest::existing_sidecar_for(&file) else {
            // A subject whose content the record carries only as `added`
            // entries was authored during the ceremony, never imported:
            // no adopt sidecar exists and none is expected.
            let ceremony_authored = predicate
                .blocks
                .iter()
                .filter(|block| block.id.starts_with(&format!("{}:", subject.name)))
                .all(|block| block.verdict == "added");
            if ceremony_authored {
                warnings.push(format!(
                    "{label}: {} was authored during the review (no adopt sidecar; covered by added entries)",
                    subject.name
                ));
            } else {
                errors.push(format!(
                    "{label}: sidecar for imported subject {} is missing",
                    subject.name
                ));
            }
            continue;
        };
        match manifest::provenance::read(&sidecar_path) {
            Ok(sidecar) => {
                if sidecar.provenance.predicate.run_details.metadata.review != "reviewed" {
                    errors.push(format!(
                        "{label}: sidecar for {} does not say reviewed",
                        subject.name
                    ));
                }
                if let Some(side_subject) = sidecar.provenance.subject.first()
                    && side_subject.digest.sha256 != subject.digest.sha256
                {
                    errors.push(format!(
                        "{label}: sidecar digest for {} disagrees with the review record",
                        subject.name
                    ));
                }
            }
            Err(error) => errors.push(format!(
                "{label}: cannot read sidecar for {}: {error}",
                subject.name
            )),
        }
    }
}

/// The imported blocks, grouped by normalized content, must reconcile with
/// the edited files: every kept block still appears as a whole block, no
/// cut or adapted block survives verbatim, and files that appeared or grew
/// content outside the review are caught by the subject inventory.
fn check_consistency(session: &Session) -> Result<(), String> {
    #[derive(Default)]
    struct Group {
        keep: usize,
        removed: Vec<String>,
        kept_ids: Vec<String>,
    }

    let mut final_counts: BTreeMap<String, usize> = BTreeMap::new();
    for subject in &session.record.review.subject {
        let path = session.artifact_root_file(&subject.name);
        let Ok(bytes) = fs::read(&path) else {
            continue;
        };
        for block in segment_bytes(&path, &bytes) {
            *final_counts.entry(block.normalized()).or_insert(0) += 1;
        }
    }

    let mut groups: BTreeMap<String, Group> = BTreeMap::new();
    for block in &session.record.review.predicate.blocks {
        let content = block.content.as_deref().unwrap_or("");
        let key = segment::normalize(block.kind, content);
        let group = groups.entry(key).or_default();
        match block.verdict.as_str() {
            "keep" => {
                group.keep += 1;
                group.kept_ids.push(block.id.clone());
            }
            _ => group.removed.push(block.id.clone()),
        }
    }

    let mut violations = Vec::new();
    for (key, group) in &groups {
        let actual = final_counts.get(key).copied().unwrap_or(0);
        if actual < group.keep {
            violations.push(format!(
                "kept content missing from the edited artifact: {}",
                group.kept_ids.join(", ")
            ));
        }
        if !group.removed.is_empty() && actual > group.keep {
            violations.push(format!(
                "content marked {} still appears verbatim: {}",
                if group.keep == 0 { "cut/adapt" } else { "cut" },
                group.removed.join(", ")
            ));
        }
    }
    if violations.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "verdict consistency failed:\n  {}",
            violations.join("\n  ")
        ))
    }
}

/// Structural validation of the artifact's primary document against the
/// kind's `.mdschema`. The schema is resolved from OUTSIDE the artifact —
/// nearest `.mdschema` walking up from the artifact's parent directory,
/// falling back to the embedded kind template — so an imported tree can
/// never supply its own permissive schema. The resolved schema's origin and
/// digest land in the review record.
fn check_schema(root: &Path, session: &mut Session) -> Result<(), String> {
    let primary = primary_document(&session.artifact_root);
    let Some(primary) = primary else {
        return Ok(());
    };
    let kind_directory = kind_directory_of(&session.artifact_root);
    let (schema_source, schema_content) =
        resolve_schema(&session.artifact_root, root, kind_directory.as_deref())?;

    let content = fs::read_to_string(&primary)
        .map_err(|error| format!("cannot read {}: {error}", primary.display()))?;
    let label = display_relative(root, &primary);
    let diagnostics = rune::validate::mdschema::check(&content, &label, &schema_content);
    session.record.review.predicate.schema = schema_source;
    session.record.review.predicate.schema_digest = manifest::content_sha256(&schema_content);
    if diagnostics.is_empty() {
        return Ok(());
    }
    let report: Vec<String> = diagnostics
        .iter()
        .map(|diagnostic| match diagnostic.line {
            Some(line) => format!("{}:{line}: {}", diagnostic.file, diagnostic.message),
            None => format!("{}: {}", diagnostic.file, diagnostic.message),
        })
        .collect();
    Err(format!(
        "mdschema check failed for {label}:\n  {}",
        report.join("\n  ")
    ))
}

/// The document the kind schema governs: `SKILL.md` for a skill directory,
/// the file itself for single-file kinds. Companions are reviewed but not
/// schema-checked.
fn primary_document(artifact_root: &Path) -> Option<PathBuf> {
    if artifact_root.is_dir() {
        let skill = artifact_root.join("SKILL.md");
        skill.is_file().then_some(skill)
    } else {
        Some(artifact_root.to_path_buf())
    }
}

fn kind_directory_of(artifact_root: &Path) -> Option<String> {
    artifact_root
        .parent()
        .and_then(Path::file_name)
        .map(|name| name.to_string_lossy().to_string())
        .filter(|name| matches!(name.as_str(), "skills" | "agents" | "rules" | "decisions"))
}

fn resolve_schema(
    artifact_root: &Path,
    root: &Path,
    kind_directory: Option<&str>,
) -> Result<(String, String), String> {
    let mut directory = artifact_root.parent();
    while let Some(current) = directory {
        let candidate = current.join(".mdschema");
        if candidate.is_file() {
            let content = fs::read_to_string(&candidate)
                .map_err(|error| format!("cannot read {}: {error}", candidate.display()))?;
            return Ok((display_relative(root, &candidate), content));
        }
        if current == root {
            break;
        }
        directory = current.parent();
    }
    if let Some(kind) = kind_directory
        && let Some(embedded) = crate::cli::validate::templates::embedded_mdschema(kind)
    {
        return Ok((format!("embedded:{kind}"), embedded));
    }
    Err(format!(
        "no .mdschema governs {}; commit one beside the kind directory (e.g. skills/.mdschema) before finalizing",
        artifact_root.display()
    ))
}

/// A recorded subject whose file was deleted during review (every block cut)
/// leaves a pending sidecar behind; drop it so nothing dangles.
fn remove_orphaned_sidecar(artifact_root: &Path, recorded_name: &str) {
    let file = if artifact_root.is_dir() {
        artifact_root.join(recorded_name)
    } else {
        artifact_root.to_path_buf()
    };
    let Some(sidecar_path) = manifest::existing_sidecar_for(&file) else {
        return;
    };
    if let Err(error) = fs::remove_file(&sidecar_path) {
        eprintln!(
            "warning: cannot remove orphaned sidecar {}: {error}",
            sidecar_path.display()
        );
    }
}

fn resync_sidecar(artifact_root: &Path, file: &Path, digest: &str) -> Result<(), String> {
    let Some(sidecar_path) = manifest::existing_sidecar_for(file) else {
        return Err(format!(
            "{} has no adopt sidecar at {}; was this imported with rune?",
            display_relative(artifact_root, file),
            manifest::sidecar_for(file).display()
        ));
    };
    let mut sidecar = manifest::provenance::read(&sidecar_path)?;
    if let Some(subject) = sidecar.provenance.subject.first_mut() {
        subject.digest.sha256 = digest.to_string();
    }
    sidecar.provenance.predicate.run_details.metadata.review = "reviewed".to_string();
    let yaml = serde_yaml::to_string(&sidecar)
        .map_err(|error| format!("cannot serialize {}: {error}", sidecar_path.display()))?;
    atomic_write(&sidecar_path, &yaml)
}

fn git_identity(root: &Path) -> Result<String, String> {
    let value = |key: &str| -> Option<String> {
        Command::new("git")
            .arg("-C")
            .arg(root)
            .arg("config")
            .arg(key)
            .env_remove("GIT_DIR")
            .env_remove("GIT_WORK_TREE")
            .env_remove("GIT_INDEX_FILE")
            .output()
            .ok()
            .filter(|output| output.status.success())
            .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
            .filter(|text| !text.is_empty())
    };
    match (value("user.name"), value("user.email")) {
        (Some(name), Some(email)) => Ok(format!("{name} <{email}>")),
        (Some(name), None) => Ok(name),
        _ => Err(
            "cannot resolve the reviewer from git config; pass --reviewer \"Name <email>\""
                .to_string(),
        ),
    }
}

impl Session {
    fn artifact_root_file(&self, relative: &str) -> PathBuf {
        if self.artifact_root.is_dir() {
            self.artifact_root.join(relative)
        } else {
            self.artifact_root.clone()
        }
    }
}

fn segment_bytes(path: &Path, bytes: &[u8]) -> Vec<Block> {
    let is_markdown = path
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("md"));
    if is_markdown {
        match std::str::from_utf8(bytes) {
            Ok(text) => segment::segment_markdown(text),
            Err(_) => segment::segment_file(bytes),
        }
    } else {
        segment::segment_file(bytes)
    }
}

fn artifact_files(artifact_root: &Path) -> Result<Vec<PathBuf>, String> {
    if artifact_root.is_file() {
        return Ok(vec![artifact_root.to_path_buf()]);
    }
    if !artifact_root.is_dir() {
        return Err(format!("no artifact at {}", artifact_root.display()));
    }
    let mut files = Vec::new();
    walk(artifact_root, &mut files)?;
    files.sort();
    Ok(files)
}

fn walk(directory: &Path, files: &mut Vec<PathBuf>) -> Result<(), String> {
    let entries = fs::read_dir(directory)
        .map_err(|error| format!("cannot read {}: {error}", directory.display()))?;
    for entry in entries {
        let entry = entry.map_err(|error| format!("directory entry error: {error}"))?;
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy().to_string();
        let file_type = entry
            .file_type()
            .map_err(|error| format!("cannot stat {}: {error}", path.display()))?;
        if file_type.is_symlink() {
            return Err(format!(
                "{} is a symlink; review covers real files only — replace it with the file it points at",
                path.display()
            ));
        }
        if file_type.is_dir() {
            if SKIP_WALK.contains(&name.as_str()) {
                continue;
            }
            walk(&path, files)?;
        } else if file_type.is_file() && name != ".DS_Store" {
            files.push(path);
        }
    }
    Ok(())
}

fn relative_id(artifact_root: &Path, file: &Path) -> String {
    if artifact_root.is_dir() {
        file.strip_prefix(artifact_root)
            .unwrap_or(file)
            .components()
            .map(|component| component.as_os_str().to_string_lossy().to_string())
            .collect::<Vec<_>>()
            .join("/")
    } else {
        file.file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string()
    }
}

fn record_path_for(artifact_root: &Path) -> Result<PathBuf, String> {
    if artifact_root.is_dir() {
        Ok(artifact_root
            .join(manifest::PROVENANCE_DIRECTORY)
            .join(REVIEW_FILE))
    } else {
        let parent = artifact_root
            .parent()
            .ok_or_else(|| format!("{} has no parent", artifact_root.display()))?;
        let stem = artifact_root
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy();
        Ok(parent
            .join(manifest::PROVENANCE_DIRECTORY)
            .join(format!("{stem}.review.yaml")))
    }
}

fn find_sessions(root: &Path) -> Result<Vec<Session>, String> {
    let mut sessions = Vec::new();
    let mut records = Vec::new();
    find_records(root, &mut records)?;
    for record_path in records {
        let record = read_record(&record_path)?;
        let artifact_root = artifact_root_for(&record_path, &record);
        sessions.push(Session {
            record_path,
            artifact_root,
            record,
        });
    }
    sessions.sort_by(|a, b| a.record_path.cmp(&b.record_path));
    Ok(sessions)
}

fn find_records(directory: &Path, records: &mut Vec<PathBuf>) -> Result<(), String> {
    let Ok(entries) = fs::read_dir(directory) else {
        return Ok(());
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if file_type.is_dir() {
            if name == manifest::PROVENANCE_DIRECTORY {
                let Ok(inner) = fs::read_dir(&path) else {
                    continue;
                };
                for inner_entry in inner.flatten() {
                    let inner_name = inner_entry.file_name().to_string_lossy().to_string();
                    if inner_name == REVIEW_FILE || inner_name.ends_with(".review.yaml") {
                        records.push(inner_entry.path());
                    }
                }
            } else if !SKIP_WALK.contains(&name.as_str()) && name != "build" {
                find_records(&path, records)?;
            }
        }
    }
    Ok(())
}

fn artifact_root_for(record_path: &Path, _record: &ReviewRecord) -> PathBuf {
    let provenance_dir = record_path.parent().unwrap_or_else(|| Path::new("."));
    let holder = provenance_dir
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .to_path_buf();
    let file_name = record_path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    if file_name == REVIEW_FILE {
        holder
    } else {
        let stem = file_name.trim_end_matches(".review.yaml");
        holder.join(format!("{stem}.md"))
    }
}

fn single_session(root: &Path, artifact: Option<&str>) -> Result<Session, String> {
    let sessions = find_sessions(root)?;
    let mut open: Vec<Session> = sessions
        .into_iter()
        .filter(|session| session.record.review.predicate.status == "pending")
        .collect();
    match artifact {
        Some(selector) => {
            let selector_path = root.join(selector);
            open.retain(|session| {
                session.artifact_root == selector_path
                    || display_relative(root, &session.artifact_root) == selector
            });
            open.pop()
                .ok_or_else(|| format!("no open review session matches '{selector}'"))
        }
        None => match open.len() {
            0 => Err("no open review session; run `rune adopt start <source>`".to_string()),
            1 => Ok(open.pop().expect("length checked")),
            _ => Err(format!(
                "several sessions are open; pass --artifact: {}",
                open.iter()
                    .map(|session| display_relative(root, &session.artifact_root))
                    .collect::<Vec<_>>()
                    .join(", ")
            )),
        },
    }
}

fn read_record(path: &Path) -> Result<ReviewRecord, String> {
    let content = fs::read_to_string(path)
        .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
    serde_yaml::from_str(&content)
        .map_err(|error| format!("invalid review record {}: {error}", path.display()))
}

fn write_record(path: &Path, record: &ReviewRecord) -> Result<(), String> {
    let yaml = serde_yaml::to_string(record)
        .map_err(|error| format!("cannot serialize review record: {error}"))?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("cannot create {}: {error}", parent.display()))?;
    }
    atomic_write(path, &yaml)
}

/// Temp-file-and-rename so a crash mid-write never leaves a torn record.
fn atomic_write(path: &Path, content: &str) -> Result<(), String> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let temp = parent.join(format!(
        ".{}.tmp-{}",
        path.file_name().unwrap_or_default().to_string_lossy(),
        std::process::id()
    ));
    fs::write(&temp, content)
        .map_err(|error| format!("cannot write {}: {error}", temp.display()))?;
    fs::rename(&temp, path).map_err(|error| {
        let _ = fs::remove_file(&temp);
        format!("cannot replace {}: {error}", path.display())
    })
}

fn display_relative(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .components()
        .map(|component| component.as_os_str().to_string_lossy().to_string())
        .collect::<Vec<_>>()
        .join("/")
}
