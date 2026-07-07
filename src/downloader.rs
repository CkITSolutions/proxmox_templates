use std::env;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context};
use futures::StreamExt;
use indicatif::{ProgressBar, ProgressStyle};
use regex::Regex;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use tokio::time::sleep;

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Template {
    pub name: String,
    pub vmid: i32,
    pub link: String,
    #[serde(default)]
    pub artifact: Option<String>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Group {
    pub name: String,
    pub templates: Vec<Template>,
}

pub async fn fetch_templates() -> anyhow::Result<Vec<Group>> {
    let args: Vec<String> = env::args().skip(1).collect();
    let custom_source = args.iter().find(|arg| {
        !arg.starts_with('-')
            && (arg.starts_with("http://")
                || arg.starts_with("https://")
                || arg.ends_with(".json"))
    });

    let json = if let Some(source) = custom_source {
        if source.starts_with("http://") || source.starts_with("https://") {
            reqwest::get(source)
                .await?
                .error_for_status()?
                .text()
                .await?
        } else {
            tokio::fs::read_to_string(source).await?
        }
    } else if Path::new("images.json").exists() {
        tokio::fs::read_to_string("images.json").await?
    } else if let Ok(exe) = env::current_exe() {
        let bundled = exe.parent().unwrap_or(Path::new(".")).join("images.json");
        if bundled.exists() {
            tokio::fs::read_to_string(bundled).await?
        } else {
            include_str!("../images.json").to_string()
        }
    } else {
        include_str!("../images.json").to_string()
    };

    let groups = serde_json::from_str::<Vec<Group>>(&json)?;
    Ok(groups)
}

pub async fn resolve_download_url(client: &Client, template: &Template) -> anyhow::Result<String> {
    let Some(artifact) = &template.artifact else {
        return Ok(template.link.clone());
    };

    let directory = template.link.trim_end_matches('/');
    let listing = client
        .get(format!("{directory}/"))
        .send()
        .await?
        .error_for_status()?
        .text()
        .await
        .with_context(|| format!("Failed to read image index for {}", template.name))?;

    let pattern = format!(
        r#"href="({}[^"]+\.qcow2)""#,
        regex::escape(artifact)
    );
    let matcher = Regex::new(&pattern)?;

    let mut matches = matcher
        .captures_iter(&listing)
        .filter_map(|capture| capture.get(1).map(|value| value.as_str().to_string()))
        .collect::<Vec<_>>();

    matches.sort();
    matches.reverse();

    let filename = matches
        .first()
        .cloned()
        .ok_or_else(|| anyhow!("No matching cloud image found for {}", template.name))?;

    Ok(format!("{directory}/{filename}"))
}

pub async fn download_template(
    tmp_dir: &Path,
    client: &Client,
    template: &Template,
    pb: &ProgressBar,
) -> anyhow::Result<PathBuf> {
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{prefix:.bold.dim} {bar} {percent}% [{elapsed_precise}] {bytes}/{total_bytes} {msg}")
            .unwrap(),
    );
    pb.set_message(format!("Resolving latest image for {}", template.name));

    let download_url = resolve_download_url(client, template).await?;
    pb.set_message(format!("Downloading {}", template.name));

    let retry_count = 3;
    let mut attempt = 0;

    let response = loop {
        let response = client.get(&download_url).send().await?;
        if response.status().is_success() || attempt >= retry_count {
            break response;
        }
        attempt += 1;
        sleep(std::time::Duration::from_secs(attempt)).await;
    };

    if !response.status().is_success() {
        return Err(anyhow!(
            "Failed to download {} after {} attempts (HTTP {})",
            template.name,
            attempt,
            response.status()
        ));
    }

    let filename = response
        .url()
        .path_segments()
        .and_then(|segments| segments.last())
        .map(str::to_string)
        .unwrap_or_else(|| format!("template-{}.img", template.vmid));

    pb.set_length(response.content_length().unwrap_or(0));

    let temp_file_path = tmp_dir.join(filename);
    let mut file = File::create(&temp_file_path).await?;

    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let data = chunk?;
        file.write_all(&data).await?;
        pb.inc(data.len() as u64);
    }

    pb.finish_with_message(format!("Downloaded {}", template.name));

    Ok(temp_file_path)
}
