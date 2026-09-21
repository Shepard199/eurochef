use serde::Serialize;

/// Engine-neutral projection of the common Character Physics lanes used by the
/// recovered AI handlers. This is intentionally smaller than a physics engine:
/// it owns only fields whose writers/readers are already proven in Robots.exe.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct RobotsCharacterPhysicsRuntimeState {
    /// Character Physics +0x1B8/+0x1BC/+0x1C0 (the local Physics object sees the
    /// same XYZ velocity lanes at +0x2C/+0x30/+0x34).
    pub velocity_xyz: [f32; 3],
    /// Base Physics +0x124 gravity accumulator. `0x00419400` subtracts
    /// `Physics+0x128 / 60` before owner-pose integration while Physics+0x08 bit0
    /// clears this lane instead. Keep it separate from handler-authored velocity.
    pub gravity_velocity_y: f32,
    /// Base Physics +0x128. Common Physics ctor `0x004191A0` seeds exactly 9.8.
    pub gravity_acceleration: f32,
    /// Base Physics +0x0A contact-response word. World-contact slot4
    /// `0x0041A3A0 -> 0x004F0414` clears/rebuilds it after slot0 each tick.
    /// Bit0 is consumed by the next `0x00419400` gravity update.
    pub contact_response_bits: u16,
    /// Base Physics +0x150. Slot0 copies +0x0A here before the later world-contact
    /// pass, so gravity can distinguish a continuing contact from the first
    /// airborne tick after contact was lost.
    pub previous_contact_response_bits: u16,
    /// Physics object +0x08 bit0 toggled by AI helper `0x00455E30` and common
    /// Monster state0 vslot +0x164 = `0x00452180`.
    /// Its wider semantic name is deliberately left unspecified.
    pub object_flag_bit0: bool,
    /// Physics object +0x08 bit0x2. EF01 Mine first-update `0x00463970` sets this
    /// directly when it installs FollowFlyingPath and latches Handler+0x644.
    pub object_flag_bit2: bool,
    /// Physics object +0x08 bit0x4, paired with +0x14C bit0x4 by AI-node
    /// helpers `0x004535D0` / `0x00453600`.
    pub object_flag_bit4: bool,
    /// Physics object +0x14C. Common Monster state0 vslot +0x160 = `0x00452120`
    /// owns bit 0x20000 from NavMesh/Handler state.
    pub object_flags_14c: u32,
    /// Handler+0x606 written by `0x00455E30`; common Monster ctor `0x004514F0`
    /// seeds this byte to 1 before the first state0 update.
    pub handler_606: bool,
}

impl Default for RobotsCharacterPhysicsRuntimeState {
    fn default() -> Self {
        Self {
            velocity_xyz: [0.0; 3],
            gravity_velocity_y: 0.0,
            gravity_acceleration: 9.8,
            contact_response_bits: 0,
            previous_contact_response_bits: 0,
            object_flag_bit0: false,
            object_flag_bit2: false,
            object_flag_bit4: false,
            object_flags_14c: 0,
            handler_606: false,
        }
    }
}

impl RobotsCharacterPhysicsRuntimeState {
    pub const MONSTER_NAVMESH_FLAG_14C: u32 = 0x0002_0000;

    /// Common Monster ctor `0x004514F0` seeds Handler+0x606 to 1. Keep this
    /// constructor separate from universal `Default`: the shared physics state is
    /// also used by non-Monster reducers whose bootstrap is not this ctor.
    pub const fn monster_ctor_default() -> Self {
        Self {
            velocity_xyz: [0.0; 3],
            gravity_velocity_y: 0.0,
            gravity_acceleration: 9.8,
            contact_response_bits: 0,
            previous_contact_response_bits: 0,
            object_flag_bit0: false,
            object_flag_bit2: false,
            object_flag_bit4: false,
            object_flags_14c: 0,
            handler_606: true,
        }
    }

    /// Common Monster state0 vslot +0x160 = `0x00452120`.
    /// When Handler+0x604 is enabled, Physics+0x14C bit0x20000 mirrors the
    /// existence of a valid Monster/NavMesh face unless Handler+0x628 explicitly
    /// suppresses the bit with its own 0x20000 flag.
    pub fn service_monster_navmesh_flag_14c(
        &mut self,
        handler_604_enabled: bool,
        navmesh_face_valid: bool,
        handler_flags_628: u32,
    ) {
        if !handler_604_enabled {
            return;
        }
        if navmesh_face_valid && handler_flags_628 & Self::MONSTER_NAVMESH_FLAG_14C == 0 {
            self.object_flags_14c |= Self::MONSTER_NAVMESH_FLAG_14C;
        } else {
            self.object_flags_14c &= !Self::MONSTER_NAVMESH_FLAG_14C;
        }
    }

