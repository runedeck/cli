//! Deterministic segmentation of markdown into review blocks.
//!
//! Blocks come from the CommonMark/GFM block structure (pulldown-cmark with
//! byte offsets), so fenced code stays atomic, a list with internal blank
//! lines is one block, and setext headings, HTML blocks, and footnote
//! definitions land as their own units. Content that produces no events
//! (link reference definitions) is recovered from the gaps between block
//! spans so every non-blank line of the file belongs to exactly one block.
//!
//! The segmenter is versioned: a review record pins [`SEGMENTER_VERSION`],
//! and any change to the block boundaries here must bump it.

use pulldown_cmark::{Event, Options, Parser};
use serde::{Deserialize, Serialize};

pub const SEGMENTER_VERSION: &str = "segment/v1";

#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum BlockKind {
    Frontmatter,
    Heading,
    Paragraph,
    Code,
    List,
    Table,
    Quote,
    Html,
    Footnote,
    Rule,
    Other,
    /// A whole non-markdown file reviewed as one unit.
    File,
}

#[derive(Clone, Debug)]
pub struct Block {
    pub ordinal: usize,
    pub kind: BlockKind,
    pub content: String,
    pub start_line: usize,
}

impl Block {
    /// Whitespace handling depends on what the block is: prose survives
    /// rewrapping, while code, frontmatter, and whole files compare exactly
    /// (modulo a trailing newline) because their whitespace is meaning.
    #[must_use]
    pub fn normalized(&self) -> String {
        normalize(self.kind, &self.content)
    }
}

#[must_use]
pub fn normalize(kind: BlockKind, content: &str) -> String {
    match kind {
        BlockKind::Code | BlockKind::Frontmatter | BlockKind::File => {
            content.trim_end_matches('\n').to_string()
        }
        _ => content.split_whitespace().collect::<Vec<_>>().join(" "),
    }
}

/// Segment a markdown document into review blocks. Identical input yields
/// identical blocks: the parse is pure and the ordinals are positional.
#[must_use]
pub fn segment_markdown(content: &str) -> Vec<Block> {
    let mut blocks = Vec::new();

    let body = rune::parse::split_frontmatter(content).map_or(content, |(_, body)| body);
    let body_offset = content.len() - body.len();
    if body_offset > 0 {
        let frontmatter_raw = &content[..body_offset];
        if !frontmatter_raw.trim().is_empty() {
            blocks.push(Block {
                ordinal: 0,
                kind: BlockKind::Frontmatter,
                content: frontmatter_raw.to_string(),
                start_line: 1,
            });
        }
    }

    let mut spans: Vec<(std::ops::Range<usize>, BlockKind)> = Vec::new();
    let mut depth = 0usize;
    let options = Options::ENABLE_TABLES
        | Options::ENABLE_FOOTNOTES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS;
    for (event, range) in Parser::new_ext(body, options).into_offset_iter() {
        match event {
            Event::Start(tag) => {
                if depth == 0 {
                    spans.push((range, block_kind(&tag)));
                }
                depth += 1;
            }
            Event::End(_) => {
                depth = depth.saturating_sub(1);
            }
            Event::Rule if depth == 0 => {
                spans.push((range, BlockKind::Rule));
            }
            _ => {}
        }
    }

    let mut cursor = 0usize;
    let mut covered: Vec<(std::ops::Range<usize>, BlockKind)> = Vec::new();
    for (range, kind) in spans {
        push_gap(body, cursor..range.start, &mut covered);
        cursor = cursor.max(range.end);
        covered.push((range, kind));
    }
    push_gap(body, cursor..body.len(), &mut covered);

    for (range, kind) in covered {
        let raw = &body[range.clone()];
        if raw.trim().is_empty() {
            continue;
        }
        blocks.push(Block {
            ordinal: 0,
            kind,
            content: raw.trim_end_matches('\n').to_string(),
            start_line: line_of(content, body_offset + range.start),
        });
    }

    for (index, block) in blocks.iter_mut().enumerate() {
        block.ordinal = index + 1;
    }
    blocks
}

/// A non-markdown text file is one reviewable block; binary content is
/// represented by its digest placeholder so it still gets a verdict.
#[must_use]
pub fn segment_file(bytes: &[u8]) -> Vec<Block> {
    let content = match std::str::from_utf8(bytes) {
        Ok(text) => text.to_string(),
        Err(_) => format!(
            "(binary file, sha256:{})",
            rune::manifest::content_sha256_bytes(bytes)
        ),
    };
    vec![Block {
        ordinal: 1,
        kind: BlockKind::File,
        content,
        start_line: 1,
    }]
}

fn push_gap(
    body: &str,
    gap: std::ops::Range<usize>,
    covered: &mut Vec<(std::ops::Range<usize>, BlockKind)>,
) {
    if gap.start >= gap.end {
        return;
    }
    if body[gap.clone()].trim().is_empty() {
        return;
    }
    covered.push((gap, BlockKind::Other));
}

fn block_kind(tag: &pulldown_cmark::Tag) -> BlockKind {
    use pulldown_cmark::Tag;
    match tag {
        Tag::Heading { .. } => BlockKind::Heading,
        Tag::CodeBlock(_) => BlockKind::Code,
        Tag::List(_) => BlockKind::List,
        Tag::Table(_) => BlockKind::Table,
        Tag::BlockQuote(_) => BlockKind::Quote,
        Tag::HtmlBlock => BlockKind::Html,
        Tag::FootnoteDefinition(_) => BlockKind::Footnote,
        Tag::MetadataBlock(_) => BlockKind::Frontmatter,
        _ => BlockKind::Paragraph,
    }
}

fn line_of(content: &str, byte_offset: usize) -> usize {
    content[..byte_offset.min(content.len())]
        .bytes()
        .filter(|byte| *byte == b'\n')
        .count()
        + 1
}
