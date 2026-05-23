import { Component, ErrorInfo, ReactNode } from "react";
import { AlertTriangle, RefreshCw } from "lucide-react";

interface Props {
    children?: ReactNode;
}

interface State {
    hasError: boolean;
    error: Error | null;
    errorInfo: ErrorInfo | null;
}

export class ErrorBoundary extends Component<Props, State> {
    public state: State = {
        hasError: false,
        error: null,
        errorInfo: null
    };

    public static getDerivedStateFromError(error: Error): State {
        return { hasError: true, error, errorInfo: null };
    }

    public componentDidCatch(error: Error, errorInfo: ErrorInfo) {
        console.error("Uncaught error:", error, errorInfo);
        this.setState({ errorInfo });
    }

    public render() {
        if (this.state.hasError) {
            return (
                <div className="h-screen w-screen bg-obsidian flex flex-col items-center justify-center text-neon-cyan p-8 font-mono relative overflow-hidden">
                    <div className="absolute inset-0 bg-red-500/5 z-0 animate-pulse"></div>

                    <div className="z-10 bg-black/80 backdrop-blur-xl border border-red-500/50 p-8 rounded-2xl shadow-[0_0_50px_rgba(239,68,68,0.2)] max-w-2xl w-full">
                        <div className="flex items-center gap-4 mb-6">
                            <AlertTriangle className="w-12 h-12 text-red-500 animate-bounce" />
                            <h1 className="text-3xl font-black tracking-widest text-white">CRITICAL SYSTEM FAILURE</h1>
                        </div>

                        <div className="bg-black/50 p-4 rounded-lg border border-white/10 mb-6 overflow-auto max-h-60 custom-scrollbar">
                            <p className="text-red-400 font-bold mb-2">{this.state.error?.toString()}</p>
                            <pre className="text-[10px] text-gray-500 whitespace-pre-wrap font-mono">
                                {this.state.errorInfo?.componentStack}
                            </pre>
                        </div>

                        <button
                            onClick={() => window.location.reload()}
                            className="w-full py-4 bg-red-600/20 hover:bg-red-600/40 border border-red-500 text-red-100 font-bold rounded-xl transition-all flex items-center justify-center gap-2 group"
                        >
                            <RefreshCw className="w-5 h-5 group-hover:rotate-180 transition-transform duration-500" />
                            SYSTEM REBOOT
                        </button>
                    </div>
                </div>
            );
        }

        return this.props.children;
    }
}
