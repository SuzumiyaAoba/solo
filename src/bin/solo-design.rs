#[cfg(target_os = "macos")]
#[path = "../gallery/mod.rs"]
mod gallery;

#[cfg(target_os = "macos")]
fn main() {
    gallery::run();
}

#[cfg(not(target_os = "macos"))]
fn main() {
    eprintln!("Solo Design の GUI 検証対象は macOS です。");
    std::process::exit(1);
}
