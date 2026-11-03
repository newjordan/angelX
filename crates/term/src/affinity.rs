//! The hot read path on the largest last-level cache.
//!
//! A flood of output scrolls lines into a history of ten thousand lines,
//! about 37 MB of cells for a full-screen window, and every line scrolled
//! rewrites the oldest one. Where that history fits in the cache the
//! parser runs at the speed of its own work; where it does not, every
//! line waits on memory. Some processors carry last-level caches of
//! different sizes on different dies (AMD's X3D parts: one die has the
//! stacked cache), and the scheduler may put a thread on either. So the
//! two threads of the hot read path, the pty read-ahead and the parsing,
//! are placed on the CPUs of the largest L3 when the machine has domains
//! of more than one size. Nothing else is placed, and nothing is done on a
//! machine with one L3 (or several of the same size), on another system,
//! when sysfs cannot be read, or when the CPUs glass was given include
//! none of the chosen ones.
//!
//! `GLASS_AFFINITY=off` leaves the threads where the scheduler puts them;
//! `auto`, the default, places them as above.

#[cfg(target_os = "linux")]
use std::path::Path;

/// CPUs past this are not described; `cpu_set_t` holds 1024.
const MAX_CPUS: usize = 1024;

/// One last-level cache and the CPUs that share it.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Domain {
    bytes: u64,
    cpus: Vec<usize>,
}

/// A sysfs CPU list (`0-7,16-23`), or `None` if it is not one. CPUs past
/// [`MAX_CPUS`] are left out.
fn cpu_list(text: &str) -> Option<Vec<usize>> {
    let mut cpus = Vec::new();
    for part in text.trim().split(',') {
        let (first, last) = match part.split_once('-') {
            Some((a, b)) => (a.trim().parse::<usize>().ok()?, b.trim().parse::<usize>().ok()?),
            None => {
                let cpu = part.trim().parse::<usize>().ok()?;
                (cpu, cpu)
            }
        };
        if last < first {
            return None;
        }
        cpus.extend(first.min(MAX_CPUS)..=last.min(MAX_CPUS - 1));
    }
    cpus.sort_unstable();
    cpus.dedup();
    (!cpus.is_empty()).then_some(cpus)
}

/// A sysfs cache size (`98304K`, `32M`, or bytes), or `None`.
fn cache_size(text: &str) -> Option<u64> {
    let text = text.trim();
    let (digits, unit) = match text.char_indices().find(|(_, c)| !c.is_ascii_digit()) {
        Some((at, _)) => text.split_at(at),
        None => (text, ""),
    };
    let scale = match unit {
        "" => 1,
        "K" => 1 << 10,
        "M" => 1 << 20,
        "G" => 1 << 30,
        _ => return None,
    };
    digits.parse::<u64>().ok()?.checked_mul(scale)
}

/// The CPUs to place the hot path on: the largest domains', when some
/// domain is smaller than they are. `None` with one size of domain, or
/// none at all.
fn largest(domains: &[Domain]) -> Option<Vec<usize>> {
    let most = domains.iter().map(|d| d.bytes).max()?;
    if domains.iter().all(|d| d.bytes == most) {
        return None;
    }
    let mut cpus: Vec<usize> = domains.iter().filter(|d| d.bytes == most).flat_map(|d| d.cpus.iter().copied()).collect();
    cpus.sort_unstable();
    cpus.dedup();
    Some(cpus)
}

