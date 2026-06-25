# 实现计划

## 目标

1. 新增 Hugging Face 镜像支持，并复用现有工具的 `status`、`test`、`use`、`restore` 命令流程。
2. 新增一个批量自动换源命令：扫描所有已支持工具，未安装则跳过，已安装则测速并应用最快镜像，同时输出每个工具的处理进度。
3. 保持现有代码风格：每个工具独立管理器，通过 `SourceManager` 统一抽象，在不直观的逻辑处添加简洁注释。

## 设计

- 为 `SourceManager` 增加 `is_installed()` 方法。
  - 各管理器可按工具特点实现检测逻辑。
  - 文件型系统工具可检查配置路径或可执行文件。
  - 环境变量型工具可检查命令行工具或相关环境变量。
- 在 `src/sources/` 下新增 `huggingface` 模块。
  - 候选镜像从 `assets/mirrors.json` 加载。
  - 当前源从 `HF_ENDPOINT` 读取。
  - 应用和恢复时打印命令行外壳命令，因为子进程无法直接修改父级命令行外壳环境变量。
- 新增 `auto` 命令。
  - 遍历 `SUPPORTED_TOOLS`。
  - 每个工具按以下流程处理：
    - 打印清晰的工具标题。
    - 通过 `is_installed()` 检测是否安装。
    - 未安装则跳过。
    - 如通常需要 root 权限，则给出提示。
    - 对候选镜像测速。
    - 如果全部超时则跳过。
    - 应用最快候选镜像。
    - 单个工具失败时继续处理下一个工具。
- 复用现有测速和备份工具函数。

## 修改文件

- `src/traits.rs`：增加 `is_installed()`。
- `src/sources/*.rs`：为已有管理器实现安装检测。
- `src/sources/huggingface.rs`：新增 Hugging Face 管理器。
- `src/sources/mod.rs`：注册 `huggingface`。
- `assets/mirrors.json`：增加 Hugging Face 镜像候选项。
- `src/main.rs`：增加 `auto` 命令和处理逻辑。
- `README.md`：补充 Hugging Face 和新命令说明。
- `change.md`：记录最终改动。

## 验证

- 运行 `cargo fmt`。
- 在环境允许 Cargo 写入依赖缓存时运行 `cargo test`。
- 必要时运行 `cargo check` 做轻量编译验证。

## 本次追加计划

### Hugging Face 增加更多镜像候选

- 在 `assets/mirrors.json` 的 `huggingface` 列表中补充更多可用候选项。
- 仍然复用现有 `HuggingFaceManager`，不增加新的写入逻辑。
- `test`、`use --fastest`、`auto` 会自动使用扩展后的候选列表。

### apt 适配新版 Ubuntu 源配置

- 新版 Ubuntu 可能使用 `/etc/apt/sources.list.d/ubuntu.sources`，而不是传统 `/etc/apt/sources.list`。
- `AptManager::config_path()` 自动选择配置文件：
  - 如果显式传入测试路径，则继续使用该路径。
  - 如果系统存在 `/etc/apt/sources.list.d/ubuntu.sources`，优先使用它。
  - 否则回退到 `/etc/apt/sources.list`。
- `current_url()` 同时支持两种格式：
  - 传统格式：解析 `deb http://...` 行。
  - Deb822 格式：解析 `URIs: http://...` 行。
- `set_source()` 根据文件内容自动判断格式：
  - 传统格式继续替换当前检测到的源 URL。
  - Deb822 格式也先读取第一条 `URIs:`，只替换文件中与第一条 URL 完全相同的源，保留其他 `URIs:` 和 `Types`、`Suites`、`Components`、`Signed-By` 等字段。
- `restore()` 继续复用现有备份恢复逻辑。
- 补充单元测试覆盖 `ubuntu.sources` 的读取、写入和恢复。
