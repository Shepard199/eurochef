use super::*;

impl eframe::App for EurochefApp {
    /// Called by the frame work to save state before shutdown.
    fn save(&mut self, _storage: &mut dyn eframe::Storage) {}

    /// Called each time the UI needs repainting, which may be many times per second.
    /// Put your widgets into a `SidePanel`, `TopPanel`, `CentralPanel`, `Window` or `Area`.
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        puffin::set_scopes_on(self.show_profiler);
        puffin::GlobalProfiler::lock().new_frame();

        egui::Window::new("Profiler")
            .open(&mut self.show_profiler)
            .show(&ctx, |ui| {
                ui.label("Puffin profiling is active; capture analysis is available in the profiler output.");
            });

        if let Some((data, load_path)) = self.load_input.take() {
            #[cfg(not(target_arch = "wasm32"))]
            let _ = &data;

            // ROBOTS_PATCH_0023_REV6_NATIVE_OPEN_PATH_AWARE
            // Native File->Open and drag/drop must pass through load_file_with_path()
            // so external-reference indexing and ROBOTS_EDB_MANIFEST processing run.
            #[cfg(not(target_arch = "wasm32"))]
            {
                println!("[Robots] opening EDB via path-aware loader: {}", load_path);
                match self.load_file_with_path(&load_path) {
                    Ok(_) => {}
                    Err(e) => {
                        self.state = AppState::Error(e);
                    }
                }
            }

            #[cfg(target_arch = "wasm32")]
            {
                let platform = Platform::from_path(load_path);
                self.pending_file = Some((data, platform));
            }
        }

        if let Some((data, platform)) = self.pending_file.as_ref() {
            if let Some(platform) = platform {
                let cur = Cursor::new(data.clone()); // FIXME: Cloning the data hurts my soul
                match self.load_file(*platform, Box::new(cur), &ctx) {
                    Ok(_) => {}
                    Err(e) => {
                        self.state = AppState::Error(e);
                    }
                }
                self.pending_file = None;
            } else {
                self.state = AppState::SelectPlatform;
            }
        }

        let Self {
            state,
            current_panel,
            spreadsheetlist,
            fileinfo,
            textures,
            load_input,
            entities,
            scripts,
            animations,
            maps,
            selected_platform,
            ..
        } = self;

        let load_clone = load_input.clone();

        // swy: queue a load for the first drag-and-dropped file we encounter here
        ctx.input(|i| {
            if !i.raw.dropped_files.is_empty() {
                for file in &i.raw.dropped_files {
                    let info = if let Some(path) = &file.path {
                        path.display().to_string()
                    } else if !file.name.is_empty() {
                        file.name.clone()
                    } else {
                        "???".to_owned()
                    };

                    info!("Dragged a into the main window: '{info}'");

                    // swy: put the path and its data inside load_input, load_clone is like a pointer
                    match File::open(&info) {
                        Err(why) => warn!("Couldn't read '{info}', skipping: {why}"),
                        Ok(mut f) => {
                            let mut data = vec![];
                            f.read_to_end(&mut data).unwrap();

                            load_clone.store(Some((data, info)));

                            // swy: skip the rest, for the time being, we only care about the first one
                            break;
                        }
                    }
                }
            }
        });

