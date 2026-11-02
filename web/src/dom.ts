export function area(tag: string, className: string, ...children: (Node | string)[]): HTMLElement {
  const element = document.createElement(tag);
  element.className = className;
  element.append(...children);
  return element;
}

export function button(label: string, className: string, click: () => void): HTMLButtonElement {
  const element = area("button", className, label) as HTMLButtonElement;
  element.type = "button";
  element.onclick = click;
  return element;
}
