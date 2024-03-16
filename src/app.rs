use crate::sections::ExecutableFile;

#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum View {
    #[default]
    Inspector,
}

#[derive(Default)]
pub struct ExampleApp {
    /// Options for configuring how the Inspector is displayed.
    pub inspector_options: crate::inspector::Options,

    /// What view is active.
    pub view: View,
    pub files: Vec<ExecutableFile>,
}

impl eframe::App for ExampleApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.view, View::Inspector, "Inspector");
            });

            ui.separator();

            match self.view {
                View::Inspector => {
                    crate::inspector::ui(ui, &mut self.inspector_options, &mut self.files)
                }
            }
        });
    }
}
