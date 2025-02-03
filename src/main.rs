mod deserialization;

use core::mem;
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File},
    io::Write,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Condvar, Mutex,
    },
    thread,
    time::Duration,
};

use clap::Parser;
use futures::{stream, StreamExt};
use mlua::{IntoLua, Lua, UserData};
use reqwest::{
    blocking as req_blocking,
    header::{self, HeaderMap, HeaderValue},
    Client,
};
use tokio::{
    fs::{self as tokio_fs},
    runtime::Runtime,
};

use deserialization::{
    Dep, DepPrefix, FullInfoJson, Image, LatestRelease, License, ModFull,
    ModList, Release,
};

const USER_AGENT: &str = "factorio-crater/0.1.0 (by Shadow0133 aka Aurora)";
const INTERNAL_MODS: &[&str] =
    &["base", "elevated-rails", "quality", "space-age"];

fn download_mod_list() {
    let url = "https://mods.factorio.com/api/mods?page_size=max";
    let resp = req_blocking::get(url).unwrap().text().unwrap();
    fs::write("mods.json", resp).unwrap();
}

type Error = Box<dyn core::error::Error + Send + Sync + 'static>;

#[derive(serde::Deserialize)]
struct PlayerCreds {
    #[serde(rename = "service-username")]
    username: String,
    #[serde(rename = "service-token")]
    token: String,
}

async fn download_mod(
    req: &Client,
    file_name: &str,
    download_url: &str,
    mods_folder: &Path,
    creds: &PlayerCreds,
) -> Result<(), Error> {
    let url = format!(
        "https://mods.factorio.com/{}?username={}&token={}",
        download_url, creds.username, creds.token
    );
    let resp = req
        .execute(req.get(url).build()?)
        .await?
        .error_for_status()?
        .bytes()
        .await?;
    tokio_fs::write(mods_folder.join(file_name), resp).await?;
    Ok(())
}

fn download_mods<'a>(
    factorio_instance: &Path,
    mod_list: impl Iterator<Item = &'a str> + Clone,
    mod_version_list: &BTreeMap<&'a str, Option<&'a LatestRelease>>,
) -> Result<(), Error> {
    let player_creds: PlayerCreds = simd_json::from_reader(File::open(
        factorio_instance.join("player-data.json"),
    )?)?;

    let mod_count = mod_list.clone().count();
    let counter = Arc::new(AtomicUsize::new(0));

    let mut headers = HeaderMap::new();
    headers.insert(header::USER_AGENT, HeaderValue::from_static(USER_AGENT));
    let req = Client::builder().default_headers(headers).build()?;
    let rt = Runtime::new()?;

    let mut futures = vec![];
    for name in mod_list {
        let player_creds = &player_creds;
        let req = &req;
        let counter = counter.clone();
        futures.push(async move {
            let release = mod_version_list[name].unwrap();
            download_mod(
                req,
                &release.file_name,
                &release.download_url,
                &factorio_instance.join("mods"),
                player_creds,
            )
            .await
            .unwrap();
            counter.fetch_add(1, Ordering::Relaxed);
        });
    }

    let pair = Arc::new((Mutex::new(()), Condvar::new()));
    let pair2 = pair.clone();
    let _ui = thread::spawn(move || {
        let (_, cvar) = &*pair;
        loop {
            let v = counter.load(Ordering::Relaxed);
            if v >= mod_count {
                break;
            }
            eprintln!("Downloaded {v}/{mod_count} mods");
            thread::sleep(Duration::from_secs(1));
            cvar.notify_one();
        }
    });
    rt.block_on(stream::iter(futures).for_each_concurrent(64, |x| x));
    let (lock, cvar) = &*pair2;
    let _a = cvar.wait(lock.lock().unwrap()).unwrap();
    Ok(())
}

async fn download_mod_meta_full(req: &Client, name: &str) -> Result<(), Error> {
    let url = format!("https://mods.factorio.com/api/mods/{name}/full");
    let resp = req.execute(req.get(url).build()?).await?.text().await?;
    tokio_fs::write(format!("mods/{name}.json"), resp).await?;
    Ok(())
}

