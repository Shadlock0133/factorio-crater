mod deserialization;

use core::mem;
use std::{
    collections::{BTreeMap, BTreeSet},
    env,
    fs::{self, File},
    io::{self, Write},
};

use futures::{stream, StreamExt};
use reqwest::{
    blocking as req_blocking,
    header::{self, HeaderMap, HeaderValue},
    Client,
};
use tokio::{fs as tokio_fs, runtime::Runtime};

use deserialization::{Dep, DepPrefix, ModFull, ModList};

const USER_AGENT: &str = "factorio-crater/0.1.0 (by Shadow0133 aka Aurora)";
const INTERNAL_MODS: &[&str] =
    &["base", "elevated-rails", "quality", "space-age"];

fn download_mod_list() {
    let url = "https://mods.factorio.com/api/mods?page_size=max";
    let resp = req_blocking::get(url).unwrap().text().unwrap();
    fs::write("mods.json", resp).unwrap();
}

async fn download_mod_meta_full(req: &Client, name: &str) {
    let url = format!("https://mods.factorio.com/api/mods/{name}/full");
    let resp = req
        .execute(req.get(url).build().unwrap())
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    tokio_fs::write(format!("mods/{name}.json"), resp)
        .await
        .unwrap();
}

fn download_mods_meta_full<'a>(mod_list: impl Iterator<Item = &'a str>) {
    let mut headers = HeaderMap::new();
    headers.insert(header::USER_AGENT, HeaderValue::from_static(USER_AGENT));
    let req = Client::builder().default_headers(headers).build().unwrap();
    let rt = Runtime::new().unwrap();
    let mut futures = vec![];
    for name in mod_list {
        let req = &req;
        futures.push(async move {
            download_mod_meta_full(req, name).await;
            eprintln!("finished downloading {name}");
        });
    }
    rt.block_on(stream::iter(futures).for_each_concurrent(64, |x| x));
}

fn main() {
    let update_files = env::args().skip(1).any(|x| x == "-U");
    if update_files {
        download_mod_list();
        eprintln!("finished downloading the modlist");
    }

    let modlist: ModList =
        simd_json::from_reader(File::open("mods.json").unwrap()).unwrap();
    let mod_version_list: BTreeMap<String, Option<String>> = modlist
        .results
        .into_iter()
        .map(|x| (x.name, x.latest_release.map(|x| x.version)))
        .collect();

    if update_files {
        download_mods_meta_full(mod_version_list.keys().map(|x| x.as_str()));
        return;
    }
    find_broken_mods(mod_version_list);
}

#[derive(Clone)]
struct ModWithInfo {
    deprecated: bool,
    mod_version: String,
    factorio_version: String,
    dependencies: Vec<Dep>,
}

fn find_broken_mods(mod_version_list: BTreeMap<String, Option<String>>) {
    eprintln!("all mods: {}", mod_version_list.len());

    let mut mod_map = BTreeMap::new();
    for (name, latest_version) in mod_version_list {
        let file = File::open(format!("mods/{name}.json")).unwrap();
        let mod_full: ModFull = simd_json::from_reader(file).unwrap();
        if latest_version.is_some() == mod_full.releases.is_empty() {
            eprintln!("release mismatch for {name}");
        }
        if let Some(release) = mod_full
            .releases
            .into_iter()
            .find(|x| Some(x.version.as_str()) == latest_version.as_deref())
        {
            mod_map.insert(
                name,
                ModWithInfo {
                    deprecated: mod_full.deprecated,
                    mod_version: release.version,
                    factorio_version: release.info_json.factorio_version,
                    dependencies: release.info_json.dependencies,
                },
            );
        }
    }
    write_mods_with_deps(&mod_map).unwrap();
    let mut deprecated = BTreeSet::<String>::new();
    let mut rest = BTreeMap::<String, ModWithInfo>::new();
    for (name, m) in &mod_map {
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
                !mod_map.contains_key(&x.name)
                    && !INTERNAL_MODS.contains(&x.name.as_str())
            }) {
                typod.insert(name, typod_dep.name.clone());
            } else if iter.clone().any(|x| {
                deprecated.contains(&x.name) | broken.contains_key(&x.name)
            }) {
                if &*m.factorio_version >= "2.0" {
                    continue;
                }
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
            eprintln!()
        }
    }
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
    eprintln!("done");
}

fn write_mods_with_deps(
    mod_list: &BTreeMap<String, ModWithInfo>,
) -> io::Result<()> {
    let mut deps_file = File::create("deps.txt")?;
    for (name, dep) in mod_list {
        writeln!(deps_file, "name: {name}")?;
        writeln!(deps_file, "  deprecated: {}", dep.deprecated)?;
        writeln!(deps_file, "  mod version: {}", dep.mod_version)?;
        writeln!(deps_file, "  factorio version: {}", dep.factorio_version)?;
        writeln!(deps_file, "  deps:")?;
        for dep in &dep.dependencies {
            write!(deps_file, "    {}", dep.name)?;
            if !dep.version.is_empty() {
                write!(deps_file, ": {}", dep.version)?;
            }
            writeln!(deps_file, " ({:?})", dep.prefix)?;
        }
    }
    Ok(())
}
