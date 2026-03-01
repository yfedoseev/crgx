class Crgx < Formula
  desc "npx for Rust — download and run any crate binary in one command"
  homepage "https://crgx.dev"
  version "{{VERSION}}"
  license "MIT"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/yfedoseev/crgx/releases/download/v{{VERSION}}/crgx-macos-aarch64-{{VERSION}}.tar.gz"
      sha256 "{{SHA256_MACOS_ARM}}"
    else
      url "https://github.com/yfedoseev/crgx/releases/download/v{{VERSION}}/crgx-macos-x86_64-{{VERSION}}.tar.gz"
      sha256 "{{SHA256_MACOS_X86}}"
    end
  end

  on_linux do
    url "https://github.com/yfedoseev/crgx/releases/download/v{{VERSION}}/crgx-linux-x86_64-{{VERSION}}.tar.gz"
    sha256 "{{SHA256_LINUX_X86}}"
  end

  def install
    bin.install "crgx"
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/crgx --version")
  end
end
