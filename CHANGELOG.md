# Changelog
English | 中文

All notable changes to this project will be documented in this file.

The format is based on Keep a Changelog, and the project follows Semantic Versioning where practical.

Note:
- 以下内容按 `git` 提交记录和改动文件整理。
- 同一 `Cargo.toml` 版本跨多个日期时，使用 `-1`、`-2` 这类后缀区分时间点。
- 每条只记录提交里能直接看到的事实，不依赖 `README.md` 或 `AGENTS.md` 的补充描述。

## [0.2.8] - 2026-06-26
Commit: `pending`

### Added
- 新增接入概览规则摘要接口，后端可直接返回设备类型与日志类型聚合结果

### Changed
- `Cargo.toml`: version `0.2.7` -> `0.2.8`
- `Cargo.toml`: `zip` 依赖从 `version = "2"` 调整为 `8.6.0`
- `Dockerfile`: 二进制复制改为 `COPY --chown=appuser:appgroup`，去掉额外的 `/app` 递归 `chown -R`
- 接入概览页面改为直接读取后端规则摘要，避免前端逐页枚举 WPL 并逐个加载 `parse.wpl`，显著减少页面打开耗时
- WPL 摘要提取逻辑增强：支持 `#[copy_raw(...), tag(...)]` 等混合注解顺序，并优先使用 `dev_name` 作为设备展示名
- 调试页布局收敛：JSON 模式切换时不再改变整体布局，日志输入框改为固定高度并使用内部滚动
- 补充接入概览规则摘要接口与 WPL 提取逻辑测试

## [0.2.7] - 2026-06-22
Commit: `1d71d47`

### Changed
- `Cargo.toml`: version `0.2.6` -> `0.2.7`
- `Cargo.toml`: `wp_oml` / `wp-proj` / `wp_data_utils` tag `v1.22.4` -> `v1.23.2`
- `Cargo.toml`: `wp-knowledge "0.13"` -> `~0.14`，`wp-model-core "0.8.7"` -> `~0.8`，`wp-parse-api "0.10"` -> `~0.10`
- `Cargo.toml`: `wpl` / `wp_primitives` 依赖写法调整为带 `package` 和 `~` 约束的形式
- 代码改动集中在 `src/server/debug.rs`、`src/server/release.rs`、`src/utils/knowledge.rs`、`src/utils/sandbox.rs`
- 前端改动集中在 `web/src/services/config.js`、`web/src/services/release.js`、`web/src/views/pages/system-release/detail.jsx`

## [0.2.6-2] - 2026-06-17
Commit: `e72a713`

### Changed
- `Cargo.toml`: `sea-orm` 增加 `sqlx-sqlite` feature
- 新增接入概览能力的首版后端实现，并接通前端入口与页面展示
- 调整系统设置与数据库初始化相关实现，适配新的运行方式并同步补充测试

## [0.2.6-1] - 2026-06-16
Commit: `72bc910`

### Changed
- `Cargo.toml`: version `0.2.5` -> `0.2.6`

## [0.2.5] - 2026-05-31
Commit: `809dd7f`

### Changed
- `Cargo.toml`: version `0.2.4` -> `0.2.5`

## [0.2.4-2] - 2026-05-31
Commit: `8f303fb`

### Added
- `Cargo.toml`: 新增 `futures-util`、`tempfile`、`tar`、`flate2`、`zip`
- 新增一组默认配置模板文件，覆盖 `default_configs/data/out_dat/*` 与 `default_configs/runtime/*`

### Changed
- 项目导入导出与 Git 冲突处理链路同步更新

## [0.2.4-1] - 2026-05-28
Commit: `eff9c65`

### Changed
- `Cargo.toml`: version `0.2.3` -> `0.2.4`
- `Cargo.toml`: `config "0.15.22"` -> `0.15.23`
- `Cargo.toml`: 新增 `orion-error = "0.8.1"`
- `Cargo.toml`: `wp_oml` / `wp-proj` / `wp_data_utils` tag `v1.22.2` -> `v1.22.4`