        egui::Panel::top("top_panel").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("Open").clicked() {
                        // TODO(cohae): drag and drop loading
                        #[cfg(target_arch = "wasm32")]
                        {
                            wasm_bindgen_futures::spawn_local(async move {
                                let future = rfd::AsyncFileDialog::new()
                                    .add_filter("Eurocom DB", &["edb"])
                                    .pick_file();
                                if let Some(file) = future.await {
                                    let data = file.read().await;
                                    info!("{}", file.file_name());
                                    load_clone.store(Some((data, file.file_name())));
                                } else {
                                }
                            });
                        }

                        #[cfg(not(target_arch = "wasm32"))]
                        std::thread::spawn(move || {
                            if let Some(path) = rfd::FileDialog::new()
                                .add_filter("EngineX Database", &["edb"])
                                .pick_file()
                            {
                                let mut f = File::open(&path).unwrap();
                                let mut data = vec![];
                                f.read_to_end(&mut data).unwrap();

                                load_clone.store(Some((data, path.to_string_lossy().to_string())));
                            } else {
                                load_clone.store(None);
                            }
                        });

                        ui.close()
                    }
                });

                if ui.button("Profiler").clicked() {
                    self.show_profiler = true;
                }

                if ui.button("About").clicked() {
                    self.about_window = true;
                }
            });
        });

        // Run the app at refresh rate on the texture panel (for animated textures)
        match current_panel {
            Panel::Entities
            | Panel::Textures
            | Panel::Maps
            | Panel::Scripts
            | Panel::Animations => ctx.request_repaint(),
            _ => {
                ctx.request_repaint_after(std::time::Duration::from_secs_f32(1.));
            }
        }

        let screen_rect = ctx.content_rect();
        let max_height = 320.0.at_most(screen_rect.height());

        if self.about_window {
            egui::Window::new("About")
                .pivot(egui::Align2::CENTER_TOP)
                .fixed_pos(screen_rect.center() - 0.5 * max_height * egui::Vec2::Y)
                .frame(egui::Frame::window(ui.style()).inner_margin(egui::Margin::symmetric(16, 0)))
                .resizable(false)
                .collapsible(false)
                .open(&mut self.about_window)
                .show(&ctx, |ui| {
                    ui.add_space(12.0);
                    ui.horizontal(|ui| {
                        ui.heading(egui::RichText::new("Eurochef").color(egui::Color32::WHITE));

                        // PATCH_0045: never byte-slice a build-time string at a
                        // fixed index. A failed `git rev-parse` can yield an empty
                        // GIT_HASH, and arbitrary UTF-8 also makes byte slicing unsafe.
                        let git_hash = option_env!("GIT_HASH").unwrap_or("unknown").trim();
                        let git_hash_short = if git_hash.is_empty() {
                            "unknown".to_string()
                        } else {
                            git_hash.chars().take(7).collect::<String>()
                        };

                        ui.heading(format!(
                            "- {} ({})",
                            env!("CARGO_PKG_VERSION"),
                            git_hash_short
                        ));
                    });
                    ui.add_space(8.0);

                    ui.label(format!("Compiler: {}", env!("RUSTC_VERSION")));
                    ui.label(format!("Build date: {}", env!("BUILD_DATE")));

                    ui.add_space(12.0);
                });
        }

        // TODO(cohae): More generic dialog (use for loading and error)
        if self.ps2_warning {
            egui::Window::new("PS2 Support")
            .pivot(egui::Align2::CENTER_TOP)
            .fixed_pos(screen_rect.center() - 0.5 * max_height * egui::Vec2::Y)
            .frame(egui::Frame::window(ui.style()).inner_margin(16))
            .resizable(false)
            .collapsible(false)
            .show(&ctx, |ui| {
                ui.horizontal(|ui| {
                    let (irect, _) =
                        ui.allocate_exact_size([54., 54.].into(), egui::Sense::hover());
                    ui.painter().text(
                        irect.center() + [0., 8.].into(),
                        egui::Align2::CENTER_CENTER,
                        font_awesome::EXCLAMATION_TRIANGLE,
                        egui::FontId::proportional(48.),
                        Color32::from_rgb(249, 239, 40),
                    );

                    ui.label("PS2 support is currently highly experimental.\nTextures work, but most entities will not draw properly.");
                });
                if ui.button("I understand").clicked() {
                    self.ps2_warning = false;
                }
            });
        }

        match state {
            AppState::Ready => {}
            AppState::Loading(s) => {
                egui::Window::new("Loading")
                    .title_bar(false)
                    .pivot(egui::Align2::CENTER_TOP)
                    .fixed_pos(screen_rect.center() - 0.5 * max_height * egui::Vec2::Y)
                    .frame(egui::Frame::window(ui.style()).inner_margin(16))
                    .resizable(false)
                    .show(&ctx, |ui| {
                        ui.horizontal(|ui| {
                            ui.spinner();
                            ui.label(s.as_str());
                        });
                    });
            }
            AppState::SelectPlatform => {
                egui::Window::new("Select platform")
                    .title_bar(false)
                    .pivot(egui::Align2::CENTER_TOP)
                    .fixed_pos(screen_rect.center() - 0.5 * max_height * egui::Vec2::Y)
                    .frame(egui::Frame::window(ui.style()).inner_margin(16))
                    .resizable(false)
                    .show(&ctx, |ui| {
                        ui.heading("Please select the platform for this file");
                        ui.separator();
                        ui.add_space(4.0);
                        ui.horizontal(|ui| {
                            ui.strong("Platform: ");
                            egui::ComboBox::from_label("")
                                .selected_text(selected_platform.to_string())
                                .show_ui(ui, |ui| {
                                    ui.selectable_value(
                                        selected_platform,
                                        Platform::GameCube,
                                        "GameCube",
                                    );
                                    ui.selectable_value(selected_platform, Platform::Pc, "PC");
                                    ui.selectable_value(
                                        selected_platform,
                                        Platform::Ps2,
                                        "PlayStation 2",
                                    );
                                    ui.selectable_value(
                                        selected_platform,
                                        Platform::Ps3,
                                        "PlayStation 3",
                                    );
                                    ui.selectable_value(
                                        selected_platform,
                                        Platform::ThreeDS,
                                        "3DS",
                                    );
                                    ui.selectable_value(selected_platform, Platform::Wii, "Wii");
                                    ui.selectable_value(selected_platform, Platform::WiiU, "Wii U");
                                    ui.selectable_value(selected_platform, Platform::Xbox, "Xbox");
                                    ui.selectable_value(
                                        selected_platform,
                                        Platform::Xbox360,
                                        "Xbox 360",
                                    );
                                });
                        });

                        ui.horizontal(|ui| {
                            if ui.button("Load").clicked() {
                                if let Some((_, platform)) = self.pending_file.as_mut() {
                                    *platform = Some(*selected_platform);
                                }
                                self.state = AppState::Loading("Loading file".to_string());
                            }

                            if ui.button("Cancel").clicked() {
                                self.pending_file = None;
                                self.state = AppState::Ready;
                            }
                        });
                    });
            }
            AppState::Error(e) => {
                let mut open = true;
                egui::Window::new("Error")
                    .pivot(egui::Align2::CENTER_TOP)
                    .fixed_pos(screen_rect.center() - 0.5 * max_height * egui::Vec2::Y)
                    // .frame(egui::Frame::window(&ctx.style()).inner_margin(16.))
                    .resizable(false)
                    .collapsible(false)
                    .open(&mut open)
                    .show(&ctx, |ui| {
                        ui.horizontal(|ui| {
                            let (irect, _) =
                                ui.allocate_exact_size([48., 48.].into(), egui::Sense::hover());
                            ui.painter().text(
                                irect.center() + [0., 8.].into(),
                                egui::Align2::CENTER_CENTER,
                                '\u{f00d}',
                                egui::FontId::proportional(48.),
                                Color32::from_rgb(250, 40, 40),
                            );

                            ui.label(remove_stacktrace(&format!("{e:?}")));
                        });

                        if !e.backtrace().to_string().starts_with("disabled backtrace") {
                            ui.add_space(4.);
                            ui.collapsing("Backtrace", |ui| {
                                egui::ScrollArea::vertical()
                                    .show(ui, |ui| ui.label(e.backtrace().to_string()));
                            });
                        }
                    });

                if !open {
                    *state = AppState::Ready;
                }
            }
        }

        egui::CentralPanel::default().show(ui, |ui| {
            if fileinfo.is_none() {
                ui.heading("No file loaded");
                return;
            }

            ui.horizontal(|ui| {
                if fileinfo.is_some() {
                    ui.selectable_value(current_panel, Panel::FileInfo, "File info");
                }

                if spreadsheetlist.is_some() {
                    ui.selectable_value(current_panel, Panel::Spreadsheets, "Text");
                }

                if textures.is_some() {
                    ui.selectable_value(current_panel, Panel::Textures, "Textures");
                }

                if entities.is_some() {
                    ui.selectable_value(current_panel, Panel::Entities, "Entities");
                }

                if scripts.is_some() {
                    ui.selectable_value(current_panel, Panel::Scripts, "Scripts");
                }

                if animations.is_some() {
                    ui.selectable_value(current_panel, Panel::Animations, "Animations");
                }

                if maps.is_some() {
                    ui.selectable_value(current_panel, Panel::Maps, "Maps");
                }
            });
            ui.separator();

            match current_panel {
                Panel::FileInfo => fileinfo
                    .as_mut()
                    .map(|s| s.show(ui, &self.hashcodes, &self.render_store.read())),
                Panel::Textures => textures.as_mut().map(|s| s.show(ui)),
                Panel::Entities => entities.as_mut().map(|s| s.show(&ctx, ui)),
                Panel::Spreadsheets => spreadsheetlist.as_mut().map(|s| s.show(ui)),
                Panel::Maps => {
                    if let Some(Err(e)) = maps.as_mut().map(|s| s.show(&ctx, ui)) {
                        self.state = AppState::Error(e);
                    };
                    Some(())
                }
                Panel::Scripts => scripts.as_mut().map(|s| s.show(ui)),
                Panel::Animations => animations.as_mut().map(|s| s.show(ui)),
            };
        });

        // TODO(cohae): Should be implemented in `TextureList::show`
        match current_panel {
            Panel::Textures => textures.as_mut().map(|s| s.show_enlarged_window(&ctx)),
            _ => None,
        };
    }
}

fn remove_stacktrace(s: &str) -> &str {
    if let Some(v) = s.to_lowercase().find("stack backtrace:") {
        s[..v].trim()
    } else {
        s
    }
}
