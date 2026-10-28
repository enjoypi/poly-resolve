import type { connect } from "./client";

export async function renderHealth(client: ReturnType<typeof connect>, root: HTMLElement) {
  const { data, error } = await client.POST("/health/check", { body: {} });
  root.textContent = data ? data.status : (error?.code ?? "");
}
