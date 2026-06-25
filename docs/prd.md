# 产品需求文档：Cmirror（中国镜像源管理器）

| 项目         | 内容                                                                     |
| :----------- | :----------------------------------------------------------------------- |
| **产品名称** | Cmirror（cmir）                                                          |
| **核心价值** | 一键解决中国大陆开发环境网络慢的问题，提供“测速-对比-自动配置”闭环方案。 |
| **技术栈**   | Rust（Clap、Tokio、Reqwest）                                             |
| **目标用户** | 开发人员、运维工程师、Linux 爱好者                                       |

---

## 1\. 产品概述

**Cmirror** 是一个跨平台的命令行工具。它通过内置的高质量国内源列表（阿里云、腾讯云、清华 TUNA、中科大 USTC 等），并发测试网络延迟，并支持**一键修改**系统或语言包管理器的配置文件。它解决了手动搜索源、手动修改配置繁琐、且不知道哪个源当前最快的痛点。

## 2\. 功能矩阵

### 2.1 支持的源类型

| 类别               | 工具名称              | 配置文件路径（示例）      | 是否需要 sudo | 备注                           |
| :----------------- | :-------------------- | :------------------------ | :--------- | :----------------------------- |
| **系统工具**       | `apt`（Ubuntu/Debian） | `/etc/apt/sources.list`   | ✅ 是      | 需智能替换域名，保留发行版代号 |
|                    | `yum` / `dnf`         | `/etc/yum.repos.d/*.repo` | ✅ 是      |                                |
| **基础设施工具**   | `docker`              | `/etc/docker/daemon.json` | ✅ 是      | 修改 registry-mirrors 数组     |
|                    | `brew`（Homebrew）    | 环境变量 / Git 远程地址   | ❌ 否      | 涉及 core、cask、bottles       |
| **语言生态工具**   | `pip`（Python）       | `~/.pip/pip.conf`         | ❌ 否      |                                |
|                    | `npm`（Node）         | `~/.npmrc`                | ❌ 否      |                                |
|                    | `go`（Golang）        | `GO111MODULE` 环境变量    | ❌ 否      |                                |
|                    | `cargo`（Rust）       | `~/.cargo/config.toml`    | ❌ 否      | 替换 crates.io-index           |

### 2.2 核心功能详情

#### F1. 状态透视

- **功能**: 自动定位并读取本地配置文件。
- **逻辑**: 解析当前配置，提取正在使用的 URL。如果未配置，显示“默认官方源”。
- **展示**: 清晰打印当前源地址，方便用户确认是否已经被篡改或配置错误。

#### F2. 全局并发测速

- **对比测试**: 将“用户当前源”与“内置推荐源”混合在一起进行测试。
- **真实延迟**: 使用 HTTP/HTTPS `HEAD` 请求计算首字节时间，而非 ICMP Ping。
- **超时控制**: 单个请求默认 3 秒超时，所有源并发执行。

#### F3. 一键配置

- **智能应用**: 支持 `use <name>` 指定源，或 `use --fastest` 自动应用最快源。
- **安全备份**: 修改任何文件前，**强制**在同级目录生成 `.bak.<timestamp>` 文件。
- **权限提升**: 检测到需要写入受保护路径（如 `/etc/`）时，若无权限则友好报错提示 `sudo`。

#### F4. 灾难恢复

- **功能**: 允许用户一键回滚到上一次的配置，或重置为官方默认配置。

---

## 3\. CLI 交互设计

### 3.1 查看状态 (`status`)

```bash
$ cmirror status
-----------------------------------------------------
工具    当前源地址                      状态
-----------------------------------------------------
pip     https://pypi.org/simple         [官方]
docker  https://docker.mirrors.ustc...  [自定义：USTC]
apt     http://archive.ubuntu.com/...   [官方]
-----------------------------------------------------
```

### 3.2 测速与对比 (`test`)

```bash
$ cmirror test pip
[||||||||||||||||] 100% 测试完成。

排名  延迟     名称       URL
1     25ms     Aliyun     https://mirrors.aliyun.com/pypi/simple/
2     38ms     Tuna       https://pypi.tuna.tsinghua.edu.cn/simple
3     900ms    当前源     https://pypi.org/simple  <--（你的当前源）
-----------------------------------------------------
推荐：`Aliyun` 比当前源快 36 倍。
运行 `cmirror use pip aliyun` 进行应用。
```

### 3.3 设置源 (`use`)

```bash
# 场景 A：指定源
$ cmirror use pip aliyun
> 备份配置到 ~/.pip/pip.conf.bak
> 正在更新配置...
> 成功！当前源已切换为 Aliyun。

# 场景 B：极速模式（一键变快）
$ sudo cmirror use apt --fastest
> 正在测试镜像... 发现 `Tuna` 最快（18ms）。
> 正在备份 /etc/apt/sources.list...
> 已将 `archive.ubuntu.com` 替换为 `mirrors.tuna.tsinghua.edu.cn`，并保留发行版代号 `jammy`。
> 成功！
```

---

## 4\. 技术架构

### 4.1 模块划分

```text
src/
├── main.rs           // 入口，CLI 参数解析（Clap）
├── config/           // 内置源列表 (const 或 json)
├── network/          // 测速模块（Reqwest、异步）
└── sources/          // 核心逻辑 trait
    ├── mod.rs        // 定义 SourceManager trait
    ├── pip.rs        // 实现 pip 的解析与写入
    ├── docker.rs     // 实现 docker 的解析与写入
    └── apt.rs        // 实现 apt 的解析与写入
```

### 4.2 核心 trait 定义

利用 Rust 的类型系统统一管理不同工具的异构逻辑：

```rust
use async_trait::async_trait;

#[async_trait]
pub trait SourceManager {
    // 标识符，例如 "pip"
    fn name(&self) -> &'static str;

    // 获取当前配置的 URL，用于 status 和 test 对比
    async fn current_url(&self) -> Result<String>;

    // 扫描所有内置源 + 当前源，返回带延迟的列表
    async fn benchmark(&self) -> Result<Vec<MirrorResult>>;

    // 应用新源 (自动处理备份)
    async fn set_source(&self, mirror_url: &str) -> Result<()>;

    // 检查是否需要 root 权限
    fn require_sudo(&self) -> bool;
}
```

### 4.3 难点处理逻辑

1.  **Docker JSON 解析**:

    - 使用 `serde_json` 读取 `/etc/docker/daemon.json`。
    - 如果文件不存在，创建新结构。
    - 如果存在，仅修改 `registry-mirrors` 字段，**保留**其他配置（如 `insecure-registries`, `log-driver` 等）。

2.  **APT 源智能替换**:

    - APT 文件通常包含 `deb http://archive.ubuntu.com/ubuntu/ jammy main`。
    - **不能**简单覆盖文件，否则会导致系统版本错乱。
    - **算法**: 使用正则读取当前文件的域名部分，将其替换为镜像源域名，保留后面的 `jammy main restricted` 等参数。

## 5\. 开发路线图

- **P0（最小可用版本）**:
  - 完成 CLI 框架。
  - 实现 `pip` 和 `npm` 的 `status`、`test`、`use`，因无需 root，风险最低，易于验证。
  - 实现核心 HTTP 测速模块。
- **P1（系统工具）**:
  - 实现 `docker` 和 `apt` 支持。
  - 增加 `sudo` 权限检查机制。
  - 实现配置文件的自动备份功能。
- **P2（体验完善）**:
  - 增加 `--fastest` 自动化参数。
  - 增加终端交互式选择列表（使用 `dialoguer` 库）。

---
