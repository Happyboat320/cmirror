# 改动记录

## 新增 Hugging Face 镜像支持

- 新增 `huggingface` / `hf` 作为支持工具。
- 新增内置镜像候选项：
  - `HF-Mirror`：`https://hf-mirror.com`
  - `Official`：`https://huggingface.co`
- 新增 `HuggingFaceManager`。
  - 从 `HF_ENDPOINT` 读取当前源。
  - 通过 `hf`、`huggingface-cli` 或已有 `HF_ENDPOINT` 检测是否安装或已配置。
  - 应用镜像时输出 `export HF_ENDPOINT="..."` 命令提示。
  - 恢复配置时输出 `unset HF_ENDPOINT` 命令提示。

## 新增批量自动换源命令

- 新增命令：

```bash
cmirror auto
```

- 该命令会遍历所有支持工具。
- 每个工具的处理流程：
  - 输出当前工具名称。
  - 检查工具是否安装或是否有可检测配置。
  - 跳过未安装工具。
  - 对通常需要 sudo/root 权限的工具输出提示。
  - 对候选镜像测速。
  - 应用最快可用镜像。
  - 单个工具失败时继续处理下一个工具。
- 最后输出已应用、已跳过、失败数量汇总。

## 公共基础能力

- 为 `SourceManager` 增加方法：

```rust
async fn is_installed(&self) -> bool;
```

- 新增 `utils::command_exists()`，用于检测 `PATH` 中是否存在可执行文件。
- 为所有已有管理器实现安装检测：
  - `pip`
  - `npm`
  - `docker`
  - `go`
  - `cargo`
  - `brew`
  - `apt`
  - `uv`
  - `conda`

## 文档

- 新增 `plan.md`，记录实现计划。
- 更新 `README.md`，补充 Hugging Face 支持。
- 更新 `README.md`，补充 `cmirror auto` 说明。

## 验证

- 已运行 `cargo fmt`。
- 已运行 `cargo test`。
- 结果：6 个测试全部通过。

## 追加改动：Hugging Face 候选源扩展

已删除

- 未加入 ModelScope、OpenCSG 等非 Hugging Face Hub `HF_ENDPOINT` 兼容平台，避免配置后 `huggingface_hub` 客户端不可用。

## 追加改动：apt 新版 Ubuntu 配置适配

- apt 自动识别新版 Ubuntu 配置路径：
  - 优先使用 `/etc/apt/sources.list.d/ubuntu.sources`
  - 不存在时回退到 `/etc/apt/sources.list`
- `current_url()` 新增 Deb822 格式解析，支持读取 `URIs: http://...`。
- `set_source()` 新增 Deb822 格式写入，行为与传统格式一致：先读取第一条 `URIs:`，只替换文件中与第一条 URL 完全相同的源，并保留其他 `URIs:` 和 `Types`、`Suites`、`Components`、`Signed-By` 等字段。
- 传统 `sources.list` 格式仍保留原有行为。
- 新增 `ubuntu.sources` 单元测试，覆盖读取、写入和恢复。

## 追加验证

- 已运行 `cargo fmt`。
- 已运行 `cargo test`。
- 结果：7 个测试全部通过。

## 追加改动：状态和操作前安装检测

- `status` 不再把未读到配置统一显示为 `Default`。
- 已安装但未检测到当前源时显示：
  - 当前源地址：`Not Detected`
  - 状态：`[Not Detected]`
- 未安装或没有可检测配置时显示：
  - 当前源地址：`Not Installed`
  - 状态：`[Not Installed]`
- `use` 执行前先调用 `is_installed()`，未安装时直接报错并跳过写入。
- `restore` 执行前先调用 `is_installed()`，未安装时直接报错并跳过恢复。
