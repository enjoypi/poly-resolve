import createClient, { type ClientOptions } from "openapi-fetch";
import type { paths } from "./api.gen";

export function connect(baseUrl: string, fetch?: ClientOptions["fetch"]) {
  return createClient<paths>({ baseUrl, fetch });
}
