//! Minimal N-Triples parser.
//!
//! N-Triples is the line-oriented RDF serialization: one `<s> <p> <o> .` per
//! line, where terms are absolute IRIs (`<...>`), blank nodes (`_:label`) or
//! literals (`"..."` with an optional `^^<datatype>` or `@lang`). We parse the
//! serialization only — there is no triple store and no SPARQL engine; the
//! triples feed the RDF -> property-graph loader in [`super::loader`].

/// An RDF term: an IRI, a blank node, or a literal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Term {
    /// Absolute IRI (without the surrounding angle brackets).
    Iri(String),
    /// Blank-node label (without the `_:` prefix).
    Blank(String),
    /// A literal value with optional datatype IRI and language tag.
    Literal {
        value: String,
        datatype: Option<String>,
        lang: Option<String>,
    },
}

impl Term {
    /// Whether this term is a resource (IRI or blank node) rather than a literal.
    pub fn is_resource(&self) -> bool {
        !matches!(self, Term::Literal { .. })
    }
}

/// One RDF statement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Triple {
    pub subject: Term,
    pub predicate: Term,
    pub object: Term,
}

/// Parse a whole N-Triples document, skipping blank and comment lines and any
/// line that does not parse as a valid triple.
pub fn parse(input: &str) -> impl Iterator<Item = Triple> + '_ {
    input.lines().filter_map(parse_line)
}

/// Parse a single line into a triple, or `None` for blank/comment/malformed
/// lines. The subject must be a resource and the predicate an IRI.
pub fn parse_line(line: &str) -> Option<Triple> {
    let line = line.trim();
    if line.is_empty() || line.starts_with('#') {
        return None;
    }
    let mut i = 0;
    let subject = read_term(line, &mut i)?;
    let predicate = read_term(line, &mut i)?;
    let object = read_term(line, &mut i)?;
    if !subject.is_resource() || !matches!(predicate, Term::Iri(_)) {
        return None;
    }
    Some(Triple {
        subject,
        predicate,
        object,
    })
}

/// Decode N-Triples `UCHAR` escapes (`\uXXXX`, `\UXXXXXXXX`) in an IRI reference
/// to their characters, matching how an RDF store canonicalizes IRIs on load
/// (the SPB generator escapes non-ASCII this way, while other inputs keep raw
/// UTF-8 — without this they would intern as distinct nodes). `UCHAR` is the only
/// escape an IRI permits; all other bytes are copied verbatim.
fn unescape_iri(s: &str) -> String {
    if !s.contains('\\') {
        return s.to_string();
    }
    let b = s.as_bytes();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'\\' && i + 1 < b.len() && (b[i + 1] == b'u' || b[i + 1] == b'U') {
            let n = if b[i + 1] == b'u' { 4 } else { 8 };
            if let Some(ch) = s
                .get(i + 2..i + 2 + n)
                .and_then(|h| u32::from_str_radix(h, 16).ok())
                .and_then(char::from_u32)
            {
                out.push(ch);
                i += 2 + n;
                continue;
            }
        }
        let ch = s[i..].chars().next().unwrap();
        out.push(ch);
        i += ch.len_utf8();
    }
    out
}

/// The local name of an IRI: the substring after the last `#` or `/`.
pub fn local_name(iri: &str) -> &str {
    let after_hash = iri.rsplit('#').next().unwrap_or(iri);
    after_hash.rsplit('/').next().unwrap_or(after_hash)
}

fn skip_ws(b: &[u8], i: &mut usize) {
    while *i < b.len() && (b[*i] == b' ' || b[*i] == b'\t') {
        *i += 1;
    }
}

fn read_term(s: &str, i: &mut usize) -> Option<Term> {
    let b = s.as_bytes();
    skip_ws(b, i);
    if *i >= b.len() {
        return None;
    }
    match b[*i] {
        b'<' => {
            let start = *i + 1;
            let end = s[start..].find('>')? + start;
            *i = end + 1;
            Some(Term::Iri(unescape_iri(&s[start..end])))
        }
        b'_' => {
            *i += 1;
            if *i >= b.len() || b[*i] != b':' {
                return None;
            }
            *i += 1;
            let start = *i;
            while *i < b.len() && is_name_char(b[*i]) {
                *i += 1;
            }
            Some(Term::Blank(s[start..*i].to_string()))
        }
        b'"' => read_literal(s, i),
        _ => None,
    }
}

