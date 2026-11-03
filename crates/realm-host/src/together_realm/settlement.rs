//! Durable destination for game-owned bank claims. An uncertain write keeps
//! the same source identity; retry reads the receipt before paying anything.
use super::*;
use angel_delve::rules::host_commands::RealmClaim;
use angel_delve::rules::home_settlement::{
    HomeClaim, HomeResult, HomeCompletion, HomePolicyRequest, MAX_POLICY_RESULT_BYTES,
};
use crate::together_shooter as shooter;
use angel_live::host::workspace_store;

const MAX_SESSIONS: usize = 4096;
const MAX_REALM_BYTES: usize = 16 * 1024 * 1024;
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct HomeCommit {
    result: Option<HomeResult>,
    result_fingerprint: String,
    delivery_fingerprint: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Cursor {
    raid: u64,
    request: u64,
    fingerprint: String,
    before: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    home: Option<HomeCommit>,
}

#[derive(Debug)]
pub struct Settlement {
    pub lines: Vec<String>,
    pub tribute_paid: bool,
    pub affordable: Vec<String>,
    pub applied: bool,
}

struct Disk {
    realm: Realm,
    stamp: String,
}

fn read(path: &Path) -> Result<Option<Disk>, String> {
    use std::io::Read as _;
    let file = match std::fs::File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.to_string()),
    };
    let mut bytes = Vec::new();
    file.take(MAX_REALM_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > MAX_REALM_BYTES {
        return Err("realm checkpoint exceeds byte capacity".into());
    }
    let mut realm: Realm =
        serde_json::from_slice(&bytes).map_err(|error| format!("invalid realm: {error}"))?;
    let stamp = angel_live::host::sha256_hex(&bytes);
    realm.path = Some(path.to_owned());
    *realm.stamp.get_mut() = Some(stamp.clone());
    Ok(Some(Disk { realm, stamp }))
}

fn check_stamp(realm: &Realm, disk: Option<&Disk>) -> Result<(), String> {
    if realm.stamp.borrow().as_deref() != disk.map(|disk| disk.stamp.as_str()) {
        return Err("realm changed on disk; reload it before saving".into());
    }
    Ok(())
}

fn encode(realm: &Realm) -> Result<Vec<u8>, String> {
    // Preserve the realm's existing secret redaction, while receipt identities
    // must round-trip exactly. Prepared game results are normalized before
    // entering a receipt; its identity/counters/hashes are never redacted.
    let mut body = serde_json::to_value(realm).map_err(|e| e.to_string())?;
    body.as_object_mut()
        .ok_or("invalid realm body")?
        .remove("settlements");
    let redacted = angel_live::host::secrets::redact_bytes(
        &serde_json::to_vec(&body).map_err(|e| e.to_string())?,
    );
    let mut body: serde_json::Value =
        serde_json::from_slice(&redacted).map_err(|e| e.to_string())?;
    body.as_object_mut()
        .ok_or("invalid redacted realm body")?
        .insert(
            "settlements".into(),
            serde_json::to_value(&realm.settlements).map_err(|e| e.to_string())?,
        );
    let bytes = serde_json::to_vec_pretty(&body).map_err(|e| e.to_string())?;
    if bytes.len() > MAX_REALM_BYTES {
        return Err("realm checkpoint exceeds byte capacity".into());
    }
    Ok(bytes)
}

fn publish_file(realm: &Realm, path: &Path) -> Result<(), String> {
    let bytes = encode(realm)?;
    workspace_store::write_private_atomic_exact(path, &bytes)?;
    *realm.stamp.borrow_mut() = Some(angel_live::host::sha256_hex(&bytes));
    Ok(())
}

pub(super) fn save(realm: &Realm) -> Result<(), String> {
    if let Some(path) = &realm.path {
        let _guard = workspace_store::try_private_lock(path)?;
        let disk = read(path)?;
        check_stamp(realm, disk.as_ref())?;
        publish_file(realm, path)?;
    }
    realm.publish();
    Ok(())
}

fn affordable(realm: &Realm) -> Vec<String> {
    realm
        .wishes
        .iter()
        .filter(|wish| wish.status == Status::Drafted && realm.treasury.covers(&wish.price))
        .map(|wish| wish.name.clone())
        .collect()
}

impl Realm {
    pub fn home_context(&self) -> angel_delve::rules::home_settlement::HomeContext {
        angel_delve::rules::home_settlement::HomeContext {
            home: self.home.clone(),
            treasury: self.treasury.clone(),
            reclaimed: self.reclaimed.clone(),
            raids_won: self.raids_won,
        }
    }
    pub fn persistent(&self) -> bool {
        self.path.is_some()
    }

