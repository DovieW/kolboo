import { useEffect, useRef, useState } from "react";
import { useBackendEvent } from "../lib/tauri/useBackendEvent";
import {
	createLiveWaveformScale,
	LIVE_WAVE_BARS,
	liveWaveformBars,
} from "./liveWaveform";

const SILENCE = Array<number>(LIVE_WAVE_BARS).fill(0);
const BAR_IDS = Array.from({ length: LIVE_WAVE_BARS }, (_, index) => index);

/** Native capture drives this display: no second microphone, RAF or idle loop. */
export function BackendAudioWave({
	isActive,
	isProcessing = false,
	isVisible = true,
	className = "",
}: {
	isActive: boolean;
	isProcessing?: boolean;
	isVisible?: boolean;
	className?: string;
}) {
	const [bars, setBars] = useState(SILENCE);
	const expiry = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
	const scale = useRef(createLiveWaveformScale());
	const enabled = isActive && isVisible;
	const processing = isProcessing && isVisible;
	useBackendEvent(
		"overlay-audio-level",
		(frame) => {
			setBars(liveWaveformBars(frame, scale.current));
			clearTimeout(expiry.current);
			// Paused capture and missing frames settle gently instead of freezing high.
			expiry.current = setTimeout(() => {
				setBars(SILENCE);
				scale.current = createLiveWaveformScale();
			}, 350);
		},
		enabled,
	);
	useEffect(() => {
		if (!enabled) {
			setBars(SILENCE);
			scale.current = createLiveWaveformScale();
		}
		return () => clearTimeout(expiry.current);
	}, [enabled]);
	return (
		<svg
			width={168}
			height={24}
			viewBox="0 0 168 24"
			aria-hidden="true"
			className={`overlay-wave${processing ? " overlay-wave--processing" : ""} ${className}`}
			style={{ display: "block", fill: "var(--accent-primary, #22c55e)" }}
		>
			{BAR_IDS.map((index) => {
				const edgeDistance =
					Math.min(index, LIVE_WAVE_BARS - 1 - index) /
					((LIVE_WAVE_BARS - 1) / 2);
				return (
					<g key={index} transform={`translate(${index * 5 + 5} 12)`}>
						<rect
							className="overlay-wave-base"
							x={0}
							y={-11}
							width={3}
							height={22}
							rx={1.5}
							style={{
								transformBox: "fill-box",
								transformOrigin: "center",
								transform: `scaleY(${processing ? 0.08 : Math.max(0.07, bars[index] as number)})`,
								transition: "transform 180ms ease-out",
							}}
						/>
						{processing ? (
							<>
								<rect
									className="overlay-wave-pulse"
									x={0}
									y={-11}
									width={3}
									height={22}
									rx={1.5}
									style={{ animationDelay: `${edgeDistance * 0.72}s` }}
								/>
								<rect
									className="overlay-wave-pulse"
									x={0}
									y={-11}
									width={3}
									height={22}
									rx={1.5}
									style={{
										animationDelay: `${0.92 + (1 - edgeDistance) * 0.72}s`,
									}}
								/>
							</>
						) : null}
					</g>
				);
			})}
		</svg>
	);
}
