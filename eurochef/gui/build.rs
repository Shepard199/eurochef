use std::process::Command;

fn main() {
    // ROBOTS_PATCH_0045_GUI_EMPTY_GIT_HASH_PANIC
    // Command::output() can be Ok even when git exits unsuccessfully. Require a
    // successful exit status and non-empty stdout before exporting GIT_HASH.
    let git_hash = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|hash| hash.trim().to_string())
        .filter(|hash| !hash.is_empty())
        .unwrap_or_else(|| "unknown".to_string());

    println!("cargo:rustc-env=GIT_HASH={}", git_hash);

    let date_time = chrono::Utc::now();
    println!(
        "cargo:rustc-env=BUILD_DATE={}",
        date_time.format("%Y-%m-%d %H:%M:%S")
    );

    let output = Command::new("rustc").args(["--version"]).output().unwrap();
    let rustc_version = String::from_utf8(output.stdout).unwrap();
    println!("cargo:rustc-env=RUSTC_VERSION={}", rustc_version);
}
