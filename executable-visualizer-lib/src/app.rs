use crate::file_loader::FileLoader;
use crate::sections::ExecutableFile;

#[derive(Default)]
pub struct ExampleApp {
    /// Options for configuring how the Inspector is displayed.
    pub inspector_options: crate::inspector::Options,

    pub files: Vec<ExecutableFile>,
    file_loader: FileLoader,
}

impl ExampleApp {
    pub fn new(files: Vec<ExecutableFile>) -> Self {
        ExampleApp {
            inspector_options: Default::default(),
            files,
            file_loader: FileLoader::default(),
        }
    }
}

impl eframe::App for ExampleApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            // TODO: actual UI should go inline:
            //       --------
            //       v new file
            //            Load file from disk       Load file from preset      Or just drag file onto window
            ui.menu_button("File", |ui| {
                if ui.button("Load file").clicked() {
                    self.file_loader.request_file_from_user(ui);
                }
            });
            self.file_loader.display_error(ui);
            if let Some(file) = self.file_loader.recive_file_from_user() {
                self.files.push(file);
            }

            ui.separator();

            crate::inspector::ui(ui, &mut self.inspector_options, &mut self.files)
        });
    }
}
