import { COPY_FEEDBACK_MS, TEXT } from "./config";

async function write(text: string): Promise<boolean> {
  try {
    await navigator.clipboard.writeText(text);
    return true;
  } catch {
    return false;
  }
}

export function copyButton(text: string, delay: number = COPY_FEEDBACK_MS): HTMLButtonElement {
  const button = document.createElement("button");
  button.type = "button";
  button.className = "copy";
  button.textContent = TEXT.copy;
  button.onclick = async (event) => {
    event.stopPropagation();
    button.textContent = (await write(text)) ? TEXT.copied : TEXT.copyFailed;
    setTimeout(() => {
      button.textContent = TEXT.copy;
    }, delay);
  };
  return button;
}
