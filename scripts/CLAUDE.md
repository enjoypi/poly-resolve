# scripts

## 通用

- Bun + TypeScript，scripts/ 内无 package.json；只用 Bun 内置 API 与 `node:` 标准库；前端包在 web/（见 web/CLAUDE.md）
- 从仓库根运行（`bun scripts/<x>.ts`），路径相对根
- 子进程用 Bun `$`，失败后需继续时用 `.nothrow()`；报错与提示用中文
- 脚本不进覆盖率门禁
- `replace` 不用替换串（`$\`` 等会被展开），改用函数或 `split`/`join`

## check.ts

- 并发跑 `lints.ts --check`、`forbid.ts`、`cargo fmt --all --check`、`cov.ts`、web（`bun install --frozen-lockfile`、`bun x tsc --noEmit`、`bun test`、`bun run build`）
- 成功步骤只打印 `ok 名称`（lints、cov 附末行摘要），失败步骤整段打印输出并滤掉 clippy-driver 超长命令行，任一步失败即非 0 退出
- 非 TTY 时 `CARGO_TERM_COLOR=never`

## lints.ts

- 从 `clippy-driver rustc -W help` 解析 lint 与组名，删除失效名，缺失名补 deny
- rustc lint 只保留 nightly 与 stable 都认识的，否则 release 报 unknown_lints
- 解析出的 lint 数量过少时报错，防止解析失效
- 按“对象值、allow 或 forbid、deny”分块，块内按 priority 再按名字码位排序（不用 `localeCompare`）
- `--check` 只比对不写盘

## forbid.ts

- 用 `git ls-files -z` 列出跟踪与未忽略的 `*.rs`、`Cargo.toml`、`rust-toolchain.toml`、`.cargo/config.toml`、`scripts/*.ts`（排除 forbid.ts 自身），出现属性内的 `feature(`（含 `cfg_attr`）、`coverage(off)`、`coverage_nightly`、`cargo-features`、`[unstable]`（容空白与 `[unstable.x]`）、`-Z` 即失败
- 读文件只放过 ENOENT（已跟踪但工作区已删），其他错误抛出；扫描清单为空也失败

## gen.ts

- `cargo run -q -p cli -- spec` 写 `api/openapi.json`，再在 web/ 跑 `bun gen.ts` 写 `src/api.gen.ts`

## llvm-cov.ts

- cov.ts 与 build.ts 共用的模块（导出 `bin`、`test(extra, quiet)`）；两者须用同一套编译参数与 stamp，中间产物才共用，改动只改这里
- `test()` 先 `sync()` 再跑 `cargo llvm-cov test`，用 `.nothrow()` 并原样透传退出码，避免 Bun 的 ShellError 把输出再打印一遍
- cov、build 共用 `target/llvm-cov-target`，勿并发运行（会互删 profraw，只导致误报失败）
- 一次编译完成 clippy、test、cov：`env` 设 `RUSTC_WORKSPACE_WRAPPER=clippy-driver`，工作区 crate 编译时顺带 lint，lint 违规即编译失败；不要拆成 clippy 与 cov 两次编译
- `args` 含 `--no-cfg-coverage --no-cfg-coverage-nightly`，插桩代码与 release 一致
- 保留 `CARGO_INCREMENTAL=0`（开增量不更快）
- stamp 哈希须覆盖所有影响编译或 lint 的输入，不用 `cargo config get`（需 `-Z`）：
  - 工具链与 cargo-llvm-cov 版本、`Cargo.toml`、`Cargo.lock`、`cargo metadata`
  - cwd 各级祖先目录、各成员目录、`CLIPPY_CONF_DIR` 下的 `.cargo/config(.toml)`、`clippy.toml`、`.clippy.toml`、`rust-toolchain(.toml)`，以及 `$CARGO_HOME/config(.toml)`；`clippy.toml` 改动不触发重编，不进 stamp 就不会重新 lint
  - 名字匹配 `RUSTFLAGS$`、`RUSTC_WRAPPER`、`CLIPPY_CONF_DIR`、`CARGO_PROFILE_*` 的环境变量；用白名单，前缀匹配会被 `CARGO_TERM_COLOR` 等无关变量搅得 stamp 抖动
- stamp 未变只删 profraw；变了用 `cargo llvm-cov clean` 全清，因为改名 crate 与同名 crate 的旧哈希产物都会混入报告

## build.ts

- 为 `/serve` 编译，与 cov.ts 共用中间产物：`cargo llvm-cov test --no-report -- --list`（quiet），`--list` 只列出测试不执行；`--no-run` 已弃用且 test 子命令不支持，`--no-report` 不能与 `--no-clean` 并用但不清旧产物
- 产物 `bin` 带插桩；运行时须设 `LLVM_PROFILE_FILE` 指向 `.debug/`，否则 profraw 落到当前目录，落进 `target/llvm-cov-target` 则会混入覆盖率
- 同样经 clippy，lint 违规即编译失败

## cov.ts

- 始终 `--no-clean`
- 门禁策略见根 CLAUDE.md；输出：branch 未满只打印真或假计数为 0 的 `文件:行:列`，不看 region；满了再打印 region 缺失行（提示与报告同走 stderr，顺序一致）
