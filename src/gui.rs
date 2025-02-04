use eframe::{
    egui::{self, Context},
    App, CreationContext, Frame,
};

use crate::Error;

struct Gui {}

impl Gui {
    fn new(_ctx: &CreationContext) -> Result<Self, Error> {
        let gui = Gui {};
        Ok(gui)
    }
}

impl App for Gui {
    fn update(&mut self, _ctx: &Context, _frame: &mut Frame) {
        egui::CentralPanel::default().show(_ctx, |ui| ui.heading("hello"));
    }
}

pub fn run_gui() {
    let options = eframe::NativeOptions::default();
    eframe::run_native(
        "factorio-crater",
        options,
        Box::new(|ctx| Ok(Box::new(Gui::new(ctx)?))),
    )
    .unwrap();
}
