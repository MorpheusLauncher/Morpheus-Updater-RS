#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

use eframe::egui;
use futures_util::StreamExt;
use reqwest::Client;
use std::fs::File;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tokio::runtime::Runtime;
use zip::ZipArchive;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
#[cfg(windows)]
use std::os::windows::process::CommandExt;

const BETA: bool = false;
const HOST: &str = "https://morpheuslauncher.it/downloads/";
const VERSIONS_URL: &str = "https://morpheuslauncher.it/version.txt";

#[cfg(windows)]
const ZIP_NAME: &str = "morpheus_win.zip";
#[cfg(unix)]
const ZIP_NAME: &str = "morpheus_tux.zip";

#[cfg(windows)]
const LAUNCHER_EXE: &str = "morpheus_launcher_gui.exe";
#[cfg(unix)]
const LAUNCHER_EXE: &str = "morpheus_launcher_gui";

#[cfg(windows)]
const UPDATER_EXE: &str = "morpheus_updater.exe";
#[cfg(unix)]
const UPDATER_EXE: &str = "morpheus_updater";

const FILES: &[&str] = &[ZIP_NAME, "Launcher.jar", "authlib-injector.jar"];

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
    let launcher_path = target_dir.join(LAUNCHER_EXE);

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
        #[cfg(unix)]
        {
            check_and_install_dependencies(&logs).await?;
            // Ensure executable permission
            if let Ok(metadata) = std::fs::metadata(&launcher_path) {
                let mut perms = metadata.permissions();
                perms.set_mode(0o755);
                let _ = std::fs::set_permissions(&launcher_path, perms);
            }
        }
        launch_and_exit(logs, &launcher_path, &target_dir).await?;
    } else {
        log(&logs, format!("{} not found!", LAUNCHER_EXE));
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

    #[cfg(unix)]
    {
        // On Linux just spawn the process
        std::process::Command::new(launcher_path)
            .current_dir(target_dir)
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

            #[cfg(unix)]
            {
                if let Some(mode) = file.unix_mode() {
                    let mut perms = std::fs::metadata(&outpath)?.permissions();
                    perms.set_mode(mode);
                    std::fs::set_permissions(&outpath, perms)?;
                }
            }
        }
    }

    Ok(())
}

fn log(logs: &Arc<Mutex<String>>, msg: String) {
    let mut l = logs.lock().unwrap();
    l.push_str(&format!("{}\n", msg));
}

#[cfg(unix)]
async fn check_and_install_dependencies(
    logs: &Arc<Mutex<String>>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    log(logs, "Checking system dependencies...".to_string());

    let distro = get_linux_distro();
    log(logs, format!("Detected distro: {}", distro));

    let mut missing = Vec::new();

    // Map library names to package names based on distro
    let (secret_pkg, json_pkg) = match distro.as_str() {
        "fedora" | "centos" | "rhel" => ("libsecret", "jsoncpp"),
        "arch" | "manjaro" => ("libsecret", "jsoncpp"),
        _ => ("libsecret-1-0", "libjsoncpp-dev"), // Ubuntu/Debian default
    };

    if !is_library_present("libsecret-1") {
        missing.push(secret_pkg);
    }

    if !is_library_present("libjsoncpp") {
        missing.push(json_pkg);
    }

    if !missing.is_empty() {
        log(
            logs,
            format!(
                "Missing dependencies: {:?}. Attempting installation...",
                missing
            ),
        );
        install_packages(logs, &missing).await?;
    }

    // Handle libjsoncpp symlink if necessary
    handle_jsoncpp_symlink(logs)?;

    Ok(())
}

#[cfg(unix)]
fn is_library_present(name: &str) -> bool {
    let mut output = std::process::Command::new("ldconfig").arg("-p").output();
    if output.is_err() {
        output = std::process::Command::new("/sbin/ldconfig")
            .arg("-p")
            .output();
    }

    if let Ok(out) = output {
        let stdout = String::from_utf8_lossy(&out.stdout);
        stdout.contains(name)
    } else {
        // Fallback: search in common library directories if ldconfig fails
        let common_dirs = ["/usr/lib", "/usr/lib64", "/lib", "/lib64"];
        for dir in common_dirs {
            if let Ok(entries) = std::fs::read_dir(dir) {
                for entry in entries.flatten() {
                    if let Some(file_name) = entry.file_name().to_str() {
                        if file_name.contains(name) {
                            return true;
                        }
                    }
                }
            }
        }
        false
    }
}

