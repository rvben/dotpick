//! Dotpath parsing.
//!
//! Grammar (deliberately small):
//! - `.key`           bareword key (`[A-Za-z0-9_-]+`)
//! - `["k.e y"]`      quoted key, for keys containing dots, spaces or brackets
//! - `[0]`            array index (non-negative)
//! - `[]`             iterate every element of an array
//! - `.`              the whole document (root)
//!
//! Segments chain freely: `.a.b[0].c[].d`. Multiple paths are comma-separated
//! at the top level; commas inside `[...]` or quotes are literal.

use crate::error::DotpickError;

/// One step in a dotpath.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Segment {
    /// Object key access.
    Key(String),
    /// Array index access.
    Index(usize),
    /// Iterate every element of an array.
    Iter,
}

/// A parsed dotpath plus its original text (kept for error messages).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Path {
    pub segments: Vec<Segment>,
    pub display: String,
}

impl Path {
    /// The last object-key segment, used to name leaves in `--flat` output.
    pub fn leaf_name(&self) -> Option<&str> {
        self.segments.iter().rev().find_map(|s| match s {
            Segment::Key(k) => Some(k.as_str()),
            _ => None,
        })
    }

    /// Whether this path iterates an array (`[]`), and so can match many values.
    pub fn has_iter(&self) -> bool {
        self.segments.iter().any(|s| matches!(s, Segment::Iter))
    }
}

fn is_bareword(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_' || c == '-'
}

fn syntax(path: &str, message: &str) -> DotpickError {
    DotpickError::PathSyntax {
        path: path.to_string(),
        message: message.to_string(),
    }
}

/// Parse a comma-separated list of dotpaths.
pub fn parse_paths(raw: &str) -> Result<Vec<Path>, DotpickError> {
    let parts = split_top_level(raw);
    if parts.len() == 1 && parts[0].is_empty() {
        return Err(syntax(raw, "no paths given"));
    }
    parts.iter().map(|p| parse_one(p)).collect()
}

/// Split on top-level commas, treating commas inside `[...]` or quotes as literal.
fn split_top_level(raw: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut cur = String::new();
    let mut depth = 0i32;
    let mut quote: Option<char> = None;
    for c in raw.chars() {
        match quote {
            Some(q) => {
                cur.push(c);
                if c == q {
                    quote = None;
                }
            }
            None => match c {
                '"' | '\'' => {
                    quote = Some(c);
                    cur.push(c);
                }
                '[' => {
                    depth += 1;
                    cur.push(c);
                }
                ']' => {
                    depth -= 1;
                    cur.push(c);
                }
                ',' if depth == 0 => {
                    parts.push(cur.trim().to_string());
                    cur.clear();
                }
                _ => cur.push(c),
            },
        }
    }
    parts.push(cur.trim().to_string());
    parts
}

/// Parse a single dotpath.
fn parse_one(raw: &str) -> Result<Path, DotpickError> {
    let display = raw.trim().to_string();
    if display.is_empty() {
        return Err(syntax(raw, "empty path"));
    }
    if display == "." {
        return Ok(Path {
            segments: Vec::new(),
            display,
        });
    }

    let s: Vec<char> = display.chars().collect();
    let n = s.len();
    let mut segments = Vec::new();
    let mut i = 0;

    // Optional leading dot.
    if s[i] == '.' {
        i += 1;
    }

    while i < n {
        match s[i] {
            '.' => {
                i += 1;
                let start = i;
                while i < n && is_bareword(s[i]) {
                    i += 1;
                }
                if i == start {
                    return Err(syntax(&display, "expected a key after '.'"));
                }
                segments.push(Segment::Key(s[start..i].iter().collect()));
            }
            '[' => {
                i += 1;
                if i < n && s[i] == ']' {
                    i += 1;
                    segments.push(Segment::Iter);
                } else if i < n && (s[i] == '"' || s[i] == '\'') {
                    let quote = s[i];
                    i += 1;
                    let start = i;
                    while i < n && s[i] != quote {
                        i += 1;
                    }
                    if i >= n {
                        return Err(syntax(&display, "unterminated quoted key"));
                    }
                    let key: String = s[start..i].iter().collect();
                    i += 1; // closing quote
                    if i >= n || s[i] != ']' {
                        return Err(syntax(&display, "expected ']' after quoted key"));
                    }
                    i += 1;
                    segments.push(Segment::Key(key));
                } else {
                    let start = i;
                    while i < n && s[i].is_ascii_digit() {
                        i += 1;
                    }
                    if i == start {
                        return Err(syntax(&display, "expected an index, '[]' or a quoted key"));
                    }
                    let idx: usize = s[start..i]
                        .iter()
                        .collect::<String>()
                        .parse()
                        .map_err(|_| syntax(&display, "index is too large"))?;
                    if i >= n || s[i] != ']' {
                        return Err(syntax(&display, "expected ']' after index"));
                    }
                    i += 1;
                    segments.push(Segment::Index(idx));
                }
            }
            c if is_bareword(c) => {
                let start = i;
                while i < n && is_bareword(s[i]) {
                    i += 1;
                }
                segments.push(Segment::Key(s[start..i].iter().collect()));
            }
            other => {
                return Err(syntax(&display, &format!("unexpected character {other:?}")));
            }
        }
    }

    Ok(Path { segments, display })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn segs(raw: &str) -> Vec<Segment> {
        parse_paths(raw).unwrap().pop().unwrap().segments
    }

    #[test]
    fn parses_nested_keys() {
        assert_eq!(
            segs(".a.b.c"),
            vec![
                Segment::Key("a".into()),
                Segment::Key("b".into()),
                Segment::Key("c".into())
            ]
        );
    }

    #[test]
    fn leading_dot_is_optional() {
        assert_eq!(segs("a.b"), segs(".a.b"));
    }

    #[test]
    fn parses_index_iter_and_quoted_keys() {
        assert_eq!(
            segs(r#".a[0].b[]["c.d"]"#),
            vec![
                Segment::Key("a".into()),
                Segment::Index(0),
                Segment::Key("b".into()),
                Segment::Iter,
                Segment::Key("c.d".into()),
            ]
        );
    }

    #[test]
    fn root_is_empty_segments() {
        assert!(segs(".").is_empty());
    }

    #[test]
    fn splits_top_level_commas_only() {
        let paths = parse_paths(r#".a,["x,y"].z"#).unwrap();
        assert_eq!(paths.len(), 2);
        assert_eq!(paths[1].segments[0], Segment::Key("x,y".into()));
    }

    #[test]
    fn leaf_name_is_last_key() {
        assert_eq!(parse_paths(".a.b[]").unwrap()[0].leaf_name(), Some("b"));
    }

    #[test]
    fn rejects_trailing_dot() {
        assert!(parse_paths(".a.").is_err());
    }

    #[test]
    fn rejects_empty_path_in_list() {
        assert!(parse_paths(".a,,.b").is_err());
    }

    #[test]
    fn rejects_unterminated_bracket() {
        assert!(parse_paths(".a[0").is_err());
        assert!(parse_paths(r#".a["k]"#).is_err());
    }
}
