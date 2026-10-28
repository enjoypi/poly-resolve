const MARGIN = 8;
const GAP = 4;

export type Content = (cell: HTMLElement) => Node[] | undefined;

function place(tip: HTMLElement, anchor: DOMRect): void {
  const box = tip.getBoundingClientRect();
  const left = Math.max(MARGIN, Math.min(anchor.left, window.innerWidth - box.width - MARGIN));
  const below = anchor.bottom + GAP;
  const fits = below + box.height + MARGIN <= window.innerHeight;
  const top = fits ? below : Math.max(MARGIN, anchor.top - box.height - GAP);
  tip.style.left = `${left}px`;
  tip.style.top = `${top}px`;
}

export function attachTip(
  scroller: HTMLElement,
  tip: HTMLElement,
  content: Content,
  delay: number,
): void {
  let timer: ReturnType<typeof setTimeout> | undefined;
  const hide = () => {
    clearTimeout(timer);
    timer = undefined;
    tip.hidden = true;
  };
  const reveal = (cell: HTMLElement) => {
    const nodes = content(cell);
    const clipped = cell.dataset.always !== undefined || cell.scrollWidth > cell.clientWidth;
    if (!nodes || !clipped) return;
    tip.replaceChildren(...nodes);
    tip.hidden = false;
    place(tip, cell.getBoundingClientRect());
  };
  scroller.addEventListener("pointerover", (event) => {
    hide();
    const target = event.target;
    const cell = target instanceof Element ? target.closest("td") : null;
    if (cell && (event as PointerEvent).pointerType === "mouse") {
      timer = setTimeout(() => reveal(cell), delay);
    }
  });
  scroller.addEventListener("pointerout", hide);
  scroller.addEventListener("pointerdown", hide);
  scroller.addEventListener("scroll", hide);
  scroller.addEventListener("keydown", hide);
}
