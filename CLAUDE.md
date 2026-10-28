# Harness of Rust

## 文档

- 使用简体中文、全角标点，不用繁体中文、日文、韩文；文件名用英文
- 文档与沟通精简无歧义、人工可维护；同一内容只写一处
- 分工：README.md 只写使用方法；docs/ 写业务需求，给人看，不含技术细节；给 AI 的规则写 CLAUDE.md
- 根 CLAUDE.md 只放全仓通用规则；只涉及某目录的规则放该目录的 CLAUDE.md（cli/、domain/、scripts/、web/）；单个 CLAUDE.md 少于 256 行
- 可复用的规则、原则、坑写进对应 CLAUDE.md，并与既有条目去重合并；一次性规则丢弃；不记入 TODO.md 或 MEMORY.md
- 新建文档（.md .rst .adoc .txt 等）先征得用户同意；TODO.md 与 Plan 文件除外
- TODO.md 是任务清单：完成一项删一项，清空后删除文件

## 协作

- 给用户的选项带数字编号，第 1 项为推荐或 All
- Plan 用 subagent 从至少 3 个角度（含 1 个批判角度）推敲；定稿后把 `~/.claude/plans/<name>.md` 移到项目根目录，无需确认；Plan 完成后删除
- 批量改动用脚本或工具完成；脚本用 Bun + TypeScript
- 不自行新建 git branch 或 worktree
- 多轮改动合并为一个 commit；message 单行、不多于 64 个 UTF-8 字符、不含 Co-Authored-By 或 Claude 签名
- commit 格式 `[TASK-ID ]type: 业务描述`；type ∈ build/chore/ci/docs/feat/fix/perf/refactor/revert/style/test；TASK-ID 用用户提供的，否则沿用最近一次 commit 的，都没有则连同空格省略

## 设计

- SOLID：一个类、模块、函数只负责一项功能；针对接口编程；拆分胖接口；用 Interface/Trait 而非继承
- DRY：同一逻辑只保留一份，抽取为函数或模块
- YAGNI：只写当前需要的功能；依次优先复用仓内代码、stdlib、平台原生能力、已装依赖，都不行才写能跑通的最小实现
- 代码文件（含测试）不超过 512 行，函数不超过 64 行，超限即拆分
- 不写注释或 docstring，见到即删；lint 要求的 reason 字符串（如 `#[must_use = "…"]`）不算注释；本条优先于插件的注释约定，`shortcut:` 等标记注释也不写
- 新配置文件用 yaml；默认值、数值常量、业务字符串常量从配置读取
- secret 只经环境变量加载，不设默认值，写法 `db_password: "${DB_PASSWORD}"`；值存于 sops 加密的 `secrets.enc.yaml`（扁平 YAML，键即变量名），运行时 `sops exec-env secrets.enc.yaml '<命令>'` 注入；AI 不运行 `sops -d`、不读取明文、不生成或提交该文件
- 存储与日志时间用 UTC；面向人展示时标注时区（如 2026-07-17 09:20:09 UTC+8）并支持 TZ 环境变量
- 输出 debug 级结构化日志供 AI 分析，不含 secret

## 测试与覆盖率

- 测试先行：先写失败的测试再实现
- 测试与业务代码分文件
- 日常检查只跑 `bun scripts/check.ts`，不单独 build、test、clippy、tsc
- 覆盖率门禁：branches、regions 均须 covered == count，不看 percent，count 为 0 视为失败；不足时先按缺失 branch 补测试，branch 满了再按缺失行补 region
- 依赖不插桩（cargo-llvm-cov 默认）；禁止 `#[coverage(off)]`，禁止靠排除文件凑 100%
- 泛型函数与 `#[inline]` 函数按实例（含跨 crate）各插一份桩，llvm-cov 取单个实例最大值而非并集，故每个实例都要测全；pub API 不用泛型、不加 `#[inline]`
- 计入方式：`if`、`&&`、`||` 计 branch；`?` 的错误返回、`match` 每个分支、每个闭包体各计 1 个 region；derive、`tracing` 宏、`#[tokio::main]` 不产生 region
- 每个能失败的 `?` 都要有一条让它失败的测试
- 不可能失败的调用不写 `?` 或 `map_err` 闭包，改用 `unwrap_or_default()` 等无分支写法
- 组合子（`.map(f)`、`.and_then(f)`）只在写法自然时用，不用来躲覆盖