fn download_mods_meta_full<'a>(
    mod_list: impl Iterator<Item = &'a str> + Clone,
) {
    let mod_count = mod_list.clone().count();
    let counter = Arc::new(AtomicUsize::new(0));

    let mut headers = HeaderMap::new();
    headers.insert(header::USER_AGENT, HeaderValue::from_static(USER_AGENT));
    let req = Client::builder().default_headers(headers).build().unwrap();
    let rt = Runtime::new().unwrap();

    let mut futures = vec![];
    for name in mod_list {
        let req = &req;
        let counter = counter.clone();
        futures.push(async move {
            download_mod_meta_full(req, name).await.unwrap();
            counter.fetch_add(1, Ordering::Relaxed);
            // eprintln!("finished downloading {name}");
        });
    }

    let _ui = thread::spawn(move || loop {
        let v = counter.load(Ordering::Relaxed);
        if v >= mod_count {
            break;
        }
        eprintln!("Downloaded {v}/{mod_count} mods' metadata");
        thread::sleep(Duration::from_secs(1));
    });
    rt.block_on(stream::iter(futures).for_each_concurrent(64, |x| x));
}

#[derive(clap::Parser)]
struct Opt {
    #[arg(short = 'U')]
    update_files: bool,

    #[command(subcommand)]
    command: Command,
}

#[derive(clap::Subcommand, Clone)]
enum Command {
    Run {
        lua_script: PathBuf,
    },
    Download {
        #[arg(short = 'f')]
        factorio_instance: PathBuf,
        mods: Vec<String>,
    },
    FindBrokenMods,
}

fn main() {
    let opts = Opt::parse();
    if opts.update_files {
        download_mod_list();
        eprintln!("finished downloading the modlist");
    }

    let modlist: ModList =
        simd_json::from_reader(File::open("mods.json").unwrap()).unwrap();
    let mod_version_list: BTreeMap<_, Option<_>> = modlist
        .results
        .iter()
        .map(|x| (x.name.as_str(), x.latest_release.as_ref()))
        .collect();

    let mod_list = mod_version_list.keys().copied();

    if opts.update_files {
        download_mods_meta_full(mod_list.clone());
    }

    match opts.command {
        Command::Run { lua_script } => run_lua(mod_list, &lua_script),
        Command::Download {
            factorio_instance,
            mods,
        } => download_mods(
            &factorio_instance,
            mods.iter().map(|x| x.as_str()),
            &mod_version_list,
        )
        .unwrap(),
        Command::FindBrokenMods => find_broken_mods(mod_version_list),
    }
}

