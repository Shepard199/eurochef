use serde::Serialize;

use super::{
    generic_attack::{
        enter_generic_attack, generic_attack_priority, leave_generic_attack, tick_generic_attack,
        RobotsGenericAttackConfig, RobotsGenericAttackGateInput, RobotsGenericAttackRuntimeState,
        ROBOTS_GENERIC_ATTACK_PRIORITY, ROBOTS_MALFBOT_ATTACK_VERTICAL_LIMIT,
    },
    locomotion::shortest_yaw_delta,
};

pub const ROBOTS_TURRET_YAW_STEP_RADIANS: f32 = std::f32::consts::PI / 60.0;
pub const ROBOTS_EP02_PITCH_STEP_RADIANS: f32 = std::f32::consts::PI / 120.0;
pub const ROBOTS_TURRET_ANGLE_EPSILON: f32 = 0.001;
pub const ROBOTS_EP02_TRACK_TARGET_PRIORITY: u8 = 0x1e;
pub const ROBOTS_EP02_TRACK_TARGET_RADIUS: f32 = 20.0;
pub const ROBOTS_EP02_IDLE_PITCH_ANIM_MODE: u32 = 0x0900_0001;
pub const ROBOTS_TURRET_YAW_ANIM_MODE: u32 = 0x0900_0003;
pub const ROBOTS_EP02_ATTACK_ANIM_MODE: u32 = 0x0900_0025;
pub const ROBOTS_EP04_ATTACK_ANIM_MODE: u32 = 0x0900_0025;
pub const ROBOTS_EP05_ATTACK_ANIM_MODE: u32 = 0x0900_0025;
pub const ROBOTS_EP05_IDLE_ANIM_MODE: u32 = 0x0900_0087;
pub const ROBOTS_EP05_DEACTIVATE_IDLE_ANIM_MODE: u32 = 0x0900_0078;
pub const ROBOTS_EP05_DEACTIVATE_EXIT_ANIM_MODE: u32 = 0x0900_0077;
pub const ROBOTS_EP05_DEACTIVATE_IDLE2_ANIM_MODE: u32 = 0x0900_008d;
pub const ROBOTS_EP05_DEACTIVATE_EXIT2_ANIM_MODE: u32 = 0x0900_008c;
pub const ROBOTS_EP05_TRACK_TARGET_PRIORITY: u8 = 0x1e;
pub const ROBOTS_EP05_DEACTIVATE_PRIORITY: u8 = 0x5a;
pub const ROBOTS_EP05_IDLE_PRIORITY: u8 = 2;
pub const ROBOTS_EP05_TRACK_TARGET_RADIUS: f32 = 10.0;
pub const ROBOTS_EP05_YAW_STEP_RADIANS: f32 = std::f32::consts::PI / 120.0;
pub const ROBOTS_EP02_PITCH_SOUND_UID: u32 = 0x1af0_015d;
pub const ROBOTS_EP02_YAW_SOUND_UID: u32 = 0x1af0_015e;
pub const ROBOTS_EP04_YAW_SOUND_UID: u32 = 0x1af0_0164;
pub const ROBOTS_EP06_PRIMARY_IDLE_ANIM_MODE: u32 = 0x0900_0087;
pub const ROBOTS_EP06_PRIMARY_ATTACK_ANIM_MODE: u32 = 0x0900_0025;
pub const ROBOTS_EP06_SECONDARY_IDLE_ANIM_MODE: u32 = 0x0900_0088;
pub const ROBOTS_EP06_SECONDARY_ATTACK_ANIM_MODE: u32 = 0x0900_0027;
pub const ROBOTS_EP06_MAGNETIC_PRIORITY: u8 = 0x69;
pub const ROBOTS_EP05_YAW_SOUND_UID: u32 = 0x1af0_015e;

