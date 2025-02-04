use core::{fmt, str::FromStr};

use serde::{
    de::{self, Visitor},
    Deserialize, Deserializer,
};

#[derive(Deserialize)]
pub struct ModList {
    pub results: Vec<Mod>,
}

pub type LatestRelease = Release<ShortInfoJson>;

#[derive(Debug, Deserialize)]
pub struct Mod {
    pub name: String,
    pub latest_release: Option<LatestRelease>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ModFull {
    pub category: String,
    pub changelog: Option<String>,
    pub created_at: String,
    pub downloads_count: u64,
    #[serde(default)]
    pub deprecated: bool,
    pub description: Option<String>,
    pub homepage: String,
    pub images: Vec<Image>,
    pub license: Option<License>,
    pub name: String,
    pub owner: String,
    pub releases: Vec<Release<FullInfoJson>>,
    #[serde(default)]
    pub score: f32,
    pub source_url: Option<String>,
    pub summary: String,
    pub tags: Option<Vec<String>>,
    pub thumbnail: Option<String>,
    pub title: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Image {
    pub id: String,
    pub thumbnail: String,
    pub url: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct License {
    pub description: String,
    pub id: String,
    pub name: String,
    pub title: String,
    pub url: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Release<INFO> {
    pub download_url: String,
    pub file_name: String,
    pub info_json: INFO,
    pub released_at: String,
    pub sha1: String,
    pub version: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ShortInfoJson {
    pub factorio_version: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FullInfoJson {
    #[serde(deserialize_with = "dep_or_vec_dep")]
    pub dependencies: Vec<Dep>, // vec of strings, or single string
    pub factorio_version: String,
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
pub struct Dep {
    pub original: String,
    pub prefix: DepPrefix,
    pub name: String,
    pub version: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DepPrefix {
    Incompatible,
    Optional,
    HiddenOptional,
    LoadOrderIndependent,
    Required,
}

impl FromStr for Dep {
    type Err = ();
    fn from_str(original: &str) -> Result<Self, Self::Err> {
        let mut s = original;
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
            original: original.to_string(),
            prefix,
            name: name.trim().to_string(),
            version: version.trim().to_string(),
        })
    }
}
