import { useEffect, useRef } from 'react';

export function MatrixRain() {
    const canvasRef = useRef<HTMLCanvasElement>(null);

    useEffect(() => {
        const canvas = canvasRef.current;
        if (!canvas) return;

        const ctx = canvas.getContext('2d');
        if (!ctx) return;

        let width = window.innerWidth;
        let height = window.innerHeight;

        canvas.width = width;
        canvas.height = height;

        const fontSize = 16;
        const columns = Math.floor(width / fontSize);

        const drops: number[] = new Array(columns).fill(1).map(() => Math.random() * -100);

        const chars = "ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789アカサタナハマヤラワガザダバパイキシチニヒミリギジヂビピウクスツヌフムユルグズヅブプエケセテネヘメレゲゼデベペオコソトノホモヨロヲゴゾドボポ@#$%^&*";

        let frameId: number;
        let lastDrawTime = 0;
        const FRAME_INTERVAL = 50; // ~20fps — smooth but not seizure-inducing

        const draw = (timestamp: number) => {
            // THROTTLE: Only draw if enough time has elapsed
            if (timestamp - lastDrawTime < FRAME_INTERVAL) {
                frameId = requestAnimationFrame(draw);
                return;
            }
            lastDrawTime = timestamp;

            // Semi-transparent black veil to create trail effect
            ctx.fillStyle = 'rgba(0, 0, 0, 0.05)';
            ctx.fillRect(0, 0, width, height);

            ctx.font = `${fontSize}px monospace`;

            for (let i = 0; i < drops.length; i++) {
                const text = chars.charAt(Math.floor(Math.random() * chars.length));
                const x = i * fontSize;
                const y = drops[i] * fontSize;

                const isHead = Math.random() > 0.95;

                if (isHead) {
                    ctx.fillStyle = '#FFF';
                    ctx.shadowBlur = 8;
                    ctx.shadowColor = '#FFF';
                } else {
                    ctx.fillStyle = '#0F0';
                    ctx.shadowBlur = 0;
                }

                ctx.fillText(text, x, y);
                ctx.shadowBlur = 0;

                if (y > height && Math.random() > 0.975) {
                    drops[i] = 0;
                }

                drops[i]++;
            }
            frameId = requestAnimationFrame(draw);
        };

        const handleResize = () => {
            width = window.innerWidth;
            height = window.innerHeight;
            canvas.width = width;
            canvas.height = height;
        };

        // Reset timer when tab becomes visible to prevent burst
        const handleVisibility = () => {
            if (!document.hidden) {
                lastDrawTime = performance.now();
            }
        };

        window.addEventListener('resize', handleResize);
        document.addEventListener('visibilitychange', handleVisibility);
        frameId = requestAnimationFrame(draw);

        return () => {
            window.removeEventListener('resize', handleResize);
            document.removeEventListener('visibilitychange', handleVisibility);
            cancelAnimationFrame(frameId);
        };
    }, []);

    return (
        <canvas
            ref={canvasRef}
            className="absolute inset-0 z-0 pointer-events-none opacity-40 mix-blend-screen"
        />
    );
}
