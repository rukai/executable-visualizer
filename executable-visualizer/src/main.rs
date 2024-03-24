#![forbid(unsafe_code)]

use executable_visualizer_lib::app::ExampleApp;
use executable_visualizer_lib::sections::ExecutableFile;

fn main() -> eframe::Result<()> {
    let files = vec![ExecutableFile::load_self()];
    let app = ExampleApp::new(files);

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([320.0, 240.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Executable Inspector",
        native_options,
        Box::new(|_cc| Box::new(app)),
    )
}