## 前后端契约

- 事实源是 domain 类型：`cli spec` 生成 `api/openapi.json`，再生成 `web/src/api.gen.ts`；两者都提交，由 `bun scripts/gen.ts` 更新，禁手改

## Rust 工程

- Cargo workspace，crate 放根目录 `<name>/`，`members` 显式列出；成员的 package 字段与 `[lints]` 均继承 workspace
- 依赖版本与 features 只写在根 `[workspace.dependencies]`，版本写 `*`，由 Cargo.lock 锁定；成员与 dev-dependencies 只写 `x.workspace = true`
- 日常用不锁日期的 nightly（rust-toolchain.toml），随 `rustup update` 升级；不用 `#![feature]`、`cargo-features`、`[unstable]` 与 `-Z`
- 开发环境：rustup（nightly 随 rust-toolchain.toml 自动安装；lints 步骤对照 lint 名另需 stable：`rustup toolchain install stable`）、cargo-llvm-cov（`cargo install cargo-llvm-cov`）、Bun
- clippy.toml 的 `msrv` 取当前 stable 版本，stable 升级后同步；`incompatible_msrv` 借它拦住 stable 编不过的新 API
- `.cargo/config.toml`（cargo 只认 toml）固定 `MACOSX_DEPLOYMENT_TARGET = "11.0"`（rustc 默认值）：内置 C 库（如 rusqlite bundled）默认按本机 SDK 编译，链接时报 linker-info，被 deny 成错误
- release 只用 stable：`cargo +stable test --release --locked && cargo +stable build --release --locked`
- 开服（`/serve`）默认用 debug 版，与覆盖率共用中间产物：BUILD_CMD `bun scripts/build.ts`，BIN `target/llvm-cov-target/debug/cli`（带插桩，起服命令前加 `LLVM_PROFILE_FILE=.debug/serve-%p.profraw`），ARGS `serve --config cli/config.yaml`，READY_PATTERN `listening`，BUSINESS_GREP `"level":"(WARN|ERROR)"`，SMOKE_CMD `curl -fsS -m 3 -d "{}" http://<addr>/health/check`（`<addr>` 取 `cli/config.yaml`），日志与 PID 放 `.debug/`；启用 `triage` 时 up.ts 外包 `sops exec-env secrets.enc.yaml '<原命令>'`，未启用无需包装；启用前须存在 `.debug/codex-home`、`.debug/codex-work`（沙箱参数里的目录必须已存在）；用户明确要求时才用 release
- lib 设 `test = false`、`bench = false`、`doctest = false`；bin 设 `test = false`、`bench = false`；保证 RA 默认 `--all-targets` 检查通过
- crate 依赖单向：`cli`（传输层）→ `adapters`（I/O：子进程、存储）→ `domain`（业务）；业务代码只写在 domain，domain 不依赖 tokio
- 每个 crate 只有一个集成测试二进制 `tests/it/main.rs`，其余为其子模块，按被测模块或功能命名
- 业务代码禁 unwrap、expect；clippy.toml 只对测试放开，测试 helper 不在豁免内，须返回 `Result`

## Lint

### 配置

- 根 Cargo.toml `[workspace.lints]` 逐条列出全部 lint；组级 all、cargo、nursery、pedantic 为 deny，不设组级 `restriction`，restriction lint 逐条 deny，故 `blanket_clippy_restriction_lints` 为 deny；`unsafe_code` 为 forbid
- 改级别就地改值，不新增同名键；改后或 check 报 lint 不同步时跑 `bun scripts/lints.ts`，再修代码或按白名单补 allow
- clippy.toml 的 `disallowed-macros`、`disallowed-methods` 已列出禁用项与 reason，此处不重复；不开 `check-private-items`（测试函数会被要求写 `# Panics` 文档，与无注释规则冲突）

### allow 白名单

allow 只限下列各项，新增须归入其一并写进本节：

