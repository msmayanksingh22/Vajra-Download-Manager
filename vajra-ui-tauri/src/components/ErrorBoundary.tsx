import React, { Component, ErrorInfo, ReactNode } from 'react';
import { AlertTriangle, RefreshCw } from 'lucide-react';

interface Props {
  children: ReactNode;
}

interface State {
  hasError: boolean;
  error: Error | null;
}

export class ErrorBoundary extends Component<Props, State> {
  public state: State = {
    hasError: false,
    error: null
  };

  public static getDerivedStateFromError(error: Error): State {
    return { hasError: true, error };
  }

  public componentDidCatch(error: Error, errorInfo: ErrorInfo) {
    console.error('Uncaught error:', error, errorInfo);
  }

  public render() {
    if (this.state.hasError) {
      return (
        <div className="flex flex-col items-center justify-center min-h-screen bg-gray-900 text-gray-100 p-6 font-sans">
          <div className="bg-gray-800 p-8 rounded-xl shadow-2xl border border-red-900/50 max-w-lg w-full">
            <div className="flex items-center gap-4 text-red-400 mb-6">
              <AlertTriangle className="w-10 h-10" />
              <h1 className="text-2xl font-bold">Something went wrong</h1>
            </div>
            
            <p className="text-gray-300 mb-4">
              Vajra encountered an unexpected error. 
            </p>
            
            {this.state.error && (
              <div className="bg-black/50 p-4 rounded-lg overflow-x-auto mb-6 text-sm font-mono text-red-300 border border-gray-700">
                {this.state.error.message}
              </div>
            )}
            
            <button
              onClick={() => window.location.reload()}
              className="flex items-center justify-center gap-2 w-full py-3 bg-red-600 hover:bg-red-700 active:bg-red-800 transition-colors rounded-lg font-semibold text-white"
            >
              <RefreshCw className="w-5 h-5" />
              Reload Application
            </button>
          </div>
        </div>
      );
    }

    return this.props.children;
  }
}
