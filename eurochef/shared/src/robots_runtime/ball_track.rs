use serde::Serialize;

pub const ROBOTS_BALL_TRACK_EVENT_CREATE: u32 = 0x0000_0100;
pub const ROBOTS_BALL_TRACK_EVENT_STOP: u32 = 0x0000_0200;
/// `DAT_00620034 * DAT_005DD93C = 1.0 * (1/60)` in the shipped PC runtime.
pub const ROBOTS_BALL_TRACK_FIXED_STEP_SECONDS: f32 = 1.0 / 60.0;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct RobotsBallTrackRuntimeState {
    pub initialized: bool,
    pub owned_track_created: bool,
    pub spawn_stopped: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct RobotsBallTrackEventStep {
    /// Event 0x100 creates the trigger-owned Track XItem only when one is not
    /// already present. UE should realize the actor/component at this boundary.
    pub create_owned_track: bool,
    /// First accepted 0x200 on data[1] > 0 writes Handler +0x33B=1 and leaves
    /// ownership intact, preventing subsequent spawn attempts.
    pub stop_spawning: bool,
    /// Second accepted 0x200 enters common cleanup 0x0047D840 and drops the
    /// trigger's ownership of the Track XItem.
    pub cleanup_owned_track: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RobotsBallTrackSpawnSlot {
    /// Track +0x2F2 counts only loaded entries. A missing live Handler inside that
    /// count is a malformed host projection, so shared fails closed instead of
    /// reproducing the native circular scan's unsafe assumption.
    pub live_handler_present: bool,
    /// Spawned ball Handler +0x314. Zero means this pool slot is available.
    pub in_use: bool,
    /// Pool row +0x06 copied into Track slot +0x08. Native variants 1/2 use the
    /// external `0x0041FDC0` start-parameter path; every other value uses RNG distance.
    pub variant: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RobotsBallTrackStartPlan {
    RandomDistance {
        distance: f32,
    },
    /// Legacy one-iteration preview boundary. Exact burst execution resolves this
    /// through `RobotsBallTrackSpawnParameterRequest::NearestOwnerNodeIndex`.
    ExternalVariant12Parameter,
    /// Final native path parameter committed to the spawned child Handler +0x318.
    ResolvedParameter {
        parameter: f32,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct RobotsBallTrackSpawnPlan {
    pub slot_index: usize,
    pub variant: u16,
    pub path_lane: usize,
    pub path_hashcode: u32,
    pub runtime_speed: f32,
    pub start: RobotsBallTrackStartPlan,
    /// Track child Handler +0x330 is written only outside the strict [-0.1,+0.1]
    /// dead band after the first/last path-lane sign clamps.
    pub auxiliary_scalar_330: Option<f32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RobotsBallTrackSpawnPlanError {
    MalformedLoadedSlot,
    MissingFreeSlotDraw,
    MissingOrdinaryRandomDraws,
    InvalidRandomUnit,
    InvalidPathLane,
    InvalidResolvedParameter,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize)]
pub struct RobotsBallTrackOrdinaryBurstRuntime {
    /// Native `0x004E3F00` carries the last spawned path parameter across iterations
    /// of the same call. A new Track lane service starts from zero.
    pub previous_path_parameter: f32,
    pub spawned_count: u16,
    pub complete: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RobotsBallTrackSpawnParameterRequest {
    AdvanceDistance {
        from_parameter: f32,
        distance: f32,
    },
    /// Pool variants 1/2 call `0x0041FDC0` on the current path controller with
    /// Track owner +0x04 position and use the nearest node index as parameter.
    NearestOwnerNodeIndex,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct RobotsBallTrackSpawnIterationRequest {
    pub slot_index: usize,
    pub variant: u16,
    pub path_lane: usize,
    pub path_hashcode: u32,
    pub runtime_speed: f32,
    pub parameter: RobotsBallTrackSpawnParameterRequest,
    pub auxiliary_scalar_330: Option<f32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct RobotsBallTrackSpawnIterationCommit {
    pub plan: RobotsBallTrackSpawnPlan,
    /// Native continues its do/while only when the newly spawned parameter is
    /// strictly greater than the previous comparison lane. For variants 1/2 that
    /// comparison lane remains zero for the whole call.
    pub continue_burst: bool,
}

impl RobotsBallTrackOrdinaryBurstRuntime {
    pub fn prepare_iteration(
        &self,
        slots: &[RobotsBallTrackSpawnSlot],
        free_slot_draw: Option<u32>,
        path_lane: usize,
        path_lane_count: usize,
        path_hashcode: u32,
        runtime_speed: f32,
        start_distance_min: f32,
        start_distance_max: f32,
        start_unit: Option<f32>,
        auxiliary_unit: Option<f32>,
    ) -> Result<Option<RobotsBallTrackSpawnIterationRequest>, RobotsBallTrackSpawnPlanError> {
        if self.complete {
            return Ok(None);
        }
        let Some(selected_slot) = select_ball_track_free_slot(slots, free_slot_draw)? else {
            return Ok(None);
        };
        let slot = slots[selected_slot];
        if path_lane_count == 0 || path_lane >= path_lane_count {
            return Err(RobotsBallTrackSpawnPlanError::InvalidPathLane);
        }

        let (parameter, auxiliary_scalar_330) = if matches!(slot.variant, 1 | 2) {
            (
                RobotsBallTrackSpawnParameterRequest::NearestOwnerNodeIndex,
                None,
            )
        } else {
            let (Some(start_unit), Some(auxiliary_unit)) = (start_unit, auxiliary_unit) else {
                return Err(RobotsBallTrackSpawnPlanError::MissingOrdinaryRandomDraws);
            };
            if !(0.0..1.0).contains(&start_unit) || !(0.0..1.0).contains(&auxiliary_unit) {
                return Err(RobotsBallTrackSpawnPlanError::InvalidRandomUnit);
            }
            let distance =
                (start_distance_max - start_distance_min) * start_unit + start_distance_min;
            let mut auxiliary = auxiliary_unit * 0.6 - 0.3;
            if path_lane == 0 && auxiliary < 0.0 {
                auxiliary = 0.0;
            }
            if path_lane + 1 == path_lane_count && auxiliary > 0.0 {
                auxiliary = 0.0;
            }
            (
                RobotsBallTrackSpawnParameterRequest::AdvanceDistance {
                    from_parameter: self.previous_path_parameter,
                    distance,
                },
                if auxiliary > 0.1 || auxiliary < -0.1 {
                    Some(auxiliary)
                } else {
                    None
                },
            )
        };

        Ok(Some(RobotsBallTrackSpawnIterationRequest {
            slot_index: selected_slot,
            variant: slot.variant,
            path_lane,
            path_hashcode,
            runtime_speed,
            parameter,
            auxiliary_scalar_330,
        }))
    }

    pub fn commit_iteration(
        &mut self,
        request: RobotsBallTrackSpawnIterationRequest,
        resolved_path_parameter: f32,
        last_node_index: f32,
        path_allows_last_to_first_segment: bool,
    ) -> Result<Option<RobotsBallTrackSpawnIterationCommit>, RobotsBallTrackSpawnPlanError> {
        if !resolved_path_parameter.is_finite() || !last_node_index.is_finite() {
            return Err(RobotsBallTrackSpawnPlanError::InvalidResolvedParameter);
        }
        if !path_allows_last_to_first_segment && last_node_index < resolved_path_parameter {
            self.complete = true;
            return Ok(None);
        }

        let comparison_parameter = if matches!(request.variant, 1 | 2) {
            0.0
        } else {
            self.previous_path_parameter
        };
        let continue_burst = resolved_path_parameter > comparison_parameter;
        self.previous_path_parameter = resolved_path_parameter;
        self.spawned_count = self.spawned_count.wrapping_add(1);
        self.complete = !continue_burst;

        Ok(Some(RobotsBallTrackSpawnIterationCommit {
            plan: RobotsBallTrackSpawnPlan {
                slot_index: request.slot_index,
                variant: request.variant,
                path_lane: request.path_lane,
                path_hashcode: request.path_hashcode,
                runtime_speed: request.runtime_speed,
                start: RobotsBallTrackStartPlan::ResolvedParameter {
                    parameter: resolved_path_parameter,
                },
                auxiliary_scalar_330: request.auxiliary_scalar_330,
            },
            continue_burst,
        }))
    }
}

/// Exact free-ball search `0x004E4360`. The initial slot intentionally uses
/// `draw % (count - 1)`, not `% count`; the last slot can only be reached by the
/// circular scan. This oddity is original behavior, not a typo to sanitize.
pub fn select_ball_track_free_slot(
    slots: &[RobotsBallTrackSpawnSlot],
    free_slot_draw: Option<u32>,
) -> Result<Option<usize>, RobotsBallTrackSpawnPlanError> {
    if slots.iter().any(|slot| !slot.live_handler_present) {
        return Err(RobotsBallTrackSpawnPlanError::MalformedLoadedSlot);
    }
    if slots.is_empty() {
        return Ok(None);
    }
    let mut index = if slots.len() == 1 {
        0
    } else {
        (free_slot_draw.ok_or(RobotsBallTrackSpawnPlanError::MissingFreeSlotDraw)? as usize)
            % (slots.len() - 1)
    };
    let start = index;
    loop {
        if !slots[index].in_use {
            return Ok(Some(index));
        }
        index += 1;
        if index >= slots.len() {
            index = 0;
        }
        if index == start {
            return Ok(None);
        }
    }
}

/// Engine-neutral ordinary/special spawn plan after `0x004E4360` selected a free
/// Track child. Path-distance-to-parameter conversion and live Actor pose application
/// stay with the host's existing XPath runtime; this reducer owns exact RNG math.
pub fn plan_ball_track_spawn_for_slot(
    slots: &[RobotsBallTrackSpawnSlot],
    selected_slot: usize,
    path_lane: usize,
    path_lane_count: usize,
    path_hashcode: u32,
    runtime_speed: f32,
    start_distance_min: f32,
    start_distance_max: f32,
    start_unit: Option<f32>,
    auxiliary_unit: Option<f32>,
) -> Result<RobotsBallTrackSpawnPlan, RobotsBallTrackSpawnPlanError> {
    let Some(slot) = slots.get(selected_slot).copied() else {
        return Err(RobotsBallTrackSpawnPlanError::MalformedLoadedSlot);
    };
    if !slot.live_handler_present || slot.in_use {
        return Err(RobotsBallTrackSpawnPlanError::MalformedLoadedSlot);
    }
    if path_lane_count == 0 || path_lane >= path_lane_count {
        return Err(RobotsBallTrackSpawnPlanError::InvalidPathLane);
    }

    if matches!(slot.variant, 1 | 2) {
        return Ok(RobotsBallTrackSpawnPlan {
            slot_index: selected_slot,
            variant: slot.variant,
            path_lane,
            path_hashcode,
            runtime_speed,
            start: RobotsBallTrackStartPlan::ExternalVariant12Parameter,
            auxiliary_scalar_330: None,
        });
    }

    let (Some(start_unit), Some(auxiliary_unit)) = (start_unit, auxiliary_unit) else {
        return Err(RobotsBallTrackSpawnPlanError::MissingOrdinaryRandomDraws);
    };
    if !(0.0..1.0).contains(&start_unit) || !(0.0..1.0).contains(&auxiliary_unit) {
        return Err(RobotsBallTrackSpawnPlanError::InvalidRandomUnit);
    }

    let distance = (start_distance_max - start_distance_min) * start_unit + start_distance_min;
    let mut auxiliary = auxiliary_unit * 0.6 - 0.3;
    if path_lane == 0 && auxiliary < 0.0 {
        auxiliary = 0.0;
    }
    if path_lane + 1 == path_lane_count && auxiliary > 0.0 {
        auxiliary = 0.0;
    }
    let auxiliary_scalar_330 = if auxiliary > 0.1 || auxiliary < -0.1 {
        Some(auxiliary)
    } else {
        None
    };

    Ok(RobotsBallTrackSpawnPlan {
        slot_index: selected_slot,
        variant: slot.variant,
        path_lane,
        path_hashcode,
        runtime_speed,
        start: RobotsBallTrackStartPlan::RandomDistance { distance },
        auxiliary_scalar_330,
    })
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize)]
pub struct RobotsBallTrackAlternateLaneRuntime {
    pub initialized: bool,
    /// Native lane +0x00, seeded from `path_node_count - 1`.
    pub path_last_node_index: i32,
    /// Native lane +0x04. Rows are consumed backwards and wrap to the last row.
    pub schedule_row_cursor: i32,
    /// Native lane +0x08. Accumulates travelled distance, not wall-clock seconds.
    pub distance_accumulator: f32,
    /// Native lane +0x10. Host path-controller parameter advanced by `-spacing_distance`.
    pub path_parameter: f32,
    /// Native lane +0x14. Incremented whenever the backwards row cursor wraps.
    pub completed_schedule_cycles: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct RobotsBallTrackAlternateRowRequest {
    pub schedule_row_index: usize,
    pub path_parameter: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct RobotsBallTrackAlternateSpawnPlan {
    pub slot_index: usize,
    pub path_lane: usize,
    /// Native `0x004E41A0` always takes the controller/context from lane zero while
    /// binding the spawned child to the current path lane.
    pub controller_path_lane: usize,
    pub path_hashcode: u32,
    pub runtime_speed: f32,
    pub path_parameter: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RobotsBallTrackAlternateError {
    InvalidPathNodeCount,
    MissingScheduleRows,
    InvalidScheduleCursor,
    InvalidPathParameter,
    InvalidSpacingDistance,
    InvalidDistanceDelta,
    InvalidSelectedSlot,
}

impl RobotsBallTrackAlternateLaneRuntime {
    fn initialize(
        &mut self,
        path_node_count: usize,
        schedule_row_count: usize,
    ) -> Result<(), RobotsBallTrackAlternateError> {
        if self.initialized {
            return Ok(());
        }
        if path_node_count < 2 {
            return Err(RobotsBallTrackAlternateError::InvalidPathNodeCount);
        }
        if schedule_row_count == 0 {
            return Err(RobotsBallTrackAlternateError::MissingScheduleRows);
        }
        self.initialized = true;
        self.path_last_node_index = path_node_count as i32 - 1;
        self.schedule_row_cursor = schedule_row_count as i32 - 1;
        self.distance_accumulator = 0.0;
        self.path_parameter = (path_node_count as i32 - 2) as f32;
        self.completed_schedule_cycles = 0;
        Ok(())
    }

    fn prepare_due_row(
        &mut self,
        spacing_distance: f32,
    ) -> Result<Option<RobotsBallTrackAlternateRowRequest>, RobotsBallTrackAlternateError> {
        if !spacing_distance.is_finite() || spacing_distance <= 0.0 {
            return Err(RobotsBallTrackAlternateError::InvalidSpacingDistance);
        }
        if !self.path_parameter.is_finite() {
            return Err(RobotsBallTrackAlternateError::InvalidPathParameter);
        }
        if !(self.path_parameter > 0.0 || self.distance_accumulator > spacing_distance) {
            return Ok(None);
        }
        if self.distance_accumulator > spacing_distance {
            self.distance_accumulator = 0.0;
            self.path_parameter = 0.0;
        }
        if self.schedule_row_cursor < 0 {
            return Err(RobotsBallTrackAlternateError::InvalidScheduleCursor);
        }
        Ok(Some(RobotsBallTrackAlternateRowRequest {
            schedule_row_index: self.schedule_row_cursor as usize,
            path_parameter: self.path_parameter,
        }))
    }

    /// Entry of native `0x004E41A0` for one Track update. `distance_delta` is the
    /// already-resolved `global_step * first_path_speed * 1/60` contribution.
    /// The cycle-limit check intentionally occurs only here; native does not recheck
    /// it inside the same multi-row service loop.
    pub fn begin_tick(
        &mut self,
        spawn_stopped: bool,
        path_node_count: usize,
        schedule_row_count: usize,
        cycle_limit: i32,
        distance_delta: f32,
        spacing_distance: f32,
    ) -> Result<Option<RobotsBallTrackAlternateRowRequest>, RobotsBallTrackAlternateError> {
        if spawn_stopped {
            return Ok(None);
        }
        self.initialize(path_node_count, schedule_row_count)?;
        if self.completed_schedule_cycles >= cycle_limit {
            return Ok(None);
        }
        if !distance_delta.is_finite() {
            return Err(RobotsBallTrackAlternateError::InvalidDistanceDelta);
        }
        self.distance_accumulator += distance_delta;
        self.prepare_due_row(spacing_distance)
    }

    /// Called after UE/path runtime resolves native
    /// `0x00420B60(controller_lane0, current_parameter, -spacing_distance)`.
    pub fn finish_row(
        &mut self,
        next_path_parameter: f32,
        schedule_row_count: usize,
    ) -> Result<(), RobotsBallTrackAlternateError> {
        if !next_path_parameter.is_finite() {
            return Err(RobotsBallTrackAlternateError::InvalidPathParameter);
        }
        if schedule_row_count == 0 {
            return Err(RobotsBallTrackAlternateError::MissingScheduleRows);
        }
        self.path_parameter = next_path_parameter;
        self.schedule_row_cursor -= 1;
        if self.schedule_row_cursor < 0 {
            self.schedule_row_cursor = schedule_row_count as i32 - 1;
            self.completed_schedule_cycles = self.completed_schedule_cycles.wrapping_add(1);
        }
        Ok(())
    }

    /// Continue the same native call after `finish_row`; unlike `begin_tick`, this
    /// does not add another distance delta or recheck the cycle limit.
    pub fn prepare_followup_row(
        &mut self,
        spacing_distance: f32,
    ) -> Result<Option<RobotsBallTrackAlternateRowRequest>, RobotsBallTrackAlternateError> {
        self.prepare_due_row(spacing_distance)
    }
}

pub fn plan_ball_track_alternate_spawn_for_slot(
    slots: &[RobotsBallTrackSpawnSlot],
    selected_slot: usize,
    path_lane: usize,
    path_lane_count: usize,
    path_hashcode: u32,
    first_path_runtime_speed: f32,
    path_parameter: f32,
) -> Result<RobotsBallTrackAlternateSpawnPlan, RobotsBallTrackAlternateError> {
    let Some(slot) = slots.get(selected_slot) else {
        return Err(RobotsBallTrackAlternateError::InvalidSelectedSlot);
    };
    if !slot.live_handler_present || slot.in_use || path_lane >= path_lane_count {
        return Err(RobotsBallTrackAlternateError::InvalidSelectedSlot);
    }
    if !first_path_runtime_speed.is_finite() || !path_parameter.is_finite() {
        return Err(RobotsBallTrackAlternateError::InvalidPathParameter);
    }
    Ok(RobotsBallTrackAlternateSpawnPlan {
        slot_index: selected_slot,
        path_lane,
        controller_path_lane: 0,
        path_hashcode,
        runtime_speed: first_path_runtime_speed,
        path_parameter,
    })
}

impl RobotsBallTrackRuntimeState {
    /// Seed the trigger-side view from an already-existing native XItem lifecycle.
    pub fn ensure_initialized(&mut self, initial_created: bool) {
        if self.initialized {
            return;
        }
        self.initialized = true;
        self.owned_track_created = initial_created;
        self.spawn_stopped = false;
    }

    /// XTrigger_BallTrack +0x6C = 0x00482750 local class-event behavior.
    pub fn dispatch_event(
        &mut self,
        initial_created: bool,
        two_phase_stop: bool,
        event_mask: u32,
    ) -> RobotsBallTrackEventStep {
        self.ensure_initialized(initial_created);
        let mut step = RobotsBallTrackEventStep::default();

        if event_mask & ROBOTS_BALL_TRACK_EVENT_CREATE != 0 && !self.owned_track_created {
            self.owned_track_created = true;
            self.spawn_stopped = false;
            step.create_owned_track = true;
        }

        if event_mask & ROBOTS_BALL_TRACK_EVENT_STOP != 0
            && self.owned_track_created
            && two_phase_stop
        {
            if self.spawn_stopped {
                self.owned_track_created = false;
                self.spawn_stopped = false;
                step.cleanup_owned_track = true;
            } else {
                self.spawn_stopped = true;
                step.stop_spawning = true;
            }
        }

        step
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_event_is_idempotent_and_preserves_owned_track() {
        let mut state = RobotsBallTrackRuntimeState::default();
        let first = state.dispatch_event(false, false, ROBOTS_BALL_TRACK_EVENT_CREATE);
        assert!(first.create_owned_track);
        assert!(state.owned_track_created);
        assert!(!state.spawn_stopped);

        let repeated = state.dispatch_event(false, false, ROBOTS_BALL_TRACK_EVENT_CREATE);
        assert_eq!(repeated, RobotsBallTrackEventStep::default());
        assert!(state.owned_track_created);
    }

    #[test]
    fn stop_event_is_two_phase_only_when_serialized_gate_is_positive() {
        let mut shipped_noop = RobotsBallTrackRuntimeState::default();
        shipped_noop.dispatch_event(false, false, ROBOTS_BALL_TRACK_EVENT_CREATE);
        let ignored = shipped_noop.dispatch_event(false, false, ROBOTS_BALL_TRACK_EVENT_STOP);
        assert_eq!(ignored, RobotsBallTrackEventStep::default());
        assert!(shipped_noop.owned_track_created);
        assert!(!shipped_noop.spawn_stopped);

        let mut two_phase = RobotsBallTrackRuntimeState::default();
        two_phase.dispatch_event(false, true, ROBOTS_BALL_TRACK_EVENT_CREATE);
        let stop = two_phase.dispatch_event(false, true, ROBOTS_BALL_TRACK_EVENT_STOP);
        assert!(stop.stop_spawning);
        assert!(!stop.cleanup_owned_track);
        assert!(two_phase.owned_track_created);
        assert!(two_phase.spawn_stopped);

        let cleanup = two_phase.dispatch_event(false, true, ROBOTS_BALL_TRACK_EVENT_STOP);
        assert!(!cleanup.stop_spawning);
        assert!(cleanup.cleanup_owned_track);
        assert!(!two_phase.owned_track_created);
        assert!(!two_phase.spawn_stopped);
    }

    #[test]
    fn initialization_can_start_from_preexisting_owned_track_without_emitting_create() {
        let mut state = RobotsBallTrackRuntimeState::default();
        let step = state.dispatch_event(true, true, 0);
        assert_eq!(step, RobotsBallTrackEventStep::default());
        assert!(state.initialized);
        assert!(state.owned_track_created);
        assert!(!state.spawn_stopped);
    }

    #[test]
    fn free_slot_search_preserves_count_minus_one_modulus_and_circular_scan() {
        let slots = [
            RobotsBallTrackSpawnSlot {
                live_handler_present: true,
                in_use: true,
                variant: 0,
            },
            RobotsBallTrackSpawnSlot {
                live_handler_present: true,
                in_use: true,
                variant: 0,
            },
            RobotsBallTrackSpawnSlot {
                live_handler_present: true,
                in_use: true,
                variant: 0,
            },
            RobotsBallTrackSpawnSlot {
                live_handler_present: true,
                in_use: false,
                variant: 0,
            },
        ];
        // Native starts at 5 % (4 - 1) = 2, then advances to the otherwise
        // unreachable-as-initial-slot index 3.
        assert_eq!(
            select_ball_track_free_slot(&slots, Some(5)).unwrap(),
            Some(3)
        );
    }

    #[test]
    fn ordinary_spawn_uses_native_distance_auxiliary_and_edge_lane_clamps() {
        let slots = [RobotsBallTrackSpawnSlot {
            live_handler_present: true,
            in_use: false,
            variant: 0,
        }];
        let middle = plan_ball_track_spawn_for_slot(
            &slots,
            0,
            1,
            3,
            0x0B00_000E,
            18.0,
            100.0,
            120.0,
            Some(0.25),
            Some(0.9),
        )
        .unwrap();
        assert_eq!(
            middle.start,
            RobotsBallTrackStartPlan::RandomDistance { distance: 105.0 }
        );
        assert!((middle.auxiliary_scalar_330.unwrap() - 0.24).abs() < 0.000_001);

        let first_lane = plan_ball_track_spawn_for_slot(
            &slots,
            0,
            0,
            3,
            0x0B00_000E,
            18.0,
            100.0,
            120.0,
            Some(0.25),
            Some(0.0),
        )
        .unwrap();
        assert_eq!(first_lane.auxiliary_scalar_330, None);

        let last_lane = plan_ball_track_spawn_for_slot(
            &slots,
            0,
            2,
            3,
            0x0B00_000E,
            18.0,
            100.0,
            120.0,
            Some(0.25),
            Some(0.99),
        )
        .unwrap();
        assert_eq!(last_lane.auxiliary_scalar_330, None);
    }

    #[test]
    fn pool_variants_one_and_two_do_not_consume_ordinary_random_inputs() {
        let slots = [RobotsBallTrackSpawnSlot {
            live_handler_present: true,
            in_use: false,
            variant: 2,
        }];
        let plan = plan_ball_track_spawn_for_slot(
            &slots,
            0,
            0,
            1,
            0x0B00_000E,
            18.0,
            0.0,
            0.0,
            None,
            None,
        )
        .unwrap();
        assert_eq!(
            plan.start,
            RobotsBallTrackStartPlan::ExternalVariant12Parameter
        );
        assert_eq!(plan.auxiliary_scalar_330, None);
    }

    #[test]
    fn ordinary_burst_carries_previous_parameter_and_stops_after_nonincrease_spawn() {
        let slots = [
            RobotsBallTrackSpawnSlot {
                live_handler_present: true,
                in_use: false,
                variant: 0,
            },
            RobotsBallTrackSpawnSlot {
                live_handler_present: true,
                in_use: false,
                variant: 0,
            },
        ];
        let mut burst = RobotsBallTrackOrdinaryBurstRuntime::default();
        let first = burst
            .prepare_iteration(
                &slots,
                Some(0),
                0,
                2,
                0x0B00_000E,
                18.0,
                100.0,
                120.0,
                Some(0.25),
                Some(0.5),
            )
            .unwrap()
            .unwrap();
        assert_eq!(
            first.parameter,
            RobotsBallTrackSpawnParameterRequest::AdvanceDistance {
                from_parameter: 0.0,
                distance: 105.0,
            }
        );
        let committed = burst
            .commit_iteration(first, 1.25, 4.0, false)
            .unwrap()
            .unwrap();
        assert!(committed.continue_burst);
        assert_eq!(burst.previous_path_parameter, 1.25);

        let mut second_slots = slots;
        second_slots[committed.plan.slot_index].in_use = true;
        let second = burst
            .prepare_iteration(
                &second_slots,
                Some(0),
                0,
                2,
                0x0B00_000E,
                18.0,
                100.0,
                120.0,
                Some(0.5),
                Some(0.5),
            )
            .unwrap()
            .unwrap();
        assert!(matches!(
            second.parameter,
            RobotsBallTrackSpawnParameterRequest::AdvanceDistance {
                from_parameter: 1.25,
                ..
            }
        ));
        let wrapped = burst
            .commit_iteration(second, 0.4, 4.0, true)
            .unwrap()
            .unwrap();
        assert!(!wrapped.continue_burst);
        assert!(burst.complete);
        assert_eq!(burst.spawned_count, 2);
    }

    #[test]
    fn shipped_variant2_uses_nearest_owner_node_and_can_fill_multiple_free_slots() {
        let mut slots = [
            RobotsBallTrackSpawnSlot {
                live_handler_present: true,
                in_use: false,
                variant: 2,
            },
            RobotsBallTrackSpawnSlot {
                live_handler_present: true,
                in_use: false,
                variant: 2,
            },
        ];
        let mut burst = RobotsBallTrackOrdinaryBurstRuntime::default();
        let first = burst
            .prepare_iteration(
                &slots,
                Some(0),
                0,
                1,
                0x0B00_0010,
                12.0,
                0.0,
                0.0,
                None,
                None,
            )
            .unwrap()
            .unwrap();
        assert_eq!(
            first.parameter,
            RobotsBallTrackSpawnParameterRequest::NearestOwnerNodeIndex
        );
        let first_commit = burst
            .commit_iteration(first, 2.0, 4.0, false)
            .unwrap()
            .unwrap();
        assert!(first_commit.continue_burst);
        slots[first_commit.plan.slot_index].in_use = true;

        let second = burst
            .prepare_iteration(
                &slots,
                Some(0),
                0,
                1,
                0x0B00_0010,
                12.0,
                0.0,
                0.0,
                None,
                None,
            )
            .unwrap()
            .unwrap();
        let second_commit = burst
            .commit_iteration(second, 2.0, 4.0, false)
            .unwrap()
            .unwrap();
        assert!(second_commit.continue_burst);
        slots[second_commit.plan.slot_index].in_use = true;
        assert_eq!(
            burst
                .prepare_iteration(
                    &slots,
                    Some(0),
                    0,
                    1,
                    0x0B00_0010,
                    12.0,
                    0.0,
                    0.0,
                    None,
                    None
                )
                .unwrap(),
            None
        );

        let mut zero = RobotsBallTrackOrdinaryBurstRuntime::default();
        let request = zero
            .prepare_iteration(
                &[RobotsBallTrackSpawnSlot {
                    live_handler_present: true,
                    in_use: false,
                    variant: 2,
                }],
                None,
                0,
                1,
                0x0B00_0010,
                12.0,
                0.0,
                0.0,
                None,
                None,
            )
            .unwrap()
            .unwrap();
        assert!(
            !zero
                .commit_iteration(request, 0.0, 4.0, false)
                .unwrap()
                .unwrap()
                .continue_burst
        );
    }

    #[test]
    fn ordinary_burst_rejects_nonloop_fractional_last_to_first_before_spawn_commit() {
        let mut burst = RobotsBallTrackOrdinaryBurstRuntime::default();
        let request = burst
            .prepare_iteration(
                &[RobotsBallTrackSpawnSlot {
                    live_handler_present: true,
                    in_use: false,
                    variant: 0,
                }],
                None,
                0,
                1,
                0x0B00_0010,
                12.0,
                0.0,
                1.0,
                Some(0.5),
                Some(0.5),
            )
            .unwrap()
            .unwrap();
        assert_eq!(
            burst.commit_iteration(request, 4.25, 4.0, false).unwrap(),
            None
        );
        assert!(burst.complete);
        assert_eq!(burst.spawned_count, 0);
    }

    #[test]
    fn alternate_scheduler_initial_fill_walks_rows_backwards_and_keeps_limit_out_of_inner_loop() {
        let mut lane = RobotsBallTrackAlternateLaneRuntime::default();
        let first = lane
            .begin_tick(false, 5, 2, 1, 18.0 / 60.0, 5.0)
            .unwrap()
            .unwrap();
        assert_eq!(first.schedule_row_index, 1);
        assert_eq!(first.path_parameter, 3.0);
        assert_eq!(lane.path_last_node_index, 4);

        lane.finish_row(2.0, 2).unwrap();
        let second = lane.prepare_followup_row(5.0).unwrap().unwrap();
        assert_eq!(second.schedule_row_index, 0);
        assert_eq!(second.path_parameter, 2.0);
        lane.finish_row(1.0, 2).unwrap();
        assert_eq!(lane.completed_schedule_cycles, 1);

        // Native checks +0x33C only at function entry. The same call continues
        // after the cursor wrapped, even though completed cycles now equals limit.
        let third = lane.prepare_followup_row(5.0).unwrap().unwrap();
        assert_eq!(third.schedule_row_index, 1);
        assert_eq!(third.path_parameter, 1.0);
        lane.finish_row(0.0, 2).unwrap();
        assert!(lane.prepare_followup_row(5.0).unwrap().is_none());

        // Next native call observes the cycle limit and performs no work.
        assert!(lane
            .begin_tick(false, 5, 2, 1, 18.0 / 60.0, 5.0)
            .unwrap()
            .is_none());
    }

    #[test]
    fn alternate_scheduler_spacing_crossing_is_strict_and_resets_distance_and_parameter() {
        let mut lane = RobotsBallTrackAlternateLaneRuntime {
            initialized: true,
            path_last_node_index: 4,
            schedule_row_cursor: 1,
            distance_accumulator: 4.5,
            path_parameter: 0.0,
            completed_schedule_cycles: 0,
        };
        assert!(lane
            .begin_tick(false, 5, 2, 10, 0.5, 5.0)
            .unwrap()
            .is_none());
        assert_eq!(lane.distance_accumulator, 5.0);

        let row = lane
            .begin_tick(false, 5, 2, 10, 0.01, 5.0)
            .unwrap()
            .unwrap();
        assert_eq!(row.schedule_row_index, 1);
        assert_eq!(row.path_parameter, 0.0);
        assert_eq!(lane.distance_accumulator, 0.0);
    }

    #[test]
    fn alternate_spawn_uses_current_parameter_first_lane_controller_and_no_ordinary_rng_math() {
        let slots = [RobotsBallTrackSpawnSlot {
            live_handler_present: true,
            in_use: false,
            variant: 7,
        }];
        let plan =
            plan_ball_track_alternate_spawn_for_slot(&slots, 0, 2, 4, 0x0B00_0010, 12.0, 3.25)
                .unwrap();
        assert_eq!(plan.slot_index, 0);
        assert_eq!(plan.path_lane, 2);
        assert_eq!(plan.controller_path_lane, 0);
        assert_eq!(plan.runtime_speed, 12.0);
        assert_eq!(plan.path_parameter, 3.25);
    }
}
