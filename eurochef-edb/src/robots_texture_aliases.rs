//! Proven owner-scoped aliases for Robots local Texture UIDs.
//!
//! Ghidra shows that local resource hashes are array indices in the owning EDB rather
//! than encoded global hashes. Therefore every alias in this table is keyed by both
//! the owning EDB UID and the local Texture UID. The current entries were recovered by
//! a full 179-EDB decoded Texture corpus comparison and then verified against the named
//! global Texture export with SHA-256.

use crate::Hashcode;

pub const HT_TEXTURE_BLANK_WHITE: Hashcode = 0x0600_0010;

const PROVEN_ALIASES: &[(Hashcode, Hashcode, Hashcode)] = &[
    (0x0100_0022, 0x8600_0018, HT_TEXTURE_BLANK_WHITE), // h05_shop
    (0x0100_0071, 0x8600_00D7, HT_TEXTURE_BLANK_WHITE), // m03_hub1
    (0x0100_0072, 0x8600_0087, HT_TEXTURE_BLANK_WHITE), // m03_hub2
    (0x0100_0050, 0x8600_000C, HT_TEXTURE_BLANK_WHITE), // m05_outm
    (0x0100_006F, 0x8600_0054, HT_TEXTURE_BLANK_WHITE), // m08_chas
    (0x0100_001A, 0x8600_0001, HT_TEXTURE_BLANK_WHITE), // m09_chop
    (0x0100_0003, 0x8600_0005, HT_TEXTURE_BLANK_WHITE), // o01_pick
];

pub fn resolve(owner_edb_uid: Hashcode, local_texture_uid: Hashcode) -> Option<Hashcode> {
    if let Some(global_uid) =
        crate::robots_texture_identity::resolve_global(owner_edb_uid, local_texture_uid)
    {
        return Some(global_uid);
    }
    PROVEN_ALIASES
        .iter()
        .find(|(owner, local, _)| *owner == owner_edb_uid && *local == local_texture_uid)
        .map(|(_, _, global)| *global)
}

pub fn entries() -> &'static [(Hashcode, Hashcode, Hashcode)] {
    PROVEN_ALIASES
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_texture_alias_is_owner_scoped() {
        assert_eq!(
            resolve(0x0100_0071, 0x8600_00D7),
            Some(HT_TEXTURE_BLANK_WHITE)
        );
        assert_eq!(resolve(0x0100_0012, 0x8600_00D7), None);
    }
}
