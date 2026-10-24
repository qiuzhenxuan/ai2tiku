# 网页端与 Docker 部署

网页端由独立的 Vue 用户门户、Rust 多用户 API 和 Nginx 入口组成。桌面版继续使用原入口；网页端的数据目录独立。

## 用户流程

1. 管理员在后台创建用户，或由站长通过部署配置开启注册。
2. 用户登录，选择供应商，填写自己的模型 API Key；稍候自动读取模型列表，从下拉框选择模型并保存配置。
3. 创建题库访问令牌，复制生成的完整 OCS JSON 到 OCS 题库配置。
4. 令牌及包含令牌的 OCS 配置只在创建后显示。关掉窗口后只能查看名称、前缀、使用情况并撤销；遗失时创建新令牌，撤销旧令牌。
5. OCS 使用新的题库令牌访问本服务，由服务器代用户调用模型。模型费用由该用户填写的模型 Key 对应账号承担。

登录页和站内顶部都有 GitHub 图标与“GitHub”按钮，链接到 <https://github.com/qiuzhenxuan/ai2tiku>。

获取模型列表仅向所选供应商的模型元数据接口请求，不调用答题模型、不消耗本站搜题额度，也不会自动保存草稿 Key。再次进入“模型与接入”时，可用已保存的 Key 获取列表；Key 不回显。供应商未开放兼容列表接口、账号未授权或连接失败时，会说明原因，可重新获取或手动输入模型名称／接入点 ID。列表表示供应商返回的可见模型，余额、地区及具体模型的调用权限仍以实际调用结果为准。

一次展示是本服务不提供再次读取明文的接口。用户仍可自行保存、复制或截图；已经复制到 OCS 的配置也包含访问令牌，需要用户妥善保管。

### OCS 连接检测

