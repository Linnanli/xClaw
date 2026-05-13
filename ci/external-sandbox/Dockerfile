# B6-1b — ExternalSandbox 嵌套契约测试容器
#
# Issue: https://github.com/Linnanli/xClaw/issues/472
# 用途：把外部宿主机预编译好的 `req_external_sandbox_container` 测试二进制
#       拷进一个最小 debian 镜像里跑，验证 ExternalSandbox 模式下沙箱启动
#       路径不依赖 bubblewrap / landlock / CAP_SYS_ADMIN。
#
# 设计要点：
#   - 不在容器里跑 cargo（避免在 `--network none` 容器里依赖 crates.io）
#   - 不在容器里编译 Rust（避免冷启动 3+ 分钟，cache 也复杂）
#   - workflow 用 `actions/cache` 缓存外部宿主 target/，容器只做"运行测试"
#   - 镜像保持最小：只装 `/bin/true` 所在的 coreutils（debian:bookworm-slim 自带）
#   - **glibc 兼容**：runner pin 在 ubuntu-22.04（glibc 2.35），bookworm-slim
#     glibc 2.36 向后兼容；不混 newer/older glibc 版本，避免 binary 跑不起来
#
# 使用：
#   docker build -f ci/external-sandbox/Dockerfile -t xclaw-external-sandbox-test .
#   docker run --rm --network none --read-only --tmpfs /tmp \
#     -v "$PWD/target/debug/deps/req_external_sandbox_container-XXXX:/test:ro" \
#     xclaw-external-sandbox-test
#
# 不主动 COPY 测试 binary 进 image —— workflow 用 `-v` 挂载，让单 image 重用
FROM debian:bookworm-slim

# 显式声明：本镜像不需要任何网络工具、不需要 sudo / setuid、不需要 cgroup
# 写权限。任何引入"包管理 install 一个网络/沙箱依赖"的 patch 都违反 B6-1b
# 的非目标声明（详见 issue #472），review 时必拒。
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /workspace

# 默认入口由 workflow 显式覆盖（exec 形式 `/test --nocapture`）。
# 此处只防呆：直接 `docker run xclaw-external-sandbox-test` 时打印用法。
CMD ["/bin/sh", "-c", "echo 'usage: docker run --rm -v <test-bin>:/test:ro xclaw-external-sandbox-test /test [--nocapture]'; exit 64"]
