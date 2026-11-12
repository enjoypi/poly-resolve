import { connect } from "./client";
import { FAILED_HASH, OUTBOUND_HASH, PROMPTS_HASH, TEXT, TOPIC_HASH, TOPICS_HASH } from "./config";
import { renderFailed } from "./failed";
import { renderMessages } from "./messages";
import { renderOutbound } from "./outbound";
import { renderPrompts } from "./prompts";
import { renderTopic } from "./topic";
import { renderTopics } from "./topics";

function link(root: HTMLElement, hash: string, label: string): void {
  const anchor = document.createElement("a");
  anchor.className = "back";
  anchor.href = hash;
  anchor.textContent = label;
  root.querySelector(".bar")?.append(anchor);
}

const root = document.getElementById("app");
if (root) {
  const client = connect(location.origin);
  if (location.hash === OUTBOUND_HASH) {
    await renderOutbound(client, root);
    link(root, FAILED_HASH, TEXT.failed);
  } else if (location.hash === FAILED_HASH) {
    await renderFailed(client, root);
  } else if (location.hash === PROMPTS_HASH) {
    await renderPrompts(client, root);
  } else if (location.hash === TOPICS_HASH) {
    await renderTopics(client, root);
  } else if (location.hash.startsWith(TOPIC_HASH)) {
    await renderTopic(client, root, decodeURIComponent(location.hash.slice(TOPIC_HASH.length)));
  } else {
    await renderMessages(client, root);
    link(root, TOPICS_HASH, TEXT.topics);
    link(root, OUTBOUND_HASH, TEXT.outbound);
    link(root, PROMPTS_HASH, TEXT.prompts);
  }
  addEventListener("hashchange", () => location.reload());
}
