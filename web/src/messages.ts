import { Board } from "./board";
import type { connect } from "./client";

export function renderMessages(
  client: ReturnType<typeof connect>,
  root: HTMLElement,
  timeZone?: string,
): Promise<void> {
  return new Board(client, root, timeZone).start();
}
