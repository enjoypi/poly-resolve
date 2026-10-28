export function anchor(label: string, address: string): Node {
  const url = target(address);
  if (!url) return document.createTextNode(`[${label}](${address})`);
  const link = document.createElement("a");
  link.href = url.href;
  link.target = "_blank";
  link.rel = "noopener noreferrer";
  link.title = `${url.host}${url.pathname}`;
  link.textContent = label || url.host;
  return link;
}

export function target(address: string): URL | undefined {
  try {
    return new URL(address);
  } catch {
    return undefined;
  }
}
