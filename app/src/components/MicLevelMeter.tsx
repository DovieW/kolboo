type MicLevelMeterProps = {
	isActive: boolean;
	level: number;
	color: string;
	label?: string;
	width?: number | string;
};

export function MicLevelMeter({
	isActive,
	level,
	color,
	label = "Microphone level",
	width,
}: MicLevelMeterProps) {
	return (
		<div
			className={
				isActive ? "mic-test-meter mic-test-meter--active" : "mic-test-meter"
			}
			style={width ? { width } : undefined}
			role="progressbar"
			aria-label={label}
			aria-valuemin={0}
			aria-valuemax={100}
			aria-valuenow={Math.round(level * 100)}
			title={isActive ? "Speak into the mic to see the level" : label}
		>
			<div
				className="mic-test-meter-fill"
				style={{
					width: `${Math.round(level * 100)}%`,
					backgroundColor: color,
				}}
			/>
		</div>
	);
}
