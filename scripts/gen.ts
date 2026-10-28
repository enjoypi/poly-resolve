import { $ } from "bun";

await Bun.write("api/openapi.json", await $`cargo run -q -p cli -- spec`.text());
await $`bun gen.ts`.cwd("web");
