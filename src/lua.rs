use std::{collections::BTreeMap, fs::File, path::Path};

use mlua::{IntoLua, Lua, UserData};

use crate::deserialization::{
    Dep, DepPrefix, FullInfoJson, Image, License, ModFull, Release,
};

pub fn run_lua<'a>(
    mod_list: impl Iterator<Item = &'a str>,
    lua_script: &Path,
) {
    let mod_map = load_mod_map(mod_list);

    let lua = Lua::new();
    lua.globals().set("mods", mod_map).unwrap();
    let chunk = lua.load(lua_script);
    chunk.exec().unwrap();
}

fn load_mod_map<'a>(
    mod_list: impl Iterator<Item = &'a str>,
) -> BTreeMap<&'a str, ModFull> {
    let mut mod_map = BTreeMap::new();
    for name in mod_list {
        let file = File::open(format!("mods/{name}.json")).unwrap();
        let mod_full: ModFull = simd_json::from_reader(file).unwrap();
        mod_map.insert(name, mod_full);
    }
    mod_map
}

impl UserData for ModFull {
    fn add_fields<F: mlua::UserDataFields<Self>>(fields: &mut F) {
        fields.add_field_method_get("category", |_, this| {
            Ok(this.category.clone())
        });
        fields.add_field_method_get("changelog", |_, this| {
            Ok(this.changelog.clone())
        });
        fields.add_field_method_get("created_at", |_, this| {
            Ok(this.created_at.clone())
        });
        fields.add_field_method_get("downloads_count", |_, this| {
            Ok(this.downloads_count)
        });
        fields.add_field_method_get("deprecated", |_, this| {
            Ok(this.deprecated)
        });
        fields.add_field_method_get("description", |_, this| {
            Ok(this.description.clone())
        });
        fields.add_field_method_get("homepage", |_, this| {
            Ok(this.homepage.clone())
        });
        fields.add_field_method_get("images", |_, this| {
            Ok(this.images.clone())
        });
        fields.add_field_method_get("license", |_, this| {
            Ok(this.license.clone())
        });
        fields
            .add_field_method_get("name", |_, this| Ok(this.name.clone()));
        fields.add_field_method_get("owner", |_, this| {
            Ok(this.owner.clone())
        });
        fields.add_field_method_get("releases", |_, this| {
            Ok(this.releases.clone())
        });
        fields.add_field_method_get("score", |_, this| Ok(this.score));
        fields.add_field_method_get("source_url", |_, this| {
            Ok(this.source_url.clone())
        });
        fields.add_field_method_get("summary", |_, this| {
            Ok(this.summary.clone())
        });
        fields
            .add_field_method_get("tags", |_, this| Ok(this.tags.clone()));
        fields.add_field_method_get("thumbnail", |_, this| {
            Ok(this.thumbnail.clone())
        });
        fields.add_field_method_get("title", |_, this| {
            Ok(this.title.clone())
        });
        fields.add_field_method_get("updated_at", |_, this| {
            Ok(this.updated_at.clone())
        });
    }
}

impl UserData for Image {
    fn add_fields<F: mlua::UserDataFields<Self>>(fields: &mut F) {
        fields.add_field_method_get("id", |_, this| Ok(this.id.clone()));
        fields.add_field_method_get("thumbnail", |_, this| {
            Ok(this.thumbnail.clone())
        });
        fields.add_field_method_get("url", |_, this| Ok(this.url.clone()));
    }
}

impl UserData for License {
    fn add_fields<F: mlua::UserDataFields<Self>>(fields: &mut F) {
        fields.add_field_method_get("description", |_, this| {
            Ok(this.description.clone())
        });
        fields.add_field_method_get("id", |_, this| Ok(this.id.clone()));
        fields
            .add_field_method_get("name", |_, this| Ok(this.name.clone()));
        fields.add_field_method_get("title", |_, this| {
            Ok(this.title.clone())
        });
        fields.add_field_method_get("url", |_, this| Ok(this.url.clone()));
    }
}

impl<INFO: IntoLua + Clone> UserData for Release<INFO> {
    fn add_fields<F: mlua::UserDataFields<Self>>(fields: &mut F) {
        fields.add_field_method_get("download_url", |_, this| {
            Ok(this.download_url.clone())
        });
        fields.add_field_method_get("file_name", |_, this| {
            Ok(this.file_name.clone())
        });
        fields.add_field_method_get("info_json", |_, this| {
            Ok(this.info_json.clone())
        });
        fields.add_field_method_get("released_at", |_, this| {
            Ok(this.released_at.clone())
        });
        fields
            .add_field_method_get("sha1", |_, this| Ok(this.sha1.clone()));
        fields.add_field_method_get("version", |_, this| {
            Ok(this.version.clone())
        });
    }
}

impl UserData for FullInfoJson {
    fn add_fields<F: mlua::UserDataFields<Self>>(fields: &mut F) {
        fields.add_field_method_get("dependencies", |_, this| {
            Ok(this.dependencies.clone())
        });
        fields.add_field_method_get("factorio_version", |_, this| {
            Ok(this.factorio_version.clone())
        });
    }
}

impl UserData for Dep {
    fn add_fields<F: mlua::UserDataFields<Self>>(fields: &mut F) {
        fields.add_field_method_get("original", |_, this| {
            Ok(this.original.clone())
        });
        fields.add_field_method_get("prefix", |_, this| Ok(this.prefix));
        fields
            .add_field_method_get("name", |_, this| Ok(this.name.clone()));
        fields.add_field_method_get("version", |_, this| {
            Ok(this.version.clone())
        });
    }
}

impl IntoLua for DepPrefix {
    fn into_lua(self, lua: &Lua) -> mlua::Result<mlua::Value> {
        mlua::String::wrap(match self {
            DepPrefix::Incompatible => "incompatible",
            DepPrefix::Optional => "optional",
            DepPrefix::HiddenOptional => "hidden-optional",
            DepPrefix::LoadOrderIndependent => "load-order-independent",
            DepPrefix::Required => "required",
        })
        .into_lua(lua)
    }
}
