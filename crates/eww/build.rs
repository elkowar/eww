use std::process::Command;
fn main() {
    let output = Command::new("git").args(["rev-parse", "HEAD"]).output();
    if let Ok(output) = output {
        if let Ok(hash) = String::from_utf8(output.stdout) {
            println!("cargo:rustc-env=GIT_HASH={}", hash);
            println!("cargo:rustc-env=CARGO_PKG_VERSION={} {}", env!("CARGO_PKG_VERSION"), hash);
        }
    }
    let output = Command::new("git").args(["show", "-s", "--format=%ci"]).output();
    if let Ok(output) = output {
        if let Ok(date) = String::from_utf8(output.stdout) {
            println!("cargo:rustc-env=GIT_COMMIT_DATE={}", date);
        }
    }
}