/// The L3 domains under a sysfs CPU directory (`/sys/devices/system/cpu`):
/// for each online CPU not yet in a domain, the cache of level 3 it lists
/// and the CPUs that share it. Anything missing or malformed ends the
/// reading with what was read before it.
#[cfg(target_os = "linux")]
fn domains(root: &Path) -> Vec<Domain> {
    let read = |path: std::path::PathBuf| std::fs::read_to_string(path).ok();
    let Some(online) = read(root.join("online")).as_deref().and_then(cpu_list) else {
        return Vec::new();
    };
    // Which cache index is level 3 is read from the first CPU, and the
    // same index is read for every other.
    let first = root.join(format!("cpu{}/cache", online[0]));
    let Some(index) = (0..8).find(|i| read(first.join(format!("index{i}/level"))).is_some_and(|l| l.trim() == "3")) else {
        return Vec::new();
    };
    let mut found: Vec<Domain> = Vec::new();
    for cpu in online {
        if found.iter().any(|d| d.cpus.contains(&cpu)) {
            continue;
        }
        let dir = root.join(format!("cpu{cpu}/cache/index{index}"));
        let bytes = read(dir.join("size")).as_deref().and_then(cache_size);
        let cpus = read(dir.join("shared_cpu_list")).as_deref().and_then(cpu_list);
        match (bytes, cpus) {
            (Some(bytes), Some(cpus)) if cpus.contains(&cpu) => found.push(Domain { bytes, cpus }),
            _ => break,
        }
    }
    found
}

/// Whether `GLASS_AFFINITY` leaves the placing on.
fn wanted(setting: Option<&str>) -> bool {
    !matches!(setting.map(str::trim), Some("off" | "0" | "no" | "false"))
}

/// The CPUs this machine's hot path goes to, read once.
#[cfg(target_os = "linux")]
fn chosen() -> Option<&'static [usize]> {
    static CHOSEN: std::sync::OnceLock<Option<Vec<usize>>> = std::sync::OnceLock::new();
    CHOSEN
        .get_or_init(|| {
            wanted(std::env::var("GLASS_AFFINITY").ok().as_deref())
                .then(|| largest(&domains(Path::new("/sys/devices/system/cpu"))))
                .flatten()
        })
        .as_deref()
}

