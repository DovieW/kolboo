// Default accent for the app when the user has no override.
// Kept as a single constant so index.html/overlay.html can mirror it.
export const DEFAULT_ACCENT_HEX = "#22c55e";

function clampByte(n: number): number {
  return Math.max(0, Math.min(255, Math.round(n)));
}

function expandShortHex(hex: string): string {
  // "abc" -> "aabbcc"
  return hex
    .split("")
    .map((c) => `${c}${c}`)
    .join("");
}

export function normalizeHexColor(
  input: string | null | undefined
): string | null {
  if (input === null || input === undefined) return null;
  const raw = input.trim();
  if (raw.length === 0) return null;

  const withHash = raw.startsWith("#") ? raw : `#${raw}`;
  const m = /^#([0-9a-fA-F]{3}|[0-9a-fA-F]{6})$/.exec(withHash);
  if (!m) return null;

  const group = m[1];
  if (!group) return null;

  const body = group.length === 3 ? expandShortHex(group) : group;
  return `#${body.toLowerCase()}`;
}

function hexToRgb(hex: string): { r: number; g: number; b: number } {
  const normalized = normalizeHexColor(hex);
  if (!normalized) {
    return { r: 34, g: 197, b: 94 };
  }
  const body = normalized.slice(1);
  const r = Number.parseInt(body.slice(0, 2), 16);
  const g = Number.parseInt(body.slice(2, 4), 16);
  const b = Number.parseInt(body.slice(4, 6), 16);
  return { r, g, b };
}

function rgbToHex(rgb: { r: number; g: number; b: number }): string {
  const to = (n: number) => clampByte(n).toString(16).padStart(2, "0");
  return `#${to(rgb.r)}${to(rgb.g)}${to(rgb.b)}`;
}

function mix(
  a: { r: number; g: number; b: number },
  b: { r: number; g: number; b: number },
  t: number
) {
  const tt = Math.max(0, Math.min(1, t));
  return {
    r: a.r + (b.r - a.r) * tt,
    g: a.g + (b.g - a.g) * tt,
    b: a.b + (b.b - a.b) * tt,
  };
}

/**
 * Apply the app accent by overriding CSS variables on :root.
 *
 * Pass null/"" to reset to the default CSS-defined accent.
 */
export function applyAccentColor(accentHex: string | null | undefined): void {
  if (typeof document === "undefined") return;

  const style = document.documentElement.style;
  const normalized = normalizeHexColor(accentHex);

  // Reset to defaults (defined in CSS)
  if (!normalized) {
    style.removeProperty("--accent-rgb");
    style.removeProperty("--accent-hover");
    style.removeProperty("--accent-primary");

    // Keep Mantine "orange" mapped to our *default* accent so components that use
    // `color="orange"` still match the app accent when the user has no override.
    const { r, g, b } = hexToRgb(DEFAULT_ACCENT_HEX);
    const hover = rgbToHex(mix({ r, g, b }, { r: 255, g: 255, b: 255 }, 0.22));

    style.setProperty("--mantine-color-orange-6", DEFAULT_ACCENT_HEX);
    style.setProperty("--mantine-color-orange-7", hover);
    style.setProperty("--mantine-color-orange-filled", DEFAULT_ACCENT_HEX);
    style.setProperty("--mantine-color-orange-filled-hover", hover);
    style.setProperty(
      "--mantine-color-orange-light",
      `rgba(${r}, ${g}, ${b}, 0.10)`
    );
    style.setProperty(
      "--mantine-color-orange-light-hover",
      `rgba(${r}, ${g}, ${b}, 0.12)`
    );
    style.setProperty("--mantine-color-orange-light-color", DEFAULT_ACCENT_HEX);
    style.setProperty("--mantine-color-orange-outline", DEFAULT_ACCENT_HEX);
    style.setProperty(
      "--mantine-color-orange-outline-hover",
      `rgba(${r}, ${g}, ${b}, 0.05)`
    );
    return;
  }

  const { r, g, b } = hexToRgb(normalized);

  // Hover is a gentle mix towards white (keeps it vibrant in dark mode)
  const hover = rgbToHex(mix({ r, g, b }, { r: 255, g: 255, b: 255 }, 0.22));

  // Set both rgb + hex so existing CSS continues to work.
  // Using space-separated channels enables `rgba(var(--accent-rgb) / <alpha>)`.
  style.setProperty("--accent-rgb", `${r} ${g} ${b}`);
  style.setProperty("--accent-primary", normalized);
  style.setProperty("--accent-hover", hover);

  // Mantine integration:
  // The app theme sets `primaryColor: "orange"` and many components explicitly use
  // `color="orange"`. Mantine maps those to CSS variables like:
  //   --mantine-color-orange-6, --mantine-color-orange-filled, etc.
  // Override them to point to our dynamic accent so all those "orange" accents follow
  // the user's selected accent color.
  style.setProperty("--mantine-color-orange-6", normalized);
  style.setProperty("--mantine-color-orange-7", hover);
  style.setProperty("--mantine-color-orange-filled", normalized);
  style.setProperty("--mantine-color-orange-filled-hover", hover);
  style.setProperty(
    "--mantine-color-orange-light",
    `rgba(${r}, ${g}, ${b}, 0.10)`
  );
  style.setProperty(
    "--mantine-color-orange-light-hover",
    `rgba(${r}, ${g}, ${b}, 0.12)`
  );
  style.setProperty("--mantine-color-orange-light-color", normalized);
  style.setProperty("--mantine-color-orange-outline", normalized);
  style.setProperty(
    "--mantine-color-orange-outline-hover",
    `rgba(${r}, ${g}, ${b}, 0.05)`
  );
}
