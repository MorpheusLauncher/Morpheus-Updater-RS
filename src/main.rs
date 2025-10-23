#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

use eframe::egui;
use futures_util::StreamExt;
use reqwest::Client;
use std::fs::File;
use std::io::Read;
use std::os::windows::process::CommandExt;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tokio::runtime::Runtime;
use zip::ZipArchive;

const BETA: bool = true;
const FILES: &[&str] = &["morpheus_win.zip", "Launcher.jar", "authlib-injector.jar"];
const HOST: &str = "https://morpheuslauncher.it/downloads/";
const VERSIONS_URL: &str = "https://morpheuslauncher.it/version.txt";

fn build_url(file: &str) -> String {
    if BETA {
        format!("{}beta/{}", HOST, file)
    } else {
        format!("{}{}", HOST, file)
    }
}

fn get_morpheus_dir() -> PathBuf {
    if cfg!(windows) {
        std::env::var("APPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|_| dirs::home_dir().unwrap())
            .join(".morpheus")
    } else {
        dirs::home_dir().unwrap().join(".morpheus")
    }
}

struct InstallerApp {
    logs: Arc<Mutex<String>>,
    started: bool,
}

impl eframe::App for InstallerApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Avvio automatico download
        if !self.started {
            self.started = true;
            let logs = self.logs.clone();
            std::thread::spawn(move || {
                let rt = Runtime::new().unwrap();
                rt.block_on(async move {
                    if let Err(e) = check_and_run(logs).await {
                        eprintln!("Errore: {}", e);
                    }
                });
            });
        }

        // GUI log con dimensioni adattive
        egui::CentralPanel::default().show(ctx, |ui| {
            let logs_text = self.logs.lock().unwrap().clone();
            let available_height = ui.available_height();

            egui::ScrollArea::vertical()
                .stick_to_bottom(true)
                .show(ui, |ui| {
                    ui.add_sized(
                        [ui.available_width(), available_height],
                        egui::TextEdit::multiline(&mut logs_text.as_str())
                            .font(egui::TextStyle::Monospace)
                            .interactive(false),
                    );
                });
        });

        ctx.request_repaint(); // aggiorna log in tempo reale
    }
}

async fn check_and_run(
    logs: Arc<Mutex<String>>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let target_dir = get_morpheus_dir();
    let local_versions = target_dir.join("version.txt");
    let launcher_path = target_dir.join("morpheus_launcher_gui.exe");

    // Check if launcher and local version.txt exist
    let has_local_installation = launcher_path.exists() && local_versions.exists();

    if has_local_installation {
        log(&logs, "Local installation found".to_string());

        // Read local version
        let local_version = std::fs::read_to_string(&local_versions).ok();

        // Try to verify online version
        log(&logs, "Checking for updates...".to_string());
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .build()?;

        match client.get(VERSIONS_URL).send().await {
            Ok(resp) if resp.status().is_success() => {
                let remote_version = resp.text().await?;

                if let Some(local) = local_version {
                    if local.trim() == remote_version.trim() {
                        log(&logs, "Version is up to date!".to_string());
                        launch_and_exit(logs, &launcher_path, &target_dir).await?;
                        return Ok(());
                    } else {
                        log(&logs, "New version available, updating...".to_string());
                    }
                } else {
                    log(&logs, "Update required...".to_string());
                }
            }
            Ok(_) => {
                // Non-successful response (404, 500, etc.)
                log(
                    &logs,
                    "Server error, launching local version...".to_string(),
                );
                launch_and_exit(logs, &launcher_path, &target_dir).await?;
                return Ok(());
            }
            Err(_) => {
                // Offline or network error
                log(&logs, "Cannot verify updates (offline?)".to_string());
                log(&logs, "Launching local version...".to_string());
                launch_and_exit(logs, &launcher_path, &target_dir).await?;
                return Ok(());
            }
        }
    } else {
        log(&logs, "First installation".to_string());
    }

    // Download and installation
    download_all(logs.clone()).await?;

    // Download and save version.txt
    log(&logs, "Downloading version.txt...".to_string());
    let client = Client::new();
    match client.get(VERSIONS_URL).send().await {
        Ok(resp) if resp.status().is_success() => {
            let version = resp.text().await?;
            tokio::fs::write(&local_versions, &version).await?;
            log(&logs, "version.txt updated".to_string());
        }
        Ok(resp) => {
            log(
                &logs,
                format!(
                    "Warning: server error {} while saving version.txt",
                    resp.status()
                ),
            );
        }
        Err(e) => {
            log(&logs, format!("Warning: cannot save version.txt: {}", e));
        }
    }

    // Launch launcher
    if launcher_path.exists() {
        launch_and_exit(logs, &launcher_path, &target_dir).await?;
    } else {
        log(&logs, "morpheus_launcher_gui.exe not found!".to_string());
    }

    Ok(())
}

