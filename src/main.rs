use log::{error, info, warn};
use notify_rust::Notification;
use std::{
    fs::OpenOptions,
    io::{self, Read, Write},
};
use tokio::signal;
use clap::Parser;

use dirs::cache_dir;
use reqwest::{self, Error};
use serde_json::Value;

/// Simple program to check GitHub releases
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// List of repositories to check, separated by commas
    #[arg(short, long)]
    repos: String,

    /// Cache file name
    #[arg(short, long, default_value = "github-release.txt")]
    cache_file: String,
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    env_logger::init();
    
    let args = Args::parse();
    
    let repos: Vec<&str> = args.repos.trim().split(',').collect();
    let cache_file = args.cache_file;
    println!("🐺 Got {:?}", repos);

    let ctrl_c = signal::ctrl_c();
    tokio::select! {
        _ = run_daemon(repos, &cache_file) => (),
        _ = ctrl_c => (),
    }

    Ok(())
}

async fn run_daemon(repos: Vec<&str>, cache_file: &str) -> Result<(), Error> {
    let client = reqwest::Client::new();
    let mut release_info = match read_cache_file(cache_file).await {
        Ok(json) => json.as_object().unwrap().clone(),
        Err(_) => serde_json::Map::new(),
    };

    loop {
        info!("Checking for new releases");
        let mut new_release_info = serde_json::Map::new();
        for repo in &repos {
            let url = format!("https://api.github.com/repos/{}/releases", repo);
            tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
            // add user agent to avoid 403
            let body = match client
                .get(&url)
                .header("User-Agent", "curl/8.11.0")
                .send()
                .await
            {
                Ok(resp) => match resp.text().await {
                    Ok(text) => text,
                    Err(e) => {
                        error!("Error getting response: {}", e);
                        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
                        continue;
                    }
                },
                Err(e) => {
                    error!("Error sending request: {}", e);
                    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
                    continue;
                }
            };

            match serde_json::from_str::<Value>(&body) {
                Ok(json) => match json.as_array() {
                    Some(releases) => {
                        let newest_release = releases.get(0).unwrap();
                        if let Some(tag_name) = newest_release["tag_name"].as_str() {
                            info!("{}: {}", repo, tag_name);
                            if let Some(old_release) = release_info.get(&repo.to_string()) {
                                if old_release != tag_name {
                                    new_release_info.insert(
                                        repo.to_string(),
                                        Value::String(tag_name.to_string()),
                                    );
                                }
                            }
                            release_info
                                .insert(repo.to_string(), Value::String(tag_name.to_string()));
                        }
                    }
                    None => {
                        warn!("No releases found for {}", repo);
                    }
                },
                Err(e) => {
                    error!("Error parsing JSON: {}", e);
                }
            }
        }

        if !new_release_info.is_empty() {
            release_notify(new_release_info).await?;
        }

        match write_cache_file(cache_file, &release_info).await {
            Err(e) => error!("Error writing cache file: {}", e),
            _ => (),
        }
        tokio::time::sleep(tokio::time::Duration::from_secs(3600)).await;
    }
}

async fn read_cache_file(filename: &str) -> Result<Value, io::Error> {
    let cache_dir = cache_dir().expect("No cache dir found");
    let cache_file_path = cache_dir.join(filename);
    if !cache_file_path.exists() {
        let mut cache_file = OpenOptions::new()
            .write(true)
            .create(true)
            .open(cache_file_path.clone())
            .unwrap();
        cache_file.write_all(b"{}").unwrap();
    }

    let mut cache_file = OpenOptions::new().read(true).open(cache_file_path).unwrap();

    let mut cache_content = String::new();
    match cache_file.read_to_string(&mut cache_content) {
        Ok(_) => (),
        Err(e) => return Err(e),
    }
    match serde_json::from_str(&cache_content) {
        Ok(json) => Ok(json),
        Err(e) => Err(io::Error::new(io::ErrorKind::Other, e)),
    }
}

async fn write_cache_file(
    filename: &str,
    json: &serde_json::Map<std::string::String, Value>,
) -> Result<(), io::Error> {
    let cache_dir = cache_dir().expect("No cache dir found");
    let cache_file_path = cache_dir.join(filename);
    let mut cache_file = OpenOptions::new()
        .write(true)
        .create(true)
        .open(cache_file_path)
        .unwrap();

    match cache_file.write_all(serde_json::to_string(&json).unwrap().as_bytes()) {
        Ok(_) => Ok(()),
        Err(e) => Err(e),
    }
}

async fn release_notify(json: serde_json::Map<std::string::String, Value>) -> Result<(), Error> {
    let mut content = String::from("");

    for (repo, release) in json {
        content.push_str(&format!("{}: {}\n", repo, release));
    }
    Notification::new()
        .summary("New release")
        .body(&content)
        .icon("librewolf")
        .show()
        .unwrap();
    Ok(())
}
