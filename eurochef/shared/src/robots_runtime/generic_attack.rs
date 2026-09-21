use serde::Serialize;

use super::{
    events::{event_type, RobotsScriptEventView},
    locomotion::shortest_yaw_delta,
    projectile::RobotsCreateProjectileRequest,
};

pub const ROBOTS_GENERIC_ATTACK_PRIORITY: u8 = 0x32;
pub const ROBOTS_GENERIC_ATTACK_INITIAL_INACTIVE_TICKS: u32 = 0x1_0000;
pub const ROBOTS_MALFBOT_ATTACK_REENTRY_TICKS: u32 = 0x78;
pub const ROBOTS_MALFBOT_ATTACK_YAW_TOLERANCE_RADIANS: f32 = std::f32::consts::PI / 6.0;
pub const ROBOTS_MALFBOT_ATTACK_VERTICAL_LIMIT: f32 = 1000.0;
pub const ROBOTS_MALFBOT_ATTACK1_ANIM_MODE: u32 = 0x0900_0025;
pub const ROBOTS_MALFBOT_ATTACK2_ANIM_MODE: u32 = 0x0900_0037;
pub const ROBOTS_MALFBOT_ATTACK1_OUTER_RADIUS: f32 = 4.5;
pub const ROBOTS_MALFBOT_ATTACK2_OUTER_RADIUS: f32 = 1.5;
pub const ROBOTS_ELECTRICITY_ARC_SEED_COUNT: usize = 5;
pub const ROBOTS_ELECTRICITY_ARC_CONSTRUCTOR_RNG_DRAWS: usize =
    ROBOTS_ELECTRICITY_ARC_SEED_COUNT * 2;
pub const ROBOTS_ELECTRICITY_ARC_PHASE_SCALE: f32 = f32::from_bits(0x40c9_0fdb);
pub const ROBOTS_PROCESS_RNG_UNIT_SCALE: f32 = f32::from_bits(0x3000_0000);

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct RobotsElectricityArcSeed {
    pub phase_radians: [f32; ROBOTS_ELECTRICITY_ARC_SEED_COUNT],
    pub segment_counts: [u8; ROBOTS_ELECTRICITY_ARC_SEED_COUNT],
}

pub fn electricity_arc_seed_from_draws(
    draws: [u32; ROBOTS_ELECTRICITY_ARC_CONSTRUCTOR_RNG_DRAWS],
) -> RobotsElectricityArcSeed {
    let mut phase_radians = [0.0; ROBOTS_ELECTRICITY_ARC_SEED_COUNT];
    let mut segment_counts = [0; ROBOTS_ELECTRICITY_ARC_SEED_COUNT];
    for index in 0..ROBOTS_ELECTRICITY_ARC_SEED_COUNT {
        phase_radians[index] = ((draws[index * 2] >> 1) as f32)
            * ROBOTS_PROCESS_RNG_UNIT_SCALE
            * ROBOTS_ELECTRICITY_ARC_PHASE_SCALE;
        segment_counts[index] = (draws[index * 2 + 1] % 15 + 15) as u8;
    }
    RobotsElectricityArcSeed {
        phase_radians,
        segment_counts,
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize)]
pub struct RobotsElectricityArcRuntimeState {
    pub attachment_datum: Option<u32>,
    pub seed: Option<RobotsElectricityArcSeed>,
}

/// Native `AI_Attack::AttachBeam` reuses an existing ElectricityArc without
/// constructor RNG. A fresh `XItemHandler_ElectricityArc` runs `0x0047C8A0`,
/// which consumes ten process-global draws (two draws for each of five seeds).
pub fn attach_electricity_arc(
    state: &mut RobotsElectricityArcRuntimeState,
    attachment_datum: u32,
    constructor_draws: Option<[u32; ROBOTS_ELECTRICITY_ARC_CONSTRUCTOR_RNG_DRAWS]>,
) -> Option<bool> {
    if state.attachment_datum.is_some() {
        state.attachment_datum = Some(attachment_datum);
        return Some(false);
    }
    let draws = constructor_draws?;
    state.attachment_datum = Some(attachment_datum);
    state.seed = Some(electricity_arc_seed_from_draws(draws));
    Some(true)
}

