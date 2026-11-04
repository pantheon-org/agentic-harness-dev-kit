//! Validating a markdown document against a JSON Schema and a heading list.
//!
//! The tools in this kit write documents with YAML frontmatter and a fixed set
//! of section headings (a decision record, a journal entry). Each tool ships
//! the shape as data: a `*.schema.json` for the frontmatter and a list of
//! required headings. This module checks a document against both and reports
//! every problem it finds, so the check lives in one place instead of being
//! rewritten per tool.
//!
//! It checks structure only. Whether the prose is any good, or whether a link
//! points at something that exists, is the calling tool's business.
//!
//! Needs the `validate` feature.

use std::fmt;

use serde::Serialize;
use serde_json::Value;

use crate::error::{Error, Result};
use crate::frontmatter;

/// One problem found in a document.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Issue {
    /// Where it was found: `frontmatter /status`, `body`, and so on.
    pub location: String,
    /// What is wrong.
    pub message: String,
}

impl Issue {
    /// Build an issue.
    pub fn new(location: impl Into<String>, message: impl Into<String>) -> Self {
        Issue {
            location: location.into(),
            message: message.into(),
        }
    }
}

impl fmt::Display for Issue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.location, self.message)
    }
}

/// Every way `instance` fails `schema`, sorted by location then message.
///
/// The draft is taken from the schema's `$schema`, defaulting to the latest.
/// The location is the instance's JSON pointer (`/status`), empty for the root.
/// A schema that is not itself valid is an [`Error::Config`], so a broken
/// schema is never mistaken for a passing document.
pub fn schema_issues(schema: &Value, instance: &Value) -> Result<Vec<Issue>> {
    let validator = jsonschema::validator_for(schema)
        .map_err(|e| Error::Config(format!("invalid JSON Schema: {e}")))?;
    let mut issues: Vec<Issue> = validator
        .iter_errors(instance)
        .map(|e| Issue::new(e.instance_path().to_string(), e.to_string()))
        .collect();
    issues.sort_by(|a, b| (&a.location, &a.message).cmp(&(&b.location, &b.message)));
    Ok(issues)
}

/// One issue for each line in `required` that is absent from `body`.
///
/// A heading matches when a whole line, ignoring trailing whitespace, equals
/// it, so pass the full line (`"### Context"`). Lines inside fenced code
/// blocks do not count.
pub fn missing_headings<S: AsRef<str>>(body: &str, required: &[S]) -> Vec<Issue> {
    let mut present = Vec::new();
    let mut in_fence = false;
    for line in body.lines().map(str::trim_end) {
        if line.trim_start().starts_with("```") || line.trim_start().starts_with("~~~") {
            in_fence = !in_fence;
        } else if !in_fence {
            present.push(line);
        }
    }
    required
        .iter()
        .map(AsRef::as_ref)
        .filter(|heading| !present.contains(heading))
        .map(|heading| Issue::new("body", format!("missing heading {heading:?}")))
        .collect()
}

/// Validate a whole document: its frontmatter against `schema`, and its body
/// for every heading in `required_headings`.
///
/// A document with no frontmatter, or frontmatter that is not YAML, yields a
/// single `frontmatter` issue and the body is not checked, because the tool
/// cannot tell which headings apply.
pub fn document_issues<S: AsRef<str>>(
    content: &str,
    schema: &Value,
    required_headings: &[S],
) -> Result<Vec<Issue>> {
    let Some((yaml, body)) = frontmatter::split(content) else {
        return Ok(vec![Issue::new(
            "frontmatter",
            "no YAML frontmatter found (expected a leading `---` block)",
        )]);
    };
    let instance: Value = match serde_yaml_ng::from_str(yaml) {
        Ok(value) => value,
        Err(e) => {
            return Ok(vec![Issue::new(
                "frontmatter",
                format!("invalid YAML: {e}"),
            )])
        }
    };

    let mut issues: Vec<Issue> = schema_issues(schema, &instance)?
        .into_iter()
        .map(|i| Issue::new(format!("frontmatter {}", i.location).trim_end(), i.message))
        .collect();
    issues.extend(missing_headings(body, required_headings));
    Ok(issues)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn schema() -> Value {
        json!({
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "type": "object",
            "required": ["title", "status"],
            "additionalProperties": false,
            "properties": {
                "title": {"type": "string"},
                "status": {"enum": ["open", "closed"]}
            }
        })
    }

    fn doc(frontmatter: &str, body: &str) -> String {
        format!("---\n{frontmatter}---\n\n{body}")
    }

    #[test]
    fn a_conforming_document_has_no_issues() {
        let content = doc("title: T\nstatus: open\n", "# T\n\n## Why\ntext\n");
        assert!(document_issues(&content, &schema(), &["## Why"])
            .unwrap()
            .is_empty());
    }

    #[test]
    fn schema_failures_carry_the_field_location() {
        let content = doc("title: T\nstatus: nope\ncolour: red\n", "");
        let issues = document_issues::<&str>(&content, &schema(), &[]).unwrap();
        let locations: Vec<&str> = issues.iter().map(|i| i.location.as_str()).collect();
        assert!(locations.contains(&"frontmatter /status"), "{issues:?}");
        assert!(locations.contains(&"frontmatter"), "{issues:?}");
    }

    #[test]
    fn a_missing_required_field_is_reported_at_the_root() {
        let content = doc("title: T\n", "");
        let issues = document_issues::<&str>(&content, &schema(), &[]).unwrap();
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].location, "frontmatter");
        assert!(issues[0].message.contains("status"), "{issues:?}");
    }

    #[test]
    fn missing_headings_are_reported_one_each() {
        let issues = missing_headings("# T\n\n## A\n", &["## A", "## B", "### C"]);
        let messages: Vec<&str> = issues.iter().map(|i| i.message.as_str()).collect();
        assert_eq!(
            messages,
            vec!["missing heading \"## B\"", "missing heading \"### C\""]
        );
    }

    #[test]
    fn a_heading_inside_a_code_fence_does_not_count() {
        let body = "# T\n\n```md\n## A\n```\n";
        assert_eq!(missing_headings(body, &["## A"]).len(), 1);
    }

    #[test]
    fn a_longer_heading_does_not_satisfy_a_shorter_one() {
        assert_eq!(
            missing_headings("## Context and more\n", &["## Context"]).len(),
            1
        );
        assert!(missing_headings("## Context  \n", &["## Context"]).is_empty());
    }

    #[test]
    fn no_frontmatter_is_one_issue_and_skips_the_body_check() {
        let issues = document_issues("# Just prose\n", &schema(), &["## A"]).unwrap();
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].location, "frontmatter");
    }

    #[test]
    fn invalid_yaml_is_one_issue() {
        let content = doc("title: [unclosed\n", "");
        let issues = document_issues::<&str>(&content, &schema(), &[]).unwrap();
        assert_eq!(issues.len(), 1);
        assert!(issues[0].message.starts_with("invalid YAML"), "{issues:?}");
    }

    #[test]
    fn a_broken_schema_is_an_error_not_a_pass() {
        let broken = json!({"type": "not-a-type"});
        assert!(matches!(
            schema_issues(&broken, &json!({})),
            Err(Error::Config(_))
        ));
    }

    #[test]
    fn issues_display_as_location_and_message() {
        assert_eq!(Issue::new("body", "x").to_string(), "body: x");
    }
}
