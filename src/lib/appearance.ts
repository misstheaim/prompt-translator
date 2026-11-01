import type { Accent } from "../bindings/Accent";
import type { Theme } from "../bindings/Theme";

/**
 * Applies the theme and accent to the document (T67). `system` removes `data-theme`, so the
 * `prefers-color-scheme` media query in `index.css` keeps following Windows.
 */
export function applyAppearance(theme: Theme, accent: Accent): void {
  const root = document.documentElement;
  if (theme === "system") root.removeAttribute("data-theme");
  else root.setAttribute("data-theme", theme);
  root.setAttribute("data-accent", accent);
  if (accent === "rainbow") startRainbow();
  else stopRainbow();
}

/** Rainbow (#199): the inline custom property that `index.css` builds each accent from. */
const HUE_PROPERTY = "--accent-hue";

let observer: MutationObserver | null = null;

function paint(el: Element): void {
  if (el instanceof HTMLElement || el instanceof SVGElement) {
    el.style.setProperty(HUE_PROPERTY, String(Math.floor(Math.random() * 360)));
  }
}

function paintTree(root: Element): void {
  paint(root);
  for (const el of root.querySelectorAll("*")) paint(el);
}

/** Gives every element, and every element added later, its own random accent hue. */
function startRainbow(): void {
  if (observer) return;
  paintTree(document.body);
  observer = new MutationObserver((mutations) => {
    for (const m of mutations) {
      for (const node of m.addedNodes) {
        if (node instanceof Element) paintTree(node);
      }
    }
  });
  observer.observe(document.body, { childList: true, subtree: true });
}

/** Stops painting and removes the hues, so the single accent of `index.css` applies again. */
function stopRainbow(): void {
  if (!observer) return;
  observer.disconnect();
  observer = null;
  for (const el of document.querySelectorAll<HTMLElement | SVGElement>("[style]")) {
    el.style.removeProperty(HUE_PROPERTY);
  }
}
