use serde::Serialize;

/// Native `0x00425C70` query flags that affect candidate admission before the
/// geometry-specific narrowphase.
pub const ROBOTS_HIT_QUERY_COARSE_BOUNDS_FLAG: u32 = 0x0000_0080;
/// The same native bit also bypasses source-shape preparation in `0x00425690`.
pub const ROBOTS_HIT_QUERY_ALTERNATE_NARROWPHASE_FLAG: u32 =
    super::hit_query::ROBOTS_HIT_QUERY_BYPASS_SOURCE_SHAPE_FLAG;
pub const ROBOTS_HIT_QUERY_ALLOW_RAW_GROUP1_PEER_FLAG: u32 = 0x0000_2000;
pub const ROBOTS_HIT_QUERY_EXCLUDE_RAW_GROUP1_FLAG: u32 = 0x0002_0000;
pub const ROBOTS_HIT_QUERY_ALLOW_EXPLICIT_SOURCE_FLAG: u32 = 0x0020_0000;

/// `XItem+0x260` is kept deliberately raw. Native code proves the admission
/// rules for values 1 and 2, but not a stable gameplay name for either group.
pub const ROBOTS_HIT_QUERY_RAW_GROUP1: i32 = 1;
pub const ROBOTS_HIT_QUERY_RAW_GROUP2: i32 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RobotsHitQueryNarrowphaseKind {
    /// Native `0x00425EF0`: resolve/query the candidate `HT_AnimDatum_HitArea`
    /// path against the already prepared query shape.
    AnimDatumHitArea,
    /// Native `0x00426170`: alternate contact/query path selected by flag 0x200.
    Alternate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RobotsHitQueryCandidateRejectReason {
    MissingCandidate,
    ExplicitSource,
    AlreadyHitByQuerySerial,
    RawGroup1GloballyExcluded,
    CoarseBoundsMiss,
    RawGroup1Peer,
    RawGroup2Peer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RobotsHitQueryCandidateContext {
    pub flags: u32,
    pub query_serial: u16,
    pub source_raw_group: Option<i32>,
    pub secondary_source_raw_group: Option<i32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RobotsHitQueryCandidateView {
    pub present: bool,
    pub is_source: bool,
    pub is_secondary_source: bool,
    pub raw_group: i32,
    /// Native candidate Handler `+0x37C`. `0x00425C70` copies the current query
    /// serial here after a confirmed hit and rejects an equal serial on later
    /// traversal of the same query.
    pub last_hit_query_serial: u16,
    /// Result of native coarse helper `0x004D65F0`. `true` means the candidate
    /// is outside the query/candidate enclosing bounds and can be rejected
    /// before narrowphase. The host computes this only when flag 0x80 is set.
    pub coarse_bounds_miss: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RobotsHitQueryCandidateDecision {
    Reject(RobotsHitQueryCandidateRejectReason),
    Test(RobotsHitQueryNarrowphaseKind),
}

/// Engine-neutral transcription of the pre-narrowphase gates in native
/// `0x00425C70`.
///
/// The function intentionally does not enumerate XItems or perform geometry.
/// UE5.8 can provide both through its world/collision adapter while preserving
/// the exact Robots admission order and raw-group behavior.
pub fn classify_hit_query_candidate(
    context: RobotsHitQueryCandidateContext,
    candidate: RobotsHitQueryCandidateView,
) -> RobotsHitQueryCandidateDecision {
    if !candidate.present {
        return RobotsHitQueryCandidateDecision::Reject(
            RobotsHitQueryCandidateRejectReason::MissingCandidate,
        );
    }

    if context.flags & ROBOTS_HIT_QUERY_ALLOW_EXPLICIT_SOURCE_FLAG == 0
        && (candidate.is_source || candidate.is_secondary_source)
    {
        return RobotsHitQueryCandidateDecision::Reject(
            RobotsHitQueryCandidateRejectReason::ExplicitSource,
        );
    }

    if candidate.last_hit_query_serial == context.query_serial {
        return RobotsHitQueryCandidateDecision::Reject(
            RobotsHitQueryCandidateRejectReason::AlreadyHitByQuerySerial,
        );
    }

    if context.flags & ROBOTS_HIT_QUERY_EXCLUDE_RAW_GROUP1_FLAG != 0
        && candidate.raw_group == ROBOTS_HIT_QUERY_RAW_GROUP1
    {
        return RobotsHitQueryCandidateDecision::Reject(
            RobotsHitQueryCandidateRejectReason::RawGroup1GloballyExcluded,
        );
    }

    if context.flags & ROBOTS_HIT_QUERY_COARSE_BOUNDS_FLAG != 0 && candidate.coarse_bounds_miss {
        return RobotsHitQueryCandidateDecision::Reject(
            RobotsHitQueryCandidateRejectReason::CoarseBoundsMiss,
        );
    }

    let source_is_group1 = context.source_raw_group == Some(ROBOTS_HIT_QUERY_RAW_GROUP1)
        || context.secondary_source_raw_group == Some(ROBOTS_HIT_QUERY_RAW_GROUP1);
    if context.flags & ROBOTS_HIT_QUERY_ALLOW_RAW_GROUP1_PEER_FLAG == 0
        && candidate.raw_group == ROBOTS_HIT_QUERY_RAW_GROUP1
        && source_is_group1
    {
        return RobotsHitQueryCandidateDecision::Reject(
            RobotsHitQueryCandidateRejectReason::RawGroup1Peer,
        );
    }

    let source_is_group2 = context.source_raw_group == Some(ROBOTS_HIT_QUERY_RAW_GROUP2)
        || context.secondary_source_raw_group == Some(ROBOTS_HIT_QUERY_RAW_GROUP2);
    if candidate.raw_group == ROBOTS_HIT_QUERY_RAW_GROUP2 && source_is_group2 {
        return RobotsHitQueryCandidateDecision::Reject(
            RobotsHitQueryCandidateRejectReason::RawGroup2Peer,
        );
    }

    let narrowphase = if context.flags & ROBOTS_HIT_QUERY_ALTERNATE_NARROWPHASE_FLAG != 0 {
        RobotsHitQueryNarrowphaseKind::Alternate
    } else {
        RobotsHitQueryNarrowphaseKind::AnimDatumHitArea
    };
    RobotsHitQueryCandidateDecision::Test(narrowphase)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn context(
        flags: u32,
        source_group: Option<i32>,
        secondary_group: Option<i32>,
    ) -> RobotsHitQueryCandidateContext {
        RobotsHitQueryCandidateContext {
            flags,
            query_serial: 0x1234,
            source_raw_group: source_group,
            secondary_source_raw_group: secondary_group,
        }
    }

    fn candidate(group: i32) -> RobotsHitQueryCandidateView {
        RobotsHitQueryCandidateView {
            present: true,
            is_source: false,
            is_secondary_source: false,
            raw_group: group,
            last_hit_query_serial: 0x4321,
            coarse_bounds_miss: false,
        }
    }

    #[test]
    fn explicit_sources_are_rejected_unless_native_0x200000_flag_is_set() {
        let mut source = candidate(5);
        source.is_source = true;
        assert_eq!(
            classify_hit_query_candidate(context(0, Some(5), None), source),
            RobotsHitQueryCandidateDecision::Reject(
                RobotsHitQueryCandidateRejectReason::ExplicitSource
            )
        );
        assert_eq!(
            classify_hit_query_candidate(
                context(ROBOTS_HIT_QUERY_ALLOW_EXPLICIT_SOURCE_FLAG, Some(5), None),
                source,
            ),
            RobotsHitQueryCandidateDecision::Test(RobotsHitQueryNarrowphaseKind::AnimDatumHitArea)
        );
    }

    #[test]
    fn query_serial_prevents_the_same_query_from_hitting_candidate_twice() {
        let mut repeated = candidate(5);
        repeated.last_hit_query_serial = 0x1234;
        assert_eq!(
            classify_hit_query_candidate(context(0, None, None), repeated),
            RobotsHitQueryCandidateDecision::Reject(
                RobotsHitQueryCandidateRejectReason::AlreadyHitByQuerySerial
            )
        );
    }

    #[test]
    fn raw_group1_global_and_peer_filters_match_native_flags() {
        let group1 = candidate(ROBOTS_HIT_QUERY_RAW_GROUP1);
        assert_eq!(
            classify_hit_query_candidate(
                context(ROBOTS_HIT_QUERY_EXCLUDE_RAW_GROUP1_FLAG, None, None),
                group1,
            ),
            RobotsHitQueryCandidateDecision::Reject(
                RobotsHitQueryCandidateRejectReason::RawGroup1GloballyExcluded
            )
        );
        assert_eq!(
            classify_hit_query_candidate(
                context(0, Some(ROBOTS_HIT_QUERY_RAW_GROUP1), None),
                group1,
            ),
            RobotsHitQueryCandidateDecision::Reject(
                RobotsHitQueryCandidateRejectReason::RawGroup1Peer
            )
        );
        assert_eq!(
            classify_hit_query_candidate(
                context(
                    ROBOTS_HIT_QUERY_ALLOW_RAW_GROUP1_PEER_FLAG,
                    Some(ROBOTS_HIT_QUERY_RAW_GROUP1),
                    None,
                ),
                group1,
            ),
            RobotsHitQueryCandidateDecision::Test(RobotsHitQueryNarrowphaseKind::AnimDatumHitArea)
        );
    }

    #[test]
    fn raw_group2_peer_filter_has_no_flag_bypass_in_native_scan() {
        let group2 = candidate(ROBOTS_HIT_QUERY_RAW_GROUP2);
        assert_eq!(
            classify_hit_query_candidate(
                context(0, None, Some(ROBOTS_HIT_QUERY_RAW_GROUP2)),
                group2,
            ),
            RobotsHitQueryCandidateDecision::Reject(
                RobotsHitQueryCandidateRejectReason::RawGroup2Peer
            )
        );
        assert_eq!(
            classify_hit_query_candidate(context(0, Some(7), None), group2),
            RobotsHitQueryCandidateDecision::Test(RobotsHitQueryNarrowphaseKind::AnimDatumHitArea)
        );
    }

    #[test]
    fn bit_0x80_enables_coarse_bounds_reject_before_narrowphase() {
        let mut outside = candidate(5);
        outside.coarse_bounds_miss = true;
        assert_eq!(
            classify_hit_query_candidate(context(0, None, None), outside),
            RobotsHitQueryCandidateDecision::Test(RobotsHitQueryNarrowphaseKind::AnimDatumHitArea)
        );
        assert_eq!(
            classify_hit_query_candidate(
                context(ROBOTS_HIT_QUERY_COARSE_BOUNDS_FLAG, None, None),
                outside,
            ),
            RobotsHitQueryCandidateDecision::Reject(
                RobotsHitQueryCandidateRejectReason::CoarseBoundsMiss
            )
        );
    }

    #[test]
    fn bit_0x200_selects_the_alternate_native_narrowphase() {
        assert_eq!(
            classify_hit_query_candidate(
                context(ROBOTS_HIT_QUERY_ALTERNATE_NARROWPHASE_FLAG, None, None),
                candidate(5),
            ),
            RobotsHitQueryCandidateDecision::Test(RobotsHitQueryNarrowphaseKind::Alternate)
        );
    }
}
