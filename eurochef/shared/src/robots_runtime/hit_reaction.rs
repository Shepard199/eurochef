use serde::Serialize;

use super::locomotion::shortest_yaw_delta;

pub const ROBOTS_HIT_CALLBACK_NOOP_TARGET: u32 = 0x0040_5F80;
pub const ROBOTS_HIT_CALLBACK_SCRIPT_LATCH_TARGET: u32 = 0x0041_7950;
pub const ROBOTS_HIT_CALLBACK_AI_CHARACTER_TARGET: u32 = 0x0045_3480;
pub const ROBOTS_HIT_CALLBACK_TURRET_BOT_TARGET: u32 = 0x0045_D140;
pub const ROBOTS_HIT_CALLBACK_TURRET_TARGET: u32 = 0x0046_09F0;
pub const ROBOTS_HIT_CALLBACK_DODGEM_TARGET: u32 = 0x0046_2710;
pub const ROBOTS_HIT_CALLBACK_ROLLER_BOT_TARGET: u32 = 0x0046_2F60;
pub const ROBOTS_HIT_CALLBACK_EF01_MINE_TARGET: u32 = 0x0046_3920;
pub const ROBOTS_HIT_CALLBACK_WATCHBOT_TARGET: u32 = 0x0049_0C10;
pub const ROBOTS_HIT_CALLBACK_PLAYER_BALL_TARGET: u32 = 0x004B_4A60;
pub const ROBOTS_HIT_CALLBACK_PLAYER_TARGET: u32 = 0x004B_A8E0;
pub const ROBOTS_HIT_CALLBACK_BOSS_EXEC_TARGET: u32 = 0x004C_99E0;
pub const ROBOTS_HIT_CALLBACK_BOSS_SEWER_TARGET: u32 = 0x004C_BE10;
pub const ROBOTS_HIT_CALLBACK_BOSS_SEWER_CANON_TARGET: u32 = 0x004C_CC20;
pub const ROBOTS_HIT_CALLBACK_SWEEPER_BOSS_TARGET: u32 = 0x004C_F410;
pub const ROBOTS_HIT_CALLBACK_SWEEPER_RATCHET_TARGET: u32 = 0x004C_FB60;
pub const ROBOTS_HIT_CALLBACK_RATCHET_MISSILE_TARGET: u32 = 0x004D_01C0;

pub const ROBOTS_HIT_FLAG_DOUBLE_AI_DAMAGE: u32 = 0x0000_0100;
pub const ROBOTS_HIT_FLAG_AI_DAMAGE_SUPPRESSION_MASK: u32 = 0x0000_401C;
pub const ROBOTS_HIT_FLAG_AI_FORCE_ZERO_NON_EXEMPT: u32 = 0x0004_0000;
pub const ROBOTS_HIT_FLAG_AI_REQUIRE_OWNER_SOURCE: u32 = 0x0008_0000;
pub const ROBOTS_HIT_FLAG_ALLOW_OWNER_SOURCE: u32 = 0x0020_0000;
pub const ROBOTS_HIT_METADATA_REQUIRES_AI_CAPABILITY: u16 = 0x8000;
pub const ROBOTS_AI_HIT_CAPABILITY_FLAG: u32 = 0x0000_0008;
pub const ROBOTS_AI_HIT_EXEMPT_OWNER_CATEGORY_MIN: u32 = 0x0E;
pub const ROBOTS_AI_HIT_EXEMPT_OWNER_CATEGORY_MAX: u32 = 0x15;
/// Type value passed to the embedded Handler collection at +0x13C through
/// `0x0042A670`. This is not the AI behavior-selector list at +0x4C4..+0x4D0.
pub const ROBOTS_AI_HIT_EMBEDDED_TYPE: u32 = 3;
pub const ROBOTS_ANIM_MODE_HIT_DEATH_F: u32 = 0x0900_0033;
pub const ROBOTS_ANIM_MODE_HIT_DEATH_B: u32 = 0x0900_0032;
pub const ROBOTS_ANIM_MODE_HIT_FRONT: u32 = 0x0900_0029;
pub const ROBOTS_ANIM_MODE_HIT_BACK: u32 = 0x0900_002A;
pub const ROBOTS_AI_HIT_NODE_PRIORITY: u8 = 100;
pub const ROBOTS_AI_HIT_STATUS_EXCLUSION_MASK: u32 = 0x0000_4018;
pub const ROBOTS_AI_HIT_FRONT_HALF_ANGLE_RADIANS: f32 = std::f32::consts::FRAC_PI_2;
pub const ROBOTS_AI_HIT_TURN_MAX_YAW_PER_TICK: f32 = std::f32::consts::PI / 60.0;
pub const ROBOTS_AI_HIT_YAW_EPSILON: f32 = 0.001;
pub const ROBOTS_HIT_FLAG_DERIVED_FORCE_ZERO: u32 = 0x0040_0000;
pub const ROBOTS_HIT_FLAG_DODGEM_FORCE_ZERO: u32 = 0x0000_0010;
pub const ROBOTS_HIT_FLAG_EF01_MINE_USE_COMMON_AI: u32 = 0x0000_0010;
pub const ROBOTS_TURRET_HIT_EFFECT_UID: u32 = 0x1AF0_026C;
/// Native `DAT_005E23BC[0..2]` selected by `AI_HitFatal` event `0x16000039`
/// before calling common Monster action dispatcher `0x004550A0`. The event
/// payload is an index into this table, not the probability modulus itself.
pub const ROBOTS_MONSTER_EXPLOSION_ACTION_MODULI: [u32; 3] = [1, 3, 3];
pub const ROBOTS_MONSTER_ACTION_ALT_EXPLOSION_FLAG: u32 = 0x0004_0000;

pub const fn robots_monster_explosion_action_modulus(selector: u32) -> u32 {
    if selector < ROBOTS_MONSTER_EXPLOSION_ACTION_MODULI.len() as u32 {
        ROBOTS_MONSTER_EXPLOSION_ACTION_MODULI[selector as usize]
    } else {
        ROBOTS_MONSTER_EXPLOSION_ACTION_MODULI[0]
    }
}

