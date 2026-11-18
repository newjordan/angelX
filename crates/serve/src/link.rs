//! The link a host shares: `<door>/#<join-key>`, its key and seat tokens,
//! the door on the local network, and the QR code drawn in half blocks.

use qrcode::bits::Bits;
use qrcode::{Color, EcLevel, QrCode, Version};
use std::net::{IpAddr, Ipv4Addr, UdpSocket};

const BASE32: &[u8; 32] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";

pub fn random_key() -> String {
    let mut bytes = [0u8; 16];
    getrandom::fill(&mut bytes).expect("the OS gives random bytes");
    base32(&bytes)
}

pub fn base32(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len().div_ceil(5) * 8);
    let (mut acc, mut bits) = (0u32, 0u32);
    for &b in bytes {
        acc = (acc << 8) | u32::from(b);
        bits += 8;
        while bits >= 5 {
            bits -= 5;
            out.push(BASE32[(acc >> bits & 31) as usize] as char);
        }
    }
    if bits > 0 {
        out.push(BASE32[(acc << (5 - bits) & 31) as usize] as char);
    }
    out
}

pub fn normalize_key(key: &str) -> String {
    key.trim().to_ascii_uppercase()
}

pub fn same_key(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x.to_ascii_uppercase() ^ y.to_ascii_uppercase())) == 0
}

pub fn lan_ip() -> Option<IpAddr> {
    let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)).ok()?;
    socket.connect((Ipv4Addr::new(192, 0, 2, 1), 9)).ok()?;
    let ip = socket.local_addr().ok()?.ip();
    (!ip.is_unspecified() && !ip.is_loopback()).then_some(ip)
}

pub fn link(door: &str, key: &str) -> String {
    format!("{}/#{key}", door.trim_end_matches('/'))
}

pub fn qr(link: &str, ink_is_light: bool) -> Option<String> {
    let after_scheme = link.split_once("://").map_or(link, |(_, rest)| rest);
    let path = after_scheme.find('/').map_or("", |i| &after_scheme[i..]);
    let text = if path.starts_with("/#") || path.is_empty() { link.to_ascii_uppercase() } else { link.to_string() };
    let code = encode(&text)?;
    let width = code.width();
    let colors = code.to_colors();
    const QUIET: usize = 4;
    let side = width + 2 * QUIET;
    let inked = |x: usize, y: usize| -> bool {
        let dark = x >= QUIET
            && y >= QUIET
            && x < QUIET + width
            && y < QUIET + width
            && colors[(y - QUIET) * width + (x - QUIET)] == Color::Dark;
        dark != ink_is_light
    };
    let mut out = String::with_capacity(side * (side / 2 + 1) * 3);
    for y in (0..side).step_by(2) {
        for x in 0..side {
            let top = inked(x, y);
            let bottom = y + 1 < side && inked(x, y + 1);
            out.push(match (top, bottom) {
                (true, true) => '█',
                (true, false) => '▀',
                (false, true) => '▄',
                (false, false) => ' ',
            });
        }
        out.push('\n');
    }
    Some(out)
}

fn alphanumeric(b: u8) -> bool {
    b.is_ascii_digit() || b.is_ascii_uppercase() || b" $%*+-./:".contains(&b)
}

fn encode(text: &str) -> Option<QrCode> {
    let bytes = text.as_bytes();
    let mut runs: Vec<(bool, &[u8])> = Vec::new();
    let mut start = 0;
    for i in 1..=bytes.len() {
        if i == bytes.len() || alphanumeric(bytes[i]) != alphanumeric(bytes[start]) {
            runs.push((alphanumeric(bytes[start]), &bytes[start..i]));
            start = i;
        }
    }
    (1..=40).find_map(|v| {
        let mut bits = Bits::new(Version::Normal(v));
        for (alnum, run) in &runs {
            let pushed = if *alnum { bits.push_alphanumeric_data(run) } else { bits.push_byte_data(run) };
            pushed.ok()?;
        }
        bits.push_terminator(EcLevel::M).ok()?;
        QrCode::with_bits(bits, EcLevel::M).ok()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base32_matches_the_rfc() {
        for (plain, coded) in [("", ""), ("f", "MY"), ("fo", "MZXQ"), ("foo", "MZXW6"), ("foobar", "MZXW6YTBOI")] {
            assert_eq!(base32(plain.as_bytes()), coded);
        }
        let key = random_key();
        assert_eq!(key.len(), 26);
        assert!(key.bytes().all(|b| BASE32.contains(&b)));
        assert_ne!(key, random_key());
    }

    #[test]
    fn keys_compare_in_any_case() {
        assert!(same_key("ABC234", "abc234"));
        assert!(!same_key("ABC234", "ABC235"));
        assert!(!same_key("ABC", "ABCD"));
    }

    #[test]
    fn a_link_puts_the_key_in_the_fragment() {
        assert_eq!(link("http://10.0.0.5:8790/", "KEY"), "http://10.0.0.5:8790/#KEY");
        assert_eq!(link("https://castle.ts.net", "KEY"), "https://castle.ts.net/#KEY");
    }

    #[test]
    fn the_qr_code_is_square_half_blocks_with_a_quiet_zone() {
        let text = qr(&link("http://192.168.1.20:8790", &random_key()), true).unwrap();
        let lines: Vec<&str> = text.lines().collect();
        let side = lines[0].chars().count();
        assert!(lines.iter().all(|l| l.chars().count() == side));
        assert_eq!(lines.len(), side.div_ceil(2));
        assert!(lines[0].chars().all(|c| c == '█'));
        assert_eq!(side, 29 + 8);
        for _ in 0..50 {
            let text = qr(&link("http://192.168.100.200:8790", &random_key()), true).unwrap();
            assert_eq!(text.lines().next().unwrap().chars().count(), 29 + 8);
        }
        let dark_terminal_off = qr("http://x/#K", false).unwrap();
        assert!(dark_terminal_off.lines().next().unwrap().chars().all(|c| c == ' '));
    }
}
