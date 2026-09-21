use serde::Serialize;

use super::events::RobotsScriptEventView;

pub const ROBOTS_HIT_QUERY_INVALID_SELECTOR: u32 = u32::MAX;
pub const ROBOTS_HIT_QUERY_MIN_BUDGET: f32 = 1.0;
pub const ROBOTS_HIT_QUERY_ACTIVE_EPSILON: f32 = 0.001;
pub const ROBOTS_HIT_QUERY_SOURCE_SHAPE_CACHED_FLAG: u32 = 0x0000_0001;
pub const ROBOTS_HIT_QUERY_BYPASS_SOURCE_SHAPE_FLAG: u32 = 0x0000_0200;

/// Engine-neutral initialization contract for the native 0x98-byte query record.
///
/// `0x00425990` consumes Event arg0 from command+0x14 as the AnimDatum selector
/// and Event arg1 from command+0x18 as a scalar that is doubled before the
/// common minimum-1.0 clamp. The resulting value is a normalized query-update
/// budget, not seconds: `0x00425690` subtracts runtime scale `0x00620034`.
/// Writer `0x004F8F5F` proves that scale is `base_rate(60) / runtime_rate`, so it
/// is 1.0 at the standard 60-rate configuration. The separate fixed-step 1/60
/// constant lives at `0x005DD93C`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct RobotsHitQueryInitPlan {
    pub selector: u32,
    pub remaining_budget: f32,
    pub initial_flags: u32,
}

impl RobotsHitQueryInitPlan {
    pub fn from_event(event: RobotsScriptEventView<'_>) -> Option<Self> {
        let selector = event.native_arg_word(0)?;
        let scalar = event.native_arg_float(1)?;
        Some(Self::direct(selector, scalar + scalar, 0))
    }

    /// `0x00425A70` direct/common initializer.
    pub fn direct(selector: u32, requested_budget: f32, initial_flags: u32) -> Self {
        let remaining_budget = if selector == ROBOTS_HIT_QUERY_INVALID_SELECTOR {
            0.0
        } else if requested_budget >= ROBOTS_HIT_QUERY_MIN_BUDGET {
            requested_budget
        } else {
            ROBOTS_HIT_QUERY_MIN_BUDGET
        };
        Self {
            selector,
            remaining_budget,
            initial_flags: if selector == ROBOTS_HIT_QUERY_INVALID_SELECTOR {
                0
            } else {
                initial_flags
            },
        }
    }