#[cfg(unix)]
async fn install_packages(
    logs: &Arc<Mutex<String>>,
    packages: &[&str],
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let distro = get_linux_distro();

    let (cmd, args) = match distro.as_str() {
        "ubuntu" | "debian" | "linuxmint" | "pop" => ("apt-get", vec!["install", "-y"]),
        "fedora" | "centos" | "rhel" => ("dnf", vec!["install", "-y"]),
        "arch" | "manjaro" => ("pacman", vec!["-S", "--noconfirm"]),
        _ => {
            log(logs, "Unsupported distribution for automatic installation. Please install dependencies manually.".to_string());
            return Ok(());
        }
    };

    let mut full_args = vec!["pkexec", cmd];
    full_args.extend(args);
    full_args.extend(packages);

    log(logs, format!("Running: {:?}", full_args));

    let status = std::process::Command::new(full_args[0])
        .args(&full_args[1..])
        .status()?;

    if status.success() {
        log(logs, "Dependencies installed successfully.".to_string());
    } else {
        log(
            logs,
            "Failed to install dependencies. You might need to install them manually.".to_string(),
        );
    }

    Ok(())
}

#[cfg(unix)]
fn get_linux_distro() -> String {
    if let Ok(os_release) = std::fs::read_to_string("/etc/os-release") {
        for line in os_release.lines() {
            if line.starts_with("ID=") {
                return line.trim_start_matches("ID=").trim_matches('"').to_string();
            }
        }
    }
    "unknown".to_string()
}

#[cfg(unix)]
fn handle_jsoncpp_symlink(
    logs: &Arc<Mutex<String>>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    log(
        logs,
        "Searching for libjsoncpp for compatibility symlinks...".to_string(),
    );

    let mut output = std::process::Command::new("ldconfig").arg("-p").output();
    if output.is_err() {
        output = std::process::Command::new("/sbin/ldconfig")
            .arg("-p")
            .output();
    }

    let mut found_libs = Vec::new();

    if let Ok(out) = output {
        let stdout = String::from_utf8_lossy(&out.stdout);
        for line in stdout.lines() {
            if line.contains("libjsoncpp.so.") {
                log(logs, format!("ldconfig match: {}", line.trim()));
                if let Some(pos) = line.rfind("=> ") {
                    let full_path_str = line[pos + 3..].trim();
                    let path = std::path::PathBuf::from(full_path_str);
                    if path.exists() {
                        if let Some(file_name) = path.file_name().and_then(|n| n.to_str()) {
                            if let Some(version) = file_name.strip_prefix("libjsoncpp.so.") {
                                if let Some(parent) = path.parent() {
                                    found_libs.push((parent.to_path_buf(), version.to_string()));
                                }
                            }
                        }
                    }
                }
            }
        }
    } else {
        log(
            logs,
            "Could not run ldconfig to find libraries.".to_string(),
        );
    }

    if found_libs.is_empty() {
        log(
            logs,
            "No libjsoncpp version found via ldconfig.".to_string(),
        );
        return Ok(());
    }

    // Ordina per versione (priorità alla più recente)
    found_libs.sort_by(|a, b| b.1.cmp(&a.1));

    let (dir, v) = &found_libs[0];
    log(
        logs,
        format!(
            "Using libjsoncpp.so.{} from {} for symlinks",
            v,
            dir.display()
        ),
    );

    // Versioni di compatibilità che vogliamo assicurarci esistano
    let targets = ["24", "25", "26"];

    for target_v in targets {
        // Se la versione trovata inizia con target_v (es. v="25" e target="25"), saltiamo
        if v.starts_with(target_v)
            && (v.len() == target_v.len() || v.as_bytes()[target_v.len()] == b'.')
        {
            continue;
        }

        let target_path = dir.join(format!("libjsoncpp.so.{}", target_v));
        if !target_path.exists() {
            log(
                logs,
                format!(
                    "Creating compatibility symlink libjsoncpp.so.{} -> libjsoncpp.so.{}",
                    target_v, v
                ),
            );

            let status = std::process::Command::new("pkexec")
                .args(&[
                    "ln",
                    "-s",
                    &format!("libjsoncpp.so.{}", v),
                    target_path.to_str().unwrap(),
                ])
                .status();

            match status {
                Ok(s) if s.success() => log(
                    logs,
                    format!("Successfully created symlink libjsoncpp.so.{}", target_v),
                ),
                Ok(s) => log(
                    logs,
                    format!(
                        "Failed to create symlink libjsoncpp.so.{} (exit code: {:?})",
                        target_v,
                        s.code()
                    ),
                ),
                Err(e) => log(logs, format!("Error running pkexec for symlink: {}", e)),
            }
        } else {
            log(
                logs,
                format!("Symlink or file libjsoncpp.so.{} already exists.", target_v),
            );
        }
    }

    Ok(())
}