pub const fn ep02_pitched_attack_config() -> RobotsGenericAttackConfig {
    RobotsGenericAttackConfig {
        primary_anim_mode: ROBOTS_EP02_ATTACK_ANIM_MODE,
        secondary_anim_mode: None,
        inner_radius: 0.0,
        outer_radius: 20.0,
        yaw_tolerance_radians: std::f32::consts::PI / 12.0,
        vertical_limit: 1000.0,
        reentry_delay_ticks: 120,
        secondary_repeat_count: 0,
        sticky_while_active: true,
        priority: 0x32,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct RobotsPitchedTurretAttackConfig {
    pub attack: RobotsGenericAttackConfig,
    pub idle_anim_mode: u32,
    pub rest_pitch_radians: f32,
    pub attack_pitch_radians: f32,
    pub pitch_step_radians: f32,
}

pub const fn ep02_pitched_attack_behavior_config() -> RobotsPitchedTurretAttackConfig {
    RobotsPitchedTurretAttackConfig {
        attack: ep02_pitched_attack_config(),
        idle_anim_mode: ROBOTS_EP02_IDLE_PITCH_ANIM_MODE,
        rest_pitch_radians: 0.0,
        attack_pitch_radians: std::f32::consts::FRAC_PI_2,
        pitch_step_radians: ROBOTS_EP02_PITCH_STEP_RADIANS,
    }
}

pub const fn ep06_pitched_attack_config(
    idle_anim_mode: u32,
    attack_anim_mode: u32,
) -> RobotsPitchedTurretAttackConfig {
    RobotsPitchedTurretAttackConfig {
        attack: RobotsGenericAttackConfig {
            primary_anim_mode: attack_anim_mode,
            secondary_anim_mode: None,
            inner_radius: 0.0,
            outer_radius: 14.0,
            yaw_tolerance_radians: std::f32::consts::PI / 180.0,
            vertical_limit: 1000.0,
            reentry_delay_ticks: 120,
            secondary_repeat_count: 0,
            sticky_while_active: true,
            priority: ROBOTS_GENERIC_ATTACK_PRIORITY,
        },
        idle_anim_mode,
        rest_pitch_radians: 0.0,
        attack_pitch_radians: -25.0 * std::f32::consts::PI / 180.0,
        pitch_step_radians: ROBOTS_EP02_PITCH_STEP_RADIANS,
    }
}

pub const fn ep06_primary_pitched_attack_config() -> RobotsPitchedTurretAttackConfig {
    ep06_pitched_attack_config(
        ROBOTS_EP06_PRIMARY_IDLE_ANIM_MODE,
        ROBOTS_EP06_PRIMARY_ATTACK_ANIM_MODE,
    )
}

pub const fn ep06_secondary_pitched_attack_config() -> RobotsPitchedTurretAttackConfig {
    ep06_pitched_attack_config(
        ROBOTS_EP06_SECONDARY_IDLE_ANIM_MODE,
        ROBOTS_EP06_SECONDARY_ATTACK_ANIM_MODE,
    )
}

/// EP04 builder `0x00460E80`: one ordinary Attack25 child inside an
/// AttackGroup. Native overrides re-entry at +0x18; the byte written at +0x20
/// is the registered attack descriptor index, not Behavior priority (+0x08).
pub const fn ep04_attack_config() -> RobotsGenericAttackConfig {
    RobotsGenericAttackConfig {
        primary_anim_mode: ROBOTS_EP04_ATTACK_ANIM_MODE,
        secondary_anim_mode: None,
        inner_radius: 0.0,
        outer_radius: 14.0,
        yaw_tolerance_radians: std::f32::consts::PI / 36.0,
        vertical_limit: 1000.0,
        reentry_delay_ticks: 420,
        secondary_repeat_count: 0,
        sticky_while_active: true,
        priority: ROBOTS_GENERIC_ATTACK_PRIORITY,
    }
}

/// EP05 builder `0x004612D0`: one ordinary Attack25 node lives directly in
/// the second BehaviorHost. Unlike EP04, native does not overwrite base +0x18,
/// so re-entry remains zero after `AI_Attack::Setup 0x0044EFB0`.
pub const fn ep05_attack_config() -> RobotsGenericAttackConfig {
    RobotsGenericAttackConfig {
        primary_anim_mode: ROBOTS_EP05_ATTACK_ANIM_MODE,
        secondary_anim_mode: None,
        inner_radius: 0.0,
        outer_radius: ROBOTS_EP05_TRACK_TARGET_RADIUS,
        yaw_tolerance_radians: std::f32::consts::PI / 12.0,
        vertical_limit: ROBOTS_MALFBOT_ATTACK_VERTICAL_LIMIT,
        reentry_delay_ticks: 0,
        secondary_repeat_count: 0,
        sticky_while_active: true,
        priority: ROBOTS_GENERIC_ATTACK_PRIORITY,
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize)]
pub struct RobotsTurretAimRuntimeState {
    /// Handler+0x644.
    pub yaw_radians: f32,
    /// Handler+0x648.
    pub target_yaw_radians: f32,
    /// Handler+0x5EC. This is the pitched-turret animation parameter, not body yaw.
    pub pitch_radians: f32,
    /// Handler+0x64C, toggled by owner vslots +0x16C/+0x170.
    pub tracking_latch: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct RobotsTurretYawStep {
    pub yaw_radians: f32,
    pub delta_radians: f32,
    pub requested_anim_mode: u32,
    pub animator_degrees: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct RobotsEp05TurretYawStep {
    pub yaw_radians: f32,
    pub delta_radians: f32,
    pub animator_degrees: f32,
    pub movement_sound_active: bool,
}

/// EP05 vslot +0x168 (`0x004616C0`). Native normalizes current and target
/// angles into [0, 2PI), then clamps the direct difference by 1.5 degrees.
/// This deliberately is not the shortest-angle reducer used by EP02/EP04/EP06.
pub fn step_ep05_turret_yaw_toward(
    state: &mut RobotsTurretAimRuntimeState,
    owner_yaw_radians: f32,
    target_yaw_radians: f32,
) -> RobotsEp05TurretYawStep {
    fn normalize(angle: f32) -> f32 {
        if angle.is_finite() {
            angle.rem_euclid(std::f32::consts::TAU)
        } else {
            angle
        }
    }

    state.target_yaw_radians = normalize(target_yaw_radians);
    state.yaw_radians = normalize(state.yaw_radians);
    let delta = (state.target_yaw_radians - state.yaw_radians)
        .clamp(-ROBOTS_EP05_YAW_STEP_RADIANS, ROBOTS_EP05_YAW_STEP_RADIANS);
    state.yaw_radians += delta;
    RobotsEp05TurretYawStep {
        yaw_radians: state.yaw_radians,
        delta_radians: delta,
        animator_degrees: shortest_yaw_delta(owner_yaw_radians, state.yaw_radians).to_degrees(),
        movement_sound_active: delta.abs() > ROBOTS_TURRET_ANGLE_EPSILON,
    }
}

pub fn ep05_distance_deactivate_priority(target_distance_squared: Option<f32>) -> u8 {
    match target_distance_squared {
        Some(distance_squared)
            if distance_squared
                <= ROBOTS_EP05_TRACK_TARGET_RADIUS * ROBOTS_EP05_TRACK_TARGET_RADIUS =>
        {
            1
        }
        _ => ROBOTS_EP05_DEACTIVATE_PRIORITY,
    }
}

pub const fn ep05_exit_priority(tracking_latch: bool) -> u8 {
    if tracking_latch {
        ROBOTS_EP05_DEACTIVATE_PRIORITY
    } else {
        1
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum RobotsEp05ActivationWinner {
    TrackTarget,
    DeactivateIdle,
    DeactivateExit,
}

/// EP05 first BehaviorHost (`Handler+0x4C4`) in exact native builder order.
pub fn ep05_activation_winner(
    distance_priority: u8,
    exit_priority: u8,
) -> Option<RobotsEp05ActivationWinner> {
    let mut best = 1;
    let mut winner = None;
    for (priority, candidate) in [
        (
            ROBOTS_EP05_TRACK_TARGET_PRIORITY,
            RobotsEp05ActivationWinner::TrackTarget,
        ),
        (
            distance_priority,
            RobotsEp05ActivationWinner::DeactivateIdle,
        ),
        (exit_priority, RobotsEp05ActivationWinner::DeactivateExit),
    ] {
        if priority > best {
            best = priority;
            winner = Some(candidate);
        }
    }
    winner
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum RobotsEp05CombatWinner {
    IdleAnimator,
    DeactivateIdle,
    DeactivateExit,
    IdleFallback,
    Attack,
}

/// EP05 second BehaviorHost (`Handler+0x4DC`) in exact native builder order.
pub fn ep05_combat_winner(
    distance_priority: u8,
    exit_priority: u8,
    attack_priority: u8,
) -> Option<RobotsEp05CombatWinner> {
    let mut best = 1;
    let mut winner = None;
    for (priority, candidate) in [
        (
            ROBOTS_EP05_TRACK_TARGET_PRIORITY,
            RobotsEp05CombatWinner::IdleAnimator,
        ),
        (distance_priority, RobotsEp05CombatWinner::DeactivateIdle),
        (exit_priority, RobotsEp05CombatWinner::DeactivateExit),
        (
            ROBOTS_EP05_IDLE_PRIORITY,
            RobotsEp05CombatWinner::IdleFallback,
        ),
        (attack_priority, RobotsEp05CombatWinner::Attack),
    ] {
        if priority > best {
            best = priority;
            winner = Some(candidate);
        }
    }
    winner
}

pub fn step_turret_yaw_toward(
    state: &mut RobotsTurretAimRuntimeState,
    owner_yaw_radians: f32,
    target_yaw_radians: f32,
) -> RobotsTurretYawStep {
    state.target_yaw_radians = target_yaw_radians;
    let delta = shortest_yaw_delta(state.yaw_radians, state.target_yaw_radians).clamp(
        -ROBOTS_TURRET_YAW_STEP_RADIANS,
        ROBOTS_TURRET_YAW_STEP_RADIANS,
    );
    state.yaw_radians += delta;
    RobotsTurretYawStep {
        yaw_radians: state.yaw_radians,
        delta_radians: delta,
        requested_anim_mode: ROBOTS_TURRET_YAW_ANIM_MODE,
        animator_degrees: shortest_yaw_delta(owner_yaw_radians, state.yaw_radians).to_degrees(),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum RobotsPitchedTurretAttackPhase {
    RaisePitch,
    Attack,
    LowerPitch,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct RobotsPitchedTurretAttackRuntimeState {
    pub base: RobotsGenericAttackRuntimeState,
    pub phase: RobotsPitchedTurretAttackPhase,
}

impl Default for RobotsPitchedTurretAttackRuntimeState {
    fn default() -> Self {
        Self {
            base: RobotsGenericAttackRuntimeState::default(),
            phase: RobotsPitchedTurretAttackPhase::RaisePitch,
        }
    }
}

pub fn pitched_turret_attack_priority_configured(
    state: RobotsPitchedTurretAttackRuntimeState,
    config: RobotsPitchedTurretAttackConfig,
    input: RobotsGenericAttackGateInput,
) -> u8 {
    generic_attack_priority(state.base, config.attack, input)
}

pub fn pitched_turret_attack_priority(
    state: RobotsPitchedTurretAttackRuntimeState,
    input: RobotsGenericAttackGateInput,
) -> u8 {
    pitched_turret_attack_priority_configured(state, ep02_pitched_attack_behavior_config(), input)
}

pub fn enter_pitched_turret_attack_configured(
    state: &mut RobotsPitchedTurretAttackRuntimeState,
    config: RobotsPitchedTurretAttackConfig,
) {
    enter_generic_attack(&mut state.base, config.attack);
    state.phase = RobotsPitchedTurretAttackPhase::RaisePitch;
}

pub fn enter_pitched_turret_attack(state: &mut RobotsPitchedTurretAttackRuntimeState) {
    enter_pitched_turret_attack_configured(state, ep02_pitched_attack_behavior_config());
}

pub fn leave_pitched_turret_attack_configured(
    state: &mut RobotsPitchedTurretAttackRuntimeState,
    aim: &mut RobotsTurretAimRuntimeState,
    config: RobotsPitchedTurretAttackConfig,
) {
    leave_generic_attack(&mut state.base);
    // Native 0x0046E990 snaps Handler+0x5EC toward the configured rest pitch.
    // Do not animate this tail.
    aim.pitch_radians += shortest_yaw_delta(aim.pitch_radians, config.rest_pitch_radians);
}

pub fn leave_pitched_turret_attack(
    state: &mut RobotsPitchedTurretAttackRuntimeState,
    aim: &mut RobotsTurretAimRuntimeState,
) {
    leave_pitched_turret_attack_configured(state, aim, ep02_pitched_attack_behavior_config());
}

pub fn pitched_turret_attack_setup_idle(state: &mut RobotsPitchedTurretAttackRuntimeState) {
    if state.phase == RobotsPitchedTurretAttackPhase::Attack {
        state.phase = RobotsPitchedTurretAttackPhase::LowerPitch;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct RobotsPitchedTurretAttackStep {
    pub requested_anim_mode: u32,
    pub pitch_radians: f32,
    pub completed: bool,
}

pub fn step_pitched_turret_attack_configured(
    state: &mut RobotsPitchedTurretAttackRuntimeState,
    aim: &mut RobotsTurretAimRuntimeState,
    config: RobotsPitchedTurretAttackConfig,
) -> RobotsPitchedTurretAttackStep {
    // Native Execute `0x0046E700` calls phase monitor `0x0046E760` before
    // pitch service `0x0046E8A0`. Reaching a target this tick therefore changes
    // phase only on the following fixed update.
    match state.phase {
        RobotsPitchedTurretAttackPhase::RaisePitch => {
            if shortest_yaw_delta(aim.pitch_radians, config.attack_pitch_radians).abs()
                < ROBOTS_TURRET_ANGLE_EPSILON
            {
                state.phase = RobotsPitchedTurretAttackPhase::Attack;
            }
        }
        RobotsPitchedTurretAttackPhase::LowerPitch => {
            if shortest_yaw_delta(aim.pitch_radians, config.rest_pitch_radians).abs()
                < ROBOTS_TURRET_ANGLE_EPSILON
            {
                state.base.completion_latch = true;
            }
        }
        RobotsPitchedTurretAttackPhase::Attack => {}
    }

    let (target_pitch, requested_anim_mode) = match state.phase {
        RobotsPitchedTurretAttackPhase::RaisePitch => {
            (config.attack_pitch_radians, config.idle_anim_mode)
        }
        RobotsPitchedTurretAttackPhase::Attack => {
            return RobotsPitchedTurretAttackStep {
                requested_anim_mode: config.attack.primary_anim_mode,
                pitch_radians: aim.pitch_radians,
                completed: false,
            };
        }
        RobotsPitchedTurretAttackPhase::LowerPitch => {
            (config.rest_pitch_radians, config.idle_anim_mode)
        }
    };

    let delta = shortest_yaw_delta(aim.pitch_radians, target_pitch)
        .clamp(-config.pitch_step_radians, config.pitch_step_radians);
    aim.pitch_radians += delta;

    RobotsPitchedTurretAttackStep {
        requested_anim_mode,
        pitch_radians: aim.pitch_radians,
        completed: state.base.completion_latch,
    }
}

pub fn step_pitched_turret_attack(
    state: &mut RobotsPitchedTurretAttackRuntimeState,
    aim: &mut RobotsTurretAimRuntimeState,
) -> RobotsPitchedTurretAttackStep {
    step_pitched_turret_attack_configured(state, aim, ep02_pitched_attack_behavior_config())
}

pub fn tick_pitched_turret_attack(state: &mut RobotsPitchedTurretAttackRuntimeState) {
    tick_generic_attack(&mut state.base);
}

pub const fn track_target_priority() -> u8 {
    ROBOTS_EP02_TRACK_TARGET_PRIORITY
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct RobotsEp06MagneticAnimationRuntimeState {
    pub active: bool,
    pub completed: bool,
}

impl RobotsEp06MagneticAnimationRuntimeState {
    pub fn priority(self, got_hit_latch: bool, query_flags: u32) -> u8 {
        if self.active && !self.completed {
            return ROBOTS_EP06_MAGNETIC_PRIORITY;
        }
        if got_hit_latch && query_flags & 0x0000_4000 != 0 {
            ROBOTS_EP06_MAGNETIC_PRIORITY
        } else {
            1
        }
    }

    pub fn enter(&mut self) {
        self.active = true;
        self.completed = false;
    }

    pub fn leave(&mut self) {
        self.active = false;
    }

    /// EP06 derived magnetic vtable 0x005E6E28 replaces only Execute (+0x10)
    /// with 0x00458450, which requests the configured AnimMode and skips the
    /// base magnetic physics/state-machine service.
    pub const fn requested_anim_mode(self) -> u32 {
        0x0900_002A
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum RobotsEp02PrimaryWinner {
    PitchedAttackGroup,
    ScrambledHit,
}

pub fn ep02_primary_winner(
    attack_group_priority: u8,
    scrambled_priority: u8,
) -> Option<RobotsEp02PrimaryWinner> {
    let mut best = 1;
    let mut winner = None;
    for (priority, candidate) in [
        (
            attack_group_priority,
            RobotsEp02PrimaryWinner::PitchedAttackGroup,
        ),
        (scrambled_priority, RobotsEp02PrimaryWinner::ScrambledHit),
    ] {
        if priority > best {
            best = priority;
            winner = Some(candidate);
        }
    }
    winner
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum RobotsEp04PrimaryWinner {
    AttackGroup,
    CommonHit,
    ScrambledHit,
}

pub fn ep04_primary_winner(
    attack_group_priority: u8,
    common_hit_priority: u8,
    scrambled_priority: u8,
) -> Option<RobotsEp04PrimaryWinner> {
    let mut best = 1;
    let mut winner = None;
    for (priority, candidate) in [
        (attack_group_priority, RobotsEp04PrimaryWinner::AttackGroup),
        (common_hit_priority, RobotsEp04PrimaryWinner::CommonHit),
        (scrambled_priority, RobotsEp04PrimaryWinner::ScrambledHit),
    ] {
        if priority > best {
            best = priority;
            winner = Some(candidate);
        }
    }
    winner
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum RobotsEp02SecondaryWinner {
    TrackTarget,
    ScrambledHit,
}

pub fn ep02_secondary_winner(
    track_priority: u8,
    scrambled_priority: u8,
) -> Option<RobotsEp02SecondaryWinner> {
    let mut best = 1;
    let mut winner = None;
    for (priority, candidate) in [
        (track_priority, RobotsEp02SecondaryWinner::TrackTarget),
        (scrambled_priority, RobotsEp02SecondaryWinner::ScrambledHit),
    ] {
        if priority > best {
            best = priority;
            winner = Some(candidate);
        }
    }
    winner
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum RobotsEp06Host4dcWinner {
    TrackTarget,
    PitchedAttack,
    CommonHit,
    MagneticHit,
    ScrambledHit,
}

pub fn ep06_host_4dc_winner(
    track_priority: u8,
    pitched_priority: u8,
    common_hit_priority: u8,
    magnetic_priority: u8,
    scrambled_priority: u8,
) -> Option<RobotsEp06Host4dcWinner> {
    let mut best = 1;
    let mut winner = None;
    for (priority, candidate) in [
        (track_priority, RobotsEp06Host4dcWinner::TrackTarget),
        (pitched_priority, RobotsEp06Host4dcWinner::PitchedAttack),
        (common_hit_priority, RobotsEp06Host4dcWinner::CommonHit),
        (magnetic_priority, RobotsEp06Host4dcWinner::MagneticHit),
        (scrambled_priority, RobotsEp06Host4dcWinner::ScrambledHit),
    ] {
        if priority > best {
            best = priority;
            winner = Some(candidate);
        }
    }
    winner
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum RobotsEp06Host4c4Winner {
    PitchedAttack,
    CommonHit,
    MagneticHit,
    ScrambledHit,
}

pub fn ep06_host_4c4_winner(
    pitched_priority: u8,
    common_hit_priority: u8,
    magnetic_priority: u8,
    scrambled_priority: u8,
) -> Option<RobotsEp06Host4c4Winner> {
    let mut best = 1;
    let mut winner = None;
    for (priority, candidate) in [
        (pitched_priority, RobotsEp06Host4c4Winner::PitchedAttack),
        (common_hit_priority, RobotsEp06Host4c4Winner::CommonHit),
        (magnetic_priority, RobotsEp06Host4c4Winner::MagneticHit),
        (scrambled_priority, RobotsEp06Host4c4Winner::ScrambledHit),
    ] {
        if priority > best {
            best = priority;
            winner = Some(candidate);
        }
    }
    winner
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ep02_yaw_service_clamps_to_three_degrees_per_tick() {
        let mut state = RobotsTurretAimRuntimeState::default();
        let step = step_turret_yaw_toward(&mut state, 0.0, std::f32::consts::FRAC_PI_2);
        assert!((step.delta_radians - ROBOTS_TURRET_YAW_STEP_RADIANS).abs() < 1.0e-6);
        assert!((step.animator_degrees - 3.0).abs() < 1.0e-4);
        assert_eq!(step.requested_anim_mode, ROBOTS_TURRET_YAW_ANIM_MODE);
    }

    #[test]
    fn pitched_attack_uses_base_priority_and_three_phase_pitch_contract() {
        let config = ep02_pitched_attack_config();
        assert_eq!(config.priority, 0x32);
        assert_eq!(config.outer_radius, 20.0);
        assert_eq!(config.reentry_delay_ticks, 120);
        assert!((config.yaw_tolerance_radians - std::f32::consts::PI / 12.0).abs() < 1.0e-6);

        let mut attack = RobotsPitchedTurretAttackRuntimeState::default();
        let mut aim = RobotsTurretAimRuntimeState::default();
        enter_pitched_turret_attack(&mut attack);
        for _ in 0..60 {
            let _ = step_pitched_turret_attack(&mut attack, &mut aim);
        }
        assert_eq!(attack.phase, RobotsPitchedTurretAttackPhase::RaisePitch);
        assert!((aim.pitch_radians - std::f32::consts::FRAC_PI_2).abs() < 2.0e-6);
        let attack_step = step_pitched_turret_attack(&mut attack, &mut aim);
        assert_eq!(attack.phase, RobotsPitchedTurretAttackPhase::Attack);
        assert_eq!(
            attack_step.requested_anim_mode,
            ROBOTS_EP02_ATTACK_ANIM_MODE
        );
        pitched_turret_attack_setup_idle(&mut attack);
        assert_eq!(attack.phase, RobotsPitchedTurretAttackPhase::LowerPitch);
        for _ in 0..60 {
            let _ = step_pitched_turret_attack(&mut attack, &mut aim);
        }
        assert!(!attack.base.completion_latch);
        let _ = step_pitched_turret_attack(&mut attack, &mut aim);
        assert!(attack.base.completion_latch);

        aim.pitch_radians = 0.4;
        leave_pitched_turret_attack(&mut attack, &mut aim);
        assert!(aim.pitch_radians.abs() < 1.0e-6);
    }

    #[test]
    fn ep06_pitched_config_keeps_native_negative_pitch_and_one_degree_gate() {
        let config = ep06_pitched_attack_config(0x0900_0087, 0x0900_0025);
        assert_eq!(config.attack.outer_radius, 14.0);
        assert_eq!(config.attack.reentry_delay_ticks, 120);
        assert_eq!(config.attack.priority, ROBOTS_GENERIC_ATTACK_PRIORITY);
        assert!(
            (config.attack.yaw_tolerance_radians - std::f32::consts::PI / 180.0).abs() < 1.0e-6
        );
        assert!((config.attack_pitch_radians + 25.0_f32.to_radians()).abs() < 1.0e-6);
        assert!((config.pitch_step_radians - std::f32::consts::PI / 120.0).abs() < 1.0e-6);

        let mut attack = RobotsPitchedTurretAttackRuntimeState::default();
        let mut aim = RobotsTurretAimRuntimeState::default();
        enter_pitched_turret_attack_configured(&mut attack, config);
        for _ in 0..17 {
            let step = step_pitched_turret_attack_configured(&mut attack, &mut aim, config);
            assert_eq!(step.requested_anim_mode, 0x0900_0087);
        }
        assert!((aim.pitch_radians - config.attack_pitch_radians).abs() < 1.0e-5);
        assert_eq!(attack.phase, RobotsPitchedTurretAttackPhase::RaisePitch);
        let attack_step = step_pitched_turret_attack_configured(&mut attack, &mut aim, config);
        assert_eq!(attack.phase, RobotsPitchedTurretAttackPhase::Attack);
        assert_eq!(attack_step.requested_anim_mode, 0x0900_0025);
    }

    #[test]
    fn ep06_two_hosts_keep_native_order_and_priority_sources() {
        let primary_attack = ep06_primary_pitched_attack_config();
        let secondary_attack = ep06_secondary_pitched_attack_config();
        assert_eq!(
            primary_attack.attack.priority,
            ROBOTS_GENERIC_ATTACK_PRIORITY
        );
        assert_eq!(
            secondary_attack.attack.priority,
            ROBOTS_GENERIC_ATTACK_PRIORITY
        );
        assert_eq!(
            primary_attack.idle_anim_mode,
            ROBOTS_EP06_PRIMARY_IDLE_ANIM_MODE
        );
        assert_eq!(
            secondary_attack.idle_anim_mode,
            ROBOTS_EP06_SECONDARY_IDLE_ANIM_MODE
        );

        assert_eq!(
            ep06_host_4dc_winner(
                ROBOTS_EP02_TRACK_TARGET_PRIORITY,
                ROBOTS_GENERIC_ATTACK_PRIORITY,
                100,
                ROBOTS_EP06_MAGNETIC_PRIORITY,
                0x55,
            ),
            Some(RobotsEp06Host4dcWinner::MagneticHit)
        );
        assert_eq!(
            ep06_host_4c4_winner(
                ROBOTS_GENERIC_ATTACK_PRIORITY,
                100,
                ROBOTS_EP06_MAGNETIC_PRIORITY,
                0x55,
            ),
            Some(RobotsEp06Host4c4Winner::MagneticHit)
        );
        assert_eq!(
            ep06_host_4dc_winner(
                ROBOTS_EP02_TRACK_TARGET_PRIORITY,
                ROBOTS_GENERIC_ATTACK_PRIORITY,
                1,
                1,
                1,
            ),
            Some(RobotsEp06Host4dcWinner::PitchedAttack)
        );
    }

    #[test]
    fn ep04_attack_and_primary_host_match_native_builder() {
        let config = ep04_attack_config();
        assert_eq!(config.primary_anim_mode, ROBOTS_EP04_ATTACK_ANIM_MODE);
        assert_eq!(config.inner_radius, 0.0);
        assert_eq!(config.outer_radius, 14.0);
        assert_eq!(config.reentry_delay_ticks, 420);
        assert_eq!(config.priority, ROBOTS_GENERIC_ATTACK_PRIORITY);
        assert!(config.sticky_while_active);
        assert!((config.yaw_tolerance_radians - std::f32::consts::PI / 36.0).abs() < 1.0e-6);

        assert_eq!(
            ep04_primary_winner(ROBOTS_GENERIC_ATTACK_PRIORITY, 100, 0x55),
            Some(RobotsEp04PrimaryWinner::CommonHit)
        );
        assert_eq!(
            ep04_primary_winner(ROBOTS_GENERIC_ATTACK_PRIORITY, 1, 0x55),
            Some(RobotsEp04PrimaryWinner::ScrambledHit)
        );
        assert_eq!(
            ep04_primary_winner(ROBOTS_GENERIC_ATTACK_PRIORITY, 1, 1),
            Some(RobotsEp04PrimaryWinner::AttackGroup)
        );
    }

    #[test]
    fn ep05_builder_contract_and_two_hosts_keep_native_order() {
        let config = ep05_attack_config();
        assert_eq!(config.primary_anim_mode, ROBOTS_EP05_ATTACK_ANIM_MODE);
        assert_eq!(config.outer_radius, 10.0);
        assert_eq!(config.reentry_delay_ticks, 0);
        assert_eq!(config.priority, ROBOTS_GENERIC_ATTACK_PRIORITY);
        assert!((config.yaw_tolerance_radians - std::f32::consts::PI / 12.0).abs() < 1.0e-6);

        let near = ep05_distance_deactivate_priority(Some(100.0));
        let far = ep05_distance_deactivate_priority(Some(100.01));
        assert_eq!(near, 1);
        assert_eq!(far, ROBOTS_EP05_DEACTIVATE_PRIORITY);
        assert_eq!(
            ep05_activation_winner(far, ROBOTS_EP05_DEACTIVATE_PRIORITY),
            Some(RobotsEp05ActivationWinner::DeactivateIdle)
        );
        assert_eq!(
            ep05_activation_winner(near, ROBOTS_EP05_DEACTIVATE_PRIORITY),
            Some(RobotsEp05ActivationWinner::DeactivateExit)
        );
        assert_eq!(
            ep05_combat_winner(near, 1, ROBOTS_GENERIC_ATTACK_PRIORITY),
            Some(RobotsEp05CombatWinner::Attack)
        );
        assert_eq!(
            ep05_combat_winner(
                far,
                ROBOTS_EP05_DEACTIVATE_PRIORITY,
                ROBOTS_GENERIC_ATTACK_PRIORITY
            ),
            Some(RobotsEp05CombatWinner::DeactivateIdle)
        );
    }

    #[test]
    fn ep05_yaw_service_uses_native_direct_one_point_five_degree_step() {
        let mut state = RobotsTurretAimRuntimeState::default();
        let step = step_ep05_turret_yaw_toward(&mut state, 0.0, std::f32::consts::FRAC_PI_2);
        assert!((step.delta_radians - ROBOTS_EP05_YAW_STEP_RADIANS).abs() < 1.0e-6);
        assert!(step.movement_sound_active);

        state.yaw_radians = 0.0;
        let wrapped = step_ep05_turret_yaw_toward(&mut state, 0.0, -0.1);
        assert!((wrapped.delta_radians - ROBOTS_EP05_YAW_STEP_RADIANS).abs() < 1.0e-6);
    }

    #[test]
    fn ep02_two_hosts_keep_native_strict_order() {
        assert_eq!(
            ep02_primary_winner(0x32, 0x55),
            Some(RobotsEp02PrimaryWinner::ScrambledHit)
        );
        assert_eq!(
            ep02_secondary_winner(0x1e, 0x55),
            Some(RobotsEp02SecondaryWinner::ScrambledHit)
        );
        assert_eq!(
            ep02_secondary_winner(0x1e, 1),
            Some(RobotsEp02SecondaryWinner::TrackTarget)
        );
    }
}
