class Lazyoav < Formula
  desc "Interactive TUI for linting, generating, and compiling OpenAPI specs"
  homepage "https://github.com/entur/openapi-validator-tui"
  version "0.1.0"
  license "EUPL-1.2"

  # Update version, urls, and sha256 values for each release.

  on_macos do
    on_intel do
      url "https://github.com/entur/openapi-validator-tui/releases/download/v#{version}/lazyoav-#{version}-x86_64-apple-darwin.tar.gz"
      sha256 "0000000000000000000000000000000000000000000000000000000000000000"
    end

    on_arm do
      url "https://github.com/entur/openapi-validator-tui/releases/download/v#{version}/lazyoav-#{version}-aarch64-apple-darwin.tar.gz"
      sha256 "0000000000000000000000000000000000000000000000000000000000000000"
    end
  end

  on_linux do
    on_intel do
      url "https://github.com/entur/openapi-validator-tui/releases/download/v#{version}/lazyoav-#{version}-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "0000000000000000000000000000000000000000000000000000000000000000"
    end
  end

  def install
    bin.install "lazyoav"
  end

  test do
    assert_match version.to_s, shell_output("#{bin}/lazyoav --version")
  end
end