    /// Common Monster state0 vslot +0x164 = `0x00452180` for XItem owner
    /// category 0x0B. Handler+0x2C0 bit0 is produced by the common environment
    /// geometry query (`0x00405320 -> 0x004F0A08 -> 0x004ED004`): lane0 stores
    /// its reference Y at Handler+0x1C0 when an accepted Map/environment face is
    /// found. It is not a NavMesh or animation-derived flag.
    pub fn service_monster_owner_category_0b_bit0(
        &mut self,
        owner_category_is_0b: bool,
        handler_2c0_bit0: bool,
    ) {
        if !owner_category_is_0b {
            return;
        }
        self.object_flag_bit0 = !(handler_2c0_bit0 && self.handler_606);
    }

    /// Exact state pair written by `0x00455E30`:
    /// value 0 => Physics bit0 set, Handler+0x606=0;
    /// value 1 => Physics bit0 clear, Handler+0x606=1.
    pub fn apply_handler_606_mode(&mut self, value: bool) {
        self.object_flag_bit0 = !value;
        self.handler_606 = value;
    }

    /// Generic Physics object +0x08 bit0x2 writer. EF01 FollowFlyingPath and the
    /// NPC diner bootstrap both use this exact lane through different native callers.
    pub fn set_object_flag_bit2(&mut self, enabled: bool) {
        self.object_flag_bit2 = enabled;
    }

    /// EF01 Mine first-update `0x00463970` sets Physics object +0x08 bit0x2 while
    /// Handler+0x644 marks the FollowFlyingPath branch.
    pub fn service_ef01_flying_path_bit2(&mut self, enabled: bool) {
        self.set_object_flag_bit2(enabled);
    }

    /// Common AI-node physics claim. Spike attack enter/leave calls both native
    /// helpers, so keep their two writes transactional in the engine-neutral state.
    pub fn service_attack_bit4(&mut self, enabled: bool) {
        self.object_flag_bit4 = enabled;
        if enabled {
            self.object_flags_14c |= 0x4;
        } else {
            self.object_flags_14c &= !0x4;
        }
    }

    /// Full velocity writer used by native locomotion/root-motion seams.
    pub fn overwrite_velocity(&mut self, velocity_xyz: [f32; 3]) {
        self.velocity_xyz = velocity_xyz;
    }

    /// Additive Physics writer. MagneticHit `0x004587E0` uses this form for the
    /// Y lane instead of replacing the complete Character Physics velocity.
    pub fn add_velocity_delta(&mut self, delta_xyz: [f32; 3]) {
        for (value, delta) in self.velocity_xyz.iter_mut().zip(delta_xyz) {
            *value += delta;
        }
    }

    pub fn add_vertical_velocity_delta(&mut self, delta_y: f32) {
        self.velocity_xyz[1] += delta_y;
    }

    /// Ballistic velocity setup used by native CharacterPhysics helper
    /// `0x0041EC80` when Physics+0x128 is active and random spread is zero.
    /// Piranha calls this exact branch with current owner position, a cached
    /// target, `data[2] + 1.0` as the extra apex height, and param5=0.
    pub fn configure_ballistic_velocity_to_target(
        &mut self,
        current_xyz: [f32; 3],
        target_xyz: [f32; 3],
        extra_apex_height: f32,
    ) -> Option<f32> {
        let dx = target_xyz[0] - current_xyz[0];
        let dy = target_xyz[1] - current_xyz[1];
        let dz = target_xyz[2] - current_xyz[2];

        // Native first writes the horizontal delta into +0x2C/+0x34 with
        // +0x30 cleared, then normalizes the velocity vector.
        let horizontal_squared = dx * dx + dz * dz;
        let horizontal_distance = if horizontal_squared >= 0.0 {
            horizontal_squared.sqrt()
        } else {
            0.0
        };
        if horizontal_distance > 1.0e-5 {
            self.velocity_xyz[0] = dx / horizontal_distance;
            self.velocity_xyz[1] = 0.0;
            self.velocity_xyz[2] = dz / horizontal_distance;
        } else {
            self.velocity_xyz = [dx, 0.0, dz];
        }

        let launch_height = dy + extra_apex_height;
        if launch_height < 0.0 {
            self.gravity_velocity_y = 0.0;
        } else {
            let base_vertical =
                (launch_height * self.gravity_acceleration * 2.0).sqrt();
            self.gravity_velocity_y = base_vertical * (1.0 + 1.0 / 60.0);
        }

        let half_acceleration = self.gravity_acceleration * -0.5;
        let discriminant =
            self.gravity_velocity_y * self.gravity_velocity_y - 2.0 * self.gravity_acceleration * dy;
        if discriminant < 0.0 || !discriminant.is_finite() {
            return None;
        }
        let root = discriminant.sqrt();
        let denominator = half_acceleration + half_acceleration;
        if denominator.abs() <= f32::EPSILON {
            return None;
        }
        let time_a = (root - self.gravity_velocity_y) / denominator;
        let time_b = (-self.gravity_velocity_y - root) / denominator;
        let flight_time = time_a.max(time_b);
        if !flight_time.is_finite() || flight_time <= 0.0 {
            return None;
        }

        let horizontal_speed = horizontal_distance / flight_time;
        self.velocity_xyz[0] *= horizontal_speed;
        self.velocity_xyz[2] *= horizontal_speed;
        Some(flight_time)
    }

