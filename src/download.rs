use core::{
    sync::atomic::{AtomicUsize, Ordering},
    time::Duration,
};
use std::{collections::BTreeMap, fs::File, path::Path, thread};

use futures::{StreamExt, stream};
use reqwest::{
    Client, blocking as req_blocking,
    header::{self, HeaderMap, HeaderValue},
};
use tokio::{
    fs::{self as tokio_fs},
    runtime::Runtime,
};

use crate::{APP_ID, Error, USER_AGENT, deserialization::LatestRelease};

#[derive(serde::Deserialize)]
pub struct PlayerCreds {
    #[serde(rename = "service-username")]
    username: String,
    #[serde(rename = "service-token")]
    token: String,
}

pub async fn toggle_bookmark(
    req: &Client,
    creds: &PlayerCreds,
    mod_name: &str,
    on: bool,
) -> reqwest::Result<()> {
    let state = match on {
        true => "on",
        false => "off",
    };
    let url = format!(
        "https://mods.factorio.com/api/bookmarks\
        /toggle?username={}&token={}&mod={}&state={}",
        creds.username, creds.token, mod_name, state
    );
    req.post(url).send().await?.error_for_status()?;
    Ok(())
}

pub async fn download_bookmark_list(
    req: &Client,
    creds: &PlayerCreds,
) -> reqwest::Result<String> {
    let url = format!(
        "https://mods.factorio.com/api/bookmarks?username={}&token={}",
        creds.username, creds.token
    );
    req.get(url).send().await?.error_for_status()?.text().await
}

pub fn download_mod_list() -> reqwest::Result<String> {
    let url = "https://mods.factorio.com/api/mods?page_size=max";
    req_blocking::get(url)?.text()
}

async fn download_mod_meta_full(req: &Client, name: &str) -> Result<(), Error> {
    let url = format!("https://mods.factorio.com/api/mods/{name}/full");
    let resp = req
        .execute(req.get(url).build()?)
        .await?
        .error_for_status()?
        .text()
        .await?;
    tokio_fs::write(
        eframe::storage_dir(APP_ID)
            .unwrap()
            .join("mods")
            .join(format!("{name}.json")),
        resp,
    )
    .await?;
    Ok(())
}

pub fn download_mods_meta_full<'a>(
    mod_list: impl Iterator<Item = &'a str> + Clone,
) {
    static COUNTER: AtomicUsize = AtomicUsize::new(0);

    let mod_count = mod_list.clone().count();
    let rt = Runtime::new().unwrap();

    let mut headers = HeaderMap::new();
    headers.insert(header::USER_AGENT, HeaderValue::from_static(USER_AGENT));
    let req = Client::builder().default_headers(headers).build().unwrap();

    // todo: zip

    let mut futures = vec![];
    for name in mod_list {
        let req = &req;
        futures.push(async move {
            download_mod_meta_full(req, name).await.unwrap();
            COUNTER.fetch_add(1, Ordering::Relaxed);
        });
    }

    let _ui = thread::spawn(move || {
        loop {
            let v = COUNTER.load(Ordering::Relaxed);
            eprint!("{:>12} {v}/{mod_count} mods metadata\r", "Downloading");
            if v >= mod_count {
                eprintln!();
                break;
            }
            thread::sleep(Duration::from_secs(1));
        }
    });
    rt.block_on(stream::iter(futures).for_each_concurrent(64, |x| x));
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

pub fn download_mods<'a>(
    factorio_instance: &Path,
    mod_list: impl Iterator<Item = &'a str> + Clone,
    mod_version_list: &BTreeMap<&'a str, Option<&'a LatestRelease>>,
) -> Result<(), Error> {
    static COUNTER: AtomicUsize = AtomicUsize::new(0);

    let player_creds: PlayerCreds = simd_json::from_reader(File::open(
        factorio_instance.join("player-data.json"),
    )?)?;

    let mod_count = mod_list.clone().count();

    let mut headers = HeaderMap::new();
    headers.insert(header::USER_AGENT, HeaderValue::from_static(USER_AGENT));
    let req = Client::builder().default_headers(headers).build()?;
    let rt = Runtime::new()?;

    let mut futures = vec![];
    for name in mod_list {
        let player_creds = &player_creds;
        let req = &req;
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
            COUNTER.fetch_add(1, Ordering::Relaxed);
        });
    }

    let _ui = thread::spawn(move || {
        loop {
            let v = COUNTER.load(Ordering::Relaxed);
            eprint!("{:>12} {v}/{mod_count} mods\r", "Downloading");
            if v >= mod_count {
                eprintln!();
                break;
            }
            thread::sleep(Duration::from_secs(1));
        }
    });
    rt.block_on(stream::iter(futures).for_each_concurrent(64, |x| x));
    Ok(())
}
