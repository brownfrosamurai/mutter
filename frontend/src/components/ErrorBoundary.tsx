import { Component, type ReactNode } from "react";

interface Props {
  children: ReactNode;
}
interface State {
  error: Error | null;
}

/** A render error in any panel must never blank the whole dashboard —
 * without this, React 18 unmounts the entire tree on an uncaught render
 * error, which is exactly what happened live while building the Settings
 * panel (a real bug this boundary caught and made debuggable instead of
 * a silent blank window). */
export class ErrorBoundary extends Component<Props, State> {
  state: State = { error: null };

  static getDerivedStateFromError(error: Error): State {
    return { error };
  }

  render() {
    if (this.state.error) {
      return (
        <div className="p-4 text-sm text-text-primary">
          <p className="font-semibold">Something went wrong.</p>
          <pre className="mt-2 whitespace-pre-wrap break-words text-xs text-text-secondary">
            {this.state.error.message}
            {"\n"}
            {this.state.error.stack}
          </pre>
        </div>
      );
    }
    return this.props.children;
  }
}
