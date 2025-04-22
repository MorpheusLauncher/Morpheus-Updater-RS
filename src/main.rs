use dirs::data_dir;
use reqwest::blocking::get;
use std::fs::{self, File};
use std::io::{copy, BufReader, BufWriter};
use std::path::{Path, PathBuf};
use std::process::Command;
use zip::ZipArchive;

const BASE_URL: &str = "https://morpheuslauncher.it";
const USE_BETA_CHANNEL: bool = false;

const ZIP_NAME: &str = "morpheus_win.zip";
const VERSION_FILE: &str = "version.txt";

fn get_working_directory() -> PathBuf {
    let mut dir = data_dir().expect("Unable to find AppData directory");
    dir.push(".morpheus");
    fs::create_dir_all(&dir).expect("Failed to create .morpheus folder");
    dir
}

fn download_file(url: &str, destination: &Path) {
    println!("Downloading: {url}");
    let mut response = get(url).expect("Failed to download file");
    let mut file = BufWriter::new(File::create(destination).expect("Cannot create file"));
    copy(&mut response, &mut file).expect("Failed to write file");
}

fn extract_zip(zip_path: &Path, output_dir: &Path) {
    println!("Extracting: {:?}", zip_path);
    let file = File::open(zip_path).expect("Cannot open ZIP");
    let mut archive = ZipArchive::new(BufReader::new(file)).expect("Invalid ZIP");

    for i in 0..archive.len() {
        let mut file = archive.by_index(i).unwrap();
        let outpath = output_dir.join(file.sanitized_name());

        if file.name().ends_with('/') {
            fs::create_dir_all(&outpath).unwrap();
        } else {
            if let Some(parent) = outpath.parent() {
                fs::create_dir_all(parent).unwrap();
            }
            let mut outfile = BufWriter::new(File::create(&outpath).unwrap());
            copy(&mut file, &mut outfile).unwrap();
        }
    }
}

fn needs_update(local_version: &Path, remote_url: &str) -> bool {
    if local_version.exists() {
        let local = fs::read_to_string(local_version).unwrap_or_default();
        let remote = get(remote_url).unwrap().text().unwrap_or_default();
        return local.trim() != remote.trim();
    }
    true
}

fn main() {
    let dest = get_working_directory();
    let zip_path = dest.join(ZIP_NAME);
    let jar_path = dest.join("Launcher.jar");
    let version_path = dest.join(VERSION_FILE);

    // Determina il percorso corretto in base al canale scelto
    let zip_url = if USE_BETA_CHANNEL {
        format!("{}/downloads/beta/{}", BASE_URL, ZIP_NAME)
    } else {
        format!("{}/downloads/{}", BASE_URL, ZIP_NAME)
    };

    let jar_url = format!("{}/downloads/Launcher.jar", BASE_URL);
    let version_url = format!("{}/{}", BASE_URL, VERSION_FILE);

    println!(
        "Checking for updates (channel: {})...",
        if USE_BETA_CHANNEL { "beta" } else { "stable" }
    );

    if needs_update(&version_path, &version_url) {
        println!("Update available. Downloading...");

        download_file(&zip_url, &zip_path);
        extract_zip(&zip_path, &dest);
        download_file(&jar_url, &jar_path);
        download_file(&version_url, &version_path);

        fs::remove_file(&zip_path).ok();
        println!("Update completed.");
    } else {
        println!("You're up to date.");
    }

    let launcher_path = dest.join("morpheus_launcher_gui.exe");
    println!("Launching: {:?}", launcher_path);

    Command::new(launcher_path)
        .spawn()
        .expect("Failed to launch application");

    println!("Done.");
}
