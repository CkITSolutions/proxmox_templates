use crate::installer::download_and_install_templates;
use crate::util::{
    ensure_cloud_init_snippet, get_storage_location, parse_args, prepare_templates,
    select_templates, show_branding,
};

mod downloader;
mod installer;
mod util;

#[tokio::main]
async fn main() -> Result<(), ()> {
    show_branding();

    let options = parse_args();

    let groups = match downloader::fetch_templates().await {
        Ok(groups) => groups,
        Err(err) => {
            eprintln!("Failed to load template list: {err}");
            return Err(());
        }
    };

    let selected = select_templates(&groups, &options);
    if selected.is_empty() {
        println!("No templates selected. Exiting.");
        return Ok(());
    }

    let templates = prepare_templates(selected, &options);
    if templates.is_empty() {
        println!("No templates to install after resolving existing VMIDs.");
        return Ok(());
    }

    let storage_volume = get_storage_location(&options);
    let vendor_snippet = match ensure_cloud_init_snippet() {
        Ok(snippet) => snippet,
        Err(err) => {
            eprintln!("Failed to prepare cloud-init snippet: {err}");
            return Err(());
        }
    };

    println!(
        "Installing {} template(s) to storage '{}'",
        templates.len(),
        storage_volume
    );

    let tmp_dir = tempfile::tempdir().unwrap();
    let tmp_path = tmp_dir.path();

    tokio::select! {
        result = download_and_install_templates(&tmp_path, &storage_volume, templates, &vendor_snippet) => {
            if let Err(err) = result {
                eprintln!("Installation failed: {err}");
                return Err(());
            }
        },
        _ = tokio::signal::ctrl_c() => {
            println!("Received Ctrl-C, exiting");
        }
    }

    tmp_dir.close().unwrap();
    println!("All selected templates were installed successfully.");

    Ok(())
}
