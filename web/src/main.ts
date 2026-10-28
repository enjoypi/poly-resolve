import { connect } from "./client";
import { renderMessages } from "./messages";

const root = document.getElementById("app");
if (root) {
  await renderMessages(connect(location.origin), root);
}