fn read_literal(s: &str, i: &mut usize) -> Option<Term> {
    let b = s.as_bytes();
    *i += 1; // opening quote
    let mut value = String::new();
    loop {
        if *i >= b.len() {
            return None; // unterminated
        }
        match b[*i] {
            b'\\' => {
                *i += 1;
                let e = *b.get(*i)?;
                match e {
                    b'"' => value.push('"'),
                    b'\\' => value.push('\\'),
                    b'n' => value.push('\n'),
                    b't' => value.push('\t'),
                    b'r' => value.push('\r'),
                    b'u' => {
                        let hex = s.get(*i + 1..*i + 5)?;
                        value.push(char::from_u32(u32::from_str_radix(hex, 16).ok()?)?);
                        *i += 4;
                    }
                    b'U' => {
                        let hex = s.get(*i + 1..*i + 9)?;
                        value.push(char::from_u32(u32::from_str_radix(hex, 16).ok()?)?);
                        *i += 8;
                    }
                    other => value.push(other as char),
                }
                *i += 1;
            }
            b'"' => {
                *i += 1;
                break;
            }
            _ => {
                let ch = s[*i..].chars().next()?;
                value.push(ch);
                *i += ch.len_utf8();
            }
        }
    }

    let (mut datatype, mut lang) = (None, None);
    if b.get(*i) == Some(&b'^') && b.get(*i + 1) == Some(&b'^') {
        *i += 2;
        if let Some(Term::Iri(dt)) = read_term(s, i) {
            datatype = Some(dt);
        } else {
            return None;
        }
    } else if b.get(*i) == Some(&b'@') {
        *i += 1;
        let start = *i;
        while *i < b.len() && is_lang_char(b[*i]) {
            *i += 1;
        }
        lang = Some(s[start..*i].to_string());
    }
    Some(Term::Literal {
        value,
        datatype,
        lang,
    })
}

fn is_name_char(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_' || c == b'-' || c == b'.'
}

fn is_lang_char(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'-'
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_iri_triple() {
        let t = parse_line("<http://ex/a> <http://ex/p> <http://ex/b> .").unwrap();
        assert_eq!(t.subject, Term::Iri("http://ex/a".into()));
        assert_eq!(t.predicate, Term::Iri("http://ex/p".into()));
        assert_eq!(t.object, Term::Iri("http://ex/b".into()));
    }

    #[test]
    fn parses_plain_and_typed_literals() {
        let t = parse_line(r#"<http://ex/a> <http://ex/title> "Hello World" ."#).unwrap();
        assert_eq!(
            t.object,
            Term::Literal {
                value: "Hello World".into(),
                datatype: None,
                lang: None
            }
        );

        let t = parse_line(
            r#"<http://ex/a> <http://ex/lat> "51.5"^^<http://www.w3.org/2001/XMLSchema#double> ."#,
        )
        .unwrap();
        match t.object {
            Term::Literal {
                value, datatype, ..
            } => {
                assert_eq!(value, "51.5");
                assert_eq!(local_name(datatype.as_deref().unwrap()), "double");
            }
            _ => panic!("expected literal"),
        }
    }

    #[test]
    fn parses_lang_tag_and_escapes() {
        let t = parse_line(r#"<http://ex/a> <http://ex/n> "café"@en ."#).unwrap();
        assert_eq!(
            t.object,
            Term::Literal {
                value: "café".into(),
                datatype: None,
                lang: Some("en".into())
            }
        );
        let t = parse_line(r#"<http://ex/a> <http://ex/q> "a \"quote\" and \\slash" ."#).unwrap();
        match t.object {
            Term::Literal { value, .. } => assert_eq!(value, r#"a "quote" and \slash"#),
            _ => panic!(),
        }
    }

    #[test]
    fn parses_blank_nodes() {
        let t = parse_line("_:b0 <http://ex/p> _:b1 .").unwrap();
        assert_eq!(t.subject, Term::Blank("b0".into()));
        assert_eq!(t.object, Term::Blank("b1".into()));
    }

    #[test]
    fn skips_comments_blanks_and_malformed() {
        assert!(parse_line("").is_none());
        assert!(parse_line("   ").is_none());
        assert!(parse_line("# a comment").is_none());
        assert!(parse_line("\"literal subject\" <http://ex/p> <http://ex/b> .").is_none());
    }

    #[test]
    fn local_name_handles_hash_and_slash() {
        assert_eq!(
            local_name("http://www.w3.org/1999/02/22-rdf-syntax-ns#type"),
            "type"
        );
        assert_eq!(local_name("http://dbpedia.org/ontology/Place"), "Place");
        assert_eq!(local_name("http://ex/geo#long"), "long");
    }

    #[test]
    fn parses_multiline_document() {
        let doc = "# header\n<http://ex/a> <http://ex/p> <http://ex/b> .\n\n<http://ex/a> <http://ex/n> \"x\" .\n";
        assert_eq!(parse(doc).count(), 2);
    }
}
