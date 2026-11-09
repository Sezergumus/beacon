import React from "react";
import { RotateCcw, TriangleAlert } from "lucide-react";

interface Props {
  children: React.ReactNode;
  /** "app" fills the window; "view" fills the page area and keeps the sidebar usable. */
  variant?: "app" | "view";
  /** When any of these change (e.g. the selected tab), the error is cleared. */
  resetKeys?: unknown[];
}

interface State {
  error: Error | null;
}

const sameKeys = (a: unknown[] = [], b: unknown[] = []) =>
  a.length === b.length && a.every((v, i) => Object.is(v, b[i]));

/** Catches a crash while drawing, so one broken screen never leaves a blank window. */
export class ErrorBoundary extends React.Component<Props, State> {
  state: State = { error: null };

  static getDerivedStateFromError(error: Error): State {
    return { error };
  }

  componentDidCatch(error: Error, info: React.ErrorInfo) {
    // Kept in the dev console / logs so a bug report can include it.
    console.error("UI crashed:", error, info.componentStack);
  }

  componentDidUpdate(prev: Props) {
    if (this.state.error && !sameKeys(prev.resetKeys, this.props.resetKeys)) {
      this.setState({ error: null });
    }
  }

  private reset = () => this.setState({ error: null });

  render() {
    const { error } = this.state;
    if (!error) return this.props.children;

    const full = this.props.variant === "app";
    return (
      <div
        role="alert"
        className={`${full ? "h-screen w-screen" : "h-full w-full"} bg-bg text-ink flex items-center justify-center p-8`}
      >
        <div className="max-w-md w-full rounded-3xl bg-surface p-8 text-center">
          <div className="mx-auto w-14 h-14 rounded-2xl bg-danger/15 text-danger flex items-center justify-center mb-4">
            <TriangleAlert className="w-7 h-7" aria-hidden />
          </div>
          <h2 className="text-xl font-extrabold">Something went wrong</h2>
          <p className="text-soft mt-2">
            {full
              ? "Beacon hit a problem drawing this window. Your worlds are safe and any open world keeps running."
              : "This page hit a problem. The rest of Beacon still works, and any open world keeps running."}
          </p>
          <pre className="mt-4 max-h-28 overflow-auto rounded-xl bg-raised p-3 text-left text-xs text-soft whitespace-pre-wrap select-text">
            {error.message || String(error)}
          </pre>
          <div className="mt-5 flex items-center justify-center gap-2">
            {!full && (
              <button
                onClick={this.reset}
                className="h-11 px-5 rounded-xl bg-raised font-bold hover:bg-line transition-colors cursor-pointer"
              >
                Try again
              </button>
            )}
            <button
              onClick={() => window.location.reload()}
              className="h-11 px-5 rounded-xl bg-brand text-brand-ink font-bold flex items-center gap-2 cursor-pointer"
            >
              <RotateCcw className="w-4 h-4" aria-hidden /> Reload Beacon
            </button>
          </div>
        </div>
      </div>
    );
  }
}