OCS 会向配置的搜题地址发出不携带题库令牌的 HEAD 请求，必要时回退到 GET。`/api/search` 对这两种方法返回 200 作为连通性响应，不读取用户题库、不调用模型、不计入额度；连接成功仅说明服务可达。真实搜题仍为 POST JSON，使用生成配置中的 `Authorization: Bearer …` 请求头，不支持将 Key 或题库令牌放入 URL。探测逻辑可参见 [OCS 官方源码](https://github.com/ocsjs/ocsjs/blob/da5c0775a9d055724149ff505eae75bd85d334f9/packages/scripts/src/utils/ui.ts)。

## 首次部署

服务器需要 Docker Compose v2。以下命令在项目根目录运行：

```sh
node scripts/init-hosted.cjs
```

初始化工具创建以下文件，已有文件不会被覆盖，密钥和密码不会打印到日志：

- `deploy/.env`：站点 URL、管理员邮箱、端口和注册开关。
- `deploy/secrets/master_key`：32 字节随机加密主密钥，以 Base64 保存。
- `deploy/secrets/admin_password`：随机管理员初始密码。请在自己的服务器终端读取此文件后登录。

服务器未安装 Node 时，可以手动复制 `.env.example`，然后生成同名密钥文件：

```sh
cp deploy/.env.example deploy/.env
mkdir -p deploy/secrets
chmod 700 deploy/secrets
openssl rand -base64 32 > deploy/secrets/master_key
openssl rand -hex 24 > deploy/secrets/admin_password
chmod 444 deploy/secrets/*
```

密钥文件所在目录使用 `700`，阻止服务器上的其他用户访问。文件使用 `444`，让 Docker 文件型 secret 的只读挂载能被容器内的非 root 服务读取；不要将这个目录发布到静态网站或共享目录。Docker Compose 文件型 secret 保留宿主机的权限，参见 [官方说明](https://docs.docker.com/reference/compose-file/services/#secrets)。

编辑 `deploy/.env`：

```dotenv
PUBLIC_URL=https://tiku.example.com
ADMIN_EMAIL=your-admin@example.com
REGISTRATION_OPEN=false
LISTEN_ADDRESS=127.0.0.1
HTTP_PORT=8080
```

`PUBLIC_URL` 必须是用户浏览器实际访问的站点地址（包括非默认端口）。服务据此生成题库 URL 并校验浏览器写操作的来源。

```sh
docker compose --env-file deploy/.env -f deploy/compose.yaml up -d --build
docker compose --env-file deploy/.env -f deploy/compose.yaml ps
```

让服务器上的 HTTPS 反向代理将 `tiku.example.com` 转发到 `http://127.0.0.1:8080`。反向代理的请求超时建议至少 180 秒，以容纳模型调用和一次格式重试。浏览器和 OCS 使用 HTTPS 公网域名，不直接访问 API 容器。

默认只监听服务器回环地址。需要接入其他代理容器时，按实际网络拓扑调整 `LISTEN_ADDRESS` 和网络配置。生产站点使用 HTTPS；`http://localhost:8080` 可用于本机测试。

## 管理与密钥

- 默认关闭自助注册，管理员登录后创建用户并设置每日请求额度。
- 后台可查看用户、停用或恢复账号、调整额度、重置登录密码、撤销该用户全部题库令牌，并查看最近的管理操作记录。
- 停用账号后，已有会话与题库令牌不能继续使用。已经发给模型供应商的请求无法追回。
- 用户模型 Key 使用 AES-256-GCM 加密保存；题库令牌和登录会话令牌保存摘要，密码使用 Argon2。
- 每个用户有独立的题库与答题记录。管理员列表不返回用户模型 Key 或题库令牌明文。
- 不将模型 Key 放进题库 URL、OCS 配置、请求记录或管理审计记录。
- 默认供应商由服务器预设控制，不允许普通用户填写任意内网地址。扩展供应商须由站长配置并审查目标地址。

“只保存摘要”的是本站生成的题库令牌。模型 Key 必须能由服务器解密，才能代用户调用模型。掌握数据库和加密主密钥的服务器管理员具有解密能力。

## 额度与记录

每日额度按 UTC 日历日重置；这是请求次数额度，不是供应商的金额上限。服务器限制用户及全局并发，额度在实际处理请求前原子扣减，防止并发绕过。已经接受的失败请求、缓存命中和无效题目均计入额度；鉴权失败及因并发超限被拒绝的请求不计入额度。

管理员可以将每日请求额度设为 `0`，表示**无上限**。无上限账号仍统计当天请求次数，仍需有效令牌和启用状态，并保留并发限制。改回有限额度时，以当天已经使用的次数判断剩余额度，不会重置计数。

单次搜题请求体上限为 8 MiB，读取超时 15 秒；最多 8 张 base64 图片，每张编码内容不超过 2 MiB，并受总请求大小限制。模型回复读取上限为 2 MiB。

请求成功率表示获得了格式有效的答案，不能等同于知识上的答案正确率。题库命中优先，未命中才调用用户配置的模型；没有配置模型时返回明确失败信息。

## 备份和升级

同时备份 Docker 数据卷 `ai2tiku-hosted_app_data` 与 `deploy/secrets/master_key`。为了得到一致的 SQLite 快照，先停 API，再复制整个数据目录，或使用 SQLite 在线备份工具。不要只复制正在写入的 `.db` 文件而遗漏 WAL。

丢失加密主密钥会导致已保存的模型 Key 无法解密；重新运行初始化脚本不会恢复原密钥。已有系统升级时也不要更换主密钥或删除数据卷。

```sh
git pull
docker compose --env-file deploy/.env -f deploy/compose.yaml up -d --build
```

管理员初始密码仅用于首次创建管理员账号，不应把修改初始密码文件当成重置已有密码的方式。请通过后台已有的密码管理功能处理。

## 本地开发

```sh
cargo build --manifest-path src-tauri/Cargo.toml --no-default-features --features hosted --bin ai2tiku-server
cd web
npm ci
npm run dev
```

Rust 服务需要配置 `PUBLIC_URL`、`ADMIN_EMAIL`、`ADMIN_PASSWORD`（或 `ADMIN_PASSWORD_FILE`）、`MASTER_KEY`（或 `MASTER_KEY_FILE`）、`DATA_DIR`，可用 `HOSTED_BIND` 设置监听地址。前端开发代理指向本机 `8080`，因此开发时 `PUBLIC_URL` 应填写浏览器打开的前端地址。

Docker 镜像构建与运行时密钥分离。相关机制参考 [Docker Compose secrets](https://docs.docker.com/reference/compose-file/secrets/)；Nginx 代理保留原请求路径，参考 [官方代理模块文档](https://nginx.org/en/docs/http/ngx_http_proxy_module.html)。
