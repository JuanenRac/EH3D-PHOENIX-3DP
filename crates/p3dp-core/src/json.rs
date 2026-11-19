//! A small JSON reader and writer (RFC 8259) with no dependency: the answers of the printers and of the server are read with it. Depth and size are limited, and
//! a number that is not finite never comes out of it.

use crate::error::CoreError;
use std::fmt::Write as _;

/// The deepest nesting that is read.
pub const MAX_DEPTH: usize = 64;
/// The largest text that is read (16 MiB).
pub const MAX_BYTES: usize = 16 << 20;

/// A JSON value. An object keeps its keys in the order they came.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Null,
    Bool(bool),
    Num(f64),
    Str(String),
    Arr(Vec<Value>),
    Obj(Vec<(String, Value)>),
}

impl Value {
    /// The member of an object (the first with that key).
    pub fn get(&self, key: &str) -> Option<&Value> {
        match self {
            Value::Obj(m) => m.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    /// A member found by a path of keys, such as `["result", "status", "extruder"]`.
    pub fn path(&self, keys: &[&str]) -> Option<&Value> {
        let mut v = self;
        for k in keys {
            v = v.get(k)?;
        }
        Some(v)
    }

    pub fn at(&self, i: usize) -> Option<&Value> {
        match self {
            Value::Arr(a) => a.get(i),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::Str(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Value::Num(n) => Some(*n),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Value::Bool(b) => Some(*b),
            _ => None,
        }
    }

    pub fn as_array(&self) -> Option<&[Value]> {
        match self {
            Value::Arr(a) => Some(a),
            _ => None,
        }
    }

    /// An object from pairs.
    pub fn object(pairs: Vec<(&str, Value)>) -> Value {
        Value::Obj(pairs.into_iter().map(|(k, v)| (k.to_string(), v)).collect())
    }

    /// The text of the value. A number that is not finite is written as `null`, so the output is always valid JSON.
    pub fn to_json(&self) -> String {
        let mut s = String::new();
        self.write(&mut s);
        s
    }

    fn write(&self, out: &mut String) {
        match self {
            Value::Null => out.push_str("null"),
            Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
            Value::Num(n) => {
                if !n.is_finite() {
                    out.push_str("null");
                } else if n.fract() == 0.0 && n.abs() < 1e15 {
                    let _ = write!(out, "{}", *n as i64);
                } else {
                    let _ = write!(out, "{n}");
                }
            }
            Value::Str(s) => write_string(s, out),
            Value::Arr(a) => {
                out.push('[');
                for (i, v) in a.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    v.write(out);
                }
                out.push(']');
            }
            Value::Obj(m) => {
                out.push('{');
                for (i, (k, v)) in m.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    write_string(k, out);
                    out.push(':');
                    v.write(out);
                }
                out.push('}');
            }
        }
    }
}

fn write_string(s: &str, out: &mut String) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

/// Reads a JSON text. Anything after the value, a nesting deeper than 64, a text over 16 MiB or a malformed value is an error with the position.
pub fn parse(text: &str) -> Result<Value, CoreError> {
    if text.len() > MAX_BYTES {
        return Err(CoreError::Invalid(format!(
            "the JSON text is {} bytes, more than {MAX_BYTES}",
            text.len()
        )));
    }
    let mut p = Parser {
        s: text.as_bytes(),
        i: 0,
    };
    p.ws();
    let v = p.value(0)?;
    p.ws();
    if p.i != p.s.len() {
        return Err(p.err("text after the value"));
    }
    Ok(v)
}

struct Parser<'a> {
    s: &'a [u8],
    i: usize,
}