## [0.2.3] - 2026-05-19
Commit: `ef70716`

### Added
- 新增配置模板接口与模板扫描能力
- 新增一组默认 connector 模板，覆盖 source / sink 的 DMDB 与计数类配置场景

### Changed
- `Cargo.toml`: version `0.2.2` -> `0.2.3`
- `Cargo.toml`: `wp_oml` / `wp-proj` / `wp_data_utils` tag `v1.22.1` -> `v1.22.2`
- 配置管理页接入模板能力

## [0.2.2] - 2026-05-14
Commit: `149e3e2`

### Changed
- `Cargo.toml`: version `0.2.1` -> `0.2.2`
- `Cargo.toml`: `cargo_metadata "0.20"` -> `0.23.1`
- `Cargo.toml`: `tokio "1.50"` -> `1.52.3`
- `Cargo.toml`: `wp_oml` / `wp-proj` / `wp_data_utils` tag `v1.22.0` -> `v1.22.1`
- `Cargo.toml`: `wp-model-core "0.8"` -> `0.8.7`
- `Cargo.toml`: `nix "0.28"` -> `0.30.1`

## [0.2.1] - 2026-05-12
Commit: `8606a10`

### Changed
- `Cargo.toml`: version `0.2.0` -> `0.2.1`
- `Cargo.toml`: `wp_oml` / `wp-proj` / `wp_data_utils` tag `v1.21.11` -> `v1.22.0`
- `src/server/sandbox_runner.rs`、`src/utils/sandbox.rs` 重点改动
- 多个测试文件同步修改，包括 `tests/api/assist_test.rs`、`tests/api/config_test.rs`、`tests/api/debug_test.rs`、`tests/api/release_test.rs`

## [0.2.0] - 2026-05-08
Commit: `02f1cab`

### Changed
- `Cargo.toml`: version `0.1.8` -> `0.2.0`
- `config/config.toml`、`src/server/app.rs`、`src/server/setting.rs`、`src/utils/warparse_service.rs` 同步改动
- `src/server/setting.rs`: `WarparseConf` 新增 `enabled: bool`
- `src/utils/warparse_service.rs`: `WarpParseService` 新增 `scheme` 字段，设备访问地址改为按 `http/https` 组装

## [0.1.8-3] - 2026-05-08
Commit: `f17361b`

### Changed
- `Cargo.toml`: `rust-embed "6.8"` -> `8.11.0`
- `Cargo.toml`: `config "0.14"` -> `0.15.22`
- `Cargo.toml`: `toml "0.8"` -> `1.1.2`
- `Cargo.toml`: `thiserror "1.0"` -> `2.0.18`
- `Cargo.toml`: `strum "0.26"` -> `0.28.0`
- `Cargo.toml`: `bcrypt "0.15"` -> `0.19.0`
- `Cargo.toml`: `rand "0.8"` -> `0.10.1`
- `Cargo.toml`: `which "6.0"` -> `8.0.2`
- `src/server/assist_task.rs`、`src/server/sandbox.rs`、`src/server/user.rs` 同步适配新的 `rand` API

## [0.1.8-2] - 2026-05-05
Commit: `3eb5c7d`

### Changed
- `Cargo.toml`: 增加 `#@gxl:set(version)` 标记
- `Cargo.toml`: `wp_oml` / `wp-proj` / `wp_data_utils` tag `v1.21.7` -> `v1.21.11`
- `Cargo.toml`: `wp-knowledge "0.11"` -> `0.13`
- `Cargo.toml`: `wp-parse-api "0.8"` -> `0.10`
- `Cargo.toml`: `wpl "0.1"` -> `0.3`
- `src/utils/knowledge.rs` 改用 `anyhow::Result` 与 `wp_knowledge::mem::RowData`
- 新增 `version.txt`

## [0.1.8-1] - 2026-04-30
Commit: `c5cdb04`

