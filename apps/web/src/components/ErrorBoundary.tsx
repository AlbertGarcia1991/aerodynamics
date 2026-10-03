/**
 * Keeps one failing panel from blanking the whole workspace. The scene lives in
 * zustand, outside React, so recovering is just re-rendering the subtree.
 */
import { Component, type ErrorInfo, type ReactNode } from 'react';

interface Props {
  label: string;
  children: ReactNode;
}

interface State {
  error: Error | null;
}

export class ErrorBoundary extends Component<Props, State> {
  state: State = { error: null };

  static getDerivedStateFromError(error: Error): State {
    return { error };
  }

  componentDidCatch(error: Error, info: ErrorInfo): void {
    console.error(`[AeroFlow] ${this.props.label} crashed:`, error, info.componentStack);
  }

  render() {
    if (!this.state.error) return this.props.children;
    return (
      <div className="error-boundary" role="alert">
        <strong>The {this.props.label} hit an unexpected error.</strong>
        <p className="muted">Your simulation is safe — nothing in the scene was lost.</p>
        <code>{this.state.error.message}</code>
        <button className="btn btn--sm" onClick={() => this.setState({ error: null })}>
          Try again
        </button>
      </div>
    );
  }
}
