#!/bin/sh

set -eu

usage() {
  echo "usage: $0 --repo <owner/repo> --version <version> [--revision <revision>] --tag <tag> --dist-dir <dir> --output <file>" >&2
  exit 1
}

repo=""
version=""
revision=""
tag=""
dist_dir=""
output=""

while [ "$#" -gt 0 ]; do
  case "$1" in
    --repo)
      [ "$#" -ge 2 ] || usage
      repo="$2"
      shift 2
      ;;
    --version)
      [ "$#" -ge 2 ] || usage
      version="$2"
      shift 2
      ;;
    --revision)
      [ "$#" -ge 2 ] || usage
      revision="$2"
      shift 2
      ;;
    --tag)
      [ "$#" -ge 2 ] || usage
      tag="$2"
      shift 2
      ;;
    --dist-dir)
      [ "$#" -ge 2 ] || usage
      dist_dir="$2"
      shift 2
      ;;
    --output)
      [ "$#" -ge 2 ] || usage
      output="$2"
      shift 2
      ;;
    *)
      usage
      ;;
  esac
done

[ -n "$repo" ] || usage
[ -n "$version" ] || usage
[ -n "$tag" ] || usage
[ -n "$dist_dir" ] || usage
[ -n "$output" ] || usage

linux_archive="shellshelf-x86_64-unknown-linux-gnu.tar.gz"
mac_intel_archive="shellshelf-x86_64-apple-darwin.tar.gz"
mac_arm_archive="shellshelf-aarch64-apple-darwin.tar.gz"

sha_from_file() {
  archive="$1"
  sha_file="$dist_dir/$archive.sha256"

  [ -f "$sha_file" ] || {
    echo "missing checksum file: $sha_file" >&2
    exit 1
  }

  awk '{ print $1; exit }' "$sha_file"
}

linux_sha="$(sha_from_file "$linux_archive")"
mac_intel_sha="$(sha_from_file "$mac_intel_archive")"
mac_arm_sha="$(sha_from_file "$mac_arm_archive")"

mkdir -p "$(dirname "$output")"

revision_line=""
if [ -n "$revision" ]; then
  revision_line="  revision $revision"
fi

cat > "$output" <<EOF
class Shellshelf < Formula
  desc "CLI for storing, searching, and sharing reusable shell commands"
  homepage "https://github.com/$repo"
  version "$version"
${revision_line}
  license "MIT"

  if OS.mac?
    if Hardware::CPU.arm?
      url "https://github.com/$repo/releases/download/$tag/$mac_arm_archive"
      sha256 "$mac_arm_sha"
    else
      url "https://github.com/$repo/releases/download/$tag/$mac_intel_archive"
      sha256 "$mac_intel_sha"
    end
  elsif OS.linux?
    url "https://github.com/$repo/releases/download/$tag/$linux_archive"
    sha256 "$linux_sha"
  end

  def install
    bin.install "shellshelf"
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/shellshelf --version")
  end
end
EOF
