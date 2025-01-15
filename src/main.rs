use std::{
    collections::{BTreeMap, BTreeSet},
    env, fmt,
    fs::{self, File},
    io::{self, Write},
    str::FromStr,
};

use futures::StreamExt;
use reqwest::header::{self, HeaderMap, HeaderValue};
use serde::{
    de::{self, Visitor},
    Deserialize, Deserializer,
};

const USER_AGENT: &str = "factorio-crater/0.1.0 (by Shadow0133 aka Aurora)";
const INTERNAL_MODS: &[&str] =
    &["base", "elevated-rails", "quality", "space-age"];

fn download_mod_list() {
    let url = "https://mods.factorio.com/api/mods?page_size=max";
    let resp = reqwest::blocking::get(url).unwrap().text().unwrap();
    fs::write("mods.json", resp).unwrap();
}

async fn download_mod(req: &reqwest::Client, name: &str) {
    let url = format!("https://mods.factorio.com/api/mods/{name}/full");
    let resp = req
        .execute(req.get(url).build().unwrap())
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    tokio::fs::write(format!("mods/{name}.json"), resp)
        .await
        .unwrap();
}

fn download_mods<'a>(mod_list: impl Iterator<Item = &'a str>) {
    let mut headers = HeaderMap::new();
    headers.insert(header::USER_AGENT, HeaderValue::from_static(USER_AGENT));
    let req = reqwest::Client::builder()
        .default_headers(headers)
        .build()
        .unwrap();
    let rt = tokio::runtime::Runtime::new().unwrap();
    let mut futures = vec![];
    for name in mod_list {
        let req = &req;
        futures.push(async move {
            download_mod(req, name).await;
            eprintln!("finished downloading {name}");
        });
    }
    rt.block_on(futures::stream::iter(futures).for_each_concurrent(64, |x| x));
}

#[derive(Deserialize)]
struct ModList {
    results: Vec<Mod>,
}

#[derive(Debug, Deserialize)]
struct Mod {
    name: String,
    latest_release: Option<LatestRelease>,
}

#[derive(Debug, Deserialize)]
struct LatestRelease {
    version: String,
}

#[derive(Debug, Deserialize)]
struct ModFull {
    name: String,
    #[serde(default)]
    deprecated: bool,
    releases: Vec<Release>,
}

#[derive(Debug, Deserialize)]
struct Release {
    version: String,
    info_json: InfoJson,
}

#[derive(Debug, Deserialize)]
struct InfoJson {
    #[serde(deserialize_with = "dep_or_vec_dep")]
    dependencies: Vec<Dep>, // vec of strings, or single string
    factorio_version: String,
}

fn dep_or_vec_dep<'de, D: Deserializer<'de>>(
    des: D,
) -> Result<Vec<Dep>, D::Error> {
    struct DepOrVecDep;
    impl<'de> Visitor<'de> for DepOrVecDep {
        type Value = Vec<Dep>;
        fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
            write!(f, "string or list of strings")
        }
        fn visit_str<E: de::Error>(self, v: &str) -> Result<Self::Value, E> {
            Ok(vec![v.parse().unwrap()])
        }
        fn visit_seq<A>(self, seq: A) -> Result<Self::Value, A::Error>
        where
            A: de::SeqAccess<'de>,
        {
            Ok(<Vec<String>>::deserialize(
                de::value::SeqAccessDeserializer::new(seq),
            )?
            .into_iter()
            .map(|x| x.parse().unwrap())
            .collect())
        }
    }
    des.deserialize_any(DepOrVecDep)
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct Dep {
    prefix: DepPrefix,
    name: String,
    version: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum DepPrefix {
    Incompatible,
    Optional,
    HiddenOptional,
    LoadOrderIndependent,
    Required,
}

impl FromStr for Dep {
    type Err = ();
    fn from_str(mut s: &str) -> Result<Self, Self::Err> {
        s = s.trim();
        let prefix = if let Some(rest) = s.strip_prefix("(?)") {
            s = rest.trim();
            DepPrefix::HiddenOptional
        } else if let Some(rest) = s.strip_prefix("!") {
            s = rest.trim();
            DepPrefix::Incompatible
        } else if let Some(rest) = s.strip_prefix("?") {
            s = rest.trim();
            DepPrefix::Optional
        } else if let Some(rest) = s.strip_prefix("~") {
            s = rest.trim();
            DepPrefix::LoadOrderIndependent
        } else {
            DepPrefix::Required
        };
        let idx = s.find(['<', '=', '>']).unwrap_or(s.len());
        let (name, version) = s.split_at(idx);
        Ok(Self {
            prefix,
            name: name.trim().to_string(),
            version: version.trim().to_string(),
        })
    }
}

#[derive(Clone)]
struct DepMod {
    deprecated: bool,
    mod_version: String,
    factorio_version: String,
    dependencies: Vec<Dep>,
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
        download_mods(mod_version_list.keys().map(|x| x.as_str()));
        return;
    }
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
                DepMod {
                    deprecated: mod_full.deprecated,
                    mod_version: release.version,
                    factorio_version: release.info_json.factorio_version,
                    dependencies: release.info_json.dependencies,
                },
            );
        }
    }
    write_deps(&mod_map).unwrap();
    let mut deprecated = BTreeSet::<String>::new();
    let mut rest = BTreeMap::<String, DepMod>::new();
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
        for (name, m) in core::mem::take(&mut rest) {
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
                if &*m.factorio_version >= "2.0" { continue; }
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
            if !mod_map.contains_key(&dep.name) && dep.name != "base" {
                eprint!("typod");
            }
            eprintln!()
        }
    }
    eprintln!("broken: {}", broken.len());
    let mut broken_file = File::create("broken.txt").unwrap();
    for (name, (m, _)) in &broken {
        writeln!(broken_file, "{name} for {}", m.factorio_version)
            .unwrap();
    }

    let mut broken_file = File::create("broken_with_reason.txt").unwrap();
    for (name, (m, broken_deps)) in &broken {
        writeln!(broken_file, "{name} for {} because of:", m.factorio_version)
            .unwrap();
        for broken_dep in broken_deps {
            writeln!(broken_file, "  {broken_dep}").unwrap();
        }
    }
    eprintln!("done");
}

fn write_deps(dep_map: &BTreeMap<String, DepMod>) -> io::Result<()> {
    let mut deps_file = File::create("deps.txt")?;
    for (name, dep) in dep_map {
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