/// Native common Monster action `0x004550A0` chooses Handler+0x630 only when
/// Handler+0x628 bit0x40000 is set; otherwise it uses Handler+0x634.
pub const fn robots_monster_action_explosion_uid(
    handler_flags_628: u32,
    explosion_uid_634: u32,
    explosion_uid_630: u32,
) -> u32 {
    if handler_flags_628 & ROBOTS_MONSTER_ACTION_ALT_EXPLOSION_FLAG != 0 {
        explosion_uid_630
    } else {
        explosion_uid_634
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct RobotsCommonAiHitConfig {
    /// `AI_Hit::setup 0x00457A40` fields +0x24/+0x28/+0x2C/+0x30.
    pub front_anim_mode: u32,
    pub back_anim_mode: u32,
    pub left_anim_mode: u32,
    pub right_anim_mode: u32,
    /// Native setup field +0x40. `0x00457A90` divides this by the 60 Hz
    /// fixed-step denominator before clamping yaw.
    pub turn_rate_radians_per_second: f32,
}

impl RobotsCommonAiHitConfig {
    /// Common front/back hit family used by RollerBot and EW11 FatBot.
    pub const fn common_front_back() -> Self {
        Self {
            front_anim_mode: ROBOTS_ANIM_MODE_HIT_FRONT,
            back_anim_mode: ROBOTS_ANIM_MODE_HIT_BACK,
            left_anim_mode: 0,
            right_anim_mode: 0,
            turn_rate_radians_per_second: std::f32::consts::PI,
        }
    }

    pub const fn rollerbot() -> Self {
        Self::common_front_back()
    }

    pub const fn fast_front_back() -> Self {
        Self {
            front_anim_mode: ROBOTS_ANIM_MODE_HIT_FRONT,
            back_anim_mode: ROBOTS_ANIM_MODE_HIT_BACK,
            left_anim_mode: 0,
            right_anim_mode: 0,
            turn_rate_radians_per_second: std::f32::consts::TAU * 2.0,
        }
    }

    /// EB11 MagnaBot builder `0x00464810`: `AI_Hit` modes 29/2A with a zero
    /// turn-rate field. Do not substitute the otherwise similar fast-front/back
    /// configuration, whose +0x40 is 4*PI.
    pub const fn magnabot() -> Self {
        Self {
            front_anim_mode: ROBOTS_ANIM_MODE_HIT_FRONT,
            back_anim_mode: ROBOTS_ANIM_MODE_HIT_BACK,
            left_anim_mode: 0,
            right_anim_mode: 0,
            turn_rate_radians_per_second: 0.0,
        }
    }

    /// EB12 EvilBot builder `0x00465260`: modes 29/2A at exactly 2*PI rad/s.
    pub const fn evilbot() -> Self {
        Self {
            front_anim_mode: ROBOTS_ANIM_MODE_HIT_FRONT,
            back_anim_mode: ROBOTS_ANIM_MODE_HIT_BACK,
            left_anim_mode: 0,
            right_anim_mode: 0,
            turn_rate_radians_per_second: std::f32::consts::TAU,
        }
    }

    /// SawBot builder `0x0045C0A0`.
    pub const fn sawbot() -> Self {
        Self::fast_front_back()
    }

    /// SpikeBot builder `0x0045BC50`.
    pub const fn spikebot() -> Self {
        Self::fast_front_back()
    }

    /// Normal ShuntBot non-fatal `AI_Hit` builder `0x0045C620`.
    pub const fn shuntbot() -> Self {
        Self {
            front_anim_mode: 0,
            back_anim_mode: 0x0900_002A,
            left_anim_mode: 0x0900_003C,
            right_anim_mode: 0x0900_003D,
            turn_rate_radians_per_second: std::f32::consts::TAU * 2.0,
        }
    }

    pub const fn common_fatal() -> Self {
        Self {
            front_anim_mode: ROBOTS_ANIM_MODE_HIT_DEATH_F,
            back_anim_mode: ROBOTS_ANIM_MODE_HIT_DEATH_B,
            left_anim_mode: 0,
            right_anim_mode: 0,
            turn_rate_radians_per_second: std::f32::consts::PI,
        }
    }

    /// EP04 builder `0x00460E80` installs an ordinary non-fatal AI_Hit with
    /// only AnimMode29 populated and zero turn-rate/alternate direction modes.
    /// EP06 Handler+0x4DC uses the same single-mode node.
    pub const fn ep04_turret() -> Self {
        Self {
            front_anim_mode: ROBOTS_ANIM_MODE_HIT_FRONT,
            back_anim_mode: 0,
            left_anim_mode: 0,
            right_anim_mode: 0,
            turn_rate_radians_per_second: 0.0,
        }
    }

    /// EP06 Handler+0x4C4 installs the same zero-turn AI_Hit shape with only
    /// AnimMode2A populated.
    pub const fn ep06_secondary_turret() -> Self {
        Self {
            front_anim_mode: ROBOTS_ANIM_MODE_HIT_BACK,
            back_anim_mode: 0,
            left_anim_mode: 0,
            right_anim_mode: 0,
            turn_rate_radians_per_second: 0.0,
        }
    }

    /// EP02/EP04/EP05/EP06 turret builders all install `AI_HitFatal` with only
    /// AnimMode33 populated and zero turn-rate/alternate direction modes.
    pub const fn turret_fatal() -> Self {
        Self {
            front_anim_mode: ROBOTS_ANIM_MODE_HIT_DEATH_F,
            back_anim_mode: 0,
            left_anim_mode: 0,
            right_anim_mode: 0,
            turn_rate_radians_per_second: 0.0,
        }
    }

    /// EW07 Dodgem builder `0x00462360`: only AnimMode33 is populated and
    /// native +0x40 is exactly 4*PI radians/second.
    pub const fn dodgem_fatal() -> Self {
        Self {
            front_anim_mode: ROBOTS_ANIM_MODE_HIT_DEATH_F,
            back_anim_mode: 0,
            left_anim_mode: 0,
            right_anim_mode: 0,
            turn_rate_radians_per_second: std::f32::consts::TAU * 2.0,
        }
    }

    /// TurretBot builder `0x0045CE30`: modes 33/32/3A/3B and +0x40 = 4*PI.
    pub const fn turretbot_fatal() -> Self {
        Self {
            front_anim_mode: ROBOTS_ANIM_MODE_HIT_DEATH_F,
            back_anim_mode: ROBOTS_ANIM_MODE_HIT_DEATH_B,
            left_anim_mode: 0x0900_003A,
            right_anim_mode: 0x0900_003B,
            turn_rate_radians_per_second: std::f32::consts::TAU * 2.0,
        }
    }

    pub const fn fast_front_back_fatal() -> Self {
        Self {
            front_anim_mode: ROBOTS_ANIM_MODE_HIT_DEATH_F,
            back_anim_mode: ROBOTS_ANIM_MODE_HIT_DEATH_B,
            left_anim_mode: 0,
            right_anim_mode: 0,
            turn_rate_radians_per_second: std::f32::consts::TAU * 2.0,
        }
    }

    /// EB12 EvilBot builder `0x00465260`: fatal modes 33/32 at exactly 2*PI rad/s.
    pub const fn evilbot_fatal() -> Self {
        Self {
            front_anim_mode: ROBOTS_ANIM_MODE_HIT_DEATH_F,
            back_anim_mode: ROBOTS_ANIM_MODE_HIT_DEATH_B,
            left_anim_mode: 0,
            right_anim_mode: 0,
            turn_rate_radians_per_second: std::f32::consts::TAU,
        }
    }

    /// EB11 MagnaBot `AI_HitFatal` modes 33/32 with native +0x40 = 0.
    pub const fn magnabot_fatal() -> Self {
        Self {
            front_anim_mode: ROBOTS_ANIM_MODE_HIT_DEATH_F,
            back_anim_mode: ROBOTS_ANIM_MODE_HIT_DEATH_B,
            left_anim_mode: 0,
            right_anim_mode: 0,
            turn_rate_radians_per_second: 0.0,
        }
    }

    pub const fn sawbot_fatal() -> Self {
        Self::fast_front_back_fatal()
    }

    pub const fn spikebot_fatal() -> Self {
        Self::fast_front_back_fatal()
    }

    /// ShuntBot and ShuntBotBoss `AI_HitFatal` setup in builders
    /// `0x0045C620` / `0x0045CAF0`.
    pub const fn shunt_fatal() -> Self {
        Self {
            front_anim_mode: 0,
            back_anim_mode: 0x0900_0032,
            left_anim_mode: 0x0900_003A,
            right_anim_mode: 0x0900_003B,
            turn_rate_radians_per_second: std::f32::consts::TAU * 2.0,
        }
    }
}

/// Exact four-way selector in common `AI_Hit::enter 0x00457C40`.
pub fn robots_common_ai_hit_direction_selection(
    config: RobotsCommonAiHitConfig,
    owner_yaw_radians: f32,
    source_yaw_radians: f32,
) -> (u32, f32) {
    let delta = shortest_yaw_delta(owner_yaw_radians + std::f32::consts::PI, source_yaw_radians);
    let abs_delta = delta.abs();

    if (config.front_anim_mode != 0 && abs_delta < std::f32::consts::FRAC_PI_4)
        || ((config.left_anim_mode == 0 || config.right_anim_mode == 0)
            && abs_delta < std::f32::consts::FRAC_PI_2)
        || config.back_anim_mode == 0
    {
        return (
            config.front_anim_mode,
            source_yaw_radians + std::f32::consts::PI,
        );
    }

    if config.left_anim_mode != 0
        && config.right_anim_mode != 0
        && abs_delta < 3.0 * std::f32::consts::FRAC_PI_4
    {
        if delta >= 0.0 {
            return (
                config.left_anim_mode,
                source_yaw_radians + std::f32::consts::FRAC_PI_2,
            );
        }
        return (
            config.right_anim_mode,
            source_yaw_radians - std::f32::consts::FRAC_PI_2,
        );
    }

    (config.back_anim_mode, source_yaw_radians)
}

/// Two-way front/back AnimMode selection used by native common hit nodes when
/// left/right modes are zero. `0x00457C40` compares source XItem yaw against
/// owner yaw + PI through `0x004F21DB`; `DAT_005DD734` is exactly PI/2 and the
/// native comparison is strict `<`.
pub fn robots_ai_hit_front_back_selection(
    owner_yaw_radians: f32,
    source_yaw_radians: f32,
    front_anim_mode: u32,
    back_anim_mode: u32,
) -> (u32, f32) {
    let delta =
        shortest_yaw_delta(owner_yaw_radians + std::f32::consts::PI, source_yaw_radians).abs();
    if delta < ROBOTS_AI_HIT_FRONT_HALF_ANGLE_RADIANS {
        (front_anim_mode, source_yaw_radians + std::f32::consts::PI)
    } else {
        (back_anim_mode, source_yaw_radians)
    }
}

pub fn robots_ai_hit_front_back_anim_mode(
    owner_yaw_radians: f32,
    source_yaw_radians: f32,
    front_anim_mode: u32,
    back_anim_mode: u32,
) -> u32 {
    robots_ai_hit_front_back_selection(
        owner_yaw_radians,
        source_yaw_radians,
        front_anim_mode,
        back_anim_mode,
    )
    .0
}

/// Common non-fatal `AI_Hit` behavior node (`0x00457980`, vtable `0x005E23D0`).
/// The class-specific +0xC8 callback owns damage; this state only owns selector,
/// facing and animation lifecycle after an admitted hit.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct RobotsCommonAiHitRuntimeState {
    pub active: bool,
    pub completion_latch: bool,
    pub requested_anim_mode: u32,
    pub target_yaw_radians: f32,
    pub captured_query_serial: u16,
}

impl Default for RobotsCommonAiHitRuntimeState {
    fn default() -> Self {
        Self {
            active: false,
            completion_latch: false,
            requested_anim_mode: 0,
            target_yaw_radians: 0.0,
            captured_query_serial: u16::MAX,
        }
    }
}

impl RobotsCommonAiHitRuntimeState {
    /// `0x00457BE0` specialized priority gate plus the common base active-node gate.
    pub fn priority(self, hit: RobotsAiHitReactionState) -> u8 {
        if self.active && !self.completion_latch {
            return ROBOTS_AI_HIT_NODE_PRIORITY;
        }
        if hit.health != 0
            && hit.got_hit_latch
            && hit.query_flags_snapshot & ROBOTS_AI_HIT_STATUS_EXCLUSION_MASK == 0
        {
            ROBOTS_AI_HIT_NODE_PRIORITY
        } else {
            1
        }
    }

    /// `0x00457C40` for the common front/back configuration used by RollerBot.
    /// Left/right AnimModes are zero in that builder, so the existing exact
    /// front/back selector is sufficient here.
    pub fn enter(
        &mut self,
        owner_yaw_radians: f32,
        source_yaw_radians: f32,
        query_serial: u16,
    ) -> u32 {
        self.enter_configured(
            RobotsCommonAiHitConfig::rollerbot(),
            owner_yaw_radians,
            source_yaw_radians,
            query_serial,
        )
    }

    pub fn enter_configured(
        &mut self,
        config: RobotsCommonAiHitConfig,
        owner_yaw_radians: f32,
        source_yaw_radians: f32,
        query_serial: u16,
    ) -> u32 {
        let (anim_mode, target_yaw) =
            robots_common_ai_hit_direction_selection(config, owner_yaw_radians, source_yaw_radians);
        self.active = true;
        self.completion_latch = false;
        self.requested_anim_mode = anim_mode;
        self.target_yaw_radians = target_yaw;
        self.captured_query_serial = query_serial;
        anim_mode
    }

    /// Compatibility wrapper for the RollerBot builder's explicit PI rad/s turn rate.
    pub fn step_owner_yaw(&self, owner_yaw_radians: f32) -> Option<f32> {
        self.step_owner_yaw_configured(RobotsCommonAiHitConfig::rollerbot(), owner_yaw_radians)
    }

    pub fn step_owner_yaw_configured(
        &self,
        config: RobotsCommonAiHitConfig,
        owner_yaw_radians: f32,
    ) -> Option<f32> {
        if !self.active {
            return None;
        }
        let delta = shortest_yaw_delta(owner_yaw_radians, self.target_yaw_radians);
        let max_yaw_per_tick = config.turn_rate_radians_per_second / 60.0;
        (delta.abs() > ROBOTS_AI_HIT_YAW_EPSILON)
            .then_some(owner_yaw_radians + delta.clamp(-max_yaw_per_tick, max_yaw_per_tick))
    }

    /// ScriptEvent SetupIdle (`0x16000001`) handled by `0x00457E00`.
    pub fn setup_idle(&mut self, hit: &mut RobotsAiHitReactionState) {
        self.completion_latch = true;
        hit.got_hit_latch = false;
    }

    /// `0x00457DD0` clears both query-serial lanes when leaving the node.
    pub fn leave(&mut self, hit: &mut RobotsAiHitReactionState) {
        self.active = false;
        hit.query_serial_snapshot = 0;
        hit.last_query_serial = 0;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RobotsHitReactionFamily {
    NoOp,
    ScriptHitLatch,
    AiCharacterDamage,
    TurretBot,
    Turret,
    Dodgem,
    RollerBot,
    Ef01Mine,
    WatchBot,
    PlayerBall,
    Player,
    BossExec,
    BossSewer,
    BossSewerCanon,
    SweeperBoss,
    SweeperRatchet,
    RatchetMissile,
}

impl RobotsHitReactionFamily {
    pub fn from_native_target(target: u32) -> Option<Self> {
        match target {
            ROBOTS_HIT_CALLBACK_NOOP_TARGET => Some(Self::NoOp),
            ROBOTS_HIT_CALLBACK_SCRIPT_LATCH_TARGET => Some(Self::ScriptHitLatch),
            ROBOTS_HIT_CALLBACK_AI_CHARACTER_TARGET => Some(Self::AiCharacterDamage),
            ROBOTS_HIT_CALLBACK_TURRET_BOT_TARGET => Some(Self::TurretBot),
            ROBOTS_HIT_CALLBACK_TURRET_TARGET => Some(Self::Turret),
            ROBOTS_HIT_CALLBACK_DODGEM_TARGET => Some(Self::Dodgem),
            ROBOTS_HIT_CALLBACK_ROLLER_BOT_TARGET => Some(Self::RollerBot),
            ROBOTS_HIT_CALLBACK_EF01_MINE_TARGET => Some(Self::Ef01Mine),
            ROBOTS_HIT_CALLBACK_WATCHBOT_TARGET => Some(Self::WatchBot),
            ROBOTS_HIT_CALLBACK_PLAYER_BALL_TARGET => Some(Self::PlayerBall),
            ROBOTS_HIT_CALLBACK_PLAYER_TARGET => Some(Self::Player),
            ROBOTS_HIT_CALLBACK_BOSS_EXEC_TARGET => Some(Self::BossExec),
            ROBOTS_HIT_CALLBACK_BOSS_SEWER_TARGET => Some(Self::BossSewer),
            ROBOTS_HIT_CALLBACK_BOSS_SEWER_CANON_TARGET => Some(Self::BossSewerCanon),
            ROBOTS_HIT_CALLBACK_SWEEPER_BOSS_TARGET => Some(Self::SweeperBoss),
            ROBOTS_HIT_CALLBACK_SWEEPER_RATCHET_TARGET => Some(Self::SweeperRatchet),
            ROBOTS_HIT_CALLBACK_RATCHET_MISSILE_TARGET => Some(Self::RatchetMissile),
            _ => None,
        }
    }

    pub fn native_target(self) -> u32 {
        match self {
            Self::NoOp => ROBOTS_HIT_CALLBACK_NOOP_TARGET,
            Self::ScriptHitLatch => ROBOTS_HIT_CALLBACK_SCRIPT_LATCH_TARGET,
            Self::AiCharacterDamage => ROBOTS_HIT_CALLBACK_AI_CHARACTER_TARGET,
            Self::TurretBot => ROBOTS_HIT_CALLBACK_TURRET_BOT_TARGET,
            Self::Turret => ROBOTS_HIT_CALLBACK_TURRET_TARGET,
            Self::Dodgem => ROBOTS_HIT_CALLBACK_DODGEM_TARGET,
            Self::RollerBot => ROBOTS_HIT_CALLBACK_ROLLER_BOT_TARGET,
            Self::Ef01Mine => ROBOTS_HIT_CALLBACK_EF01_MINE_TARGET,
            Self::WatchBot => ROBOTS_HIT_CALLBACK_WATCHBOT_TARGET,
            Self::PlayerBall => ROBOTS_HIT_CALLBACK_PLAYER_BALL_TARGET,
            Self::Player => ROBOTS_HIT_CALLBACK_PLAYER_TARGET,
            Self::BossExec => ROBOTS_HIT_CALLBACK_BOSS_EXEC_TARGET,
            Self::BossSewer => ROBOTS_HIT_CALLBACK_BOSS_SEWER_TARGET,
            Self::BossSewerCanon => ROBOTS_HIT_CALLBACK_BOSS_SEWER_CANON_TARGET,
            Self::SweeperBoss => ROBOTS_HIT_CALLBACK_SWEEPER_BOSS_TARGET,
            Self::SweeperRatchet => ROBOTS_HIT_CALLBACK_SWEEPER_RATCHET_TARGET,
            Self::RatchetMissile => ROBOTS_HIT_CALLBACK_RATCHET_MISSILE_TARGET,
        }
    }
}

/// Fields of the query snapshot that the three recovered common +0xC8 reactions
/// actually consume after native `0x00425C70` has copied query `+0x04..+0x95`
/// into candidate Handler `+0x2F0..+0x381`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RobotsAcceptedHitReactionInput {
    /// Native snapshot Handler +0x378, copied from query +0x8C.
    pub query_flags: u32,
    /// Native snapshot Handler +0x37C, copied from query +0x90.
    pub query_serial: u16,
    /// Native snapshot Handler +0x37E, copied from query +0x92.
    pub hit_metadata: u16,
    /// Identity comparison of snapshot source Handler +0x330 against candidate
    /// owner XItem. Pointer identity stays host-side for the UE adapter.
    pub source_is_candidate_owner: bool,
    /// Identity comparison of snapshot secondary source +0x334 against candidate
    /// owner XItem.
    pub secondary_source_is_candidate_owner: bool,
}

/// Result of the candidate Handler reaction. This is deliberately distinct from
/// the outer query result: native `0x00425C70` commits candidate XItem to query
/// +0x4C and returns geometric hit even when +0xC8 itself returns 0.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RobotsHitReactionResult {
    pub reaction_accepted: bool,
    pub query_hit_still_commits: bool,
    /// Native common-AI path calls `0x0042A670(3)` on the embedded collection at
    /// Handler+0x13C. This is separate from the AI behavior-selector list.
    pub activate_embedded_type: Option<u32>,
    /// Damage actually removed from the AI byte-health field +0x62E.
    pub health_damage: u8,
}

impl RobotsHitReactionResult {
    fn rejected() -> Self {
        Self {
            reaction_accepted: false,
            query_hit_still_commits: true,
            activate_embedded_type: None,
            health_damage: 0,
        }
    }
}

/// Native script/object family `0x00417950`.
pub fn robots_apply_script_hit_latch(latch_byte: &mut u8) -> RobotsHitReactionResult {
    *latch_byte |= 1;
    RobotsHitReactionResult {
        reaction_accepted: true,
        query_hit_still_commits: true,
        activate_embedded_type: None,
        health_damage: 0,
    }
}

/// Native `0x00405F80`.
pub fn robots_apply_noop_hit_reaction() -> RobotsHitReactionResult {
    RobotsHitReactionResult::rejected()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RobotsAiHitReactionState {
    /// Native Handler +0x378, copied from query +0x8C before the +0xC8 callback.
    pub query_flags_snapshot: u32,
    /// Native Handler +0x37C, copied from query +0x90 before the +0xC8 callback.
    pub query_serial_snapshot: u16,
    /// Native Handler +0x37E, copied from query +0x92 before the +0xC8 callback.
    pub hit_metadata_snapshot: u16,
    /// Native Handler +0x62C. `0xFFFF` has the same sentinel meaning as query -1.
    pub last_query_serial: u16,
    /// Native Handler +0x628. Only bit 0x08 is consumed by common +0xC8.
    pub capability_flags: u32,
    /// Native Handler +0x62E byte health/damage counter.
    pub health: u8,
    /// Native Handler +0x608, set to 1 on every admitted reaction.
    pub got_hit_latch: bool,
    /// Native owner XItem +0x264 category used by the two force-damage branches.
    pub owner_category: u32,
}

impl RobotsAiHitReactionState {
    fn owner_category_is_exempt(self) -> bool {
        (ROBOTS_AI_HIT_EXEMPT_OWNER_CATEGORY_MIN..=ROBOTS_AI_HIT_EXEMPT_OWNER_CATEGORY_MAX)
            .contains(&self.owner_category)
    }

    /// Exact common AI/NPC/Monster +0xC8 reducer `0x00453480`.
    ///
    /// `debug_force_kill_non_exempt` is executable global `DAT_007B2A40` exposed
    /// as an explicit host input rather than hidden global state.
    pub fn apply_hit(
        &mut self,
        input: RobotsAcceptedHitReactionInput,
        debug_force_kill_non_exempt: bool,
    ) -> RobotsHitReactionResult {
        // Native `0x00425C70` copies the query snapshot into Handler
        // +0x2F0..+0x381 before invoking +0xC8. Preserve those three lanes even
        // when the callback itself rejects the hit (for example duplicate serial),
        // because behavior nodes such as MalfBot status-hit gates read +0x378.
        self.query_flags_snapshot = input.query_flags;
        self.query_serial_snapshot = input.query_serial;
        self.hit_metadata_snapshot = input.hit_metadata;

        if input.query_serial != u16::MAX && input.query_serial == self.last_query_serial {
            return RobotsHitReactionResult::rejected();
        }

        let owner_is_explicit_source =
            input.source_is_candidate_owner || input.secondary_source_is_candidate_owner;
        if owner_is_explicit_source {
            if input.query_flags & ROBOTS_HIT_FLAG_ALLOW_OWNER_SOURCE == 0 {
                return RobotsHitReactionResult::rejected();
            }
        } else if input.query_flags & ROBOTS_HIT_FLAG_AI_REQUIRE_OWNER_SOURCE != 0 {
            return RobotsHitReactionResult::rejected();
        }

        if input.hit_metadata & ROBOTS_HIT_METADATA_REQUIRES_AI_CAPABILITY != 0
            && self.capability_flags & ROBOTS_AI_HIT_CAPABILITY_FLAG == 0
        {
            return RobotsHitReactionResult::rejected();
        }

        self.last_query_serial = input.query_serial;
        let before_health = self.health;

        if input.query_flags & ROBOTS_HIT_FLAG_AI_DAMAGE_SUPPRESSION_MASK == 0 {
            let mut requested_damage = if input.query_flags & ROBOTS_HIT_FLAG_DOUBLE_AI_DAMAGE != 0
            {
                2
            } else {
                1
            };
            if debug_force_kill_non_exempt && !self.owner_category_is_exempt() {
                requested_damage = self.health;
            }
            self.health = self
                .health
                .saturating_sub(requested_damage.min(self.health));
        }

        if input.query_flags & ROBOTS_HIT_FLAG_AI_FORCE_ZERO_NON_EXEMPT != 0
            && !self.owner_category_is_exempt()
        {
            self.health = 0;
        }

        self.got_hit_latch = true;
        RobotsHitReactionResult {
            reaction_accepted: true,
            query_hit_still_commits: true,
            activate_embedded_type: Some(ROBOTS_AI_HIT_EMBEDDED_TYPE),
            health_damage: before_health.saturating_sub(self.health),
        }
    }
}

/// Host-visible side effects added by thin AI +0xC8 overrides that delegate to
/// common `0x00453480`. Keeping them as a plan prevents audio/effect and
/// class-local state from leaking into the common damage reducer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RobotsDerivedAiHitResult {
    pub base: RobotsHitReactionResult,
    /// Turret family `0x004609F0` requests this native effect after an accepted
    /// common AI reaction. The UE adapter owns actual effect spawning.
    pub effect_uid: Option<u32>,
    /// TurretBot `0x0045D140` sets Handler +0x60C when health is zero after its
    /// wrapper logic.
    pub set_zero_health_latch: bool,
    /// RollerBot `0x00462F60` clears Handler +0x640..+0x64C when health is zero.
    pub clear_four_runtime_words: bool,
}

fn derived_result(base: RobotsHitReactionResult) -> RobotsDerivedAiHitResult {
    RobotsDerivedAiHitResult {
        base,
        effect_uid: None,
        set_zero_health_latch: false,
        clear_four_runtime_words: false,
    }
}

/// Turret family `0x004609F0`: common AI hit must accept first, then native
/// emits effect 0x1AF0026C and optional flag 0x400000 forces health to zero.
pub fn robots_apply_turret_hit_reaction(
    state: &mut RobotsAiHitReactionState,
    input: RobotsAcceptedHitReactionInput,
    debug_force_kill_non_exempt: bool,
) -> RobotsDerivedAiHitResult {
    let health_before = state.health;
    let mut result = derived_result(state.apply_hit(input, debug_force_kill_non_exempt));
    if !result.base.reaction_accepted {
        return result;
    }
    result.effect_uid = Some(ROBOTS_TURRET_HIT_EFFECT_UID);
    if input.query_flags & ROBOTS_HIT_FLAG_DERIVED_FORCE_ZERO != 0 {
        state.health = 0;
        result.base.health_damage = health_before.saturating_sub(state.health);
    }
    result
}

/// TurretBot `0x0045D140` deliberately ignores the common reducer return. It
/// applies flag 0x400000 after that call and then latches +0x60C whenever health
/// is zero. This means class-local state can change even when common reaction
/// admission returned false; the outer query hit is independent either way.
pub fn robots_apply_turret_bot_hit_reaction(
    state: &mut RobotsAiHitReactionState,
    input: RobotsAcceptedHitReactionInput,
    debug_force_kill_non_exempt: bool,
) -> RobotsDerivedAiHitResult {
    let health_before = state.health;
    let mut result = derived_result(state.apply_hit(input, debug_force_kill_non_exempt));
    if input.query_flags & ROBOTS_HIT_FLAG_DERIVED_FORCE_ZERO != 0 {
        state.health = 0;
        result.base.health_damage = health_before.saturating_sub(state.health);
    }
    result.set_zero_health_latch = state.health == 0;
    result
}

/// Dodgem `0x00462710`: only an accepted common hit can promote flag 0x10 to a
/// forced-zero health result.
pub fn robots_apply_dodgem_hit_reaction(
    state: &mut RobotsAiHitReactionState,
    input: RobotsAcceptedHitReactionInput,
    debug_force_kill_non_exempt: bool,
) -> RobotsDerivedAiHitResult {
    let health_before = state.health;
    let mut result = derived_result(state.apply_hit(input, debug_force_kill_non_exempt));
    if result.base.reaction_accepted && input.query_flags & ROBOTS_HIT_FLAG_DODGEM_FORCE_ZERO != 0 {
        state.health = 0;
        result.base.health_damage = health_before.saturating_sub(state.health);
    }
    result
}

/// RollerBot `0x00462F60`: common reaction runs first, but its return is ignored;
/// zero health clears the four class-local runtime words +0x640..+0x64C.
pub fn robots_apply_roller_bot_hit_reaction(
    state: &mut RobotsAiHitReactionState,
    input: RobotsAcceptedHitReactionInput,
    debug_force_kill_non_exempt: bool,
) -> RobotsDerivedAiHitResult {
    let mut result = derived_result(state.apply_hit(input, debug_force_kill_non_exempt));
    result.clear_four_runtime_words = state.health == 0;
    result
}

/// EF01 Mine `0x00463920` has two distinct native paths. Query flag 0x10
/// tail-jumps directly to common AI damage. Without that flag it only rejects a
/// duplicate serial / already-pending owner, then requests the established
/// monster action dispatcher `0x004550A0(0,1)` and returns accepted. The
/// alternate path deliberately does not write the common AI last-serial field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RobotsEf01MineHitResult {
    pub base: RobotsHitReactionResult,
    pub request_monster_action_dispatch_0_1: bool,
}