fn ensure_self_relocation() -> Result<(), Box<dyn std::error::Error>> {
    let current_exe = std::env::current_exe()?;
    let target_dir = get_morpheus_dir();

    // Create target dir if it doesn't exist
    if !target_dir.exists() {
        std::fs::create_dir_all(&target_dir)?;
    }

    let target_exe = target_dir.join(UPDATER_EXE);

    // Check if we are already in the target dir and have the right name
    if current_exe == target_exe {
        return Ok(());
    }

    // Copy itself to target location
    std::fs::copy(&current_exe, &target_exe)?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&target_exe)?.permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&target_exe, perms)?;
    }

    // Launch the new copy
    std::process::Command::new(target_exe).spawn()?;

    // Create shortcut
    if let Err(e) = create_shortcut() {
        eprintln!("Shortcut creation error: {}", e);
    }

    // Exit current process
    std::process::exit(0);
}

fn create_shortcut() -> Result<(), Box<dyn std::error::Error>> {
    let target_dir = get_morpheus_dir();
    let target_exe = target_dir.join(UPDATER_EXE);
    let icon_path = target_dir.join("morpheus.ico");

    // Ensure icon exists in .morpheus
    if !icon_path.exists() {
        let icon_bytes = include_bytes!("../morpheus.ico");
        std::fs::write(&icon_path, icon_bytes)?;
    }

    #[cfg(windows)]
    {
        let desktop = dirs::desktop_dir().ok_or("Could not find desktop directory")?;
        let lnk_path = desktop.join("Morpheus Launcher.lnk");

        let script = format!(
            "$s=(New-Object -COM WScript.Shell).CreateShortcut('{}');$s.TargetPath='{}';$s.IconLocation='{}';$s.Save()",
            lnk_path.to_str().unwrap(),
            target_exe.to_str().unwrap(),
            icon_path.to_str().unwrap()
        );

        let _ = std::process::Command::new("powershell")
            .args(&["-Command", &script])
            .creation_flags(0x08000000) // CREATE_NO_WINDOW
            .status();
    }

    #[cfg(unix)]
    {
        let home = dirs::home_dir().ok_or("Could not find home directory")?;
        let apps_dir = home.join(".local/share/applications");
        let _ = std::fs::create_dir_all(&apps_dir);

        let desktop_file = apps_dir.join("morpheus-launcher.desktop");
        let content = format!(
            r#"[Desktop Entry]
Type=Application
Name=Morpheus Launcher
Exec={}
Icon={}
Terminal=false
Categories=Game;
"#,
            target_exe.to_str().unwrap(),
            icon_path.to_str().unwrap()
        );

        std::fs::write(desktop_file, content)?;
    }

    Ok(())
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
    // Relocation check
    if let Err(e) = ensure_self_relocation() {
        eprintln!("Relocation error: {}", e);
    }

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
