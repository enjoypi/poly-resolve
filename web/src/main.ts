import { connect } from "./client";
import { REVIEW_HASH, TEXT } from "./config";
import { renderMessages } from "./messages";
import { renderReviews } from "./reviews";

const root = document.getElementById("app");
if (root) {
  const client = connect(location.origin);
  if (location.hash === REVIEW_HASH) {
    await renderReviews(client, root);
  } else {
    await renderMessages(client, root);
    const link = document.createElement("a");
    link.className = "back";
    link.href = REVIEW_HASH;
    link.textContent = TEXT.reviews;
    root.querySelector(".bar")?.append(link);
  }
  addEventListener("hashchange", () => location.reload());
}
