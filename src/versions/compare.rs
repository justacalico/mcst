//! Minecraft version ordering. Release versions are numeric tuples
//! (`1.20.4`); snapshots/pre-releases sort by their release target first
//! then by kind: snapshot < pre < rc < release.

use std::cmp::Ordering;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Kind {
    Snapshot,
    Pre,
    Rc,
    Release,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Parsed {
    major: u64,
    minor: u64,
    patch: u64,
    kind: Kind,
    seq: u64,
    /// Raw snapshot string for tie-breaks (`24w14a`).
    raw: String,
}

fn parse(v: &str) -> Parsed {
    let v = v.trim();
    // Release: digits and dots only.
    if v.chars().all(|c| c.is_ascii_digit() || c == '.') {
        let mut it = v.split('.').map(|p| p.parse::<u64>().unwrap_or(0));
        return Parsed {
            major: it.next().unwrap_or(0),
            minor: it.next().unwrap_or(0),
            patch: it.next().unwrap_or(0),
            kind: Kind::Release,
            seq: 0,
            raw: v.to_string(),
        };
    }
    // 1.21-pre3 / 1.21-rc1
    for (suffix, kind) in [("-pre", Kind::Pre), ("-rc", Kind::Rc)] {
        if let Some(idx) = v.find(suffix) {
            let base = parse(&v[..idx]);
            let seq = v[idx + suffix.len()..].parse::<u64>().unwrap_or(0);
            return Parsed { kind, seq, ..base };
        }
    }
    // Snapshot: `24w14a` → approximate to (0,0,0) + raw ordering.
    Parsed {
        major: 0,
        minor: 0,
        patch: 0,
        kind: Kind::Snapshot,
        seq: 0,
        raw: v.to_string(),
    }
}

/// Compare two Minecraft version strings. Snapshots compare lexically among
/// themselves and sort below all releases/pre/rc builds.
pub fn cmp_versions(a: &str, b: &str) -> Ordering {
    let pa = parse(a);
    let pb = parse(b);
    (pa.major, pa.minor, pa.patch)
        .cmp(&(pb.major, pb.minor, pb.patch))
        .then(pa.kind.cmp(&pb.kind))
        .then(pa.seq.cmp(&pb.seq))
        .then(pa.raw.cmp(&pb.raw))
}

/// Newest-first sort for UI lists.
pub fn sort_desc(versions: &mut Vec<String>) {
    versions.sort_by(|a, b| cmp_versions(b, a));
    versions.dedup();
}

/// True if `v` looks like a stable release (`1.21`, `1.20.4`).
pub fn is_release(v: &str) -> bool {
    !v.trim().is_empty() && v.trim().chars().all(|c| c.is_ascii_digit() || c == '.')
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cmp::Ordering::*;

    #[test]
    fn releases_order_numerically() {
        assert_eq!(cmp_versions("1.20.4", "1.20.10"), Less);
        assert_eq!(cmp_versions("1.9", "1.10"), Less);
        assert_eq!(cmp_versions("1.21", "1.21"), Equal);
        assert_eq!(cmp_versions("1.21.1", "1.21"), Greater);
        assert_eq!(cmp_versions("2.0", "1.99"), Greater);
    }

    #[test]
    fn prerelease_ordering() {
        // snapshot < pre < rc < release for same base
        assert_eq!(cmp_versions("24w14a", "1.21-pre1"), Less);
        assert_eq!(cmp_versions("1.21-pre1", "1.21-rc1"), Less);
        assert_eq!(cmp_versions("1.21-rc1", "1.21"), Less);
        assert_eq!(cmp_versions("1.21-pre2", "1.21-pre10"), Less);
    }

    #[test]
    fn snapshots_below_releases() {
        assert_eq!(cmp_versions("24w14a", "1.20"), Less);
        assert_eq!(cmp_versions("25w01a", "24w51b"), Greater);
    }

    #[test]
    fn sort_desc_works() {
        let mut v = vec![
            "1.20".to_string(),
            "1.21".to_string(),
            "1.20.4".to_string(),
            "1.21".to_string(),
        ];
        sort_desc(&mut v);
        assert_eq!(v, vec!["1.21", "1.20.4", "1.20"]);
    }

    #[test]
    fn release_detection() {
        assert!(is_release("1.21"));
        assert!(is_release("1.20.4"));
        assert!(!is_release("24w14a"));
        assert!(!is_release("1.21-pre1"));
        assert!(!is_release(""));
        assert!(!is_release(" "));
    }
}
