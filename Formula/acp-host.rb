class AcpHost < Formula
  desc "把本机 ACP agent 包成单二进制守护进程，任意语言按行 JSON 调用"
  homepage "https://github.com/thatwzzh/acp-host"
  version "0.1.1"
  license "MIT"

  on_macos do
    if Hardware::CPU.arm?
      url "https://github.com/thatwzzh/acp-host/releases/download/v#{version}/acp-host-aarch64-apple-darwin.tar.gz"
      sha256 "17ce814af94ef747d25e4a5d81488ba224b4b669b63475a163e4e041fcd4d440"
    else
      url "https://github.com/thatwzzh/acp-host/releases/download/v#{version}/acp-host-x86_64-apple-darwin.tar.gz"
      sha256 "09fd6aa5076487428bd62ce21fb916e0fcb20cd3d72dace90aeb164eae48ea14"
    end
  end

  on_linux do
    if Hardware::CPU.arm?
      url "https://github.com/thatwzzh/acp-host/releases/download/v#{version}/acp-host-aarch64-unknown-linux-gnu.tar.gz"
      sha256 "1a95805ae18b2f7754818a4a23261046aba7282632bfe0243e492103b7f3e2c9"
    else
      url "https://github.com/thatwzzh/acp-host/releases/download/v#{version}/acp-host-x86_64-unknown-linux-gnu.tar.gz"
      sha256 "76c7e850f74077e26fe2404a78735b35093ce0695b241306e93a9102d14d2ec0"
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
