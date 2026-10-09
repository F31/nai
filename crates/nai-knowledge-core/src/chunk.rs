//! Bounded document chunking with provenance. Empty parsed text is rejected
//! (a scan-only PDF without OCR must not become an empty "imported" document).
use std::error::Error;
use std::fmt;

pub const DEFAULT_BUDGET: usize = 512;
pub const MAX_CHUNKS: usize = 4096;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chunk {
    pub chunk_id: String,
    pub content: String,
    pub source_span: String,
    pub product_scope: String,
    pub acl_ref: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChunkError {
    EmptyText,
    TooLarge,
    TooManyChunks,
}

impl fmt::Display for ChunkError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "chunk: {}",
            match self {
                Self::EmptyText => "empty text cannot be indexed",
                Self::TooLarge => "document exceeds import bound",
                Self::TooManyChunks => "chunk count exceeds bound",
            }
        )
    }
}
impl Error for ChunkError {}

/// Splits `text` into chunks no longer than `budget` characters, computing
/// line:col spans. `product_scope`/`acl_ref` are copied to every chunk.
pub fn split_document(
    document_id: &str,
    text: &str,
    product_scope: &str,
    acl_ref: &str,
    budget: usize,
    max_chunks: usize,
) -> Result<Vec<Chunk>, ChunkError> {
    if text.trim().is_empty() {
        return Err(ChunkError::EmptyText);
    }
    if text.chars().count() > 16 * 1024 * 1024 {
        return Err(ChunkError::TooLarge);
    }
    let budget = budget.clamp(1, 16 * 1024);
    let max_chunks = max_chunks.min(MAX_CHUNKS);
    let mut chunks = Vec::new();
    let mut current = String::new();
    let mut start_line = 1usize;
    let mut start_col = 1usize;
    let mut line = 1usize;
    let mut col = 1usize;
    let mut current_len = 0usize;
    for c in text.chars() {
        if current_len == budget {
            let (end_line, end_col) = (line, col);
            chunks.push(Chunk {
                chunk_id: format!("{document_id}#chunk-{}", chunks.len()),
                content: std::mem::take(&mut current),
                source_span: format!("{start_line}:{start_col}-{end_line}:{end_col}"),
                product_scope: product_scope.into(),
                acl_ref: acl_ref.into(),
            });
            if chunks.len() > max_chunks {
                return Err(ChunkError::TooManyChunks);
            }
            start_line = line;
            start_col = col;
            current_len = 0;
        }
        current.push(c);
        current_len += 1;
        if c == '\n' {
            line += 1;
            col = 1;
        } else {
            col += 1;
        }
    }
    if !current.is_empty() || chunks.is_empty() {
        chunks.push(Chunk {
            chunk_id: format!("{document_id}#chunk-{}", chunks.len()),
            content: current,
            source_span: format!("{start_line}:{start_col}-{line}:{col}"),
            product_scope: product_scope.into(),
            acl_ref: acl_ref.into(),
        });
    }
    if chunks.len() > max_chunks {
        return Err(ChunkError::TooManyChunks);
    }
    Ok(chunks)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_text_is_rejected() {
        assert_eq!(
            split_document("d", "  \n ", "p", "acl", 512, 16),
            Err(ChunkError::EmptyText)
        );
    }

    #[test]
    fn bounded_chunks_carry_provenance_spans() {
        let text = "第一行内容\n第二行内容比较长一些\n第三行结尾";
        let chunks = split_document("doc", text, "sku-1", "acl-read", 8, 16).unwrap();
        assert!(chunks.len() >= 2);
        for chunk in &chunks {
            assert_eq!(chunk.product_scope, "sku-1");
            assert_eq!(chunk.acl_ref, "acl-read");
            assert!(chunk.content.chars().count() <= 8);
            assert!(
                chunk
                    .source_span
                    .chars()
                    .next()
                    .is_some_and(|c| c.is_ascii_digit())
            );
        }
        assert!(chunks.iter().all(|c| c.source_span.contains(':')));
    }

    #[test]
    fn huge_document_is_rejected() {
        let text = "x".repeat(16 * 1024 * 1024 + 1);
        assert!(split_document("d", &text, "p", "acl", 512, 4096).is_err());
        assert_eq!(
            split_document("d", &"字".repeat(MAX_CHUNKS + 1), "p", "acl", 1, usize::MAX),
            Err(ChunkError::TooManyChunks)
        );
    }
}
