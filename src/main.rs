#[cfg(target_os = "macos")]
mod ui;

#[cfg(target_os = "macos")]
fn main() {
    ui::run();
}

#[cfg(not(target_os = "macos"))]
fn main() {
    eprintln!(
        "Phase 0 の GUI 検証対象は macOS です。契約テストは cargo test --no-default-features で実行できます。"
    );
    std::process::exit(1);
}
