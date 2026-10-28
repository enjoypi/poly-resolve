import { GlobalRegistrator } from "@happy-dom/global-registrator";
import { expect, test } from "bun:test";
import spec from "../../api/openapi.json";
import { connect } from "../src/client";
import { renderHealth } from "../src/health";

if (!GlobalRegistrator.isRegistered) GlobalRegistrator.register();

const [path = ""] = Object.keys(spec.paths);

function stub(status: number, body: unknown) {
  const calls: [string, string][] = [];
  const fetch = async (request: Request) => {
    calls.push([request.method, new URL(request.url).pathname]);
    return Response.json(body, { status });
  };
  return { calls, client: connect("http://localhost", fetch) };
}

test("成功时 DOM 显示 status，请求路径取自 spec", async () => {
  const { calls, client } = stub(200, { status: "ok" });
  const root = document.createElement("div");
  await renderHealth(client, root);
  expect(calls).toEqual([["POST", path]]);
  expect(root.textContent).toBe("ok");
});

test("请求无效时 DOM 显示错误 code", async () => {
  const { client } = stub(400, { code: "bad_request" });
  const root = document.createElement("div");
  await renderHealth(client, root);
  expect(root.textContent).toBe("bad_request");
});