    /// Exact vertical gravity/contact slice of base Physics slot0 `0x00419400`.
    /// Continuing floor contact pins a negative accumulator to -3.0. On the first
    /// tick after contact is lost a negative accumulator is reset before normal
    /// gravity is applied. Collision-plane projection itself remains host-owned.
    pub fn advance_base_gravity(&mut self, delta_seconds: f32) {
        if self.object_flag_bit0 {
            self.gravity_velocity_y = 0.0;
            return;
        }

        if self.contact_response_bits & 1 != 0 && self.gravity_velocity_y < 0.0 {
            self.gravity_velocity_y = -3.0;
            return;
        }
        if self.contact_response_bits & 1 == 0
            && self.previous_contact_response_bits & 1 != 0
            && self.gravity_velocity_y < 0.0
        {
            self.gravity_velocity_y = 0.0;
        }
        self.gravity_velocity_y -= self.gravity_acceleration * delta_seconds.max(0.0);
    }

    /// End of base slot0 `0x00419400`: native copies Physics+0x0A to +0x150
    /// before the later slot4 world-contact pass rebuilds +0x0A.
    pub fn latch_contact_response_after_base_update(&mut self) {
        self.previous_contact_response_bits = self.contact_response_bits;
    }

    /// Entry of world-contact slot4 `0x0041A3A0` clears Physics+0x0A before
    /// `0x004F0414` classifies/projects contacts for the new tick.
    pub fn begin_world_contact_projection(&mut self) {
        self.contact_response_bits = 0;
    }

    /// `0x004F0414` sets Physics+0x0A bit0 for the floor/contact response lane.
    pub fn mark_floor_contact_response(&mut self) {
        self.contact_response_bits |= 1;
    }

    /// Base Physics slot0 `0x00419400` position integration. Hosts may choose not
    /// to call this when another already-proven owner commits the same motion lane.
    pub fn integrate_position(&self, position_xyz: &mut [f32; 3], delta_seconds: f32) {
        let dt = delta_seconds.max(0.0);
        for (position, velocity) in position_xyz.iter_mut().zip(self.velocity_xyz) {
            *position += velocity * dt;
        }
    }

