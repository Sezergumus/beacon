/**
 * Turns off browser-specific behavior so the app feels like a native desktop
 * app: reload, the right-click menu, print, view source, find in page, etc.
 *
 * - Shortcuts for reload / print / view source / find in page are always
 *   blocked.
 * - DevTools shortcuts (F12, Ctrl+Shift+I/J/C) are only left enabled in
 *   development mode.
 * - The right-click menu stays available in text fields
 *   (input/textarea/contenteditable); it is needed for copy/paste.
 */
export function disableBrowserBehavior(): void {
  const isDev = import.meta.env.DEV;

  const isEditable = (target: EventTarget | null): boolean => {
    if (!(target instanceof HTMLElement)) return false;
    return (
      target instanceof HTMLInputElement ||
      target instanceof HTMLTextAreaElement ||
      target.isContentEditable
    );
  };

  window.addEventListener(
    "contextmenu",
    (e) => {
      if (!isEditable(e.target)) e.preventDefault();
    },
    true,
  );

  window.addEventListener(
    "keydown",
    (e) => {
      const key = e.key.toLowerCase();
      const ctrl = e.ctrlKey || e.metaKey;

      const blocked =
        // Reload
        key === "f5" ||
        (ctrl && key === "r") ||
        // Print, view source, save, find in page, find next
        (ctrl && ["p", "u", "s", "f", "g", "j"].includes(key) && !e.shiftKey) ||
        // Back / forward
        (e.altKey && (key === "arrowleft" || key === "arrowright")) ||
        // Zoom
        (ctrl && ["+", "-", "=", "0"].includes(key)) ||
        // DevTools (production only)
        (!isDev &&
          (key === "f12" ||
            (ctrl && e.shiftKey && ["i", "j", "c"].includes(key))));

      if (blocked) {
        e.preventDefault();
        e.stopPropagation();
      }
    },
    true,
  );

  // Block zooming with Ctrl + mouse wheel
  window.addEventListener(
    "wheel",
    (e) => {
      if (e.ctrlKey) e.preventDefault();
    },
    { passive: false, capture: true },
  );
}