pub fn robots_apply_ef01_mine_hit_reaction(
    state: &mut RobotsAiHitReactionState,
    input: RobotsAcceptedHitReactionInput,
    owner_pending_destroy: bool,
    debug_force_kill_non_exempt: bool,
) -> RobotsEf01MineHitResult {
    if input.query_flags & ROBOTS_HIT_FLAG_EF01_MINE_USE_COMMON_AI != 0 {
        return RobotsEf01MineHitResult {
            base: state.apply_hit(input, debug_force_kill_non_exempt),
            request_monster_action_dispatch_0_1: false,
        };
    }

    if input.query_serial != u16::MAX && input.query_serial == state.last_query_serial {
        return RobotsEf01MineHitResult {
            base: RobotsHitReactionResult::rejected(),
            request_monster_action_dispatch_0_1: false,
        };
    }
    if owner_pending_destroy {
        return RobotsEf01MineHitResult {
            base: RobotsHitReactionResult::rejected(),
            request_monster_action_dispatch_0_1: false,
        };
    }

    RobotsEf01MineHitResult {
        base: RobotsHitReactionResult {
            reaction_accepted: true,
            query_hit_still_commits: true,
            activate_embedded_type: None,
            health_damage: 0,
        },
        request_monster_action_dispatch_0_1: true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn monster_explosion_event_selector_maps_through_native_modulus_table() {
        assert_eq!(ROBOTS_MONSTER_EXPLOSION_ACTION_MODULI, [1, 3, 3]);
        assert_eq!(robots_monster_explosion_action_modulus(0), 1);
        assert_eq!(robots_monster_explosion_action_modulus(1), 3);
        assert_eq!(robots_monster_explosion_action_modulus(2), 3);
        assert_eq!(robots_monster_explosion_action_modulus(3), 1);
        assert_eq!(robots_monster_explosion_action_modulus(u32::MAX), 1);
        assert_eq!(
            robots_monster_action_explosion_uid(0, 0x5200_003A, 0x5200_002C),
            0x5200_003A
        );
        assert_eq!(
            robots_monster_action_explosion_uid(
                ROBOTS_MONSTER_ACTION_ALT_EXPLOSION_FLAG,
                0x5200_003A,
                0x5200_002C,
            ),
            0x5200_002C
        );
    }

    fn input(flags: u32, serial: u16, metadata: u16) -> RobotsAcceptedHitReactionInput {
        RobotsAcceptedHitReactionInput {
            query_flags: flags,
            query_serial: serial,
            hit_metadata: metadata,
            source_is_candidate_owner: false,
            secondary_source_is_candidate_owner: false,
        }
    }

    fn ai() -> RobotsAiHitReactionState {
        RobotsAiHitReactionState {
            query_flags_snapshot: 0,
            query_serial_snapshot: u16::MAX,
            hit_metadata_snapshot: 0,
            last_query_serial: u16::MAX,
            capability_flags: ROBOTS_AI_HIT_CAPABILITY_FLAG,
            health: 5,
            got_hit_latch: false,
            owner_category: 1,
        }
    }

    #[test]
    fn hit_front_back_selection_matches_native_half_plane_and_boundary() {
        let owner_yaw = 0.0;
        let (mode, target_yaw) = robots_ai_hit_front_back_selection(
            owner_yaw,
            std::f32::consts::PI,
            ROBOTS_ANIM_MODE_HIT_DEATH_F,
            ROBOTS_ANIM_MODE_HIT_DEATH_B,
        );
        assert_eq!(mode, ROBOTS_ANIM_MODE_HIT_DEATH_F);
        assert!((shortest_yaw_delta(owner_yaw, target_yaw)).abs() <= 1.0e-6);

        let (mode, target_yaw) = robots_ai_hit_front_back_selection(
            owner_yaw,
            0.0,
            ROBOTS_ANIM_MODE_HIT_DEATH_F,
            ROBOTS_ANIM_MODE_HIT_DEATH_B,
        );
        assert_eq!(mode, ROBOTS_ANIM_MODE_HIT_DEATH_B);
        assert!((shortest_yaw_delta(owner_yaw, target_yaw)).abs() <= 1.0e-6);

        // Native 0x00457C40 uses a strict < PI/2 comparison, so the exact
        // half-plane boundary belongs to the back animation.
        let (mode, _) = robots_ai_hit_front_back_selection(
            owner_yaw,
            std::f32::consts::FRAC_PI_2,
            ROBOTS_ANIM_MODE_HIT_DEATH_F,
            ROBOTS_ANIM_MODE_HIT_DEATH_B,
        );
        assert_eq!(mode, ROBOTS_ANIM_MODE_HIT_DEATH_B);
    }

    #[test]
    fn common_ai_hit_priority_matches_457be0_health_flags_and_latch_gate() {
        let node = RobotsCommonAiHitRuntimeState::default();
        let mut hit = ai();
        hit.got_hit_latch = true;
        assert_eq!(node.priority(hit), ROBOTS_AI_HIT_NODE_PRIORITY);

        for blocked in [0x8, 0x10, 0x4000] {
            let mut blocked_hit = hit;
            blocked_hit.query_flags_snapshot = blocked;
            assert_eq!(node.priority(blocked_hit), 1);
        }

        let mut dead = hit;
        dead.health = 0;
        assert_eq!(node.priority(dead), 1);

        let mut no_latch = hit;
        no_latch.got_hit_latch = false;
        assert_eq!(node.priority(no_latch), 1);
    }

    #[test]
    fn common_ai_hit_sawbot_config_keeps_front_back_modes_with_four_pi_turn_rate() {
        let config = RobotsCommonAiHitConfig::sawbot();
        assert_eq!(config.front_anim_mode, ROBOTS_ANIM_MODE_HIT_FRONT);
        assert_eq!(config.back_anim_mode, ROBOTS_ANIM_MODE_HIT_BACK);
        assert_eq!(config.left_anim_mode, 0);
        assert_eq!(config.right_anim_mode, 0);
        assert!((config.turn_rate_radians_per_second - 4.0 * std::f32::consts::PI).abs() < 1.0e-6);

        let fatal = RobotsCommonAiHitConfig::sawbot_fatal();
        assert_eq!(fatal.front_anim_mode, ROBOTS_ANIM_MODE_HIT_DEATH_F);
        assert_eq!(fatal.back_anim_mode, ROBOTS_ANIM_MODE_HIT_DEATH_B);
        assert!((fatal.turn_rate_radians_per_second - 4.0 * std::f32::consts::PI).abs() < 1.0e-6);
    }

    #[test]
    fn common_ai_hit_shunt_config_matches_four_way_457c40_and_four_pi_turn_rate() {
        let config = RobotsCommonAiHitConfig::shuntbot();
        assert_eq!(config.front_anim_mode, 0);
        assert_eq!(config.back_anim_mode, 0x0900_002A);
        assert_eq!(config.left_anim_mode, 0x0900_003C);
        assert_eq!(config.right_anim_mode, 0x0900_003D);
        assert!((config.turn_rate_radians_per_second - 4.0 * std::f32::consts::PI).abs() < 1.0e-6);

        let (front_sector_mode, _) =
            robots_common_ai_hit_direction_selection(config, 0.0, std::f32::consts::PI);
        assert_eq!(front_sector_mode, 0x0900_003C);

        let (right_mode, right_target) =
            robots_common_ai_hit_direction_selection(config, 0.0, std::f32::consts::FRAC_PI_2);
        assert_eq!(right_mode, 0x0900_003D);
        assert!(shortest_yaw_delta(0.0, right_target).abs() < 1.0e-6);

        let (back_mode, back_target) = robots_common_ai_hit_direction_selection(config, 0.0, 0.0);
        assert_eq!(back_mode, 0x0900_002A);
        assert!(shortest_yaw_delta(0.0, back_target).abs() < 1.0e-6);

        let mut node = RobotsCommonAiHitRuntimeState::default();
        node.enter_configured(config, 0.0, std::f32::consts::PI, 11);
        let next_yaw = node.step_owner_yaw_configured(config, 0.0).unwrap();
        assert!((next_yaw + std::f32::consts::PI / 15.0).abs() < 1.0e-6);
    }

    #[test]
    fn common_ai_hit_enter_setup_idle_and_leave_preserve_native_lifecycle() {
        let mut node = RobotsCommonAiHitRuntimeState::default();
        let mut hit = ai();
        hit.got_hit_latch = true;
        hit.query_serial_snapshot = 37;
        hit.last_query_serial = 37;

        let requested = node.enter(0.0, std::f32::consts::PI, 37);
        assert_eq!(requested, ROBOTS_ANIM_MODE_HIT_FRONT);
        assert!(node.active);
        assert_eq!(node.captured_query_serial, 37);
        assert_eq!(node.priority(hit), ROBOTS_AI_HIT_NODE_PRIORITY);

        node.setup_idle(&mut hit);
        assert!(node.completion_latch);
        assert!(!hit.got_hit_latch);
        assert_eq!(node.priority(hit), 1);

        node.leave(&mut hit);
        assert!(!node.active);
        assert_eq!(hit.query_serial_snapshot, 0);
        assert_eq!(hit.last_query_serial, 0);
    }

    #[test]
    fn all_17_hittable_slot50_targets_round_trip_to_distinct_families() {
        let families = [
            RobotsHitReactionFamily::NoOp,
            RobotsHitReactionFamily::ScriptHitLatch,
            RobotsHitReactionFamily::AiCharacterDamage,
            RobotsHitReactionFamily::TurretBot,
            RobotsHitReactionFamily::Turret,
            RobotsHitReactionFamily::Dodgem,
            RobotsHitReactionFamily::RollerBot,
            RobotsHitReactionFamily::Ef01Mine,
            RobotsHitReactionFamily::WatchBot,
            RobotsHitReactionFamily::PlayerBall,
            RobotsHitReactionFamily::Player,
            RobotsHitReactionFamily::BossExec,
            RobotsHitReactionFamily::BossSewer,
            RobotsHitReactionFamily::BossSewerCanon,
            RobotsHitReactionFamily::SweeperBoss,
            RobotsHitReactionFamily::SweeperRatchet,
            RobotsHitReactionFamily::RatchetMissile,
        ];
        let mut targets = std::collections::BTreeSet::new();
        for family in families {
            assert!(targets.insert(family.native_target()));
            assert_eq!(
                RobotsHitReactionFamily::from_native_target(family.native_target()),
                Some(family)
            );
        }
        assert_eq!(targets.len(), 17);
        assert_eq!(
            RobotsHitReactionFamily::from_native_target(0x0041_4EF0),
            None
        );
    }

    #[test]
    fn script_latch_is_exact_bit_zero_or_and_noop_return_stays_separate_from_query_hit() {
        let mut latch = 0xA0;
        let result = robots_apply_script_hit_latch(&mut latch);
        assert_eq!(latch, 0xA1);
        assert!(result.reaction_accepted);
        assert!(result.query_hit_still_commits);

        let noop = robots_apply_noop_hit_reaction();
        assert!(!noop.reaction_accepted);
        assert!(noop.query_hit_still_commits);
    }

    #[test]
    fn ai_duplicate_serial_rejects_reaction_but_outer_query_hit_still_commits() {
        let mut state = ai();
        state.last_query_serial = 17;
        let result = state.apply_hit(input(0x4000, 17, 0x55aa), false);
        assert!(!result.reaction_accepted);
        assert!(result.query_hit_still_commits);
        assert_eq!(state.query_flags_snapshot, 0x4000);
        assert_eq!(state.query_serial_snapshot, 17);
        assert_eq!(state.hit_metadata_snapshot, 0x55aa);
        assert_eq!(state.health, 5);
        assert!(!state.got_hit_latch);
    }

    #[test]
    fn ai_source_relation_filters_match_native_0x200000_and_0x80000_rules() {
        let mut state = ai();
        let mut owner_source = input(0, 1, 0);
        owner_source.source_is_candidate_owner = true;
        assert!(!state.apply_hit(owner_source, false).reaction_accepted);

        let mut state = ai();
        owner_source.query_flags = ROBOTS_HIT_FLAG_ALLOW_OWNER_SOURCE;
        assert!(state.apply_hit(owner_source, false).reaction_accepted);

        let mut state = ai();
        let external = input(ROBOTS_HIT_FLAG_AI_REQUIRE_OWNER_SOURCE, 2, 0);
        assert!(!state.apply_hit(external, false).reaction_accepted);
    }

    #[test]
    fn ai_metadata_0x8000_requires_capability_bit_0x08() {
        let mut state = ai();
        state.capability_flags = 0;
        let result = state.apply_hit(input(0, 3, 0x8000), false);
        assert!(!result.reaction_accepted);
        assert_eq!(state.last_query_serial, u16::MAX);
    }

    #[test]
    fn ai_damage_is_one_or_two_and_clamped_to_remaining_byte_health() {
        let mut state = ai();
        let result = state.apply_hit(input(0, 4, 0), false);
        assert_eq!(state.health, 4);
        assert_eq!(result.health_damage, 1);
        assert_eq!(result.activate_embedded_type, Some(3));
        assert!(state.got_hit_latch);

        let mut state = ai();
        state.health = 1;
        let result = state.apply_hit(input(ROBOTS_HIT_FLAG_DOUBLE_AI_DAMAGE, 5, 0), false);
        assert_eq!(state.health, 0);
        assert_eq!(result.health_damage, 1);
    }

    #[test]
    fn ai_damage_suppression_mask_keeps_health_but_still_marks_admitted_hit() {
        let mut state = ai();
        let result = state.apply_hit(input(0x0000_0004, 6, 0), false);
        assert!(result.reaction_accepted);
        assert_eq!(state.health, 5);
        assert_eq!(result.health_damage, 0);
        assert_eq!(state.last_query_serial, 6);
        assert!(state.got_hit_latch);
    }

    #[test]
    fn ai_debug_and_0x40000_force_kill_only_non_exempt_owner_categories() {
        let mut state = ai();
        let result = state.apply_hit(input(0, 7, 0), true);
        assert_eq!(state.health, 0);
        assert_eq!(result.health_damage, 5);

        let mut exempt = ai();
        exempt.owner_category = ROBOTS_AI_HIT_EXEMPT_OWNER_CATEGORY_MIN;
        exempt.apply_hit(input(ROBOTS_HIT_FLAG_AI_FORCE_ZERO_NON_EXEMPT, 8, 0), false);
        assert_eq!(exempt.health, 4);

        let mut non_exempt = ai();
        non_exempt.apply_hit(input(ROBOTS_HIT_FLAG_AI_FORCE_ZERO_NON_EXEMPT, 9, 0), false);
        assert_eq!(non_exempt.health, 0);
    }

    #[test]
    fn turret_wrapper_runs_effect_only_after_common_accept_and_0x400000_zeroes_health() {
        let mut state = ai();
        let result = robots_apply_turret_hit_reaction(
            &mut state,
            input(ROBOTS_HIT_FLAG_DERIVED_FORCE_ZERO, 10, 0),
            false,
        );
        assert!(result.base.reaction_accepted);
        assert_eq!(result.effect_uid, Some(ROBOTS_TURRET_HIT_EFFECT_UID));
        assert_eq!(state.health, 0);
        assert_eq!(result.base.health_damage, 5);

        let mut rejected = ai();
        rejected.last_query_serial = 11;
        let result = robots_apply_turret_hit_reaction(&mut rejected, input(0, 11, 0), false);
        assert!(!result.base.reaction_accepted);
        assert_eq!(result.effect_uid, None);
        assert_eq!(rejected.health, 5);
    }

    #[test]
    fn turretbot_wrapper_keeps_post_base_force_zero_and_zero_health_latch_on_rejection() {
        let mut state = ai();
        state.last_query_serial = 12;
        let result = robots_apply_turret_bot_hit_reaction(
            &mut state,
            input(ROBOTS_HIT_FLAG_DERIVED_FORCE_ZERO, 12, 0),
            false,
        );
        assert!(!result.base.reaction_accepted);
        assert_eq!(state.health, 0);
        assert_eq!(result.base.health_damage, 5);
        assert!(result.set_zero_health_latch);
    }

    #[test]
    fn dodgem_force_zero_requires_common_acceptance() {
        let mut accepted = ai();
        let result = robots_apply_dodgem_hit_reaction(
            &mut accepted,
            input(ROBOTS_HIT_FLAG_DODGEM_FORCE_ZERO, 13, 0),
            false,
        );
        assert!(result.base.reaction_accepted);
        assert_eq!(accepted.health, 0);
        assert_eq!(result.base.health_damage, 5);

        let mut rejected = ai();
        rejected.last_query_serial = 14;
        let result = robots_apply_dodgem_hit_reaction(
            &mut rejected,
            input(ROBOTS_HIT_FLAG_DODGEM_FORCE_ZERO, 14, 0),
            false,
        );
        assert!(!result.base.reaction_accepted);
        assert_eq!(rejected.health, 5);
    }

    #[test]
    fn rollerbot_clears_local_runtime_words_when_health_is_zero_even_after_common_reject() {
        let mut state = ai();
        state.health = 0;
        state.last_query_serial = 15;
        let result = robots_apply_roller_bot_hit_reaction(&mut state, input(0, 15, 0), false);
        assert!(!result.base.reaction_accepted);
        assert!(result.clear_four_runtime_words);
    }

    #[test]
    fn ef01_mine_flag_0x10_tail_delegates_to_common_ai_but_suppresses_damage() {
        let mut state = ai();
        let result = robots_apply_ef01_mine_hit_reaction(
            &mut state,
            input(ROBOTS_HIT_FLAG_EF01_MINE_USE_COMMON_AI, 16, 0),
            false,
            false,
        );
        assert!(result.base.reaction_accepted);
        assert_eq!(result.base.health_damage, 0);
        assert_eq!(state.health, 5);
        assert_eq!(state.last_query_serial, 16);
        assert!(state.got_hit_latch);
        assert_eq!(
            result.base.activate_embedded_type,
            Some(ROBOTS_AI_HIT_EMBEDDED_TYPE)
        );
        assert!(!result.request_monster_action_dispatch_0_1);
    }

    #[test]
    fn ef01_mine_alternate_path_requests_action_without_mutating_common_ai_serial_or_health() {
        let mut state = ai();
        let result = robots_apply_ef01_mine_hit_reaction(&mut state, input(0, 17, 0), false, false);
        assert!(result.base.reaction_accepted);
        assert_eq!(result.base.health_damage, 0);
        assert_eq!(state.health, 5);
        assert_eq!(state.last_query_serial, u16::MAX);
        assert!(result.request_monster_action_dispatch_0_1);
    }

    #[test]
    fn ef01_mine_alternate_path_rejects_duplicate_serial_and_pending_owner() {
        let mut duplicate = ai();
        duplicate.last_query_serial = 18;
        let result =
            robots_apply_ef01_mine_hit_reaction(&mut duplicate, input(0, 18, 0), false, false);
        assert!(!result.base.reaction_accepted);
        assert!(!result.request_monster_action_dispatch_0_1);

        let mut pending = ai();
        let result =
            robots_apply_ef01_mine_hit_reaction(&mut pending, input(0, 19, 0), true, false);
        assert!(!result.base.reaction_accepted);
        assert!(!result.request_monster_action_dispatch_0_1);
    }
}