    /// Base fixed-step owner-pose integration with the separate gravity lane.
    /// Existing behavior-specific callers keep `integrate_position()` when native
    /// code proves that they own only explicit Character Physics velocity.
    pub fn integrate_position_with_gravity(&self, position_xyz: &mut [f32; 3], delta_seconds: f32) {
        let dt = delta_seconds.max(0.0);
        position_xyz[0] += self.velocity_xyz[0] * dt;
        position_xyz[1] += (self.velocity_xyz[1] + self.gravity_velocity_y) * dt;
        position_xyz[2] += self.velocity_xyz[2] * dt;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handler_606_helper_keeps_native_inverse_physics_bit_pair() {
        let mut state = RobotsCharacterPhysicsRuntimeState::default();
        state.apply_handler_606_mode(false);
        assert!(state.object_flag_bit0);
        assert!(!state.handler_606);
        state.apply_handler_606_mode(true);
        assert!(!state.object_flag_bit0);
        assert!(state.handler_606);
    }

    #[test]
    fn additive_vertical_lane_integrates_through_common_character_physics() {
        let mut state = RobotsCharacterPhysicsRuntimeState::default();
        state.overwrite_velocity([1.0, 2.0, 3.0]);
        state.add_vertical_velocity_delta(0.5);
        assert_eq!(state.velocity_xyz, [1.0, 2.5, 3.0]);

        let mut position = [0.0, 10.0, 0.0];
        state.integrate_position(&mut position, 1.0 / 60.0);
        assert!((position[0] - 1.0 / 60.0).abs() < 1.0e-6);
        assert!((position[1] - (10.0 + 2.5 / 60.0)).abs() < 1.0e-6);
        assert!((position[2] - 3.0 / 60.0).abs() < 1.0e-6);
    }

    #[test]
    fn ballistic_target_setup_matches_0041ec80_vertical_and_horizontal_solution() {
        let mut state = RobotsCharacterPhysicsRuntimeState::monster_ctor_default();
        state.gravity_acceleration = 12.74;
        let flight_time = state
            .configure_ballistic_velocity_to_target([0.0, 0.0, 0.0], [10.0, 0.0, 0.0], 2.0)
            .unwrap();

        let expected_vertical = (2.0f32 * 2.0 * 12.74).sqrt() * (1.0 + 1.0 / 60.0);
        let expected_time = (expected_vertical + expected_vertical) / 12.74;
        assert!((state.gravity_velocity_y - expected_vertical).abs() < 1.0e-6);
        assert!((flight_time - expected_time).abs() < 1.0e-6);
        assert!((state.velocity_xyz[0] - 10.0 / expected_time).abs() < 1.0e-6);
        assert_eq!(state.velocity_xyz[1].to_bits(), 0.0f32.to_bits());
        assert_eq!(state.velocity_xyz[2].to_bits(), 0.0f32.to_bits());
    }

    #[test]
    fn base_gravity_accumulator_matches_character_physics_slot0_fixed_step() {
        let mut state = RobotsCharacterPhysicsRuntimeState::monster_ctor_default();
        state.advance_base_gravity(1.0 / 60.0);
        assert!((state.gravity_velocity_y + 9.8 / 60.0).abs() < 1.0e-6);
        let mut position = [0.0, 10.0, 0.0];
        state.integrate_position_with_gravity(&mut position, 1.0 / 60.0);
        assert!((position[1] - (10.0 - 9.8 / 3600.0)).abs() < 1.0e-6);

        state.object_flag_bit0 = true;
        state.advance_base_gravity(1.0 / 60.0);
        assert_eq!(state.gravity_velocity_y.to_bits(), 0.0f32.to_bits());
    }

    #[test]
    fn base_gravity_preserves_native_floor_contact_and_contact_lost_latches() {
        let dt = 1.0 / 60.0;
        let mut state = RobotsCharacterPhysicsRuntimeState::monster_ctor_default();
        state.gravity_velocity_y = -1.25;
        state.mark_floor_contact_response();

        state.advance_base_gravity(dt);
        assert_eq!(state.gravity_velocity_y.to_bits(), (-3.0f32).to_bits());

        state.latch_contact_response_after_base_update();
        state.begin_world_contact_projection();
        assert_eq!(state.contact_response_bits, 0);
        assert_eq!(state.previous_contact_response_bits & 1, 1);

        state.advance_base_gravity(dt);
        assert!((state.gravity_velocity_y + 9.8 / 60.0).abs() < 1.0e-6);
    }

    #[test]
    fn monster_ctor_and_state0_physics_services_match_recovered_flag_writers() {
        let mut state = RobotsCharacterPhysicsRuntimeState::monster_ctor_default();
        assert!(state.handler_606);
        assert!(!state.object_flag_bit0);
        assert!(!state.object_flag_bit2);
        assert_eq!(state.object_flags_14c, 0);

        state.service_ef01_flying_path_bit2(true);
        assert!(state.object_flag_bit2);
        state.service_ef01_flying_path_bit2(false);
        assert!(!state.object_flag_bit2);

        state.service_monster_navmesh_flag_14c(true, true, 0);
        assert_ne!(state.object_flags_14c & 0x0002_0000, 0);
        state.service_monster_navmesh_flag_14c(true, true, 0x0002_0000);
        assert_eq!(state.object_flags_14c & 0x0002_0000, 0);
        state.service_monster_navmesh_flag_14c(true, false, 0);
        assert_eq!(state.object_flags_14c & 0x0002_0000, 0);
        state.object_flags_14c |= 0x0002_0000;
        state.service_monster_navmesh_flag_14c(false, false, 0x0002_0000);
        assert_ne!(state.object_flags_14c & 0x0002_0000, 0);

        state.service_monster_owner_category_0b_bit0(true, true);
        assert!(!state.object_flag_bit0);
        state.service_monster_owner_category_0b_bit0(true, false);
        assert!(state.object_flag_bit0);
        state.apply_handler_606_mode(false);
        state.service_monster_owner_category_0b_bit0(true, true);
        assert!(state.object_flag_bit0);

        state.service_attack_bit4(true);
        assert!(state.object_flag_bit4);
        assert_ne!(state.object_flags_14c & 0x4, 0);
        state.service_attack_bit4(false);
        assert!(!state.object_flag_bit4);
        assert_eq!(state.object_flags_14c & 0x4, 0);
    }
}
