use crate::sections::ExecutableFile;
use anyhow::Result;
use egui::Ui;
use std::sync::mpsc;

pub struct FileLoader {
    rx: mpsc::Receiver<Result<ExecutableFile>>,
    tx: mpsc::Sender<Result<ExecutableFile>>,
    error: Option<String>,
}

impl Default for FileLoader {
    fn default() -> Self {
        let (tx, rx) = mpsc::channel();
        let error = None;
        Self { rx, tx, error }
    }
}

impl FileLoader {
    pub fn display_error(&mut self, ui: &mut Ui) {
        if let Some(error) = &self.error {
            let mut window_open = true;
            let screen_rect = ui.ctx().input(|i| i.screen_rect()).size();
            egui::Window::new("Error loading file")
                .open(&mut window_open)
                .vscroll(true)
                .pivot(egui::Align2::CENTER_CENTER)
                .default_pos((screen_rect.x / 2.0, screen_rect.y / 2.0))
                .show(ui.ctx(), |ui| ui.label(error));

            if !window_open {
                self.error = None;
            }
        }
    }

    pub fn request_file_from_user(&self, ui: &mut Ui) {
        let task = rfd::AsyncFileDialog::new().pick_file();
        let ctx = ui.ctx().clone();
        let sender = self.tx.clone();
        execute(async move {
            let file = task.await;
            if let Some(file) = file {
                let name = file.file_name();
                let contents = file.read().await;
                sender
                    .send(ExecutableFile::load_from_bytes(name, &contents))
                    .ok();
                ctx.request_repaint();
            }
        });
    }

    pub fn recive_file_from_user(&mut self) -> Option<ExecutableFile> {
        match self.rx.try_recv().ok() {
            Some(Ok(file)) => Some(file),
            Some(Err(err)) => {
                self.error = Some(format!("{err:?}"));
                None
            }
            None => None,
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn execute<F: std::future::Future<Output = ()> + Send + 'static>(f: F) {
    std::thread::spawn(move || futures::executor::block_on(f));
}

#[cfg(target_arch = "wasm32")]
fn execute<F: std::future::Future<Output = ()> + 'static>(f: F) {
    wasm_bindgen_futures::spawn_local(f);
}
