//! Bytes on the wire: integers as varints (zigzag for signed), floats as
//! their little-endian bits, text and blobs behind their length. Every
//! message and every tree is written and read with these.

/// Why bytes did not read.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct Malformed;

pub fn put_varint(out: &mut Vec<u8>, mut n: u64) {
    loop {
        let byte = (n & 0x7f) as u8;
        n >>= 7;
        if n == 0 {
            out.push(byte);
            return;
        }
        out.push(byte | 0x80);
    }
}

pub fn zigzag(n: i64) -> u64 {
    ((n << 1) ^ (n >> 63)) as u64
}

pub fn unzigzag(n: u64) -> i64 {
    ((n >> 1) as i64) ^ -((n & 1) as i64)
}

pub fn put_f64(out: &mut Vec<u8>, x: f64) {
    out.extend_from_slice(&x.to_le_bytes());
}

pub fn put_blob(out: &mut Vec<u8>, bytes: &[u8]) {
    put_varint(out, bytes.len() as u64);
    out.extend_from_slice(bytes);
}

pub fn put_str(out: &mut Vec<u8>, text: &str) {
    put_blob(out, text.as_bytes());
}

/// Bytes being read, front to back.
pub struct Bytes<'a> {
    pub rest: &'a [u8],
}

impl<'a> Bytes<'a> {
    pub fn new(rest: &'a [u8]) -> Self {
        Bytes { rest }
    }

    pub fn byte(&mut self) -> Result<u8, Malformed> {
        let (&b, rest) = self.rest.split_first().ok_or(Malformed)?;
        self.rest = rest;
        Ok(b)
    }

    pub fn varint(&mut self) -> Result<u64, Malformed> {
        let mut n = 0u64;
        for shift in (0..64).step_by(7) {
            let b = self.byte()?;
            n |= u64::from(b & 0x7f) << shift;
            if b & 0x80 == 0 {
                return Ok(n);
            }
        }
        Err(Malformed)
    }

    /// A varint that must fit in 32 bits.
    pub fn u32(&mut self) -> Result<u32, Malformed> {
        u32::try_from(self.varint()?).map_err(|_| Malformed)
    }

    /// A zigzag varint that must fit in 32 bits.
    pub fn i32(&mut self) -> Result<i32, Malformed> {
        i32::try_from(unzigzag(self.varint()?)).map_err(|_| Malformed)
    }

    pub fn f64(&mut self) -> Result<f64, Malformed> {
        let raw: [u8; 8] = self.take(8)?.try_into().map_err(|_| Malformed)?;
        Ok(f64::from_le_bytes(raw))
    }

    pub fn take(&mut self, n: usize) -> Result<&'a [u8], Malformed> {
        if n > self.rest.len() {
            return Err(Malformed);
        }
        let (head, rest) = self.rest.split_at(n);
        self.rest = rest;
        Ok(head)
    }

    /// A blob of at most `max` bytes.
    pub fn blob(&mut self, max: usize) -> Result<&'a [u8], Malformed> {
        let n = self.varint()?;
        if n > max as u64 {
            return Err(Malformed);
        }
        self.take(n as usize)
    }

    /// UTF-8 text of at most `max` bytes.
    pub fn str(&mut self, max: usize) -> Result<&'a str, Malformed> {
        std::str::from_utf8(self.blob(max)?).map_err(|_| Malformed)
    }

    /// Everything was read.
    pub fn end(&self) -> Result<(), Malformed> {
        if self.rest.is_empty() { Ok(()) } else { Err(Malformed) }
    }
}
