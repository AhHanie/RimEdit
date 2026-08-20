// Guards the pre-hydration fallback in `index.html`: it must give the WebView useful pixels
// before Vite/React ever load, so this asserts against the
// raw file contents (imported via Vite's `?raw` loader, which works identically under Vitest)
// rather than anything React-rendered.
import html from "../../index.html?raw";

test("declares a non-white body background for both light and dark before any script runs", () => {
  expect(html).toMatch(/body\s*{[^}]*background:\s*#f0f2f5/);
  expect(html).toMatch(/prefers-color-scheme:\s*dark\s*\)\s*{\s*body\s*{[^}]*background:\s*#111318/);
});

test("#root contains accessible startup status markup before React mounts", () => {
  const doc = new DOMParser().parseFromString(html, "text/html");
  const root = doc.getElementById("root");
  expect(root).not.toBeNull();

  const status = root!.querySelector('[role="status"]');
  expect(status).not.toBeNull();
  expect(status!.getAttribute("aria-live")).toBe("polite");
  expect(status!.textContent).toBeTruthy();

  const spinner = root!.querySelector('[aria-hidden="true"]');
  expect(spinner).not.toBeNull();
});

test("respects prefers-reduced-motion by removing the spinner animation", () => {
  expect(html).toMatch(/prefers-reduced-motion:\s*reduce\s*\)\s*{[^}]*animation:\s*none/);
});
