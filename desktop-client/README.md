建议工作流：
sccache

只改前端：直接 cd desktop-client/src-ui && npm run dev，浏览器访问 localhost:5173
只改 Rust：cargo build -p desktop-client --lib 单独编译，确认无错后再启动
需要完整 Tauri：才用 start-desktop.sh

日常改代码 → cargo build -p desktop-client --lib，只重编改动的 crate，几秒到几十秒
磁盘满了 → clean-target.sh，清掉增量缓存 + 旧产物
clean 后重编 → sccache 命中缓存，速度比无缓存快很多
定期自动清理 → 设 cron 0 3 * * * cd ~/Documents/code/x-claw && cargo sweep --time 3
核心逻辑：不要再用 cargo clean 或 cargo sweep --time 0，用 cargo sweep --time 3 做温和清理就够了。


# 在项目根目录（不是 ironclaw 子目录）
cargo build -p desktop-client --lib
