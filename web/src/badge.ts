const BASIS = 0x811c9dc5;
const CELLS = 5;
const DENSITY = 6;
const HALF = 3;
const HUES = 360;
const MASK = 0x7fff;
const PRIME = 0x01000193;
const SHIFT = 15;

const cache = new Map<string, string>();

function hash(text: string): number {
  let value = BASIS;
  for (let index = 0; index < text.length; index++) {
    value = Math.imul(value ^ text.charCodeAt(index), PRIME) >>> 0;
  }
  return value;
}

function mix(value: number): number {
  let mixed = value ^ (value >>> 16);
  mixed = Math.imul(mixed, 0x85ebca6b) >>> 0;
  mixed ^= mixed >>> 13;
  mixed = Math.imul(mixed, 0xc2b2ae35) >>> 0;
  return (mixed ^ (mixed >>> 16)) >>> 0;
}

function pattern(seed: number): number {
  const bits = seed & MASK;
  let lit = 0;
  for (let index = 0; index < CELLS * HALF; index++) lit += (bits >>> index) & 1;
  return lit < DENSITY ? ~bits & MASK : bits;
}

function image(id: string): string {
  const seed = mix(hash(id));
  const bits = pattern(seed);
  const color = `hsl(${(seed >>> SHIFT) % HUES},60%,65%)`;
  let path = "";
  for (let row = 0; row < CELLS; row++) {
    for (let column = 0; column < HALF; column++) {
      if ((bits >>> (row * HALF + column)) & 1) {
        const mirror = CELLS - 1 - column;
        path += `M${column} ${row}h1v1h-1z`;
        if (mirror !== column) path += `M${mirror} ${row}h1v1h-1z`;
      }
    }
  }
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 ${CELLS} ${CELLS}" shape-rendering="crispEdges"><path fill="${color}" d="${path}"/></svg>`;
  const encoded = encodeURIComponent(svg).replace(/\(/g, "%28").replace(/\)/g, "%29");
  return `url("data:image/svg+xml,${encoded}")`;
}

export function badge(id: string): HTMLElement {
  const element = document.createElement("span");
  element.className = "badge";
  element.setAttribute("role", "img");
  element.setAttribute("aria-label", id);
  let drawn = cache.get(id);
  if (drawn === undefined) {
    drawn = image(id);
    cache.set(id, drawn);
  }
  element.style.backgroundImage = drawn;
  return element;
}
