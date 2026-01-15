import { createTheme, Switch } from "@mantine/core";

/**
 * App-wide Mantine theme.
 *
 * Note: We intentionally force Switch (toggle) ON/OFF track colors here,
 * because Mantine component styles can otherwise override plain CSS and
 * make the two states look too similar.
 */
export const darkTheme = createTheme({
	primaryColor: "orange",
	fontFamily: "'Sora', -apple-system, BlinkMacSystemFont, sans-serif",
	headings: {
		// Use a slightly more expressive sans-serif for headers.
		// Outfit is included via @fontsource/outfit.
		fontFamily:
			"'Outfit', 'Sora', -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif",
		fontWeight: "600",
	},
	colors: {
		dark: [
			"#C1C2C5",
			"#A6A7AB",
			"#909296",
			"#5C5F66",
			"#373A40",
			"#2C2E33",
			"#1A1A1A",
			"#111111",
			"#0A0A0A",
			"#000000",
		],
	},
	components: {
		// Mantine Select/MultiSelect show a check icon for the selected option by default.
		// We prefer a cleaner dropdown UI across the app.
		Select: {
			defaultProps: {
				withCheckIcon: false,
			},
		},
		MultiSelect: {
			defaultProps: {
				withCheckIcon: false,
			},
		},

		Paper: {
			defaultProps: {
				bg: "#111111",
			},
		},
		Card: {
			defaultProps: {
				bg: "#111111",
			},
		},

		// Make enabled/disabled (on/off) states visually distinct.
		//
		// Mantine v8 Switch track uses CSS variables:
		// - background-color: var(--switch-bg)
		// - checked sets: --switch-bg: var(--switch-color)
		// So we override *variables* (with dark-scheme specificity) rather than
		// trying to set background-color directly.
		Switch: Switch.extend({
			classNames: {
				root: "tv-switch-root",
				track: "tv-switch-track",
				input: "tv-switch-input",
			},
			styles: (_theme, props) => ({
				// NOTE: Using selector keys in a plain `styles` object can leak into
				// inline styles and trigger React warnings (e.g. "&[data-disabled]").
				// Use props-based styles instead.
				root: {
					opacity: props.disabled ? 0.85 : undefined,
				},
			}),
		}),
	},
});
