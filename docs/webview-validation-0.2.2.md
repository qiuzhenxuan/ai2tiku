# 0.2.2 WebView 验证记录

2026-10-23，默认界面恢复为 Vue + Tauri / Windows WebView2。通过 `pnpm build` 生成生产前端，再使用 `cargo build --release --locked --features custom-protocol` 嵌入资源；EXE 不依赖本地 Vite 开发服务器。Release 缺少此开关时编译会报错。

| 检查 | 结果 |
| --- | --- |
| TypeScript 与 Vue 生产构建 | 通过 |
| Rust 默认 WebView 业务测试 | 26 项通过 |
| 布局与主题 | 八页 × 两种主题 × 两种侧栏 × 两种窗口尺寸，共 64 张截图；页面无横向溢出 |
| 默认侧栏 | 每次启动默认 72px 图标栏，点击或 Enter 展开至 200px，再次点击收起 |
| 视觉对齐 | 展开前后图标原点一致；常规按钮、输入框和下拉框实际高度均为 34px |
| 控件反馈 | 主按钮悬停与按下填色变化，边界保持不变；可点击控件显示手型 |
| 键盘导航 | Home / End、上下首尾循环、Enter 导航及 Escape 关闭说明 |
| 右键 | 页面、弹窗和编辑区的默认 contextmenu 均被阻止 |
| 文字编辑 | 输入、全选与删除快捷键通过；未拦截复制和粘贴快捷键 |
| 火箭尾焰 | 72 个颗粒，连续核心，悬停后竖直抬升 4px，移开复位并暂停颗粒 |
| 减少动态效果 | 尾焰隐藏，火箭位移关闭 |
| 前端错误 | 未出现未处理页面异常 |

生产前端在 Edge 中使用隔离模拟 IPC 验证，另保存一张尾焰截图，共 65 张。[前端自动报告](validation/webview-0.2.2.json)记录全部页面尺寸；`scripts/check-webview.cjs` 可复现，需已有 Playwright，允许通过 `PLAYWRIGHT_MODULE` 指定安装位置。

此外，启动了真正的 Windows Tauri / WebView2 Release EXE，通过 `--data-dir` 指向独立演示目录，使用测试进程专用的本地 CDP 端口检查展开、收起、右键拦截、文字编辑和尾焰。没有模拟 Tauri 命令，没有读写用户题库或 Key，没有调用真实付费模型。[桌面实窗报告](validation/webview-desktop-0.2.2.json)与[实窗截图](screenshots/webview-desktop-rocket.png)已保存；脚本为 `scripts/check-desktop-webview.cjs`。

本机 240Hz 环境采样约两秒：Edge 预览 479 帧，真实 WebView2 窗口 480 帧；两者 requestAnimationFrame 间隔中位数约 4.2ms、95 分位约 4.3ms。这是本机动画回调节奏，不代表其他硬件的保证帧率或端到端显示延迟。

右键菜单采用两层处理：前端捕获阶段取消全局 contextmenu，Windows 设置 `AreDefaultContextMenusEnabled=false`。调试端口仅用于独立测试进程，不写入正常配置。

本地 Release 大小：9,534,464 字节。SHA-256：

```text
9bfc3540c7da9d50f4d9f066b6ea80b0b6ecd6d7e92f0524a753f3814a670a91
```
