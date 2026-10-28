import { $ } from "bun";
import { unlink } from "node:fs/promises";
import { homedir } from "node:os";
import { dirname, join } from "node:path";

const env = { ...process.env, CARGO_INCREMENTAL: "0", RUSTC_WORKSPACE_WRAPPER: "clippy-driver" };
const args = ["--workspace", "--branch", "--no-cfg-coverage", "--no-cfg-coverage-nightly"];
export const bin = "target/llvm-cov-target/debug/cli";

const stamp = Bun.file("target/llvm-cov-stamp.txt");
const optional = (path: string) => Bun.file(path).text().catch(() => "");
const configs = [".cargo/config", ".cargo/config.toml", "clippy.toml", ".clippy.toml", "rust-toolchain", "rust-toolchain.toml"];
const watched = /RUSTFLAGS$|^(RUSTC_WRAPPER|CLIPPY_CONF_DIR|CARGO_PROFILE_.+)$/;
const ancestors = (dir: string): string[] => (dir === dirname(dir) ? [dir] : [dir, ...ancestors(dirname(dir))]);

async function sync() {
  const metadata = await $`cargo metadata --no-deps --format-version 1`.text();
  const members = JSON.parse(metadata).packages.map((pkg: { manifest_path: string }) => dirname(pkg.manifest_path));
  const cargoHome = process.env.CARGO_HOME ?? `${homedir()}/.cargo`;
  const dirs = new Set([...ancestors(process.cwd()), ...members, process.env.CLIPPY_CONF_DIR ?? "."]);
  const files = [
    ...[...dirs].flatMap((dir) => configs.map((name) => join(dir, name))),
    join(cargoHome, "config"),
    join(cargoHome, "config.toml"),
  ];
  const vars = Object.entries(process.env)
    .filter(([name]) => watched.test(name))
    .map(([name, value]) => `${name}=${value}`)
    .sort();
  const inputs = await Promise.all([
    $`rustc -vV`.text(),
    $`cargo llvm-cov --version`.text(),
    Bun.file("Cargo.toml").text(),
    Bun.file("Cargo.lock").text(),
    ...files.map(optional),
    metadata,
    vars.join("\n"),
  ]);
  const key = Bun.hash(inputs.join("\0")).toString();
  if ((await stamp.exists()) && (await stamp.text()) === key) {
    const profraw = Array.from(new Bun.Glob("target/llvm-cov-target/*.profraw").scanSync());
    await Promise.all(profraw.map((f) => unlink(f)));
  } else {
    await $`cargo llvm-cov clean`;
    await Bun.write(stamp, key);
  }
}

export async function test(extra: string[], quiet: boolean) {
  await sync();
  const run = $`cargo llvm-cov test ${args} ${extra}`.env(env).nothrow();
  const done = await (quiet ? run.quiet() : run);
  if (done.exitCode === 0) return;
  if (quiet) process.stderr.write(done.stderr);
  process.exit(done.exitCode);
}
