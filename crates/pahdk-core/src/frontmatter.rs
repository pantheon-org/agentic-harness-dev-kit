//! Splitting a markdown document into its YAML frontmatter and its body.
//!
//! Dependency-free: this only finds the fences. Parsing the YAML is the
//! caller's job, or [`crate::validate`]'s when the `validate` feature is on.

/// The frontmatter delimiter.
pub const FENCE: &str = "---";

/// Split `content` into its frontmatter YAML and the body that follows it.
///
/// Returns `None` when the content does not open with a `---` fence, or the
/// fence is never closed, so a file without frontmatter is reported as such
/// instead of being read as an empty document. The body has its leading blank
/// lines removed.
///
/// Both `\n` and `\r\n` line endings are accepted. Tools write `\n`, but a
/// clone on a machine with `core.autocrlf=true` converts them on checkout, and
/// refusing those would make every command fail on an otherwise valid
/// repository.
pub fn split(content: &str) -> Option<(&str, &str)> {
    let rest = strip_line_break(content.strip_prefix(FENCE)?)?;
    let end = find_closing_fence(rest)?;
    let yaml = &rest[..end];
    let after = &rest[end + FENCE.len()..];
    Some((yaml, after.trim_start_matches(['\r', '\n'])))
}

/// Strip one leading line break, in either convention.
fn strip_line_break(text: &str) -> Option<&str> {
    text.strip_prefix("\r\n")
        .or_else(|| text.strip_prefix('\n'))
}

/// The byte offset of the closing `---` fence, which must start its own line.
fn find_closing_fence(rest: &str) -> Option<usize> {
    let mut offset = 0;
    for line in rest.split_inclusive('\n') {
        if line.trim_end() == FENCE {
            return Some(offset);
        }
        offset += line.len();
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_yaml_from_body() {
        let (yaml, body) = split("---\ntitle: x\n---\n\n# Heading\n").unwrap();
        assert_eq!(yaml, "title: x\n");
        assert_eq!(body, "# Heading\n");
    }

    #[test]
    fn accepts_crlf_line_endings() {
        let (yaml, body) = split("---\r\ntitle: x\r\n---\r\n\r\n# H\r\n").unwrap();
        assert_eq!(yaml, "title: x\r\n");
        assert_eq!(body, "# H\r\n");
    }

    #[test]
    fn rejects_content_without_an_opening_fence() {
        assert!(split("# Heading\n").is_none());
        assert!(split("--- not a fence\nx: 1\n---\n").is_none());
    }

    #[test]
    fn rejects_an_unterminated_fence() {
        assert!(split("---\ntitle: x\n# Heading\n").is_none());
    }

    #[test]
    fn a_closing_fence_must_start_its_own_line() {
        let (yaml, _) = split("---\nnote: a --- b\n---\nbody\n").unwrap();
        assert_eq!(yaml, "note: a --- b\n");
    }

    #[test]
    fn an_empty_frontmatter_block_is_found() {
        let (yaml, body) = split("---\n---\nbody\n").unwrap();
        assert_eq!(yaml, "");
        assert_eq!(body, "body\n");
    }
}
