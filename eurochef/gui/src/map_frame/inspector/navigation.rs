use super::super::*;

impl MapFrame {
    pub(super) fn go_to_trigger(&mut self, index: usize, trig: &ProcessedTrigger) {
        self.selected_trigger = Some(index);

        let mut viewer = self.viewer.lock();
        let camera = viewer.camera_mut();
        self.trigger_focus_tween = Some(Tweeny3D::new(
            tweeny::ease_out_exponential,
            camera.position() + camera.focus_offset(self.trigger_scale),
            trig.position,
            0.5,
        ));
    }
}
