//! Immutable runtime files. Prepared payload owners, queued events and active
//! readers lease bytes; the registry itself normally holds weak references.
use super::{IMMUTABLE, Request, head};
use std::collections::BTreeMap;
use std::io;
use std::sync::{Arc, Mutex, Weak};
use std::time::{Duration, Instant};

pub const MAX_FILE: usize = 2 * 1024 * 1024;
const MAX_FILES: usize = 256;
const MAX_BYTES: usize = 64 * 1024 * 1024;
/// After actual socket delivery, allow bounded clients time to fetch media.
/// A queued/writing event retains its own lease independently of this window.
const DELIVERY_WINDOW: Duration = Duration::from_secs(3);

pub struct LiveAsset {
    pub name: String,
    identity: String,
    mime: String,
    data: Box<dyn AsRef<[u8]> + Send + Sync>,
}
impl std::fmt::Debug for LiveAsset {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LiveAsset")
            .field("name", &self.name)
            .field("bytes", &self.bytes().len())
            .finish()
    }
}
impl PartialEq for LiveAsset {
    fn eq(&self, other: &Self) -> bool {
        self.identity == other.identity
            && self.name == other.name
            && self.mime == other.mime
            && self.bytes() == other.bytes()
    }
}
impl Eq for LiveAsset {}
impl LiveAsset {
    /// The caller supplies the SHA-256 identity computed by its immutable
    /// capture. 192 bits fit legacy 64-byte sound names with an extension.
    pub fn prepare(
        identity: &str,
        extension: &str,
        mime: &str,
        data: impl AsRef<[u8]> + Send + Sync + 'static,
    ) -> io::Result<Arc<Self>> {
        if identity.len() != 64
            || !identity
                .bytes()
                .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
            || extension.is_empty()
            || extension.len() > 8
            || !extension
                .bytes()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
            || mime.is_empty()
            || mime.len() > 96
            || mime.bytes().any(|c| c <= b' ' || c >= 127)
            || data.as_ref().is_empty()
            || data.as_ref().len() > MAX_FILE
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "invalid live asset identity/type/size",
            ));
        }
        Ok(Arc::new(Self {
            name: format!("live-{}.{}", &identity[..48], extension),
            identity: identity.into(),
            mime: mime.into(),
            data: Box::new(data),
        }))
    }
    pub fn bytes(&self) -> &[u8] {
        self.data.as_ref().as_ref()
    }
    pub fn response(&self, req: &Request<'_>, keep: bool) -> Vec<u8> {
        let etag = format!("\"{}\"", self.identity);
        if req.header("if-none-match") == Some(etag.as_str()) {
            return head(
                "304 Not Modified",
                &[("Cache-Control", IMMUTABLE), ("ETag", &etag)],
                0,
                keep,
            );
        }
        let mut out = head(
            "200 OK",
            &[
                ("Content-Type", &self.mime),
                ("Cache-Control", IMMUTABLE),
                ("ETag", &etag),
            ],
            self.bytes().len(),
            keep,
        );
        if req.method != "HEAD" {
            out.extend_from_slice(self.bytes());
        }
        out
    }
}
struct Entry {
    owners: Vec<Weak<LiveAsset>>,
    delivery: Option<(Instant, Arc<LiveAsset>)>,
}
#[derive(Clone, Default)]
pub struct LiveAssets(Arc<Mutex<BTreeMap<String, Entry>>>);
impl std::fmt::Debug for LiveAssets {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LiveAssets")
            .field(
                "registered",
                &self.0.lock().unwrap_or_else(|p| p.into_inner()).len(),
            )
            .finish()
    }
}
impl LiveAssets {
    /// Atomic capacity/collision admission, before the caller commits effects.
    pub fn admit(&self, assets: &[Arc<LiveAsset>]) -> io::Result<()> {
        let mut held = self.0.lock().unwrap_or_else(|p| p.into_inner());
        retire(&mut held, Instant::now());
        let mut unique = BTreeMap::<String, Vec<Arc<LiveAsset>>>::new();
        for asset in assets {
            let group = unique.entry(asset.name.clone()).or_default();
            if let Some(old) = group.first() {
                if !Arc::ptr_eq(old, asset) && old != asset {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "live asset identity collision",
                    ));
                }
            }
            if !group.iter().any(|old| Arc::ptr_eq(old, asset)) {
                group.push(Arc::clone(asset));
            }
        }
        for (name, group) in &unique {
            if let Some(old) = held
                .get(name)
                .and_then(|e| e.owners.iter().find_map(Weak::upgrade))
            {
                if !Arc::ptr_eq(&old, &group[0]) && old != group[0] {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "live asset identity collision",
                    ));
                }
            }
        }
        let mut count = held.len();
        let mut bytes: usize = held
            .values()
            .filter_map(|e| e.owners.iter().find_map(Weak::upgrade))
            .map(|a| a.bytes().len())
            .sum();
        let mut owners: usize = held.values().map(|e| e.owners.len()).sum();
        let mut additions = BTreeMap::new();
        for (name, group) in &unique {
            if !held.contains_key(name) {
                count += 1;
                bytes += group[0].bytes().len();
            }
            let added = group
                .iter()
                .filter(|asset| {
                    !held.get(name).is_some_and(|e| {
                        e.owners
                            .iter()
                            .any(|old| old.ptr_eq(&Arc::downgrade(asset)))
                    })
                })
                .map(Arc::downgrade)
                .collect::<Vec<_>>();
            owners += added.len();
            additions.insert(name.clone(), added);
        }
        if count > MAX_FILES || bytes > MAX_BYTES || owners > 4096 {
            return Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "live asset capacity is occupied",
            ));
        }
        for (name, mut added) in additions {
            held.entry(name)
                .or_insert_with(|| Entry {
                    owners: Vec::new(),
                    delivery: None,
                })
                .owners
                .append(&mut added);
        }
        Ok(())
    }
    pub fn get(&self, name: &str) -> Option<Arc<LiveAsset>> {
        self.0
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .get(name)
            .and_then(|e| e.owners.iter().find_map(Weak::upgrade))
    }
    pub(crate) fn delivered(&self, assets: &[Arc<LiveAsset>], now: Instant) {
        let mut held = self.0.lock().unwrap_or_else(|p| p.into_inner());
        for asset in assets {
            if let Some(entry) = held.get_mut(&asset.name) {
                entry.delivery = Some((now + DELIVERY_WINDOW, Arc::clone(asset)));
            }
        }
    }
    pub fn retire(&self, now: Instant) {
        retire(&mut self.0.lock().unwrap_or_else(|p| p.into_inner()), now);
    }
}
fn retire(held: &mut BTreeMap<String, Entry>, now: Instant) {
    for entry in held.values_mut() {
        if entry
            .delivery
            .as_ref()
            .is_some_and(|(until, _)| *until <= now)
        {
            entry.delivery = None;
        }
    }
    for entry in held.values_mut() {
        entry.owners.retain(|owner| owner.strong_count() > 0);
    }
    held.retain(|_, entry| !entry.owners.is_empty());
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn identical_payloads_from_independent_preparations_keep_every_queued_owner() {
        let store = LiveAssets::default();
        let a = LiveAsset::prepare(&"a".repeat(64), "wav", "audio/wav", vec![1; 32]).unwrap();
        let b = LiveAsset::prepare(&"a".repeat(64), "wav", "audio/wav", vec![1; 32]).unwrap();
        store.admit(&[a.clone(), b.clone()]).unwrap();
        let name = a.name.clone();
        drop(b);
        store.retire(Instant::now());
        assert!(store.get(&name).is_some());
        let b = LiveAsset::prepare(&"a".repeat(64), "wav", "audio/wav", vec![1; 32]).unwrap();
        store.admit(&[b.clone()]).unwrap();
        drop(a);
        store.retire(Instant::now());
        assert!(store.get(&name).is_some());
        drop(b);
        store.retire(Instant::now());
        assert!(store.get(&name).is_none());
    }
    #[test]
    fn capacity_pressure_is_atomic_and_can_recover_after_owners_retire() {
        let store = LiveAssets::default();
        let files = (0..257)
            .map(|i| {
                LiveAsset::prepare(
                    &format!("{i:048x}0000000000000000"),
                    "bin",
                    "application/octet-stream",
                    vec![1],
                )
                .unwrap()
            })
            .collect::<Vec<_>>();
        assert_eq!(
            store.admit(&files).unwrap_err().kind(),
            io::ErrorKind::WouldBlock
        );
        assert!(store.get(&files[0].name).is_none());
        store.admit(&files[..256]).unwrap();
        assert!(store.get(&files[0].name).is_some());
        drop(files);
        store.retire(Instant::now());
        let next = LiveAsset::prepare(&"f".repeat(64), "bin", "application/octet-stream", vec![1])
            .unwrap();
        store.admit(&[next]).unwrap();
    }
    #[test]
    fn queued_delivery_and_http_readers_lease_original_bytes_until_retirement() {
        let store = LiveAssets::default();
        let old = LiveAsset::prepare(&"a".repeat(64), "wav", "audio/wav", vec![1; 32]).unwrap();
        let next = LiveAsset::prepare(&"b".repeat(64), "wav", "audio/wav", vec![2; 32]).unwrap();
        store.admit(&[old.clone(), next.clone()]).unwrap();
        let name = old.name.clone();
        let weak = Arc::downgrade(&old);
        let mut events = crate::Events::default();
        events.all(vec![1]);
        events.lease_last(old.clone());
        drop(old);
        assert_eq!(store.get(&name).unwrap().bytes(), &[1; 32]);
        let now = Instant::now();
        store.delivered(&events.list()[0].assets, now);
        drop(events);
        let reader = store.get(&name).unwrap();
        store.retire(now + DELIVERY_WINDOW + Duration::from_secs(1));
        assert!(weak.upgrade().is_some());
        assert_eq!(reader.bytes(), &[1; 32]);
        drop(reader);
        store.retire(now + DELIVERY_WINDOW + Duration::from_secs(1));
        assert!(weak.upgrade().is_none());
        assert!(store.get(&name).is_none());
        assert_eq!(store.get(&next.name).unwrap().bytes(), &[2; 32]);
    }
    #[test]
    fn bad_admission_never_replaces_existing_content_and_head_does_not_send_bytes() {
        let store = LiveAssets::default();
        let first = LiveAsset::prepare(&"a".repeat(64), "wav", "audio/wav", vec![1; 32]).unwrap();
        store.admit(&[first.clone()]).unwrap();
        let bad = LiveAsset::prepare(&"a".repeat(64), "wav", "audio/wav", vec![2; 32]).unwrap();
        assert!(store.admit(&[bad]).is_err());
        assert_eq!(store.get(&first.name).unwrap().bytes(), &[1; 32]);
        let request = Request::parse("HEAD /a/test HTTP/1.1").unwrap();
        let response = first.response(&request, false);
        assert!(response.ends_with(b"\r\n\r\n"));
        assert!(
            String::from_utf8(response)
                .unwrap()
                .contains("Content-Length: 32")
        );
    }
}
