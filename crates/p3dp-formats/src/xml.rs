//! A small XML tag reader: it walks the tags of a document and gives their names and attributes. It does not build a tree and it never expands an
//! external entity; only the five predefined entities and numeric references are replaced.

use crate::error::FormatError;

/// One tag.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tag {
    /// The name, with its prefix if it has one (`p:path` is an attribute, `m:colorgroup` a name).
    pub name: String,
    pub attrs: Vec<(String, String)>,
    /// `</name>`
    pub closing: bool,
    /// `<name/>`
    pub self_closing: bool,
}

impl Tag {
    /// The value of an attribute.
    pub fn attr(&self, key: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }
}

fn bad(reason: &str) -> FormatError {
    FormatError::malformed("xml", reason)
}

/// Replaces `&amp;`, `&lt;`, `&gt;`, `&quot;`, `&apos;` and `&#N;` / `&#xN;`; anything else is kept as it is.
pub fn unescape(s: &str) -> String {
    if !s.contains('&') {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        rest = &rest[i..];
        if let Some(j) = rest.find(';').filter(|&j| j <= 10) {
            let ent = &rest[1..j];
            let rep = match ent {
                "amp" => Some('&'),
                "lt" => Some('<'),
                "gt" => Some('>'),
                "quot" => Some('"'),
                "apos" => Some('\''),
                _ => ent
                    .strip_prefix("#x")
                    .and_then(|h| u32::from_str_radix(h, 16).ok())
                    .or_else(|| ent.strip_prefix('#').and_then(|d| d.parse().ok()))
                    .and_then(char::from_u32),
            };
            if let Some(c) = rep {
                out.push(c);
                rest = &rest[j + 1..];
                continue;
            }
        }
        out.push('&');
        rest = &rest[1..];
    }
    out.push_str(rest);
    out
}

/// Escapes a text for an attribute or an element.
pub fn escape(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            '"' => o.push_str("&quot;"),
            '\'' => o.push_str("&apos;"),
            c if (c as u32) < 0x20 && c != '\n' && c != '\t' => {}
            c => o.push(c),
        }
    }
    o
}

/// The most tags that one document may have: a model with five million triangles has a few more than that, so the real bound is the size of the entry.
pub const MAX_TAGS: usize = 40_000_000;