- 互斥（每对前者 allow）：`implicit_return`/`needless_return`、`question_mark_used`/`question_mark`、`self_named_module_files`/`mod_module_files`、`pub_with_shorthand`/`pub_without_shorthand`、`redundant_pub_crate`/`unreachable_pub`；由此结尾不写 `return`、用 `?`、模块用 `x.rs` + `x/` 不用 `mod.rs`、crate 内可见用 `pub(crate)`
- 与无注释规则冲突：`missing_docs`、`missing_docs_in_private_items`、`missing_errors_doc`
- 与其他规则冲突：`single_call_fn`（64 行拆分）、`tests_outside_test_module`（测试分文件）、`missing_trait_methods`（部分默认方法 unstable）、`missing_inline_in_public_items`（见测试与覆盖率）、`unused_crate_dependencies`（cargo test 时 bin 与集成测试误报）、`wildcard_dependencies`（版本写 `*`）

### deny 带来的写法

- `semicolon_*_block` 两条：不写 `{ …; };`
- `*separated_literal_suffix` 两条：不写字面量类型后缀，用类型标注
- `big_endian_bytes`、`little_endian_bytes`、`host_endian_bytes`、`integer_division_remainder_used`：暂全 deny，真需要时放开一条
- `pattern_type_mismatch`、`ref_patterns`：对引用 match 改用组合子
- `field_scoped_visibility_modifiers`：字段私有，加访问方法
- `non_ascii_literal`：源码字符串含测试用 ASCII
- `std_instead_of_core`、`std_instead_of_alloc`、`alloc_instead_of_core`：`core::`、`alloc::` 优先于 `std::`
- `absolute_paths`（阈值 1）：多段路径一律 `use` 导入具体项；与本地项重名时改名或用 `as` 别名
- `too_many_arguments`（阈值 3，含 `self`）、`fn_params_excessive_bools`（阈值 0）：多参数打包为 `Copy` 结构体，字段私有，`new` 不超过 3 个参数，范例 `domain::features::Call`；不传 bool 参数，用 `&io::Result<()>` 等替代
- `arbitrary_source_item_ordering`：同类条目按名字母序（const 与 static 同组，struct、enum、type 同组，函数同组），impl 紧跟其类型，字段与方法同样按字母序
- `struct_field_names`：结构体字段不全带相同前后缀（`default_limit`、`max_limit` 改为 `default`、`max`）；`similar_names`：形参名不与 `self` 等相近（`shelf` 与 `self` 相近，改 `reader`）；`unused_trait_names`：只为调方法导入的 trait 写 `as _`
- `panic_in_result_fn`：返回 `Result` 的测试 helper 不写 assert，改返回 `bool` 或 `String` 由测试函数断言；`large_stack_arrays`：测试里超过 256 字节的数组字面量拆成多次 helper 调用
- `pub_use`：crate 对外导出用 `pub mod`，不写 `pub use`；`module_name_repetitions`：类型名不重复所在模块名（如 `config::Settings`、`store::Archive`）
- `result_large_err`（阈值 32 字节）：`rusqlite::Error` 等大错误装箱 `Box<Error>`，需闭包时用 `query_row_and_then` 让闭包返回装箱错误；`future_not_send`：async fn 签名含 `dyn Future` 时加 `+ Send`
- `arithmetic_side_effects`：整数运算用 `saturating_*`、`checked_*`；`unused_results`、`let_underscore_*`：丢弃返回值用 `drop(…)` 或带类型的 `let _: T`
- `multiple_crate_versions`：Cargo.lock 里未启用的可选依赖也参与检查，jiff 的 `jiff-static` 会带入第二个 syn，故日期用 `chrono`（仅 `std` feature）
- 测试：JSON 取值用 `get`（`indexing_slicing`），数值用带类型的 const（`default_numeric_fallback`）；`unwrap_err()` 加 `to_string()` 断言代替 `is_err()`（`assertions_on_result_states`）
- `exhaustive_enums`、`missing_copy_implementations`、`trivially_copy_pass_by_ref`：pub 单元枚举加 `#[non_exhaustive]`、derive `Copy`，方法按值收 `self`；跨 crate 不 match 其变体（必需的 `_` 分支无法覆盖）
