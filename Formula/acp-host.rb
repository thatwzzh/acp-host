class AcpHost < Formula
  desc "把本机 ACP agent 包成单二进制守护进程，任意语言按行 JSON 调用"
  homepage "https://github.com/thatwzzh/acp-host"
  version "0.1.2"
  license "MIT"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/thatwzzh/acp-host/releases/download/v#{version}/acp-host-aarch64-apple-darwin.tar.gz"
      sha256 "1a7ba3550ddee260b4f8bd9dc6d368d6643f71c6ca28f0d2a693f1d67ed517bd"
    else
      url "https://github.com/thatwzzh/acp-host/releases/download/v#{version}/acp-host-x86_64-apple-darwin.tar.gz"
      sha256 "6004df604e4ca50d3677b441410de581445960b647fa5c2f0e8796cc5b378840"
    end
  end

  on_linux do
    if Hardware::CPU.arm?
      url "https://github.com/thatwzzh/acp-host/releases/download/v#{version}/acp-host-aarch64-unknown-linux-gnu.tar.gz"
      sha256 "6ad6b9480cbe440f8fd8a0d203c4981518ca30d4cf08ee5f13c0e896d58434c4"
    else
      url "https://github.com/thatwzzh/acp-host/releases/download/v#{version}/acp-host-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "f6b168be6c0c07e44980b12367bac3a30aead5812299a35572e92653455fbfe9"
    end
  end

  def install
    bin.install "acp-host"
  end

  test do
    output = pipe_output("#{bin}/acp-host", '{"id":"1","method":"agents.list"}' + "\n")
    assert_match '"ok":true', output
  end
end