/// Place the calling thread on the largest L3's CPUs, among those it may
/// run on already. Does nothing where the module says it does nothing.
pub fn place_hot_thread() {
    #[cfg(target_os = "linux")]
    if let Some(cpus) = chosen() {
        // SAFETY: both sets are plain bit arrays the calls fill or read,
        // sized as the calls are told; CPU numbers are below MAX_CPUS, the
        // set's own size, so the bit helpers index inside it.
        unsafe {
            let size = std::mem::size_of::<libc::cpu_set_t>();
            let mut allowed: libc::cpu_set_t = std::mem::zeroed();
            if libc::sched_getaffinity(0, size, &mut allowed) != 0 {
                return;
            }
            let mut set: libc::cpu_set_t = std::mem::zeroed();
            let mut any = false;
            for &cpu in cpus.iter().filter(|&&cpu| cpu < MAX_CPUS) {
                if libc::CPU_ISSET(cpu, &allowed) {
                    libc::CPU_SET(cpu, &mut set);
                    any = true;
                }
            }
            if any {
                libc::sched_setaffinity(0, size, &set);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cpu_lists_and_sizes_read_as_sysfs_writes_them() {
        assert_eq!(cpu_list("0-7,16-23\n"), Some((0..8).chain(16..24).collect()));
        assert_eq!(cpu_list("3"), Some(vec![3]));
        assert_eq!(cpu_list("0,2,4-5"), Some(vec![0, 2, 4, 5]));
        for bad in ["", "x", "7-3", "1-", "-1", "0-7,,9", "1.5"] {
            assert_eq!(cpu_list(bad), None, "{bad:?}");
        }
        // A list past what a CPU set holds is cut, never allocated whole.
        assert_eq!(cpu_list("0-18446744073709551615").map(|c| c.len()), Some(MAX_CPUS));
        assert_eq!(cache_size("98304K\n"), Some(96 << 20));
        assert_eq!(cache_size("32M"), Some(32 << 20));
        assert_eq!(cache_size("4096"), Some(4096));
        for bad in ["", "K", "12Q", "1.5M", "99999999999999999999G"] {
            assert_eq!(cache_size(bad), None, "{bad:?}");
        }
    }

    #[test]
    fn only_a_larger_domain_is_chosen() {
        let d = |bytes: u64, cpus: &[usize]| Domain { bytes, cpus: cpus.to_vec() };
        // One die with the stacked cache: its CPUs.
        let x3d = [d(96 << 20, &[0, 1, 16, 17]), d(32 << 20, &[8, 9, 24, 25])];
        assert_eq!(largest(&x3d), Some(vec![0, 1, 16, 17]));
        assert_eq!(largest(&[x3d[1].clone(), x3d[0].clone()]), Some(vec![0, 1, 16, 17]));
        // One L3, or several the same size: nothing to choose.
        assert_eq!(largest(&[d(32 << 20, &[0, 1, 2, 3])]), None);
        assert_eq!(largest(&[d(32 << 20, &[0, 1]), d(32 << 20, &[2, 3])]), None);
        assert_eq!(largest(&[]), None);
        // Two of the larger kind: both.
        assert_eq!(largest(&[d(96, &[4]), d(32, &[0]), d(96, &[2])]), Some(vec![2, 4]));
    }

    #[test]
    fn the_setting_turns_it_off() {
        assert!(wanted(None));
        assert!(wanted(Some("auto")));
        assert!(!wanted(Some("off")));
        assert!(!wanted(Some(" off\n")));
        assert!(!wanted(Some("0")));
    }

    /// A sysfs tree as an X3D part and a one-L3 laptop describe themselves,
    /// and broken ones, read from a directory of files.
    #[cfg(target_os = "linux")]
    #[test]
    fn domains_are_read_from_sysfs() {
        let root = std::env::temp_dir().join(format!("glass-affinity-{}", std::process::id()));
        let tree = |name: &str, online: &str, caches: &[(usize, &str, &str)], level3: usize| {
            let dir = root.join(name);
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(dir.join("online"), online).unwrap();
            for &(cpu, size, list) in caches {
                for index in 0..4 {
                    let at = dir.join(format!("cpu{cpu}/cache/index{index}"));
                    std::fs::create_dir_all(&at).unwrap();
                    let level = if index == level3 { 3 } else if index < 2 { 1 } else { 2 };
                    std::fs::write(at.join("level"), format!("{level}\n")).unwrap();
                    std::fs::write(at.join("size"), if index == level3 { size } else { "48K" }).unwrap();
                    std::fs::write(at.join("shared_cpu_list"), if index == level3 { list } else { "0" }).unwrap();
                }
            }
            dir
        };
        let x3d = tree("x3d", "0-3\n", &[(0, "98304K", "0,2"), (1, "32768K", "1,3"), (2, "98304K", "0,2"), (3, "32768K", "1,3")], 3);
        let found = domains(&x3d);
        assert_eq!(found, vec![Domain { bytes: 96 << 20, cpus: vec![0, 2] }, Domain { bytes: 32 << 20, cpus: vec![1, 3] }]);
        assert_eq!(largest(&found), Some(vec![0, 2]));
        let laptop = tree("laptop", "0-3", &[(0, "12M", "0-3"), (1, "12M", "0-3"), (2, "12M", "0-3"), (3, "12M", "0-3")], 3);
        assert_eq!(largest(&domains(&laptop)), None);
        // The level-3 cache under another index is still found.
        let other = tree("other", "0-1", &[(0, "64M", "0"), (1, "16M", "1")], 2);
        assert_eq!(largest(&domains(&other)), Some(vec![0]));
        // Missing or malformed: nothing, or what was read before it.
        assert_eq!(domains(&root.join("none")), Vec::new());
        let broken = tree("broken", "0-1", &[(0, "lots", "0"), (1, "16M", "1")], 3);
        assert_eq!(domains(&broken), Vec::new());
        let stray = tree("stray", "0-1", &[(0, "64M", "5"), (1, "16M", "1")], 3);
        assert_eq!(domains(&stray), Vec::new());
        let _ = std::fs::remove_dir_all(&root);
    }
}