    pub fn settle_claim(
        &mut self,
        claim: &RealmClaim,
        host: &str,
    ) -> Result<Settlement, String> {
        if claim.session.is_empty()
            || claim.session.len() > 128
            || claim.request == 0
            || !claim
                .session
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'-' | b'_'))
            || claim.transfer.hauls.len() > 64
            || claim.transfer.reclaimed.len() > 64
            || claim.transfer.names.len() > shooter::MAX_PLAYERS as usize
        {
            return Err("invalid realm claim".into());
        }
        self.transact(
            &claim.session,
            &claim.session,
            claim.raid,
            claim.request,
            claim,
            |realm, cursor| {
                if cursor.home.is_some() {
                    return Err("realm claim stream differs from retained receipt".into());
                }
                Ok(Some(Settlement {
                    lines: Vec::new(),
                    tribute_paid: false,
                    affordable: affordable(realm),
                    applied: false,
                }))
            },
            |candidate, _| {
                let mut result = candidate.apply_transfer(claim, host);
                result.applied = true;
                Ok((result, None))
            },
        )
    }

    pub fn prepare_home_claim(
        &mut self,
        claim: &HomeClaim,
        host: &str,
        unsaved: bool,
        evaluate: impl FnOnce(&HomePolicyRequest) -> Result<HomeResult, String>,
    ) -> Result<HomeResult, String> {
        if !claim.transfer.valid() {
            return Err("invalid home transaction facts".into());
        }
        let key = format!("{}/home", claim.session);
        self.transact(
            &key,
            &claim.session,
            claim.raid,
            claim.request,
            claim,
            |_, cursor| {
                let home = cursor
                    .home
                    .as_ref()
                    .ok_or("home claim stream differs from retained receipt")?;
                home.result.clone().map(Some).ok_or_else(|| {
                    "home result retired; use its committed source delivery receipt".into()
                })
            },
            |candidate, _| {
                let request = HomePolicyRequest {
                    context: candidate.home_context(),
                    transfer: claim.transfer.clone(),
                    host: host.into(),
                    unsaved,
                };
                request.validate()?;
                // This branch runs only for a genuinely unprepared claim.
                // Recovery returns the exact disk result before calling code
                // from any newer implementation generation.
                let raw = evaluate(&request)?;
                request.validate_result(&raw)?;
                // Keep ordinary realm redaction without ever changing receipt
                // identity. The exact normalized result is returned and stored.
                let bytes = serde_json::to_vec(&raw).map_err(|e| e.to_string())?;
                let bytes = angel_live::host::secrets::redact_bytes(&bytes);
                if bytes.len() > MAX_POLICY_RESULT_BYTES {
                    return Err("prepared home result exceeds byte capacity".into());
                }
                let result: HomeResult =
                    serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
                request.validate_result(&result)?;
                candidate.home = result.home.clone();
                candidate.treasury = result.treasury.clone();
                let home = HomeCommit {
                    result: Some(result.clone()),
                    result_fingerprint: fingerprint(&result)?,
                    delivery_fingerprint: None,
                };
                Ok((result, Some(home)))
            },
        )
    }
    /// Called only after world+delivery receipt reached the durable source
    /// checkpoint. Credit one identified refund and retire the bulky result.
    pub fn finish_home_claim(
        &mut self,
        claim: &HomeClaim,
        completed: &HomeCompletion,
    ) -> Result<(), String> {
        if !completed
            .delivered
            .valid_for(&claim.transfer, &completed.result)
        {
            return Err("invalid home delivery/refund receipt".into());
        }
        let key = format!("{}/home", claim.session);
        let result_hash = fingerprint(&completed.result)?;
        let delivery_hash = fingerprint(completed)?;
        self.transact(
            &key,
            &claim.session,
            claim.raid,
            claim.request,
            claim,
            |_, cursor| {
                let home = cursor.home.as_ref().ok_or("home result was not prepared")?;
                if home.result_fingerprint != result_hash {
                    return Err("home result differs from committed payment".into());
                }
                match &home.delivery_fingerprint {
                    Some(previous) if previous == &delivery_hash => Ok(Some(())),
                    Some(_) => Err("home delivery ID reused with different content".into()),
                    None => Ok(None),
                }
            },
            |candidate, previous| {
                let home = previous
                    .and_then(|cursor| cursor.home.as_ref())
                    .ok_or("home result was not prepared")?;
                if home.result.as_ref() != Some(&completed.result) {
                    return Err("home result differs from prepared payment".into());
                }
                candidate.treasury.merge(&completed.delivered.refund);
                Ok((
                    (),
                    Some(HomeCommit {
                        result: None,
                        result_fingerprint: result_hash.clone(),
                        delivery_fingerprint: Some(delivery_hash.clone()),
                    }),
                ))
            },
        )
    }

    fn transact<T>(
        &mut self,
        key: &str,
        session: &str,
        raid: u64,
        request: u64,
        claim: &impl Serialize,
        lookup: impl Fn(&Realm, &Cursor) -> Result<Option<T>, String>,
        apply: impl FnOnce(&mut Realm, Option<&Cursor>) -> Result<(T, Option<HomeCommit>), String>,
    ) -> Result<T, String> {
        if session.is_empty()
            || session.len() > 128
            || request == 0
            || !session
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'-' | b'_'))
        {
            return Err("invalid realm claim identity".into());
        }
        let fingerprint = fingerprint(claim)?;
        let path = self.path.clone();
        let _guard = path
            .as_deref()
            .map(workspace_store::try_private_lock)
            .transpose()?;
        let disk = path.as_deref().map(read).transpose()?.flatten();
        let authoritative = disk.as_ref().map_or(&*self, |disk| &disk.realm);
        let previous = authoritative.settlements.get(key).cloned();
        let mut same = false;
        if let Some(previous) = &previous {
            if previous.raid != raid || previous.request > request {
                return Err("realm claim predates its committed session cursor".into());
            }
            same = previous.request == request;
            if same {
                if previous.fingerprint != fingerprint {
                    return Err("realm claim ID reused with different content".into());
                }
                let replay = lookup(authoritative, previous)?;
                let adopt = disk.as_ref().is_some_and(|disk| {
                    self.stamp.borrow().as_deref() != Some(disk.stamp.as_str())
                });
                if adopt {
                    let local = angel_live::host::sha256_hex(&encode(self)?);
                    if local != previous.before
                        && self.stamp.borrow().as_deref() != Some(local.as_str())
                    {
                        return Err("realm has unsaved changes beside an uncertain claim; reconcile before retrying".into());
                    }
                }
                if let Some(replay) = replay {
                    if let Some(path) = &path {
                        confirm(path)?;
                    }
                    if let Some(disk) = disk {
                        disk.realm.publish();
                        if adopt {
                            *self = disk.realm;
                        }
                    } else {
                        self.publish();
                    }
                    return Ok(replay);
                }
                if adopt {
                    if let Some(disk) = &disk {
                        *self = disk.realm.clone();
                    }
                }
            }
        }
        if path.is_some() {
            check_stamp(self, disk.as_ref())?;
        }
        let known = self
            .settlements
            .keys()
            .any(|existing| existing.split('/').next() == Some(session));
        if !known
            && self
                .settlements
                .keys()
                .map(|key| key.split('/').next().unwrap())
                .collect::<std::collections::BTreeSet<_>>()
                .len()
                >= MAX_SESSIONS
        {
            return Err("realm receipt session capacity reached; no receipt was discarded".into());
        }
        let mut candidate = self.clone();
        let before = angel_live::host::sha256_hex(&encode(self)?);
        let (result, home) = apply(&mut candidate, if same { previous.as_ref() } else { None })?;
        candidate.settlements.insert(
            key.to_owned(),
            Cursor {
                raid,
                request,
                fingerprint,
                before,
                home,
            },
        );
        if let Some(path) = &path {
            publish_file(&candidate, path)?;
        }
        candidate.publish();
        *self = candidate;
        Ok(result)
    }

    fn apply_transfer(&mut self, claim: &RealmClaim, host: &str) -> Settlement {
        let transfer = &claim.transfer;
        let mut lines = Vec::new();
        for haul in &transfer.hauls {
            let name = match transfer.names.get(&haul.hero).map(String::as_str) {
                Some("You") | None if haul.hero == 1 => host.to_owned(),
                Some(name) => name.to_owned(),
                None => format!("Knight {}", haul.hero),
            };
            self.bank(&name, &haul.spoils);
            lines.push(format!(
                "{name} banked {} ({})",
                haul.spoils.label(),
                haul.why
            ));
        }
        for pack in &transfer.reclaimed {
            *self.reclaimed.entry(pack.name().to_owned()).or_default() += 1;
        }
        let tribute = shooter::barony::pay_tribute(
            &mut self.home,
            &mut self.treasury,
            transfer.reclaimed.len(),
        );
        if let Some(paid) = &tribute {
            lines.push(format!("King Brannoc's tribute: {}", paid.label()));
        }
        shooter::holdings::note_realm(
            &mut self.home.holdings,
            &self.reclaimed,
            &self.home.stable.yards.clone(),
        );
        shooter::holdings::note_country(&mut self.home.holdings, &self.home.countryside);
        lines.extend(shooter::holdings::rent_day(
            &mut self.home.holdings,
            transfer.reclaimed.len(),
        ));
        shooter::squires_town::rent_day(&mut self.home.squires_lodge, transfer.reclaimed.len());
        match transfer.triumph {
            Some(shooter::Triumph::Dragon) => {
                self.raids_won += 1;
                self.home.ranch.wins = self.raids_won;
                self.offer_catalog();
            }
            Some(shooter::Triumph::Grail) => self.grails += 1,
            None => {}
        }
        Settlement {
            lines,
            tribute_paid: tribute.is_some(),
            affordable: affordable(self),
            applied: false,
        }
    }
}

fn fingerprint(value: &impl Serialize) -> Result<String, String> {
    Ok(angel_live::host::sha256_hex(
        &serde_json::to_vec(value).map_err(|e| e.to_string())?,
    ))
}
fn confirm(path: &Path) -> Result<(), String> {
    // Open read/write for fsync portability; never truncate the published file.
    std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(path)
        .and_then(|file| file.sync_all())
        .map_err(|e| e.to_string())?;
    #[cfg(unix)]
    std::fs::File::open(
        path.parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new(".")),
    )
    .and_then(|dir| dir.sync_all())
    .map_err(|e| e.to_string())?;
    Ok(())
}