async fn launch_and_exit(
    logs: Arc<Mutex<String>>,
    launcher_path: &PathBuf,
    target_dir: &PathBuf,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    log(&logs, "Starting Morpheus Launcher...".to_string());

    #[cfg(windows)]
    {
        // On Windows use cmd /c start to launch completely independent
        std::process::Command::new("cmd")
            .args(&["/C", "start", "", launcher_path.to_str().unwrap()])
            .current_dir(target_dir)
            .creation_flags(0x08000000) // CREATE_NO_WINDOW
            .spawn()?;
    }

    log(&logs, "Launcher started! Closing installer...".to_string());
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
    std::process::exit(0);
}

async fn download_all(
    logs: Arc<Mutex<String>>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let target_dir = get_morpheus_dir();
    std::fs::create_dir_all(&target_dir)?;

    let client = Client::new();
    let mut handles = Vec::new();

    for &file in FILES {
        let url = build_url(file);
        let path = target_dir.join(file);
        let logs = logs.clone();
        let client = client.clone();

        handles.push(tokio::spawn(async move {
            log(&logs, format!("Starting download: {}", file));
            if let Err(e) = download_file(&client, &url, &path).await {
                log(&logs, format!("Error: {} -> {}", file, e));
            } else {
                log(&logs, format!("Completed: {}", file));

                // If it's a zip, extract it
                if file.ends_with(".zip") {
                    log(&logs, format!("Extracting: {}", file));
                    if let Err(e) = extract_zip(&path) {
                        log(&logs, format!("Extraction error: {} -> {}", file, e));
                    } else {
                        log(&logs, format!("Extracted: {}", file));

                        // Delete zip after extraction
                        if let Err(e) = std::fs::remove_file(&path) {
                            log(&logs, format!("Error removing zip: {}", e));
                        } else {
                            log(&logs, format!("Removed: {}", file));
                        }
                    }
                }
            }
        }));
    }

    for h in handles {
        h.await?;
    }

    log(
        &logs,
        "All downloads and extractions completed!".to_string(),
    );
    Ok(())
}

async fn download_file(
    client: &Client,
    url: &str,
    path: &std::path::Path,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let resp = client.get(url).send().await?;
    if !resp.status().is_success() {
        return Err(format!("HTTP error: {}", resp.status()).into());
    }

    let tmp = path.with_extension("part");
    let mut file = tokio::fs::File::create(&tmp).await?;
    let mut stream = resp.bytes_stream();

    while let Some(chunk) = stream.next().await {
        let data = chunk?;
        tokio::io::AsyncWriteExt::write_all(&mut file, &data).await?;
    }

    tokio::fs::rename(tmp, path).await?;
    Ok(())
}

fn extract_zip(zip_path: &std::path::Path) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let file = File::open(zip_path)?;
    let mut archive = ZipArchive::new(file)?;

    let extract_dir = zip_path.parent().unwrap();

    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;
        let outpath = match file.enclosed_name() {
            Some(path) => extract_dir.join(path),
            None => continue,
        };

        if file.name().ends_with('/') {
            std::fs::create_dir_all(&outpath)?;
        } else {
            if let Some(p) = outpath.parent() {
                if !p.exists() {
                    std::fs::create_dir_all(p)?;
                }
            }
            let mut outfile = File::create(&outpath)?;
            std::io::copy(&mut file, &mut outfile)?;
        }
    }

    Ok(())
}

fn log(logs: &Arc<Mutex<String>>, msg: String) {
    let mut l = logs.lock().unwrap();
    l.push_str(&format!("{}\n", msg));
}

fn load_icon() -> egui::IconData {
    let icon_bytes = include_bytes!("../morpheus.ico");

    let image = image::load_from_memory(icon_bytes).expect(
        "Impossibile caricare l'icona. Assicurati che 'morpheus.ico' sia nella root del progetto.",
    );

    let rgba_image = image.to_rgba8();
    let (width, height) = rgba_image.dimensions();
    let rgba_data = rgba_image.into_raw();

    egui::IconData {
        rgba: rgba_data,
        width,
        height,
    }
}

fn main() {
    let app_icon = load_icon();
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([450.0, 300.0])
            .with_icon(app_icon),
        ..Default::default()
    };

    let title = format!(
        "Morpheus Installer [{}]",
        if BETA { "BETA" } else { "STABLE" }
    );

    eframe::run_native(
        title.as_str(),
        native_options,
        Box::new(|_cc| {
            Ok(Box::new(InstallerApp {
                logs: Arc::new(Mutex::new(String::new())),
                started: false,
            }) as Box<dyn eframe::App>)
        }),
    )
    .expect("An error occourred!");
}
