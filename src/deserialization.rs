use std::{fmt, str::FromStr};

use serde::{
    de::{self, Visitor},
    Deserialize, Deserializer,
};

#[derive(Deserialize)]
pub struct ModList {
    pub results: Vec<Mod>,
}

#[derive(Debug, Deserialize)]
pub struct Mod {
    pub name: String,
    pub latest_release: Option<LatestRelease>,
}

#[derive(Debug, Deserialize)]
pub struct LatestRelease {
    pub version: String,
}

#[derive(Debug, Deserialize)]
pub struct ModFull {
    pub name: String,
    #[serde(default)]
    pub deprecated: bool,
    pub releases: Vec<Release>,
}

#[derive(Debug, Deserialize)]
pub struct Release {
    pub version: String,
    pub info_json: InfoJson,
}

#[derive(Debug, Deserialize)]
pub struct InfoJson {
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