    pub fn instantiate(
        self,
        source_present: bool,
        secondary_source_present: bool,
        serial: u16,
    ) -> RobotsHitQueryState {
        let enabled = self.selector != ROBOTS_HIT_QUERY_INVALID_SELECTOR;
        RobotsHitQueryState {
            selector: if enabled {
                self.selector
            } else {
                ROBOTS_HIT_QUERY_INVALID_SELECTOR
            },
            remaining_budget: if enabled { self.remaining_budget } else { 0.0 },
            flags: if enabled { self.initial_flags } else { 0 },
            source_present: enabled && source_present,
            secondary_source_present: enabled && secondary_source_present,
            hit_present: false,
            serial,
            hit_metadata: 0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RobotsHitQueryStepKind {
    Inactive,
    InvalidSelector,
    SourceShapeUnavailable,
    ScannedMiss,
    ScannedHit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RobotsHitQueryStepResult {
    pub kind: RobotsHitQueryStepKind,
    pub expired_after_scan: bool,
}

/// Proven logical fields of the native 0x98-byte hit-query record. Pointer and
/// geometry payloads stay in the engine adapter; the shared layer owns only the
/// deterministic lifetime/result state that is independent of the renderer or
/// collision backend.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct RobotsHitQueryState {
    /// Native +0x84, -1 when inactive.
    pub selector: u32,
    /// Native +0x74. Unit is deliberately not named until all runtime callers
    /// of mutable scale 0x00620034 are closed.
    pub remaining_budget: f32,
    /// Native +0x8C raw flags. Bit0 is set after a resolved source shape;
    /// bit0x200 bypasses source-shape resolution before candidate scanning.
    pub flags: u32,
    /// Logical stand-in for native +0x44.
    pub source_present: bool,
    /// Logical stand-in for native +0x48.
    pub secondary_source_present: bool,
    /// Logical stand-in for native +0x4C candidate/result pointer.
    pub hit_present: bool,
    /// Native +0x90 16-bit query serial. Full expiry does not clear it.
    pub serial: u16,
    /// Native +0x92 metadata copied from the confirmed HitArea/AttackPoint path.
    pub hit_metadata: u16,
}

impl RobotsHitQueryState {
    pub fn is_active(self) -> bool {
        self.source_present && self.remaining_budget >= ROBOTS_HIT_QUERY_ACTIVE_EPSILON
    }

    pub fn requires_source_shape(self) -> bool {
        self.flags & ROBOTS_HIT_QUERY_BYPASS_SOURCE_SHAPE_FLAG == 0
    }

    /// Native `0x00425690` state transition with geometry/candidate traversal
    /// supplied by the host. A missing source shape returns before spending the
    /// budget. A completed scan spends `budget_step`, then expires with the
    /// native full reset if the remaining value falls below 0.001.
    pub fn step(
        &mut self,
        source_shape_resolved: bool,
        scan_hit: bool,
        hit_metadata: u16,
        budget_step: f32,
    ) -> RobotsHitQueryStepResult {
        if !self.is_active() {
            self.reset_activity_only();
            return RobotsHitQueryStepResult {
                kind: RobotsHitQueryStepKind::Inactive,
                expired_after_scan: false,
            };
        }

        if self.requires_source_shape() {
            if self.selector == ROBOTS_HIT_QUERY_INVALID_SELECTOR {
                return RobotsHitQueryStepResult {
                    kind: RobotsHitQueryStepKind::InvalidSelector,
                    expired_after_scan: false,
                };
            }
            if !source_shape_resolved {
                return RobotsHitQueryStepResult {
                    kind: RobotsHitQueryStepKind::SourceShapeUnavailable,
                    expired_after_scan: false,
                };
            }
        }

        // 0x00425C70 clears +0x4C before traversing candidates.
        self.hit_present = scan_hit;
        self.hit_metadata = if scan_hit { hit_metadata } else { 0 };

        self.remaining_budget -= budget_step;
        let expired_after_scan = self.remaining_budget < ROBOTS_HIT_QUERY_ACTIVE_EPSILON;
        let kind = if scan_hit {
            RobotsHitQueryStepKind::ScannedHit
        } else {
            RobotsHitQueryStepKind::ScannedMiss
        };

        if expired_after_scan {
            self.reset_after_expiry();
        } else if self.requires_source_shape() && source_shape_resolved {
            self.flags |= ROBOTS_HIT_QUERY_SOURCE_SHAPE_CACHED_FLAG;
        }

        RobotsHitQueryStepResult {
            kind,
            expired_after_scan,
        }
    }

    /// Early/inactive `0x00425690` path clears only +0x84/+0x8C/+0x74.
    fn reset_activity_only(&mut self) {
        self.selector = ROBOTS_HIT_QUERY_INVALID_SELECTOR;
        self.flags = 0;
        self.remaining_budget = 0.0;
    }

    /// Post-scan expiry clears the query source/result fields too, while +0x90
    /// serial survives exactly as in native.
    fn reset_after_expiry(&mut self) {
        self.reset_activity_only();
        self.source_present = false;
        self.secondary_source_present = false;
        self.hit_present = false;
        self.hit_metadata = 0;
    }
}

/// Native 16-bit query-id source `DAT_00616DA8` returns the current value then
/// increments it with wrapping short semantics.
pub fn robots_hit_query_allocate_serial(next_serial: &mut u16) -> u16 {
    let serial = *next_serial;
    *next_serial = next_serial.wrapping_add(1);
    serial
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raw_event(args: &[u32]) -> RobotsScriptEventView<'static> {
        let mut words = vec![0xDEAD_BEEF];
        words.extend_from_slice(args);
        let bytes = words
            .into_iter()
            .flat_map(u32::to_le_bytes)
            .collect::<Vec<_>>()
            .into_boxed_slice();
        RobotsScriptEventView {
            event_type: super::super::events::event_type::HIT_CHECK,
            data: Box::leak(bytes),
            start: None,
            length: None,
        }
    }

    #[test]
    fn event_init_skips_raw_plus_10_and_doubles_plus_18_scalar() {
        let plan = RobotsHitQueryInitPlan::from_event(raw_event(&[0x1000_0010, 0.75f32.to_bits()]))
            .unwrap();
        assert_eq!(plan.selector, 0x1000_0010);
        assert_eq!(plan.remaining_budget, 1.5);
        assert_eq!(plan.initial_flags, 0);

        let clamped =
            RobotsHitQueryInitPlan::from_event(raw_event(&[0x1000_0010, 0.25f32.to_bits()]))
                .unwrap();
        assert_eq!(clamped.remaining_budget, 1.0);
    }

    #[test]
    fn invalid_selector_disables_direct_and_event_initializers() {
        let plan = RobotsHitQueryInitPlan::direct(u32::MAX, 99.0, 0xFFFF_FFFF);
        let state = plan.instantiate(true, true, 7);
        assert_eq!(state.selector, u32::MAX);
        assert_eq!(state.remaining_budget, 0.0);
        assert_eq!(state.flags, 0);
        assert!(!state.source_present);
        assert!(!state.secondary_source_present);
    }

    #[test]
    fn missing_source_shape_does_not_spend_query_budget() {
        let mut state =
            RobotsHitQueryInitPlan::direct(0x1000_0009, 3.0, 0).instantiate(true, false, 11);
        let result = state.step(false, false, 0, 1.0);
        assert_eq!(result.kind, RobotsHitQueryStepKind::SourceShapeUnavailable);
        assert_eq!(state.remaining_budget, 3.0);
        assert_eq!(state.flags, 0);
    }

    #[test]
    fn scan_result_spends_host_step_and_caches_source_shape() {
        let mut state =
            RobotsHitQueryInitPlan::direct(0x1000_0009, 3.0, 0).instantiate(true, false, 12);
        let result = state.step(true, true, 0x1234, 1.0);
        assert_eq!(result.kind, RobotsHitQueryStepKind::ScannedHit);
        assert!(!result.expired_after_scan);
        assert_eq!(state.remaining_budget, 2.0);
        assert!(state.hit_present);
        assert_eq!(state.hit_metadata, 0x1234);
        assert_eq!(state.flags & ROBOTS_HIT_QUERY_SOURCE_SHAPE_CACHED_FLAG, 1);
    }

    #[test]
    fn post_scan_expiry_returns_hit_but_clears_record_payload() {
        let mut state =
            RobotsHitQueryInitPlan::direct(0x1000_0009, 1.0, 0).instantiate(true, true, 0xBEEF);
        let result = state.step(true, true, 0x4321, 1.0);
        assert_eq!(result.kind, RobotsHitQueryStepKind::ScannedHit);
        assert!(result.expired_after_scan);
        assert_eq!(state.selector, u32::MAX);
        assert_eq!(state.remaining_budget, 0.0);
        assert_eq!(state.flags, 0);
        assert!(!state.source_present);
        assert!(!state.secondary_source_present);
        assert!(!state.hit_present);
        assert_eq!(state.hit_metadata, 0);
        assert_eq!(state.serial, 0xBEEF);
    }

    #[test]
    fn bypass_flag_allows_scan_without_source_shape_resolution() {
        let mut state = RobotsHitQueryInitPlan::direct(
            0x1000_0009,
            2.0,
            ROBOTS_HIT_QUERY_BYPASS_SOURCE_SHAPE_FLAG,
        )
        .instantiate(true, false, 2);
        let result = state.step(false, false, 0, 0.5);
        assert_eq!(result.kind, RobotsHitQueryStepKind::ScannedMiss);
        assert_eq!(state.remaining_budget, 1.5);
        assert_eq!(state.flags & ROBOTS_HIT_QUERY_SOURCE_SHAPE_CACHED_FLAG, 0);
    }

    #[test]
    fn serial_allocator_matches_wrapping_native_short_increment() {
        let mut next = 0xFFFF;
        assert_eq!(robots_hit_query_allocate_serial(&mut next), 0xFFFF);
        assert_eq!(next, 0);
        assert_eq!(robots_hit_query_allocate_serial(&mut next), 0);
        assert_eq!(next, 1);
    }
}
