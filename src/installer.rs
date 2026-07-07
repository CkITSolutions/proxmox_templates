use std::path::Path;
use std::process::Stdio;

use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use reqwest::Client;
use tokio::process::Command;

use crate::downloader::{download_template, Template};
use crate::util::{template_vm_name};

const DEFAULT_MEMORY_MB: u32 = 2048;
const DEFAULT_DISK_RESIZE: &str = "8G";
const DEFAULT_BRIDGE: &str = "vmbr0";

pub async fn install_template(
    storage_volume: &str,
    template: &Template,
    image: &Path,
    vendor_snippet: &str,
    pb: &ProgressBar,
) -> anyhow::Result<()> {
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{prefix:.bold.dim} {bar} {percent}% [{elapsed_precise}] {msg}")
            .unwrap(),
    );
    pb.set_message(format!("Installing {}", template.name));
    pb.set_length(100);
    pb.set_position(10);

    let vm_name = template_vm_name(template);
    let vmid = template.vmid.to_string();

    pb.set_position(20);

    run_qm(
        &[
            "create",
            &vmid,
            "--name",
            &vm_name,
            "--memory",
            &DEFAULT_MEMORY_MB.to_string(),
            "--net0",
            &format!("virtio,bridge={DEFAULT_BRIDGE}"),
            "--agent",
            "enabled=1",
            "--ostype",
            "l26",
            "--bios",
            "seabios",
            "--machine",
            "q35",
            "--scsihw",
            "virtio-scsi-pci",
            "--serial0",
            "socket",
            "--vga",
            "serial0",
        ],
        template,
    )
    .await?;

    pb.set_position(40);

    run_qm(
        &[
            "disk",
            "import",
            &vmid,
            image.to_str().unwrap(),
            storage_volume,
        ],
        template,
    )
    .await?;

    pb.set_position(60);

    let disk_ref = format!("{storage_volume}:vm-{vmid}-disk-0");
    run_qm(
        &[
            "set",
            &vmid,
            "--scsi0",
            &format!("{disk_ref},cache=writeback,discard=on,ssd=1"),
            "--boot",
            "order=scsi0",
            "--ide2",
            &format!("{storage_volume}:cloudinit"),
            "--ipconfig0",
            "ip=dhcp",
            "--citype",
            "nocloud",
            "--cicustom",
            &format!("vendor={vendor_snippet}"),
        ],
        template,
    )
    .await?;

    pb.set_position(80);

    run_qm(
        &["resize", &vmid, "scsi0", &format!("+{DEFAULT_DISK_RESIZE}")],
        template,
    )
    .await?;

    run_qm(&["template", &vmid], template).await?;

    pb.set_position(100);
    pb.finish_with_message(format!("Installed {}", template.name));

    Ok(())
}

pub async fn download_and_install_templates(
    tmp_dir: &Path,
    storage_volume: &str,
    templates: Vec<Template>,
    vendor_snippet: &str,
) -> anyhow::Result<()> {
    if templates.is_empty() {
        println!("No templates selected.");
        return Ok(());
    }

    let client = Client::new();
    let mpb = MultiProgress::new();

    for template in templates {
        let pb = mpb.add(ProgressBar::new(100));
        pb.set_style(ProgressStyle::default_bar().template("{msg}").unwrap());
        pb.set_message(format!("Queued {}", template.name));

        let file_name = download_template(tmp_dir, &client, &template, &pb).await?;
        install_template(
            storage_volume,
            &template,
            &file_name,
            vendor_snippet,
            &pb,
        )
        .await?;
    }

    Ok(())
}

async fn run_qm(args: &[&str], template: &Template) -> anyhow::Result<()> {
    let output = Command::new("qm")
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .await?;

    if output.status.success() {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    Err(anyhow::anyhow!(
        "qm {} failed for {}: {}{}",
        args.first().unwrap_or(&""),
        template.name,
        stderr,
        stdout
    ))
}
