import { useRef, useEffect } from "react";
import { motion } from "framer-motion";

// --- CONSTANTS FROM LEGACY RUST SOURCE ---

const GLYPHS = "qwertyuiopasdfghjklzxcvbnmQWERTYUIOPASDFGHJKLZXCVBNM0123456789<>:;[]{}!@#$%^&*=+-_|?";

// MatrixTrail interface removed (unused in V3 engine)

export function AboutPanel() {
    const canvasRef = useRef<HTMLCanvasElement>(null);

    // --- MATRIX RAIN ENGINE (Optimized V2) ---
    // --- MATRIX RAIN ENGINE V6 (Correct Accumulator + First-Frame Cap) ---
    useEffect(() => {
        const canvas = canvasRef.current;
        if (!canvas) return;
        const ctx = canvas.getContext("2d", { alpha: true });
        if (!ctx) return;

        let width = window.innerWidth;
        let height = window.innerHeight;
        canvas.width = width;
        canvas.height = height;

        const fontSize = 14;
        const columns = Math.ceil(width / fontSize);
        const drops: number[] = [];

        // Initialize drops spread across screen for immediate visual impact
        const maxRow = Math.ceil(height / fontSize);
        for (let i = 0; i < columns; i++) {
            drops[i] = Math.floor(Math.random() * maxRow);
        }

        const chars = GLYPHS.split("");

        let lastTime = -1; // -1 = first frame flag
        const targetFPS = 28;
        const interval = 1000 / targetFPS;
        let accumulator = 0;
        let rafId: number;

        // VISUAL CONFIG
        const HEAD_COLOR = "#FFFFFF";
        const TRAIL_COLOR = "#00FF00";
        const GLOW_COLOR = "#00FF00";
        const FADE_COLOR = "rgba(0, 0, 0, 0.05)";

        ctx.font = `${fontSize}px monospace`;
        ctx.textAlign = "center";

        // Paint initial black canvas
        ctx.fillStyle = "#000000";
        ctx.fillRect(0, 0, width, height);

        const draw = (currentTime: number) => {
            // First frame: just set the clock, don't accumulate anything
            if (lastTime < 0) {
                lastTime = currentTime;
                rafId = requestAnimationFrame(draw);
                return;
            }

            const deltaTime = currentTime - lastTime;
            lastTime = currentTime;

            // CAP: never accumulate more than 2 frames worth
            // This prevents burst rendering after tab switch or first load
            accumulator += Math.min(deltaTime, interval * 2);

            // Tick when enough time has accumulated
            while (accumulator >= interval) {
                // 1. Fade trail
                ctx.fillStyle = FADE_COLOR;
                ctx.fillRect(0, 0, width, height);

                // 2. Draw drops
                ctx.font = `${fontSize}px monospace`;
                ctx.textAlign = "center";

                for (let i = 0; i < drops.length; i++) {
                    const char = chars[Math.floor(Math.random() * chars.length)];
                    const x = i * fontSize;
                    const y = drops[i] * fontSize;

                    // Head glow (white)
                    ctx.shadowBlur = 8;
                    ctx.shadowColor = GLOW_COLOR;
                    ctx.fillStyle = HEAD_COLOR;
                    ctx.fillText(char, x, y);
                    ctx.shadowBlur = 0;

                    // Trail char (green, one step behind)
                    if (drops[i] > 1) {
                        const trailChar = chars[Math.floor(Math.random() * chars.length)];
                        ctx.fillStyle = TRAIL_COLOR;
                        ctx.fillText(trailChar, x, y - fontSize);
                    }

                    // Reset when past bottom
                    if (y > height && Math.random() > 0.980) {
                        drops[i] = 0;
                    }
                    drops[i]++;
                }

                accumulator -= interval;
            }

            rafId = requestAnimationFrame(draw);
        };

        rafId = requestAnimationFrame(draw);

        const handleResize = () => {
            width = window.innerWidth;
            height = window.innerHeight;
            canvas.width = width;
            canvas.height = height;
            const newMaxRow = Math.ceil(height / fontSize);
            const newCols = Math.ceil(width / fontSize);
            for (let i = drops.length; i < newCols; i++) {
                drops[i] = Math.floor(Math.random() * newMaxRow);
            }
        };

        window.addEventListener('resize', handleResize);

        return () => {
            window.removeEventListener('resize', handleResize);
            cancelAnimationFrame(rafId);
        };
    }, []);

    // --- MANIFESTO OVERLAY (1:1 HTML Replica of Rust Draw Logic) ---
    return (
        <div className="h-full w-full relative overflow-hidden bg-[#020205] select-none">
            {/* 1. Canvas Background */}
            <canvas ref={canvasRef} className="absolute inset-0 z-0" />

            {/* 2. Manifesto Box */}
            <div className="absolute inset-0 z-10 flex items-center justify-center p-4">
                <motion.div
                    initial={{ scale: 0.9, opacity: 0 }}
                    animate={{ scale: 1, opacity: 1 }}
                    transition={{ duration: 0.5 }}
                    className="relative max-w-[550px]"
                >
                    {/* Outer Glow Layers - Simulating `for i in 1..5` loop from Rust */}
                    <div className="absolute -inset-1 border-[2px] border-[#32FF32]/20 rounded-lg pointer-events-none" />
                    <div className="absolute -inset-2 border-[4px] border-[#32FF32]/10 rounded-lg pointer-events-none" />
                    <div className="absolute -inset-3 border-[6px] border-[#32FF32]/5 rounded-lg pointer-events-none" />

                    {/* Main Box */}
                    <div
                        className="bg-black/95 border-[2px] border-[#32FF32] rounded-lg p-12 text-center shadow-[0_0_30px_rgba(50,255,50,0.2)]"
                        style={{ boxShadow: "0 0 15px rgba(50, 255, 50, 0.3)" }}
                    >
                        <div className="font-mono text-[15px] leading-relaxed text-[#DCFFDC] space-y-6">
                            <p className="font-bold tracking-wider text-[#32FF32] mb-8 text-lg">
                                WE ARE THE ORCHESTRATORS.
                            </p>

                            <p>
                                Steam is the cage. DarkCore is the key.
                            </p>
                            <p>
                                We build bridges where they built walls.<br />
                                We play what we want, when we want.
                            </p>

                            <p className="font-bold text-[#32FF32] pt-4 text-lg">
                                Power to the Players.
                            </p>

                            <div className="pt-8 opacity-70 text-xs tracking-widest">
                                Signed, SEBASTIAN.
                            </div>
                        </div>
                    </div>
                </motion.div>
            </div>
        </div>
    );
}