fn run_lua<'a>(mod_list: impl Iterator<Item = &'a str>, lua_script: &Path) {
    let mut mod_map = BTreeMap::new();
    for name in mod_list {
        let file = File::open(format!("mods/{name}.json")).unwrap();
        let mod_full: ModFull = simd_json::from_reader(file).unwrap();
        mod_map.insert(name, mod_full);
    }

    let lua = Lua::new();
    lua.globals().set("mods", mod_map).unwrap();
    let chunk = lua.load(lua_script);
    chunk.exec().unwrap();
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
        fields
            .add_field_method_get("deprecated", |_, this| Ok(this.deprecated));
        fields.add_field_method_get("description", |_, this| {
            Ok(this.description.clone())
        });
        fields.add_field_method_get("homepage", |_, this| {
            Ok(this.homepage.clone())
        });
        fields
            .add_field_method_get("images", |_, this| Ok(this.images.clone()));
        fields.add_field_method_get("license", |_, this| {
            Ok(this.license.clone())
        });
        fields.add_field_method_get("name", |_, this| Ok(this.name.clone()));
        fields.add_field_method_get("owner", |_, this| Ok(this.owner.clone()));
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
        fields.add_field_method_get("tags", |_, this| Ok(this.tags.clone()));
        fields.add_field_method_get("thumbnail", |_, this| {
            Ok(this.thumbnail.clone())
        });
        fields.add_field_method_get("title", |_, this| Ok(this.title.clone()));
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
        fields.add_field_method_get("name", |_, this| Ok(this.name.clone()));
        fields.add_field_method_get("title", |_, this| Ok(this.title.clone()));
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
        fields.add_field_method_get("sha1", |_, this| Ok(this.sha1.clone()));
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
        fields.add_field_method_get("name", |_, this| Ok(this.name.clone()));
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

#[derive(Clone)]
struct ModWithInfo {
    deprecated: bool,
    factorio_version: String,
    dependencies: Vec<Dep>,
}

fn find_broken_mods<'a>(
    mod_version_list: BTreeMap<&'a str, Option<&'a LatestRelease>>,
) {
    eprintln!("all mods: {}", mod_version_list.len());

    let mut mod_map = BTreeMap::new();
    for (name, latest_version) in mod_version_list {
        let file = File::open(format!("mods/{name}.json")).unwrap();
        let mod_full: ModFull = simd_json::from_reader(file).unwrap();
        if latest_version.is_some() == mod_full.releases.is_empty() {
            eprintln!("release mismatch for {name}");
        }
        if let Some(release) = mod_full.releases.into_iter().find(|x| {
            Some(x.version.as_str())
                == latest_version.as_ref().map(|x| x.version.as_str())
        }) {
            mod_map.insert(
                name,
                ModWithInfo {
                    deprecated: mod_full.deprecated,
                    factorio_version: release.info_json.factorio_version,
                    dependencies: release.info_json.dependencies,
                },
            );
        }
    }
    let mut deprecated = BTreeSet::<String>::new();
    let mut rest = BTreeMap::<String, ModWithInfo>::new();
    for (&name, m) in &mod_map {
        if m.deprecated {
            deprecated.insert(name.into());
        } else {
            rest.insert(name.into(), m.clone());
        }
    }

    eprintln!("deprecated: {}", deprecated.len());
    let mut deprecated_file = File::create("deprecated.txt").unwrap();
    for name in &deprecated {
        writeln!(deprecated_file, "{name}",).unwrap();
    }

    for &m in INTERNAL_MODS {
        deprecated.remove(m);
    }
    let deprecated = deprecated;

    let mut broken = BTreeMap::new();
    let mut working = BTreeSet::<String>::new();
    let mut typod = BTreeMap::new();
    working.extend(INTERNAL_MODS.iter().map(|x| x.to_string()));
    while !rest.is_empty() {
        for (name, m) in mem::take(&mut rest) {
            let iter = m
                .dependencies
                .iter()
                .filter(|x| matches!(x.prefix, DepPrefix::Required));
            if iter.clone().all(|x| working.contains(&x.name)) {
                working.insert(name);
            } else if let Some(typod_dep) = iter.clone().find(|x| {
                !mod_map.contains_key(&*x.name)
                    && !INTERNAL_MODS.contains(&x.name.as_str())
            }) {
                typod.insert(name, typod_dep.name.clone());
            } else if iter.clone().any(|x| {
                deprecated.contains(&x.name) | broken.contains_key(&x.name)
            }) {
                let broken_deps = iter
                    .filter(|x| {
                        deprecated.contains(&x.name)
                            | broken.contains_key(&x.name)
                    })
                    .map(|x| x.name.clone())
                    .collect::<Vec<_>>();
                broken.insert(name, (m, broken_deps));
            } else {
                rest.insert(name, m);
            }
        }
    }
    eprintln!("rest: {}", rest.len());
    for (name, m) in rest {
        eprintln!("{name}");
        for dep in m.dependencies {
            eprint!("- {}: ", dep.name);
            if working.contains(&dep.name) {
                eprint!("working");
            }
            if broken.contains_key(&dep.name) {
                eprint!("broken");
            }
            if deprecated.contains(&dep.name) {
                eprint!("deprecated");
            }
            if typod.contains_key(&dep.name) {
                eprint!("typod");
            }
            eprintln!();
        }
    }

    broken.retain(|_, (m, _)| &*m.factorio_version < "2.0");
    eprintln!("broken: {}", broken.len());
    let mut b_file = File::create("broken.txt").unwrap();
    for (name, (m, _)) in &broken {
        writeln!(b_file, "{name} for {}", m.factorio_version).unwrap();
    }

    let mut bwr_file = File::create("broken_with_reason.txt").unwrap();
    for (name, (m, broken_deps)) in &broken {
        writeln!(bwr_file, "{name} for {} because of:", m.factorio_version)
            .unwrap();
        for broken_dep in broken_deps {
            writeln!(bwr_file, "  {broken_dep}").unwrap();
        }
    }

    let mut b_file = File::create("broken_for_1.1.txt").unwrap();
    for (name, (m, _)) in &broken {
        if m.factorio_version != "1.1" {
            continue;
        }
        writeln!(b_file, "{name} for {}", m.factorio_version).unwrap();
    }

    let mut bwrf1_1_file =
        File::create("broken_with_reason_for_1.1.txt").unwrap();
    for (name, (m, broken_deps)) in &broken {
        if m.factorio_version != "1.1" {
            continue;
        }
        writeln!(
            bwrf1_1_file,
            "{name} for {} because of:",
            m.factorio_version
        )
        .unwrap();
        for broken_dep in broken_deps {
            writeln!(bwrf1_1_file, "  {broken_dep}").unwrap();
        }
    }
    eprintln!("done");
}
