//! BJ1, the worker wire codec for runtimes that cannot read JSON numbers
//! (docs/architecture/delve-in-bend.md): JSON text whose number tokens are
//! decimal `0..2^32` and `-(2^32-1)..-1`, `w` + 16 hex for `2^32..2^64`, and
//! `f` + 8 hex of the binary32 bits for every float.
use serde_json::{Map, Number, Value};
use std::io::Write;

const WORD: u64 = 1 << 32;
const DEPTH: usize = 128;

pub fn to_string(v: &Value) -> Result<String, String> {
    let mut out = Vec::new();
    write(v, &mut out)?;
    String::from_utf8(out).map_err(|e| e.to_string())
}
fn write(v: &Value, out: &mut Vec<u8>) -> Result<(), String> {
    match v {
        Value::Null => out.extend_from_slice(b"null"),
        Value::Bool(b) => out.extend_from_slice(if *b { b"true" } else { b"false" }),
        Value::Number(n) => number(n, out)?,
        Value::String(s) => serde_json::to_writer(&mut *out, s).map_err(|e| e.to_string())?,
        Value::Array(items) => {
            out.push(b'[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(b',');
                }
                write(item, out)?;
            }
            out.push(b']');
        }
        Value::Object(map) => {
            out.push(b'{');
            for (i, (key, item)) in map.iter().enumerate() {
                if i > 0 {
                    out.push(b',');
                }
                serde_json::to_writer(&mut *out, key).map_err(|e| e.to_string())?;
                out.push(b':');
                write(item, out)?;
            }
            out.push(b'}');
        }
    }
    Ok(())
}
fn number(n: &Number, out: &mut Vec<u8>) -> Result<(), String> {
    let written = if let Some(u) = n.as_u64() {
        if u < WORD {
            write!(out, "{u}")
        } else {
            write!(out, "w{u:016x}")
        }
    } else if let Some(i) = n.as_i64() {
        if i <= -(WORD as i64) {
            return Err(format!("bj1 cannot carry {i}"));
        }
        write!(out, "{i}")
    } else {
        let f = n
            .as_f64()
            .filter(|f| f.is_finite())
            .ok_or("bj1 cannot carry a non-finite float")?;
        if f64::from(f as f32) != f {
            return Err(format!("bj1 floats are binary32; {f} is not"));
        }
        write!(out, "f{:08x}", (f as f32).to_bits())
    };
    written.map_err(|e| e.to_string())
}

/// Reads BJ1 text; JSON whitespace between tokens is tolerated.
pub fn from_str(s: &str) -> Result<Value, String> {
    let mut parser = Parser { s, i: 0 };
    let v = parser.value(0)?;
    parser.ws();
    if parser.i != s.len() {
        return Err(parser.err("trailing text"));
    }
    Ok(v)
}
struct Parser<'a> {
    s: &'a str,
    i: usize,
}
impl Parser<'_> {
    fn err(&self, what: &str) -> String {
        format!("bj1: {what} at byte {}", self.i)
    }
    fn peek(&self) -> Option<u8> {
        self.s.as_bytes().get(self.i).copied()
    }
    fn ws(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\t' | b'\n' | b'\r')) {
            self.i += 1;
        }
    }
    fn expect(&mut self, c: u8) -> Result<(), String> {
        self.ws();
        if self.peek() != Some(c) {
            return Err(self.err(&format!("expected '{}'", c as char)));
        }
        self.i += 1;
        Ok(())
    }
    /// After an item: `true` on a comma, `false` on the closing bracket.
    fn more(&mut self, close: u8) -> Result<bool, String> {
        self.ws();
        let comma = self.peek() == Some(b',');
        if !comma && self.peek() != Some(close) {
            return Err(self.err("expected ',' or the closing bracket"));
        }
        self.i += 1;
        Ok(comma)
    }
    fn value(&mut self, depth: usize) -> Result<Value, String> {
        if depth > DEPTH {
            return Err(self.err("nesting too deep"));
        }
        self.ws();
        match self.peek().ok_or_else(|| self.err("unexpected end"))? {
            b'[' => {
                self.i += 1;
                let mut items = Vec::new();
                self.ws();
                if self.peek() == Some(b']') {
                    self.i += 1;
                } else {
                    while {
                        items.push(self.value(depth + 1)?);
                        self.more(b']')?
                    } {}
                }
                Ok(Value::Array(items))
            }
            b'{' => {
                self.i += 1;
                let mut map = Map::new();
                self.ws();
                if self.peek() == Some(b'}') {
                    self.i += 1;
                } else {
                    while {
                        self.ws();
                        let key = self.string()?;
                        self.expect(b':')?;
                        map.insert(key, self.value(depth + 1)?);
                        self.more(b'}')?
                    } {}
                }
                Ok(Value::Object(map))
            }
            b'"' => self.string().map(Value::String),
            _ => self.token(),
        }
    }
    fn string(&mut self) -> Result<String, String> {
        if self.peek() != Some(b'"') {
            return Err(self.err("expected a string"));
        }
        let (bytes, start) = (self.s.as_bytes(), self.i);
        let mut end = start + 1;
        while end < bytes.len() && bytes[end] != b'"' {
            end += if bytes[end] == b'\\' { 2 } else { 1 };
        }
        if end >= bytes.len() {
            return Err(self.err("unterminated string"));
        }
        self.i = end + 1;
        serde_json::from_str(&self.s[start..=end]).map_err(|e| format!("bj1: {e}"))
    }
    fn token(&mut self) -> Result<Value, String> {
        let rest = &self.s[self.i..];
        let len = rest.find([',', ']', '}', ' ', '\t', '\n', '\r']);
        let token = &rest[..len.unwrap_or(rest.len())];
        let value = match token {
            "null" => Some(Value::Null),
            "true" => Some(Value::Bool(true)),
            "false" => Some(Value::Bool(false)),
            _ => word(token),
        };
        let value = value.ok_or_else(|| self.err(&format!("bad token '{token}'")))?;
        self.i += token.len();
        Ok(value)
    }
}
fn hex(digits: &str, len: usize) -> Option<u64> {
    (digits.len() == len && digits.bytes().all(|c| c.is_ascii_hexdigit()))
        .then(|| u64::from_str_radix(digits, 16).ok())?
}
fn word(token: &str) -> Option<Value> {
    if let Some(digits) = token.strip_prefix('w') {
        return hex(digits, 16).map(Value::from);
    }
    if let Some(digits) = token.strip_prefix('f') {
        let f = f32::from_bits(hex(digits, 8)? as u32);
        return Number::from_f64(f64::from(f)).map(Value::Number);
    }
    let (negative, digits) = token
        .strip_prefix('-')
        .map_or((false, token), |d| (true, d));
    if digits.is_empty() || !digits.bytes().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let n: u64 = digits
        .parse()
        .ok()
        .filter(|&n| n < WORD && (!negative || n > 0))?;
    Some(if negative {
        Value::from(-(n as i64))
    } else {
        Value::from(n)
    })
}
