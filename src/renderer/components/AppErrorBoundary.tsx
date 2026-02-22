import { Component, type ErrorInfo, type ReactNode } from "react";

interface AppErrorBoundaryProps {
  children: ReactNode;
}

interface AppErrorBoundaryState {
  hasError: boolean;
}

export class AppErrorBoundary extends Component<
  AppErrorBoundaryProps,
  AppErrorBoundaryState
> {
  state: AppErrorBoundaryState = { hasError: false };

  static getDerivedStateFromError() {
    return { hasError: true };
  }

  componentDidCatch(error: Error, errorInfo: ErrorInfo) {
    console.error("Unhandled React error:", error, errorInfo);
  }

  render() {
    if (this.state.hasError) {
      return (
        <div className="min-h-screen bg-zinc-950 text-white flex items-center justify-center p-6">
          <div className="w-full max-w-xl rounded-2xl border border-zinc-800 bg-zinc-900 p-6">
            <h1 className="text-xl font-semibold mb-2">앱 오류가 발생했습니다.</h1>
            <p className="text-zinc-400">
              앱을 새로고침하거나 다시 실행해 주세요.
            </p>
          </div>
        </div>
      );
    }

    return this.props.children;
  }
}
