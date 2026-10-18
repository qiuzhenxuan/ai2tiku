# ai2tiku

把大模型接入 **OCS 网课助手** 的本地 AI 题库。

ai2tiku 是一个基于 **Tauri 2 + Vue 3** 的桌面应用：它在你本机跑起一个题库接口，
OCS 脚本通过标准的「题库配置」调用它。**先用本地题库匹配，未命中再交给 AI 作答**，
答过的题会自动缓存，重复题目直接命中、不再消耗 Token。

> 设计目标与 [Miaozeqiu/ZError](https://github.com/Miaozeqiu/ZError) 类似：
> 为 OCS 搭建本地 AI 题库，支持自定义 AI 供应商与模型。

## 功能

- **内置常用供应商预设** —— 智谱 / DeepSeek / 通义千问 / Kimi / 混元 / 火山方舟 / 星火 / 千帆 / MiniMax / OpenAI / Ollama，选中后**只需填 API Key**
- **保存供应商自动获取可用模型** —— 自动请求 `/models` 拉取该厂商的模型并填入，无需手抄模型名
- **OCS 题库配置一键生成** —— 支持「订阅链接」与「JSON 字符串」两种填法
- **支持 OCS 5.0 图片题优化** —— 题库配置声明了 `${images}` 等参数，题干/选项里的图片
  会以 base64 传给模型（图片题需改用支持视觉的模型，如 glm-4.5v、qwen-vl-max）
- **AI 供应商与模型自定义** —— 任何 OpenAI 兼容服务，可增删模型、设默认、测连通性
- **本地题库管理与缓存** —— AI 答过的题自动入库，支持增删改查
- **答题记录** —— 每次搜题的来源、模型、耗时、Token 全部留痕
- **统计看板** —— 缓存命中率、Token 消耗趋势、按模型用量分布
- **日夜双主题** —— 浅色 / 深色两套配色，跟随系统或手动切换
- **默认强制直连** —— 不受本机代理软件的系统代理影响；需要访问 OpenAI 等被墙服务时可在「设置」里单独填代理

## 截图

| 概览 | OCS 题库配置 |
| --- | --- |
| ![概览](docs/screenshots/dashboard.png) | ![OCS 题库配置](docs/screenshots/ocs.png) |

## 技术栈

| 层 | 选型 |
| --- | --- |
| 桌面容器 | Tauri 2 |
| 后端 | Rust + axum（本地 HTTP 服务）+ rusqlite（SQLite 存储） |
| 前端 | Vue 3 + TypeScript + Vite + vue-router |
| 样式 | 原生 CSS + CSS 变量驱动的日夜双主题 |

## 运行

前置要求：Node 18+、pnpm、Rust stable 工具链，Windows 上还需要 MSVC 生成工具。

```bash
pnpm install
pnpm tauri dev     # 开发模式
pnpm tauri build   # 打包
```

仅调试前端界面（不含本地服务）：

```bash
pnpm dev
```

## 接入 OCS

1. 打开软件，进入「AI 供应商与模型」，在「新增供应商」里**选一个预置厂商**，填上你的 API Key，
   保存后会**自动拉取该厂商的可用模型并填入**（第一个会自动设为默认）。
2. 进入「OCS 题库配置」，任选一种方式接入：
   - 复制 **订阅链接**（`http://127.0.0.1:3100/ocs`）——以后改端口或换模型都不用再动 OCS；
   - 或复制 **JSON 配置** 粘贴进去。
3. 打开 OCS 脚本的「题库配置」，填入并保存。
4. 回到网课页面开启自动答题。

生成的配置形如：

```json
[
  {
    "name": "ai2tiku (本地 AI 题库)",
    "homepage": "http://127.0.0.1:3100",
    "url": "http://127.0.0.1:3100/api/search",
    "method": "post",
    "contentType": "json",
    "responseType": "json",
    "type": "GM_xmlhttpRequest",
    "headers": { "Content-Type": "application/json" },
    "data": {
      "title": "${title}",
      "type": "${type}",
      "options": "${options}",
      "images": { "handler": "return (env) => env.images" },
      "suggestion_title": "${suggestion_title}",
      "suggestion_options": "${suggestion_options}"
    },
    "handler": "return (res)=> res.code === 1 ? [res.question, res.answer] : [res.msg, undefined]"
  }
]
```

> `contentType` 是 OCS 5.0 之前的旧字段（此后由 `responseType` 取代），两个都写以兼容新旧版本。
> `data` 里对 `${images}` / `${suggestion_title}` / `${suggestion_options}` 的引用是 OCS
> 判定「题库支持图片题优化」的依据（`isAnswererWrapperSupportImageOptimize`），
> 且该判定只认 `method: "post"`。其中 `images` 是 base64 数组，用字段解析器 `{ handler }`
> 返回可保留数组结构（字符串占位符会被拼成一整条字符串）。

> **注意**：OCS 请求本机地址需要在脚本头部 `@connect` 中放行域名。
> 请把脚本管理器的 `@connect 模式` 设为宽松模式，或使用官方「全域名通用版本」脚本。

## 本地接口

服务只监听 `127.0.0.1`，默认端口 `3100`（可在「设置」中修改）。

```
GET  /api/search?title=...&type=...&options=...
POST /api/search   {"title":"...","type":"single","options":"A. xx\nB. yy"}
                   # 图片题另带 images（base64 数组）/ suggestion_title / suggestion_options
GET  /api/status
GET  /ocs          # OCS 订阅链接：返回题库配置 JSON
```

命中返回：

```json
{
  "code": 1,
  "question": "1 + 1 = ?",
  "answer": "B",
  "source": "cache",
  "similarity": 1.0,
  "model": "本地题库",
  "tokens": 0,
  "durationMs": 1
}
```

未命中返回 `{"code": 0, "msg": "..."}`，OCS 会把它显示为提醒信息。

多选题的答案按 OCS 官方文档的要求，用**特殊符号 `#` 连接**返回（如 `A#C`）；
填空题有多个空时同样用 `#` 连接（如 `张三#李四`）。OCS 的 `splitAnswer`
分隔符列表是 `=== # --- ### | ; ；`，所以这不依赖任何额外解析约定。

## 目录结构

```
├── src/                     # Vue 前端
│   ├── api/                 # Tauri 命令封装与类型
│   ├── components/          # 通用组件
│   ├── composables/         # 主题、提示
│   ├── views/               # 页面
│   └── styles/main.css      # 设计系统（含日夜双主题变量）
└── src-tauri/               # Rust 后端
    └── src/
        ├── ai.rs            # OpenAI 兼容调用（作答 / 拉取模型 / 测连通性）
        ├── db.rs            # SQLite 存储与题库匹配
        ├── presets.rs       # 内置的常用供应商预设
        ├── ocs.rs           # OCS 题库配置的唯一来源（JSON 与订阅链接共用）
        ├── server.rs        # 本地 HTTP 服务（含 /ocs 订阅端点）
        ├── search.rs        # 缓存优先 + AI 兜底的核心流程
        └── commands.rs      # 暴露给前端的命令
```

## 许可

MIT
