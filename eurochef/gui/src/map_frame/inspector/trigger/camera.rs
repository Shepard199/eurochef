use super::super::super::*;
use super::super::widgets::{property_grid, property_row};

impl MapFrame {
    pub(super) fn draw_camera_diagnostics(
        &self,
        ui: &mut egui::Ui,
        map: &ProcessedMap,
        trig: &ProcessedTrigger,
    ) {
        macro_rules! readonly_input {
            ($ui:expr, $label:expr, $value:expr) => {{
                property_row($ui, $label, $value);
            }};
        }
        macro_rules! quick_grid {
            ($ui:expr, $label:expr, $contents:expr) => {{
                property_grid($label, $ui, $contents);
            }};
        }
        if matches!(trig.ttype, 1 | 20) {
            ui.separator();
            ui.strong("Native Camera Diagnostics");
            quick_grid!(ui, "t_native_camera", |ui| {
                readonly_input!(
                    ui,
                    "Runtime Class",
                    if trig.ttype == 1 {
                        "XTrigger_Camera"
                    } else {
                        "XTrigger_Camera_Marker"
                    }
                    .to_string()
                );
                ui.end_row();
                if let Some(mode) = robots_camera_mode(trig.ttype, &trig.data) {
                    readonly_input!(
                        ui,
                        "data[0] mode selector",
                        format!("{} / 0x{:08x}", mode, mode)
                    );
                    ui.end_row();
                }
                if let Some(value) = robots_camera_marker_scaled_data0(trig.ttype, &trig.data) {
                    readonly_input!(
                        ui,
                        "data[0] runtime scale",
                        format!("{value:.4} (signed value × 0.1)")
                    );
                    ui.end_row();
                }
                if let Some(value) = robots_camera_scaled_data4(trig.ttype, &trig.data) {
                    readonly_input!(
                        ui,
                        "data[4] runtime scale",
                        format!("{value:.4} (signed value × 0.1)")
                    );
                    ui.end_row();
                }
                if let Some(value) = robots_camera_scaled_data5(trig.ttype, &trig.data) {
                    readonly_input!(
                        ui,
                        "data[5] runtime scale",
                        format!("{value:.4} (signed value × 0.1)")
                    );
                    ui.end_row();
                }
                if let Some(flags) = robots_camera_flags(trig.ttype, &trig.data) {
                    readonly_input!(ui, "data[2] flags", format!("0x{flags:08x}"));
                    ui.end_row();
                    readonly_input!(
                                            ui,
                                            "Proven flag tests",
                                            if trig.ttype == 1 {
                                                "accessors 0x0008/0x8000; controller 0x0001/0002/0004/0010/0020/0040/0080/0100/0200"
                                            } else {
                                                "0x0002 and 0x8000"
                                            }
                                            .to_string()
                                        );
                    ui.end_row();
                }
                readonly_input!(
                                        ui,
                                        "Runtime Boundary",
                                        if trig.ttype == 1 {
                                            "Mode 4 compares data[1] with the active runtime path UID; this is not proof that the camera travels along the path"
                                        } else {
                                            "data[4] reaches native camera setup; exact camera interpolation along that EXGeoPath is not proven"
                                        }
                                        .to_string()
                                    );
                ui.end_row();
            });
        }

        if trig.ttype == 1 {
            if let Some(trigger_index) = self.selected_trigger {
                if let Some(plan) = robots_camera_controller_plan(map, trigger_index) {
                    ui.separator();
                    ui.strong("Native Controller Command Plan");
                    quick_grid!(ui, "t_native_camera_plan", |ui| {
                        readonly_input!(
                            ui,
                            "Setup dispatch",
                            plan.setup_kind.description().to_string()
                        );
                        ui.end_row();
                        readonly_input!(
                            ui,
                            "data[3] controller raw",
                            format!("0x{:08x}", plan.controller_data3_raw)
                        );
                        ui.end_row();
                        readonly_input!(
                            ui,
                            "Native-tested flag mask",
                            format!("0x{:03x}", plan.native_tested_flags)
                        );
                        ui.end_row();
                        readonly_input!(
                            ui,
                            "First linked Marker",
                            plan.linked_marker_index
                                .map(|index| {
                                    format!("#{} at {:?}", index, plan.linked_marker_position)
                                })
                                .unwrap_or_else(|| "none".to_string())
                        );
                        ui.end_row();
                        if let Some(yaw) = plan.mode1_yaw_radians {
                            readonly_input!(
                                ui,
                                "Mode 1 yaw",
                                format!("{yaw:.6} rad (atan2(camera-marker) + 2π)")
                            );
                            ui.end_row();
                        }
                        if plan.mode == 3 {
                            readonly_input!(
                                ui,
                                "Mode 3 preserve current Camera axes",
                                format!(
                                    "Y={} XZ={}",
                                    plan.mode3_preserve_current_camera_y,
                                    plan.mode3_preserve_current_camera_xz
                                )
                            );
                            ui.end_row();
                        }
                        if plan.mode == 4 {
                            readonly_input!(
                                ui,
                                "Mode 4 path",
                                plan.path_hashcode
                                    .map(|hash| format!("0x{hash:08x}"))
                                    .unwrap_or_else(|| { "null/sentinel".to_string() })
                            );
                            ui.end_row();
                            readonly_input!(
                                ui,
                                "Mode 4 data[6]/data[7]",
                                format!(
                                    "{:?} / {:?} (raw finite floats)",
                                    plan.mode4_data6, plan.mode4_data7
                                )
                            );
                            ui.end_row();
                            readonly_input!(
                                ui,
                                "Mode 4 option flags",
                                format!("0x{:02x}", plan.mode4_option_flags)
                            );
                            ui.end_row();
                        }
                    });
                }
            }
        }
    }
}
