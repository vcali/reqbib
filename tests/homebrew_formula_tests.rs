use std::fs;
use std::path::Path;
use std::process::Command;

use tempfile::TempDir;

fn write_checksum(dist_dir: &Path, archive_name: &str, sha: &str) {
    fs::write(
        dist_dir.join(format!("{archive_name}.sha256")),
        format!("{sha}  {archive_name}\n"),
    )
    .unwrap();
}

#[test]
fn generates_homebrew_formula_from_release_checksums() {
    let temp_dir = TempDir::new().unwrap();
    let dist_dir = temp_dir.path().join("dist");
    let output_path = temp_dir.path().join("combib.rb");

    fs::create_dir_all(&dist_dir).unwrap();

    write_checksum(
        &dist_dir,
        "combib-x86_64-unknown-linux-gnu.tar.gz",
        "linuxsha",
    );
    write_checksum(
        &dist_dir,
        "combib-x86_64-apple-darwin.tar.gz",
        "intelmacsha",
    );
    write_checksum(&dist_dir, "combib-aarch64-apple-darwin.tar.gz", "armmacsha");

    let status = Command::new("sh")
        .arg("scripts/generate-homebrew-formula.sh")
        .arg("--repo")
        .arg("vcali/reqbib")
        .arg("--version")
        .arg("0.1.0")
        .arg("--tag")
        .arg("v0.1.0-build.42")
        .arg("--dist-dir")
        .arg(&dist_dir)
        .arg("--output")
        .arg(&output_path)
        .status()
        .unwrap();

    assert!(status.success());

    let formula = fs::read_to_string(output_path).unwrap();

    assert!(formula.contains("class Combib < Formula"));
    assert!(formula.contains("version \"0.1.0\""));
    assert!(formula.contains("license \"MIT\""));
    assert!(formula.contains(
        "https://github.com/vcali/reqbib/releases/download/v0.1.0-build.42/combib-aarch64-apple-darwin.tar.gz"
    ));
    assert!(formula.contains(
        "https://github.com/vcali/reqbib/releases/download/v0.1.0-build.42/combib-x86_64-apple-darwin.tar.gz"
    ));
    assert!(formula.contains(
        "https://github.com/vcali/reqbib/releases/download/v0.1.0-build.42/combib-x86_64-unknown-linux-gnu.tar.gz"
    ));
    assert!(formula.contains("sha256 \"armmacsha\""));
    assert!(formula.contains("sha256 \"intelmacsha\""));
    assert!(formula.contains("sha256 \"linuxsha\""));
    assert!(formula.contains("bin.install \"combib\""));
}