impl Parser<'_> {
    fn err(&self, what: &str) -> CoreError {
        CoreError::Invalid(format!("JSON, byte {}: {what}", self.i))
    }

    fn ws(&mut self) {
        while self.i < self.s.len() && matches!(self.s[self.i], b' ' | b'\t' | b'\n' | b'\r') {
            self.i += 1;
        }
    }

    fn peek(&self) -> Option<u8> {
        self.s.get(self.i).copied()
    }

    fn eat(&mut self, lit: &str) -> bool {
        if self.s[self.i..].starts_with(lit.as_bytes()) {
            self.i += lit.len();
            true
        } else {
            false
        }
    }

    fn value(&mut self, depth: usize) -> Result<Value, CoreError> {
        if depth > MAX_DEPTH {
            return Err(self.err("nesting too deep"));
        }
        match self.peek() {
            None => Err(self.err("the text ends")),
            Some(b'{') => self.object(depth),
            Some(b'[') => self.array(depth),
            Some(b'"') => Ok(Value::Str(self.string()?)),
            Some(b't') if self.eat("true") => Ok(Value::Bool(true)),
            Some(b'f') if self.eat("false") => Ok(Value::Bool(false)),
            Some(b'n') if self.eat("null") => Ok(Value::Null),
            Some(c) if c == b'-' || c.is_ascii_digit() => self.number(),
            Some(_) => Err(self.err("unexpected character")),
        }
    }

    fn number(&mut self) -> Result<Value, CoreError> {
        let start = self.i;
        if self.peek() == Some(b'-') {
            self.i += 1;
        }
        let digits = |p: &mut Self| {
            let from = p.i;
            while p.peek().is_some_and(|c| c.is_ascii_digit()) {
                p.i += 1;
            }
            p.i > from
        };
        if !digits(self) {
            return Err(self.err("a number needs digits"));
        }
        if self.peek() == Some(b'.') {
            self.i += 1;
            if !digits(self) {
                return Err(self.err("a number needs digits after the point"));
            }
        }
        if matches!(self.peek(), Some(b'e' | b'E')) {
            self.i += 1;
            if matches!(self.peek(), Some(b'+' | b'-')) {
                self.i += 1;
            }
            if !digits(self) {
                return Err(self.err("an exponent needs digits"));
            }
        }
        let text = std::str::from_utf8(&self.s[start..self.i]).map_err(|_| self.err("not text"))?;
        let n: f64 = text.parse().map_err(|_| self.err("not a number"))?;
        if !n.is_finite() {
            return Err(self.err("the number is too large"));
        }
        Ok(Value::Num(n))
    }

    fn hex4(&mut self) -> Result<u32, CoreError> {
        if self.i + 4 > self.s.len() {
            return Err(self.err("a \\u escape is cut short"));
        }
        let t = std::str::from_utf8(&self.s[self.i..self.i + 4])
            .map_err(|_| self.err("bad \\u escape"))?;
        let n = u32::from_str_radix(t, 16).map_err(|_| self.err("bad \\u escape"))?;
        self.i += 4;
        Ok(n)
    }

    fn string(&mut self) -> Result<String, CoreError> {
        self.i += 1; // the opening quote
        let mut out: Vec<u8> = Vec::new();
        loop {
            let Some(c) = self.peek() else {
                return Err(self.err("a string is not closed"));
            };
            self.i += 1;
            match c {
                b'"' => break,
                b'\\' => {
                    let Some(e) = self.peek() else {
                        return Err(self.err("a string is not closed"));
                    };
                    self.i += 1;
                    let ch = match e {
                        b'"' => '"',
                        b'\\' => '\\',
                        b'/' => '/',
                        b'b' => '\u{8}',
                        b'f' => '\u{c}',
                        b'n' => '\n',
                        b'r' => '\r',
                        b't' => '\t',
                        b'u' => {
                            let hi = self.hex4()?;
                            if (0xD800..0xDC00).contains(&hi) {
                                if !self.eat("\\u") {
                                    return Err(self.err("a high surrogate with no low one"));
                                }
                                let lo = self.hex4()?;
                                if !(0xDC00..0xE000).contains(&lo) {
                                    return Err(self.err("a bad low surrogate"));
                                }
                                char::from_u32(0x10000 + ((hi - 0xD800) << 10) + (lo - 0xDC00))
                                    .ok_or_else(|| self.err("a bad code point"))?
                            } else {
                                char::from_u32(hi).ok_or_else(|| self.err("a lone surrogate"))?
                            }
                        }
                        _ => return Err(self.err("an unknown escape")),
                    };
                    let mut buf = [0u8; 4];
                    out.extend_from_slice(ch.encode_utf8(&mut buf).as_bytes());
                }
                c if c < 0x20 => return Err(self.err("a control character in a string")),
                c => out.push(c),
            }
        }
        String::from_utf8(out).map_err(|_| self.err("a string is not valid UTF-8"))
    }

    fn array(&mut self, depth: usize) -> Result<Value, CoreError> {
        self.i += 1;
        let mut items = Vec::new();
        self.ws();
        if self.peek() == Some(b']') {
            self.i += 1;
            return Ok(Value::Arr(items));
        }
        loop {
            self.ws();
            items.push(self.value(depth + 1)?);
            self.ws();
            match self.peek() {
                Some(b',') => self.i += 1,
                Some(b']') => {
                    self.i += 1;
                    return Ok(Value::Arr(items));
                }
                _ => return Err(self.err("expected , or ]")),
            }
        }
    }

    fn object(&mut self, depth: usize) -> Result<Value, CoreError> {
        self.i += 1;
        let mut members = Vec::new();
        self.ws();
        if self.peek() == Some(b'}') {
            self.i += 1;
            return Ok(Value::Obj(members));
        }
        loop {
            self.ws();
            if self.peek() != Some(b'"') {
                return Err(self.err("expected a key"));
            }
            let key = self.string()?;
            self.ws();
            if self.peek() != Some(b':') {
                return Err(self.err("expected :"));
            }
            self.i += 1;
            self.ws();
            members.push((key, self.value(depth + 1)?));
            self.ws();
            match self.peek() {
                Some(b',') => self.i += 1,
                Some(b'}') => {
                    self.i += 1;
                    return Ok(Value::Obj(members));
                }
                _ => return Err(self.err("expected , or }")),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rng::Rng;

    #[test]
    fn reads_what_a_printer_says() {
        let v = parse(r#"{"result":{"status":{"extruder":{"temperature":209.8,"target":210}},"ok":true,"name":"a\u00e9\n"}}"#).unwrap();
        assert_eq!(
            v.path(&["result", "status", "extruder", "target"])
                .and_then(Value::as_f64),
            Some(210.0)
        );
        assert_eq!(
            v.path(&["result", "ok"]).and_then(Value::as_bool),
            Some(true)
        );
        assert_eq!(
            v.path(&["result", "name"]).and_then(Value::as_str),
            Some("aé\n")
        );
        assert!(v.path(&["result", "nothing"]).is_none());
    }

    #[test]
    fn surrogate_pairs_and_arrays() {
        let v = parse(r#"["\ud83d\ude00", [], {}, null, -1.5e2]"#).unwrap();
        assert_eq!(v.at(0).and_then(Value::as_str), Some("😀"));
        assert_eq!(v.at(4).and_then(Value::as_f64), Some(-150.0));
        assert_eq!(v.as_array().map(<[Value]>::len), Some(5));
    }

    #[test]
    fn malformed_text_is_an_error_with_a_position() {
        for bad in [
            "",
            "{",
            "[1,]",
            "{\"a\"}",
            "tru",
            "01x",
            "\"\\q\"",
            "\"\\ud800\"",
            "1 2",
            "{\"a\":1,}",
            "-",
            "1.",
            "1e",
            "\"a\nb\"",
        ] {
            let e = parse(bad);
            assert!(e.is_err(), "{bad:?}");
        }
        assert!(parse("[1,]").unwrap_err().to_string().contains("byte"));
    }

    #[test]
    fn nesting_is_limited() {
        let deep = "[".repeat(MAX_DEPTH + 5) + &"]".repeat(MAX_DEPTH + 5);
        assert!(parse(&deep).is_err());
        let ok = "[".repeat(MAX_DEPTH) + &"]".repeat(MAX_DEPTH);
        assert!(parse(&ok).is_ok());
    }

    #[test]
    fn writing_escapes_and_never_prints_a_nan() {
        let v = Value::object(vec![
            ("a", Value::Str("q\"\\\n\u{1}".to_string())),
            ("n", Value::Num(f64::NAN)),
            ("i", Value::Num(5.0)),
            ("f", Value::Num(0.25)),
            ("l", Value::Arr(vec![Value::Bool(true), Value::Null])),
        ]);
        let t = v.to_json();
        assert_eq!(
            t,
            r#"{"a":"q\"\\\n\u0001","n":null,"i":5,"f":0.25,"l":[true,null]}"#
        );
        assert_eq!(parse(&t).unwrap().get("i"), Some(&Value::Num(5.0)));
    }

    #[test]
    fn round_trip_of_random_trees() {
        let mut r = Rng::new(99);
        for _ in 0..200 {
            let v = random(&mut r, 0);
            let back = parse(&v.to_json()).unwrap();
            assert_eq!(back, v);
        }
    }

    fn random(r: &mut Rng, depth: usize) -> Value {
        match r.below(if depth > 3 { 4 } else { 6 }) {
            0 => Value::Null,
            1 => Value::Bool(r.below(2) == 0),
            2 => Value::Num(r.below(100_000) as f64 / 8.0),
            3 => Value::Str(["", "a", "é", "q\"", "\t"][r.below(5) as usize].to_string()),
            4 => Value::Arr((0..r.below(4)).map(|_| random(r, depth + 1)).collect()),
            _ => Value::Obj(
                (0..r.below(4))
                    .map(|i| (format!("k{i}"), random(r, depth + 1)))
                    .collect(),
            ),
        }
    }

    #[test]
    fn garbage_never_panics() {
        let mut r = Rng::new(5);
        for _ in 0..500 {
            let n = r.below(40) as usize;
            let s: String = (0..n)
                .map(|_| {
                    [
                        '{', '}', '[', ']', '"', ',', ':', '1', 'e', '-', '\\', 'u', 'n', ' ',
                    ][r.below(14) as usize]
                })
                .collect();
            let _ = parse(&s);
        }
    }
}