pub fn detach_electricity_arc(state: &mut RobotsElectricityArcRuntimeState) -> bool {
    let attached = state.attachment_datum.take().is_some();
    state.seed = None;
    attached
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RobotsParticleHitcheckRequest {
    pub arg0: u32,
    pub arg1: u32,
    /// Native loads this as float and converts it through MSVC `__ftol` before
    /// entering `AI_Attack::ParticleHitcheck` helper `0x0044F2F0`.
    pub arg2_scalar_bits: u32,
    pub arg3: u32,
    pub arg4: u32,
}

impl RobotsParticleHitcheckRequest {
    pub fn from_event(event: RobotsScriptEventView<'_>) -> Option<Self> {
        Some(Self {
            arg0: event.native_arg_word(0)?,
            arg1: event.native_arg_word(1)?,
            arg2_scalar_bits: event.native_arg_word(2)?,
            arg3: event.native_arg_word(3)?,
            arg4: event.native_arg_word(4)?,
        })
    }

    pub fn arg2_scalar(self) -> f32 {
        f32::from_bits(self.arg2_scalar_bits)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub enum RobotsAttackScriptEventAction {
    CreateProjectile(RobotsCreateProjectileRequest),
    AttachBeam { attachment_datum: u32 },
    DetachBeam,
    ParticleHitcheck(RobotsParticleHitcheckRequest),
}

/// Event family shared by generic `AI_Attack::event 0x0044F180` and the
/// ShuntBotBoss override `0x00450850`. SetupIdle is intentionally excluded:
/// each concrete attack reducer owns its own state-machine transition.
pub fn classify_attack_script_event(
    event: RobotsScriptEventView<'_>,
) -> Option<RobotsAttackScriptEventAction> {
    match event.event_type {
        event_type::CREATE_PROJECTILE => Some(RobotsAttackScriptEventAction::CreateProjectile(
            RobotsCreateProjectileRequest::from_event(event)?,
        )),
        event_type::ATTACH_BEAM => Some(RobotsAttackScriptEventAction::AttachBeam {
            attachment_datum: event.native_arg_word(1)?,
        }),
        event_type::DETACH_BEAM => Some(RobotsAttackScriptEventAction::DetachBeam),
        event_type::PARTICLE_HITCHECK => Some(RobotsAttackScriptEventAction::ParticleHitcheck(
            RobotsParticleHitcheckRequest::from_event(event)?,
        )),
        _ => None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct RobotsGenericAttackConfig {
    pub primary_anim_mode: u32,
    pub secondary_anim_mode: Option<u32>,
    pub inner_radius: f32,
    pub outer_radius: f32,
    pub yaw_tolerance_radians: f32,
    pub vertical_limit: f32,
    /// Common base-node +0x18. Fresh nodes start with +0x14=0x10000, so this
    /// delays re-entry after leave; it does not delay the first attack.
    pub reentry_delay_ticks: u32,
    /// Common Attack node +0x30. MalfBot passes zero and therefore has no
    /// secondary repeat phase.
    pub secondary_repeat_count: i32,
    /// Attack node +0x44. When true, `0x0044F080` keeps the node at its own
    /// priority while active and incomplete instead of rechecking target gates.
    pub sticky_while_active: bool,
    pub priority: u8,
}

impl RobotsGenericAttackConfig {
    /// EB07 MineBot builder `0x0045ECF0`. This is the first AttackGroup child;
    /// native overrides the ordinary AI_Attack priority from 0x32 to 0x51 after
    /// setup and keeps the standard 120-tick re-entry delay.
    pub const fn eb07_minebot_primary() -> Self {
        Self {
            primary_anim_mode: 0x0900_0025,
            secondary_anim_mode: None,
            inner_radius: 0.0,
            outer_radius: 15.0,
            yaw_tolerance_radians: std::f32::consts::PI,
            vertical_limit: ROBOTS_MALFBOT_ATTACK_VERTICAL_LIMIT,
            reentry_delay_ticks: 120,
            secondary_repeat_count: 0,
            sticky_while_active: true,
            priority: 0x51,
        }
    }

    /// Common monster `AI_Attack` shape used by MalfBot/EW10/EB14 builders.
    /// These builders differ only in AnimMode, outer radius and yaw tolerance;
    /// the remaining fields are the same native node contract.
    pub const fn standard_monster_attack(
        primary_anim_mode: u32,
        outer_radius: f32,
        yaw_tolerance_radians: f32,
    ) -> Self {
        Self::standard_monster_attack_with_reentry(
            primary_anim_mode,
            outer_radius,
            yaw_tolerance_radians,
            ROBOTS_MALFBOT_ATTACK_REENTRY_TICKS,
        )
    }

    pub const fn standard_monster_attack_with_reentry(
        primary_anim_mode: u32,
        outer_radius: f32,
        yaw_tolerance_radians: f32,
        reentry_delay_ticks: u32,
    ) -> Self {
        Self::standard_monster_attack_window_with_reentry(
            primary_anim_mode,
            0.0,
            outer_radius,
            yaw_tolerance_radians,
            reentry_delay_ticks,
        )
    }

    /// Common `AI_Attack::setup` window for classes that use a non-zero inner
    /// radius. JailBotNormal mode27 is the first shipped standard-monster user
    /// requiring the full native annulus instead of the usual 0..outer range.
    pub const fn standard_monster_attack_window_with_reentry(
        primary_anim_mode: u32,
        inner_radius: f32,
        outer_radius: f32,
        yaw_tolerance_radians: f32,
        reentry_delay_ticks: u32,
    ) -> Self {
        Self {
            primary_anim_mode,
            secondary_anim_mode: None,
            inner_radius,
            outer_radius,
            yaw_tolerance_radians,
            vertical_limit: ROBOTS_MALFBOT_ATTACK_VERTICAL_LIMIT,
            reentry_delay_ticks,
            secondary_repeat_count: 0,
            sticky_while_active: true,
            priority: ROBOTS_GENERIC_ATTACK_PRIORITY,
        }
    }

    pub const fn malfbot_attack1() -> Self {
        Self::standard_monster_attack(
            ROBOTS_MALFBOT_ATTACK1_ANIM_MODE,
            ROBOTS_MALFBOT_ATTACK1_OUTER_RADIUS,
            ROBOTS_MALFBOT_ATTACK_YAW_TOLERANCE_RADIANS,
        )
    }

    pub const fn malfbot_attack2() -> Self {
        Self::standard_monster_attack(
            ROBOTS_MALFBOT_ATTACK2_ANIM_MODE,
            ROBOTS_MALFBOT_ATTACK2_OUTER_RADIUS,
            ROBOTS_MALFBOT_ATTACK_YAW_TOLERANCE_RADIANS,
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct RobotsGenericAttackGateInput {
    pub owner_position_xyz: [f32; 3],
    pub owner_yaw_radians: f32,
    pub target_position_xyz: [f32; 3],
    /// Handler+0x5FA.
    pub target_visible: bool,
    /// Handler vslot +0x158, resolved by the host because it depends on global
    /// Player/GameWnd/cooldown state.
    pub class_attack_allowed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RobotsGenericAttackRuntimeState {
    /// Common behavior node +0x09.
    pub active: bool,
    /// Common behavior node +0x21.
    pub completion_latch: bool,
    /// Common base +0x10.
    pub active_ticks: u32,
    /// Common base +0x14. Native ctor seeds 0x10000 so first use is ready.
    pub inactive_ticks: u32,
    pub secondary_phase: bool,
    pub remaining_secondary_count: i32,
}

impl Default for RobotsGenericAttackRuntimeState {
    fn default() -> Self {
        Self {
            active: false,
            completion_latch: false,
            active_ticks: 0,
            inactive_ticks: ROBOTS_GENERIC_ATTACK_INITIAL_INACTIVE_TICKS,
            secondary_phase: false,
            remaining_secondary_count: 0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RobotsGenericAttackStep {
    pub requested_anim_mode: Option<u32>,
    pub entered: bool,
    pub active: bool,
}

pub fn generic_attack_geometry_gate(
    config: RobotsGenericAttackConfig,
    input: RobotsGenericAttackGateInput,
) -> bool {
    if !input.target_visible
        || !input.class_attack_allowed
        || !input.owner_yaw_radians.is_finite()
        || !input
            .owner_position_xyz
            .iter()
            .all(|value| value.is_finite())
        || !input
            .target_position_xyz
            .iter()
            .all(|value| value.is_finite())
    {
        return false;
    }

    let dx = input.target_position_xyz[0] - input.owner_position_xyz[0];
    let dy = input.target_position_xyz[1] - input.owner_position_xyz[1];
    let dz = input.target_position_xyz[2] - input.owner_position_xyz[2];
    let distance_squared = dx * dx + dy * dy + dz * dz;
    if distance_squared < config.inner_radius * config.inner_radius
        || distance_squared > config.outer_radius * config.outer_radius
        || dy.abs() > config.vertical_limit
    {
        return false;
    }

    let target_yaw = dx.atan2(dz);
    shortest_yaw_delta(input.owner_yaw_radians, target_yaw).abs() <= config.yaw_tolerance_radians
}

/// Common node +0x20 gate (`0x00456F20`) followed by generic Attack priority
/// callback `0x0044F080` for the plain MalfBot flags=0 configuration.
pub fn generic_attack_priority(
    state: RobotsGenericAttackRuntimeState,
    config: RobotsGenericAttackConfig,
    input: RobotsGenericAttackGateInput,
) -> u8 {
    if !state.active {
        if state.inactive_ticks < config.reentry_delay_ticks {
            return 1;
        }
    } else if state.completion_latch {
        if config.reentry_delay_ticks != 0 {
            return 1;
        }
        // Native `0x0044F080` re-runs the geometry/class gate when the active
        // node's +0x30 completion predicate is true. A zero re-entry delay does
        // not make a completed sticky attack unconditional.
    } else if config.sticky_while_active {
        return config.priority;
    }

    if generic_attack_geometry_gate(config, input) {
        config.priority
    } else {
        1
    }
}

pub fn enter_generic_attack(
    state: &mut RobotsGenericAttackRuntimeState,
    config: RobotsGenericAttackConfig,
) {
    state.active = true;
    state.completion_latch = false;
    state.active_ticks = 0;
    state.secondary_phase = false;
    state.remaining_secondary_count = config.secondary_repeat_count;
}

pub fn leave_generic_attack(state: &mut RobotsGenericAttackRuntimeState) {
    state.active = false;
    state.inactive_ticks = 0;
    state.secondary_phase = false;
}

pub fn step_generic_attack(
    state: &mut RobotsGenericAttackRuntimeState,
    config: RobotsGenericAttackConfig,
) -> RobotsGenericAttackStep {
    if !state.active {
        return RobotsGenericAttackStep {
            requested_anim_mode: None,
            entered: false,
            active: false,
        };
    }

    let requested_anim_mode = if state.secondary_phase {
        config.secondary_anim_mode
    } else {
        Some(config.primary_anim_mode)
    };
    RobotsGenericAttackStep {
        requested_anim_mode,
        entered: false,
        active: true,
    }
}

/// Event `HT_ScriptEvents_SetupIdle (0x16000001)` consumed by `0x0044F180`.
pub fn generic_attack_setup_idle(
    state: &mut RobotsGenericAttackRuntimeState,
    config: RobotsGenericAttackConfig,
) {
    if config.secondary_anim_mode.is_some() && state.remaining_secondary_count != 0 {
        state.secondary_phase = true;
    } else {
        state.completion_latch = true;
    }
}

pub fn tick_generic_attack(state: &mut RobotsGenericAttackRuntimeState) {
    if state.active {
        state.active_ticks = state.active_ticks.saturating_add(1);
    } else {
        state.inactive_ticks = state.inactive_ticks.saturating_add(1);
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RobotsGenericAttackGroupRuntimeState {
    pub active: bool,
    pub selected_index: Option<usize>,
}

impl Default for RobotsGenericAttackGroupRuntimeState {
    fn default() -> Self {
        Self {
            active: false,
            selected_index: None,
        }
    }
}

pub fn generic_attack_group_priority(
    group: &RobotsGenericAttackGroupRuntimeState,
    child_priorities: &[u8],
) -> u8 {
    if group.active {
        return group
            .selected_index
            .and_then(|index| child_priorities.get(index).copied())
            .unwrap_or(1);
    }
    child_priorities.iter().copied().max().unwrap_or(1)
}

/// `AI_AttackGroup::Enter 0x00450F80` consumes one global RNG draw even when
/// there is exactly one child at the maximum priority (`draw % 1`).
pub fn enter_generic_attack_group(
    group: &mut RobotsGenericAttackGroupRuntimeState,
    child_priorities: &[u8],
    rng_draw: Option<u32>,
) -> Option<usize> {
    let max_priority = child_priorities.iter().copied().max()?;
    let candidates = child_priorities
        .iter()
        .enumerate()
        .filter_map(|(index, priority)| (*priority >= max_priority).then_some(index))
        .collect::<Vec<_>>();
    let draw = rng_draw?;
    let selected = candidates[draw as usize % candidates.len()];
    group.active = true;
    group.selected_index = Some(selected);
    Some(selected)
}

pub fn leave_generic_attack_group(group: &mut RobotsGenericAttackGroupRuntimeState) {
    group.active = false;
    group.selected_index = None;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(distance: f32) -> RobotsGenericAttackGateInput {
        RobotsGenericAttackGateInput {
            owner_position_xyz: [0.0, 0.0, 0.0],
            owner_yaw_radians: 0.0,
            target_position_xyz: [0.0, 0.0, distance],
            target_visible: true,
            class_attack_allowed: true,
        }
    }

    fn native_event_data(args: &[u32]) -> Vec<u8> {
        let mut data = 0u32.to_le_bytes().to_vec();
        for arg in args {
            data.extend_from_slice(&arg.to_le_bytes());
        }
        data
    }

    #[test]
    fn malfbot_attack_ranges_and_thirty_degree_gate_match_builder() {
        let attack1 = RobotsGenericAttackConfig::malfbot_attack1();
        let attack2 = RobotsGenericAttackConfig::malfbot_attack2();
        assert!(generic_attack_geometry_gate(attack1, input(4.5)));
        assert!(!generic_attack_geometry_gate(attack1, input(4.501)));
        assert!(generic_attack_geometry_gate(attack2, input(1.5)));
        assert!(!generic_attack_geometry_gate(attack2, input(1.501)));

        let mut edge = input(1.0);
        let inside = 29.9_f32.to_radians();
        edge.target_position_xyz = [inside.sin(), 0.0, inside.cos()];
        assert!(generic_attack_geometry_gate(attack1, edge));
        let outside = 30.1_f32.to_radians();
        edge.target_position_xyz = [outside.sin(), 0.0, outside.cos()];
        assert!(!generic_attack_geometry_gate(attack1, edge));
    }

    #[test]
    fn eb07_primary_attack_keeps_native_priority_range_and_reentry() {
        let config = RobotsGenericAttackConfig::eb07_minebot_primary();
        assert_eq!(config.primary_anim_mode, 0x0900_0025);
        assert_eq!(config.inner_radius, 0.0);
        assert_eq!(config.outer_radius, 15.0);
        assert_eq!(config.yaw_tolerance_radians, std::f32::consts::PI);
        assert_eq!(config.reentry_delay_ticks, 120);
        assert_eq!(config.priority, 0x51);
        assert!(config.sticky_while_active);
    }

    #[test]
    fn fresh_attack_is_ready_but_leave_starts_native_120_tick_reentry_delay() {
        let config = RobotsGenericAttackConfig::malfbot_attack1();
        let mut state = RobotsGenericAttackRuntimeState::default();
        assert_eq!(generic_attack_priority(state, config, input(1.0)), 0x32);
        enter_generic_attack(&mut state, config);
        leave_generic_attack(&mut state);
        assert_eq!(generic_attack_priority(state, config, input(1.0)), 1);
        for _ in 0..120 {
            tick_generic_attack(&mut state);
        }
        assert_eq!(generic_attack_priority(state, config, input(1.0)), 0x32);
    }

    #[test]
    fn attack_group_consumes_rng_even_for_one_max_priority_child() {
        let mut group = RobotsGenericAttackGroupRuntimeState::default();
        assert_eq!(
            enter_generic_attack_group(&mut group, &[0x32, 1], None),
            None
        );
        assert!(!group.active);
        assert_eq!(
            enter_generic_attack_group(&mut group, &[0x32, 1], Some(7)),
            Some(0)
        );
        leave_generic_attack_group(&mut group);
        assert_eq!(
            enter_generic_attack_group(&mut group, &[0x32, 0x32], Some(3)),
            Some(1)
        );
        leave_generic_attack_group(&mut group);
        assert_eq!(
            enter_generic_attack_group(&mut group, &[0x32, 0x32, 0x32], Some(2)),
            Some(2)
        );
        leave_generic_attack_group(&mut group);
        assert_eq!(
            enter_generic_attack_group(&mut group, &[1, 1, 0], Some(2)),
            Some(0),
            "host-only empty attack slots must never enter the RNG candidate set"
        );
    }

    #[test]
    fn setup_idle_completes_malfbot_primary_only_attack() {
        let config = RobotsGenericAttackConfig::malfbot_attack1();
        let mut state = RobotsGenericAttackRuntimeState::default();
        enter_generic_attack(&mut state, config);
        assert_eq!(
            step_generic_attack(&mut state, config).requested_anim_mode,
            Some(0x0900_0025)
        );
        generic_attack_setup_idle(&mut state, config);
        assert!(state.completion_latch);
        assert_eq!(generic_attack_priority(state, config, input(1.0)), 1);
    }

    #[test]
    fn electricity_arc_constructor_uses_ten_draws_and_reuses_existing_arc() {
        let draws = [0, 0, 0x8000_0000, 14, 0xffff_ffff, 15, 2, 29, 4, 44];
        let seed = electricity_arc_seed_from_draws(draws);
        assert_eq!(seed.phase_radians[0], 0.0);
        assert!((seed.phase_radians[1] - core::f32::consts::PI).abs() < 1.0e-6);
        assert_eq!(seed.segment_counts, [15, 29, 15, 29, 29]);

        let mut state = RobotsElectricityArcRuntimeState::default();
        assert_eq!(
            attach_electricity_arc(&mut state, 7, Some(draws)),
            Some(true)
        );
        let original_seed = state.seed;
        assert_eq!(attach_electricity_arc(&mut state, 9, None), Some(false));
        assert_eq!(state.attachment_datum, Some(9));
        assert_eq!(state.seed, original_seed);
        assert!(detach_electricity_arc(&mut state));
        assert_eq!(state, RobotsElectricityArcRuntimeState::default());
    }

    #[test]
    fn common_attack_script_events_parse_native_argument_layout() {
        let attach_data = native_event_data(&[0xaaaa_aaaa, 0x1000_0027]);
        let attach = classify_attack_script_event(RobotsScriptEventView {
            event_type: event_type::ATTACH_BEAM,
            data: &attach_data,
            start: None,
            length: None,
        });
        assert_eq!(
            attach,
            Some(RobotsAttackScriptEventAction::AttachBeam {
                attachment_datum: 0x1000_0027,
            })
        );

        let particle_data = native_event_data(&[
            0x1100_0008,
            0x1234_5678,
            3.75f32.to_bits(),
            0x0102_0304,
            0x0506_0708,
        ]);
        let particle = classify_attack_script_event(RobotsScriptEventView {
            event_type: event_type::PARTICLE_HITCHECK,
            data: &particle_data,
            start: None,
            length: None,
        });
        assert_eq!(
            particle,
            Some(RobotsAttackScriptEventAction::ParticleHitcheck(
                RobotsParticleHitcheckRequest {
                    arg0: 0x1100_0008,
                    arg1: 0x1234_5678,
                    arg2_scalar_bits: 3.75f32.to_bits(),
                    arg3: 0x0102_0304,
                    arg4: 0x0506_0708,
                }
            ))
        );
    }
}
