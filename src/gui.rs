use std::{cmp::Reverse, ops::Range};

use eframe::{
    egui::{self, Context, Image, Sense, Vec2},
    App, CreationContext, Frame,
};

use crate::{deserialization::ModFull, load_mod_list, Error, APP_ID};

struct Gui {
    mods: Vec<ModFull>,
    selected_mod: Option<ModFull>,
    selected_image: Option<String>,
}

impl Gui {
    fn new(ctx: &CreationContext) -> Result<Self, Error> {
        egui_extras::install_image_loaders(&ctx.egui_ctx);
        let mut mods = load_mod_list();
        mods.sort_unstable_by_key(|x| Reverse(x.updated_at.clone()));
        let gui = Gui {
            mods,
            selected_mod: None,
            selected_image: None,
        };
        Ok(gui)
    }
}

const SIZE: f32 = 150.0;

fn draw_mod_list_item(ui: &mut egui::Ui, m: &ModFull) -> bool {
    let mut clicked = false;
    ui.horizontal(|ui| {
        let (id, image_space) = ui.allocate_space(Vec2::splat(SIZE));
        if let Some(image) = &m.thumbnail {
            ui.put(
                image_space,
                Image::new(format!("https://assets-mod.factorio.com{image}")),
            );
        };
        clicked |= ui.interact(image_space, id, Sense::click()).clicked();
        ui.vertical(|ui| {
            clicked |= ui.heading(&m.title).clicked();
            ui.label(format!("by {}", m.owner));
            ui.label(&m.summary);
        });
    });
    clicked
}

impl App for Gui {
    fn update(&mut self, ctx: &Context, _frame: &mut Frame) {
        egui::SidePanel::left("mods_list").show(ctx, |ui| {
            ui.heading("mods");
            ui.separator();
            egui::ScrollArea::vertical().show_rows(
                ui,
                SIZE,
                self.mods.len(),
                |ui, Range { start, end }| {
                    for m in self.mods.iter().take(end).skip(start) {
                        if draw_mod_list_item(ui, m) {
                            self.selected_mod = Some(m.clone());
                            self.selected_image = None;
                        };
                        ui.separator();
                    }
                },
            )
        });
        egui::CentralPanel::default().show(ctx, |ui| {
            if let Some(m) = &self.selected_mod {
                ui.heading(&m.title);
                ui.label(format!("by {}", &m.owner));
                egui::ScrollArea::vertical().show(ui, |ui| {
                    if !m.images.is_empty() {
                        ui.group(|ui| {
                            egui::ScrollArea::horizontal().show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    for image in &m.images {
                                        let (id, rect) = ui
                                            .allocate_space(Vec2::splat(SIZE));
                                        ui.put(
                                            rect,
                                            egui::Image::new(&image.thumbnail)
                                                .max_size(Vec2::splat(SIZE)),
                                        );
                                        if ui
                                            .interact(rect, id, Sense::click())
                                            .clicked()
                                        {
                                            self.selected_image =
                                                Some(image.url.clone());
                                        }
                                    }
                                });
                            });
                            if let Some(image) = &self.selected_image {
                                ui.image(image);
                            }
                        });
                    }
                    ui.label(m.description.as_deref().unwrap_or_default());
                });
            }
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