### Added
- 新增 `crates/migrations/src/m20260428_000002_add_release_group.rs`
- 新增 `src/db/release_group.rs`
- 新增多组 `docker/station/default_configs/connectors/sink.d/*`
- 新增 `docker/station/default_configs/connectors/source.d/40-mysql.toml`

### Changed
- `Cargo.toml`: version `0.1.7` -> `0.1.8`
- `src/server/release.rs`、`src/server/release_task_runner.rs`、`src/server/sync.rs`、`src/utils/project.rs` 大幅改动
- `web/src/services/release.js`、`web/src/views/pages/system-release/detail.jsx`、`web/src/views/pages/system-release/index.jsx` 大幅改动

## [0.1.7] - 2026-04-28
Commit: `6b094e1`

### Changed
- `Cargo.toml`: version `1.1.0` -> `0.1.7`
- `src/utils/constants.rs` -> `src/utils/common.rs`
- `src/utils/check.rs` -> `src/utils/project_check.rs`
- `src/utils/sandbox_workspace.rs` -> `src/utils/sandbox.rs`
- `src/utils/process_guard.rs` 被删除

## [1.1.0-6] - 2026-04-23
Commit: `6ef65ce`

### Added
- 新增 `tasks/2026-04-15_1_performance-test-page-design.md`

### Changed
- `Cargo.toml`: `wp_oml` / `wp-proj` / `wp_data_utils` tag `v1.20.3` -> `v1.20.6`
- `src/server/debug.rs`、`src/utils/knowledge.rs`、`src/utils/sandbox_workspace.rs`、`web/src/views/pages/simulate-debug/index.jsx` 同步改动

## [1.1.0-5] - 2026-04-22
Commit: `de3d57d`

### Changed
- `Cargo.toml`: `wp_oml` / `wp-proj` / `wp_data_utils` tag `v1.20.0` -> `v1.20.3`
- `src/db/default_rules_loader.rs` 大幅改动，默认配置初始化从“仅嵌入资源”改为“优先运行时目录，缺失时回退嵌入资源”

## [1.1.0-4] - 2026-04-17
Commit: `62ba8a2`

### Changed
- `src/server/device.rs`、`src/utils/warparse_service.rs` 同步改动
- `web/src/services/config.js`、`web/src/views/pages/config-manage/index.jsx`、`web/src/views/pages/system-manage/ConnectionManage.jsx` 同步改动

## [1.1.0-3] - 2026-04-16
Commit: `40d6152`

### Changed
- `AGENTS.md`、`README.md`、`crates/migrations/README.md` 同步改动
- 删除 `crates/migrations/src/entity/knowledge_config.rs`
- 删除 `crates/migrations/src/entity/rule_config.rs`
- 删除 `src/db/knowledge_config.rs`
- 删除 `src/db/rule_config.rs`
- 新增 `src/db/rule_type.rs`
- `src/server/config.rs`、`src/server/rules.rs`、`src/utils/project.rs` 大幅改动

## [1.1.0-2] - 2026-04-13
Commit: `c2340db`

### Added
- 新增 `src/api/project.rs`
- 新增 `src/server/project.rs`
- 新增 `web/src/services/features.js`
- 新增 `web/src/services/project.js`

### Changed
- `src/server/assist_task.rs`、`src/utils/project.rs`、`web/src/views/pages/rule-manage/index.jsx`、`web/src/views/pages/system-manage/index.jsx` 同步改动

## [1.1.0-1] - 2026-04-07
Commit: `de713f8`

### Added
- 首次提交 `Cargo.toml`、`Cargo.lock`、`Dockerfile`、`build.rs`、`config/config.toml`
- 首次提交 `crates/gitea/*`、`crates/migrations/*`
- 首次提交 `default_configs/*`
- 首次提交 `src/api/*`、`src/db/*`、`src/server/*`、`src/utils/*`
- 首次提交 `tests/*`
- 首次提交 `web/src/*`、`web/public/*`、`web/package.json`、`web/package-lock.json`
- 首次提交 `.github/workflows/*`
