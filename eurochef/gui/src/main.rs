#![warn(clippy::all, rust_2018_idioms)]
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide

use color_eyre::eyre::Result;

// When compiling natively:
#[cfg(not(target_arch = "wasm32"))]
fn main() -> Result<()> {
    use clap::Parser;
    use color_eyre::Report;

    #[derive(Parser, Debug)]
    struct Args {
        /// Input file
        file: Option<String>,

        /// hashcodes.h
        #[arg(long, short = 't')]
        hashcodes: Option<String>,
        /// Start the loaded level in the dedicated Game Mode (context 0).
        #[arg(long)]
        play: bool,
    }
    let args = Args::parse();

    // Force enable backtraces
    std::env::set_var("RUST_BACKTRACE", "1");

    eurochef_gui::panic_dialog::setup();

    // Log to stdout (if you run with `RUST_LOG=debug`).
    tracing_subscriber::fmt::init();

    // Game Mode starts in a modern maximized borderless-work-area window;
    // egui uses logical points and the GPU renders at the monitor DPI scale.
    // Keep the legacy editor's original starting window unchanged.
    let viewport = if args.play {
        egui::ViewportBuilder::default()
            .with_inner_size([1920.0, 1080.0])
            .with_min_inner_size([960.0, 540.0])
            .with_maximized(true)
    } else {
        egui::ViewportBuilder::default().with_inner_size([1280.0, 1024.0])
    };
    let native_options = eframe::NativeOptions {
        viewport,
        depth_buffer: 24,
        multisampling: 0,
        ..Default::default()
    };
    let res = eframe::run_native(
        "Eurochef",
        native_options,
        Box::new(|cc| {
            Ok(Box::new(eurochef_gui::EurochefApp::new_with_launch(
                args.file,
                args.hashcodes,
                args.play,
                cc,
            )))
        }),
    );

    match res {
        Ok(()) => Ok(()),
        Err(e) => Err(Report::msg(e.to_string())),
    }
}

// when compiling to web using trunk.
#[cfg(target_arch = "wasm32")]
fn main() {
    // Make sure panics are logged using `console.error`.
    console_error_panic_hook::set_once();

    // Redirect tracing to console.log and friends:
    tracing_wasm::set_as_global_default();

    let web_options = eframe::WebOptions::default();

    wasm_bindgen_futures::spawn_local(async {
        eframe::WebRunner::new()
            .start(
                "the_canvas_id", // hardcode it
                web_options,
                Box::new(|cc| Box::new(eurochef_gui::EurochefApp::new(None, None, cc))),
            )
            .await
            .expect("failed to start eframe");
    });
}
