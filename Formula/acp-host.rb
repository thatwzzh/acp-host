class AcpHost < Formula
  desc "把本机 ACP agent 包成单二进制守护进程，任意语言按行 JSON 调用"
  homepage "https://github.com/thatwzzh/acp-host"
  version "0.1.5"
  license "MIT"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/thatwzzh/acp-host/releases/download/v#{version}/acp-host-aarch64-apple-darwin.tar.gz"
      sha256 "94b815bbf34e0821650d1b60cd9e06a607002ba0953eff52770349d722a3a4a9"
    else
      url "https://github.com/thatwzzh/acp-host/releases/download/v#{version}/acp-host-x86_64-apple-darwin.tar.gz"
      sha256 "15ba5a4750cd4ff3525d6265d20fba2c22d3e90e46029612093f3508e0bb7c2c"
    end
  end

  on_linux do
    if Hardware::CPU.arm?
      url "https://github.com/thatwzzh/acp-host/releases/download/v#{version}/acp-host-aarch64-unknown-linux-gnu.tar.gz"
      sha256 "b629d3ce85e9bc5065815b5543fc5ec0c4e67f3b44f758c133572d4a96e8adf2"
    else
      url "https://github.com/thatwzzh/acp-host/releases/download/v#{version}/acp-host-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "5449759ae424e2f0347a50f9bec625472ad0687c4b3ab29e337b016c05ee2c0b"
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
