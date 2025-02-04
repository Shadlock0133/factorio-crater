mod deserialization;
mod download;
mod gui;
mod lua;

use core::mem;
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::File,
    io::Write,
    path::PathBuf,
};

use clap::Parser;
use gui::run_gui;

use crate::{
    deserialization::{Dep, DepPrefix, LatestRelease, ModFull, ModList},
    download::{download_mod_list, download_mods, download_mods_meta_full},
    lua::run_lua,
};

const INTERNAL_MODS: &[&str] =
    &["base", "elevated-rails", "quality", "space-age"];
const USER_AGENT: &str = "factorio-crater/0.1.0 (by Shadow0133 aka Aurora)";

type Error = Box<dyn core::error::Error + Send + Sync + 'static>;

#[derive(clap::Parser)]
struct Opt {
    #[arg(short = 'U')]
    update_files: bool,

    #[command(subcommand)]
    command: Option<Command>,
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
        None => run_gui(),
        Some(Command::Run { lua_script }) => run_lua(mod_list, &lua_script),
        Some(Command::Download {
            factorio_instance,
            mods,
        }) => download_mods(
            &factorio_instance,
            mods.iter().map(|x| x.as_str()),
            &mod_version_list,
        )
        .unwrap(),
        Some(Command::FindBrokenMods) => find_broken_mods(mod_version_list),
    }
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