/// Reads every tag of a document in order.
pub fn tags(text: &str) -> Result<Vec<Tag>, FormatError> {
    let b = text.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while let Some(off) = text[i..].find('<') {
        i += off;
        let rest = &text[i..];
        if rest.starts_with("<!--") {
            let e = rest
                .find("-->")
                .ok_or_else(|| bad("a comment is not closed"))?;
            i += e + 3;
            continue;
        }
        if rest.starts_with("<![CDATA[") {
            let e = rest
                .find("]]>")
                .ok_or_else(|| bad("a CDATA section is not closed"))?;
            i += e + 3;
            continue;
        }
        if rest.starts_with("<?") {
            let e = rest
                .find("?>")
                .ok_or_else(|| bad("a processing instruction is not closed"))?;
            i += e + 2;
            continue;
        }
        if rest.starts_with("<!") {
            if rest.contains("<!ENTITY")
                && rest[..rest.find('>').unwrap_or(rest.len())].contains("<!ENTITY")
            {
                return Err(FormatError::unsupported("xml", "an entity declaration"));
            }
            let e = rest
                .find('>')
                .ok_or_else(|| bad("a declaration is not closed"))?;
            i += e + 1;
            continue;
        }
        // an ordinary tag
        let mut j = i + 1;
        let closing = b.get(j) == Some(&b'/');
        if closing {
            j += 1;
        }
        let ns = j;
        while j < b.len() && !b[j].is_ascii_whitespace() && b[j] != b'/' && b[j] != b'>' {
            j += 1;
        }
        if j == ns || j >= b.len() {
            return Err(bad("a tag with no name, or cut short"));
        }
        let name = text[ns..j].to_string();
        let mut attrs = Vec::new();
        let mut self_closing = false;
        loop {
            while j < b.len() && b[j].is_ascii_whitespace() {
                j += 1;
            }
            match b.get(j) {
                None => return Err(bad("a tag is not closed")),
                Some(b'>') => {
                    j += 1;
                    break;
                }
                Some(b'/') => {
                    if b.get(j + 1) != Some(&b'>') {
                        return Err(bad("a / that is not at the end of a tag"));
                    }
                    self_closing = true;
                    j += 2;
                    break;
                }
                Some(_) => {
                    let ks = j;
                    while j < b.len()
                        && b[j] != b'='
                        && !b[j].is_ascii_whitespace()
                        && b[j] != b'>'
                        && b[j] != b'/'
                    {
                        j += 1;
                    }
                    let key = text[ks..j].to_string();
                    while j < b.len() && b[j].is_ascii_whitespace() {
                        j += 1;
                    }
                    if b.get(j) != Some(&b'=') {
                        return Err(bad("an attribute with no value"));
                    }
                    j += 1;
                    while j < b.len() && b[j].is_ascii_whitespace() {
                        j += 1;
                    }
                    let q = match b.get(j) {
                        Some(&q) if q == b'"' || q == b'\'' => q,
                        _ => return Err(bad("an attribute value with no quotes")),
                    };
                    j += 1;
                    let vs = j;
                    while j < b.len() && b[j] != q {
                        j += 1;
                    }
                    if j >= b.len() {
                        return Err(bad("an attribute value is not closed"));
                    }
                    attrs.push((key, unescape(&text[vs..j])));
                    j += 1;
                }
            }
        }
        if out.len() >= MAX_TAGS {
            return Err(bad("too many tags"));
        }
        out.push(Tag {
            name,
            attrs,
            closing,
            self_closing,
        });
        i = j;
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_tags_and_attributes() {
        let t = tags("<?xml version=\"1.0\"?><!-- c --><model unit='millimeter'><vertex x=\"1\" y = \"2.5\" z=\"0\"/></model>").unwrap();
        assert_eq!(t.len(), 3);
        assert_eq!(t[0].name, "model");
        assert_eq!(t[0].attr("unit"), Some("millimeter"));
        assert!(t[1].self_closing);
        assert_eq!(t[1].attr("y"), Some("2.5"));
        assert!(t[2].closing);
    }

    #[test]
    fn entities_are_replaced_and_escaped_back() {
        assert_eq!(
            unescape("a &amp; b &lt;&#65;&#x42;&gt; &quot;&apos; &bogus; &"),
            "a & b <AB> \"' &bogus; &"
        );
        assert_eq!(escape("a<b>&\"'"), "a&lt;b&gt;&amp;&quot;&apos;");
        let t = tags("<a k=\"x &amp; y\"/>").unwrap();
        assert_eq!(t[0].attr("k"), Some("x & y"));
    }

    #[test]
    fn a_quoted_greater_than_does_not_end_a_tag() {
        let t = tags("<a k=\"1>2\"><b/></a>").unwrap();
        assert_eq!(t[0].attr("k"), Some("1>2"));
        assert_eq!(t.len(), 3);
    }

    #[test]
    fn malformed_documents_are_errors() {
        for bad in [
            "<a",
            "<a k>",
            "<a k=1/>",
            "<a k=\"1/>",
            "<!-- x",
            "<a / >",
            "<>",
            "<?x",
            "<![CDATA[ x",
        ] {
            assert!(tags(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn an_entity_declaration_is_refused() {
        assert!(tags("<!DOCTYPE a [<!ENTITY x \"y\">]><a/>").is_err());
        assert!(tags("<!DOCTYPE html><a/>").is_ok());
    }

    #[test]
    fn text_between_tags_is_ignored() {
        let t = tags("<a>some text <b/> more</a>").unwrap();
        assert_eq!(t.len(), 3);
    }
}
