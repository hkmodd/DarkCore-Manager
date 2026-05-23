import { createContext, useContext, useEffect, useRef, useState, ReactNode } from 'react';

interface AudioContextType {
    playHover: () => void;
    playClick: () => void;
    toggleMute: () => void;
    isMuted: boolean;
    volume: number;
    setVolume: (v: number) => void;
    isPlaying: boolean;
    togglePlay: () => void;
}

const AudioContext = createContext<AudioContextType | null>(null);

export function useAudio() {
    const context = useContext(AudioContext);
    if (!context) throw new Error("useAudio must be used within AudioProvider");
    return context;
}

interface AudioProviderProps {
    children: ReactNode;
}

export function AudioProvider({ children }: AudioProviderProps) {
    const bgmRef = useRef<HTMLAudioElement | null>(null);

    // Persist state to localStorage
    const [isMuted, setIsMuted] = useState(() => {
        return localStorage.getItem('audio_muted') === 'true';
    });

    const [volume, setVolumeState] = useState(() => {
        const saved = localStorage.getItem('audio_volume');
        return saved ? parseFloat(saved) : 0.05; // Default 5%
    });

    const [isPlaying, setIsPlaying] = useState(() => {
        // If user explicitly paused before, DON'T auto-play
        return localStorage.getItem('audio_paused') !== 'true';
    });

    useEffect(() => {
        // Initialize Audio
        const audio = new Audio('/assets/audio/sys_audio_01.mp3');
        audio.loop = true;
        bgmRef.current = audio;

        // Apply saved settings
        audio.volume = isMuted ? 0 : volume;

        // Auto-Play Logic
        // Only auto-play if NOT muted AND NOT explicitly paused by user
        const wasPaused = localStorage.getItem('audio_paused') === 'true';
        if (!isMuted && !wasPaused) {
            const playPromise = audio.play();
            if (playPromise !== undefined) {
                playPromise.then(() => {
                    setIsPlaying(true);
                }).catch(error => {
                    console.log("Auto-play blocked (waiting for interaction):", error);
                    setIsPlaying(false);
                    // Fallback: Play on first interaction if auto-play blocked
                    const enableAudio = () => {
                        if (localStorage.getItem('audio_paused') !== 'true') {
                            audio.play().then(() => setIsPlaying(true)).catch(() => { });
                        }
                        document.removeEventListener('click', enableAudio);
                        document.removeEventListener('keydown', enableAudio);
                    };
                    document.addEventListener('click', enableAudio);
                    document.addEventListener('keydown', enableAudio);
                });
            }
        }

        return () => {
            audio.pause();
            audio.src = "";
        };
    }, []);

    // Volume Updater
    const setVolume = (v: number) => {
        const value = Math.max(0, Math.min(1, v));
        setVolumeState(value);
        localStorage.setItem('audio_volume', value.toString());

        if (bgmRef.current && !isMuted) {
            bgmRef.current.volume = value;
        }
    };

    // Mute Updater
    const toggleMute = () => {
        const newState = !isMuted;
        setIsMuted(newState);
        localStorage.setItem('audio_muted', newState.toString());

        if (bgmRef.current) {
            if (newState) {
                bgmRef.current.volume = 0;
            } else {
                bgmRef.current.volume = volume;
                // Resume if it was stopped/prevented AND NOT PAUSED
                if (bgmRef.current.paused && localStorage.getItem('audio_paused') !== 'true') {
                    bgmRef.current.play().then(() => setIsPlaying(true)).catch(console.error);
                }
            }
        }
    };

    // Play/Pause Toggle
    const togglePlay = () => {
        if (!bgmRef.current) return;
        if (isPlaying || !bgmRef.current.paused) {
            bgmRef.current.pause();
            setIsPlaying(false);
            localStorage.setItem('audio_paused', 'true');
        } else {
            bgmRef.current.play().then(() => {
                setIsPlaying(true);
                localStorage.setItem('audio_paused', 'false');
            }).catch(console.error);
        }
    };

    const playHover = () => { };
    const playClick = () => { };

    return (
        <AudioContext.Provider value={{
            playHover,
            playClick,
            toggleMute,
            isMuted,
            volume,
            setVolume,
            isPlaying,
            togglePlay
        }}>
            {children}
        </AudioContext.Provider>
    );
}
