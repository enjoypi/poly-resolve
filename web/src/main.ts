import { connect } from "./client";
import { FAILED_HASH, PROMPTS_HASH, REVIEW_HASH, TEXT } from "./config";
import { renderFailed } from "./failed";
import { renderMessages } from "./messages";
import { renderPrompts } from "./prompts";
import { renderReviews } from "./reviews";

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
  if (location.hash === REVIEW_HASH) {
    await renderReviews(client, root);
    link(root, FAILED_HASH, TEXT.failed);
  } else if (location.hash === FAILED_HASH) {
    await renderFailed(client, root);
  } else if (location.hash === PROMPTS_HASH) {
    await renderPrompts(client, root);
  } else {
    await renderMessages(client, root);
    link(root, REVIEW_HASH, TEXT.reviews);
    link(root, PROMPTS_HASH, TEXT.prompts);
  }
  addEventListener("hashchange", () => location.reload());
}
