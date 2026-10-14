//! Where a hosted delve can be reached: the address it listens on (the
//! tailnet, then the local network, then this machine), the public door in
//! front of it when there is one (`ANGEL_DUNGEON_PUBLIC_URL`: Tailscale
//! Funnel or a relay), and the one link everyone opens, with its QR code.
//!
//! The link is `<door>/#<join key>` (`docs/ENGINE.md`, "Sessions and the
//! link"): a browser opens it as the Delve's page, and a friend's angelX
//! pastes it into `/dungeon join`.

use std::net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket};

/// A private address: loopback, the local network, or a tailnet's.
pub(crate) fn private_address(address: IpAddr) -> bool {
    match address {
        IpAddr::V4(ip) => {
            ip.is_loopback()
                || ip.is_private()
                || (ip.octets()[0] == 100 && (64..128).contains(&ip.octets()[1]))
        }
        IpAddr::V6(ip) => ip.is_loopback() || ip.is_unique_local(),
    }
}

/// This machine's Tailscale address, if it is on a tailnet: the route to
/// Tailscale's own resolver leaves through the tailnet interface. No packet
/// is sent.
pub(crate) fn tailnet_address() -> Option<IpAddr> {
    let socket = UdpSocket::bind("0.0.0.0:0").ok()?;
    // Tailscale's own resolver, the same quad-100 address on every tailnet.
    socket
        .connect(SocketAddr::new(
            IpAddr::V4(Ipv4Addr::new(100, 100, 100, 100)),
            53,
        ))
        .ok()?;
    let ip = socket.local_addr().ok()?.ip();
    matches!(ip, IpAddr::V4(v4) if v4.octets()[0] == 100 && (64..128).contains(&v4.octets()[1]))
        .then_some(ip)
}

/// This machine's private address on its local network, if it has one. No
/// packet is sent: connecting a UDP socket only consults the routing table.
pub(crate) fn lan_address() -> Option<IpAddr> {
    let socket = UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("192.0.2.1:9").ok()?;
    let ip = socket.local_addr().ok()?.ip();
    (private_address(ip) && !ip.is_loopback()).then_some(ip)
}

/// Where a hosted delve listens unless told otherwise: the tailnet, then the
/// local network, then this machine only. The link's key guards it.
pub(crate) fn default_bind(port: u16) -> String {
    let ip = tailnet_address()
        .or_else(lan_address)
        .unwrap_or(IpAddr::V4(Ipv4Addr::LOCALHOST));
    SocketAddr::new(ip, port).to_string()
}

/// An address to host on: numeric, and private. A wildcard or public
/// listener is not opened; a public door goes in front of a private one.
pub(crate) fn bind(text: &str) -> Result<SocketAddr, String> {
    let address: SocketAddr = text.trim().parse().map_err(|_| {
        "use a numeric private address and port, such as 127.0.0.1:8787".to_string()
    })?;
    if !private_address(address.ip()) {
        return Err("bind a loopback, LAN, or Tailscale address; wildcard/public listeners are not supported".into());
    }
    Ok(address)
}

/// The public door in front of this host (`ANGEL_DUNGEON_PUBLIC_URL`), if
/// one is named: an http(s) root URL.
pub(crate) fn public_door() -> Result<Option<String>, String> {
    let Some(raw) = std::env::var("ANGEL_DUNGEON_PUBLIC_URL")
        .ok()
        .filter(|v| !v.trim().is_empty())
    else {
        return Ok(None);
    };
    let url = url::Url::parse(raw.trim())
        .map_err(|_| "ANGEL_DUNGEON_PUBLIC_URL must be an http(s) root URL")?;
    if !matches!(url.scheme(), "http" | "https")
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || url.path() != "/"
    {
        return Err("ANGEL_DUNGEON_PUBLIC_URL must be an http(s) root URL without credentials, a query, or a fragment".into());
    }
    Ok(Some(url.as_str().trim_end_matches('/').to_string()))
}

/// The links to a session listening on `addr` with `key`: the public door's
/// first when there is one, then the address itself.
pub(crate) fn links(addr: SocketAddr, key: &str) -> Vec<String> {
    let mut doors = Vec::new();
    if let Ok(Some(public)) = public_door() {
        doors.push(public);
    }
    doors.push(match addr.ip() {
        IpAddr::V4(ip) => format!("http://{ip}:{}", addr.port()),
        IpAddr::V6(ip) => format!("http://[{ip}]:{}", addr.port()),
    });
    doors
        .iter()
        .map(|door| angel_serve::link::link(door, key))
        .collect()
}

/// What the host is told about who can reach the link.
pub(crate) fn reach(addr: SocketAddr) -> &'static str {
    if matches!(public_door(), Ok(Some(_))) {
        "The first link goes through your public door: anyone with it can join."
    } else if addr.ip().is_loopback() {
        "This link opens only on this machine (no tailnet or local network found); give /dungeon host an address:port your friends can reach."
    } else if tailnet_address() == Some(addr.ip()) {
        "Listening on your tailnet: friends on your Tailscale can join; nothing is opened to the internet."
    } else {
        "Listening on your local network: friends on it can join; nothing is opened to the internet."
    }
}

/// The invitation as the host sees it: the links, what each is for, and the
/// first link's QR code in half blocks.
pub(crate) fn invitation(links: &[String]) -> String {
    let mut text = String::from(
        "One link for everyone: open it in a browser (nothing to install), or paste it into angelX as /dungeon join <link>.\n",
    );
    for link in links {
        text.push_str(&format!("  {link}\n"));
    }
    if let Some(qr) = links
        .first()
        .and_then(|link| angel_serve::link::qr(link, true))
    {
        text.push_str(&qr);
        if !qr.ends_with('\n') {
            text.push('\n');
        }
    }
    text
}
