import { formatAppError } from "./tauri";
import { useServerStore } from "../stores/useServerStore";

/** Browser noise that is not a real problem. */
const IGNORED = [/ResizeObserver loop/i];

/**
 * Anything that slips past a try/catch (a forgotten promise, a throw inside an event
 * handler) becomes a friendly toast instead of failing silently.
 */
export function installGlobalErrorHandlers() {
  let lastMessage = "";
  let lastAt = 0;

  const report = (reason: unknown) => {
    const message = formatAppError(reason);
    if (IGNORED.some((re) => re.test(message))) return;
    // The same failure can repeat in a loop; show it once in a while.
    const now = Date.now();
    if (message === lastMessage && now - lastAt < 5000) return;
    lastMessage = message;
    lastAt = now;
    console.error("Unhandled error:", reason);
    useServerStore.getState().pushToast("error", "Something went wrong", message);
  };

  window.addEventListener("unhandledrejection", (e) => report(e.reason));
  window.addEventListener("error", (e) => report(e.error ?? e.message));
}
