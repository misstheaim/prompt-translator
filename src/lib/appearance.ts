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
}
