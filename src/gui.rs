use std::{cmp::Reverse, ops::Range};

use eframe::{
    egui::{self, Context, Image, Vec2},
    App, CreationContext, Frame,
};

use crate::{deserialization::ModFull, load_mod_list, Error, APP_ID};

struct Gui {
    mods: Vec<ModFull>,
}

impl Gui {
    fn new(ctx: &CreationContext) -> Result<Self, Error> {
        egui_extras::install_image_loaders(&ctx.egui_ctx);
        let mut mods = load_mod_list();
        mods.sort_unstable_by_key(|x| Reverse(x.updated_at.clone()));
        let gui = Gui { mods };
        Ok(gui)
    }
}

const SIZE: f32 = 150.0;

fn draw_mod_list_item(ui: &mut egui::Ui, m: &ModFull) {
    egui::Frame::new().show(ui, |ui| {
        ui.set_height(SIZE);
        ui.columns_const(|[a, b]| {
            a.set_width(SIZE);
            if let Some(image) = &m.thumbnail {
                a.add(
                    Image::new(format!(
                        "https://assets-mod.factorio.com{image}"
                    ))
                    .max_size(Vec2::splat(SIZE)),
                );
            }

            b.heading(&m.title);
            b.label(format!("by {}", m.owner));
            b.label(&m.summary);
        });
    });
}

impl App for Gui {
    fn update(&mut self, ctx: &Context, _frame: &mut Frame) {
        // egui::SidePanel::left("mods_list").show(ctx, |ui| {
        // });
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("mods");
            egui::ScrollArea::vertical().show_rows(
                ui,
                SIZE,
                self.mods.len(),
                |ui, Range { start, end }| {
                    for m in self.mods.iter().take(end).skip(start) {
                        draw_mod_list_item(ui, m);
                    }
                },
            )
        });
    }
}

pub fn run_gui() {
    let options = eframe::NativeOptions::default();
    eframe::run_native(
        APP_ID,
        options,
        Box::new(|ctx| Ok(Box::new(Gui::new(ctx)?))),
    )
    .unwrap();
}
