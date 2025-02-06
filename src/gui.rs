use std::{collections::BTreeMap, ops::Range};

use eframe::{
    egui::{self, Context, Image, Vec2},
    App, CreationContext, Frame,
};

use crate::{deserialization::ModFull, load_mod_map, Error, APP_ID};

struct Gui<'a> {
    mods: BTreeMap<&'a str, ModFull>,
}

impl<'a> Gui<'a> {
    fn new(
        ctx: &CreationContext,
        mod_list: impl Iterator<Item = &'a str>,
    ) -> Result<Self, Error> {
        egui_extras::install_image_loaders(&ctx.egui_ctx);
        let mods = load_mod_map(mod_list);
        let gui = Gui { mods };
        Ok(gui)
    }
}

const SIZE: f32 = 150.0;

fn draw_mod_list_item(ui: &mut egui::Ui, m: &ModFull) {
    egui::Frame::new().show(ui, |ui| {
        // ui.set_max_height(HEIGHT);
        ui.columns_const(|[a, b]| {
            a.set_max_width(SIZE);
            if let Some(image) = &m.thumbnail {
                a.add(
                    Image::new(format!(
                        "https://assets-mod.factorio.com{image}"
                    ))
                    .max_size(Vec2::splat(SIZE)),
                );
            }

            b.heading(&m.name);
            b.label(&m.owner);
            b.label(&m.summary);
        });
    });
}

impl App for Gui<'_> {
    fn update(&mut self, ctx: &Context, _frame: &mut Frame) {
        egui::SidePanel::left("mods_list").show(ctx, |ui| {
            egui::ScrollArea::vertical().show_rows(
                ui,
                SIZE,
                self.mods.len(),
                |ui, Range { start, end }| {
                    for m in self.mods.values().take(end).skip(start) {
                        draw_mod_list_item(ui, m);
                    }
                },
            )
        });
        egui::CentralPanel::default().show(ctx, |ui| ui.heading("hello"));
    }
}

pub fn run_gui<'a>(mod_list: impl Iterator<Item = &'a str>) {
    let options = eframe::NativeOptions::default();
    eframe::run_native(
        APP_ID,
        options,
        Box::new(|ctx| Ok(Box::new(Gui::new(ctx, mod_list)?))),
    )
    .unwrap();
}
