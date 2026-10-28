import openapiTS, { astToString } from "openapi-typescript";

const spec = new URL("../api/openapi.json", import.meta.url);

export async function generate(): Promise<string> {
  const text = astToString(await openapiTS(spec));
  return text.replace(/^[ \t]*\/\*[\s\S]*?\*\/[ \t]*\n/gm, "");
}

if (import.meta.main) {
  await Bun.write(new URL("src/api.gen.ts", import.meta.url), await generate());
}
