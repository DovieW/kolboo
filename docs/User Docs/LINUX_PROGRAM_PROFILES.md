# Program profiles on Linux

Kolboo reads the focused program only when a recording or profile query needs
it. Opening **Pick an open program** requests a current window list but shows
only executable names. These queries stay on the device. Kolboo does not
install a background window watcher or store a history of focused windows.

| Desktop session | Program picker and automatic matching |
| --- | --- |
| KDE Plasma 6 on Wayland | Built-in, via a short-lived read-only KWin script |
| GNOME on Wayland | Requires the optional [Window Calls Extended](https://extensions.gnome.org/extension/4974/window-calls-extended/) Shell extension |
| Sway | Uses `swaymsg`, normally included with Sway |
| Hyprland | Uses `hyprctl`, normally included with Hyprland |
| X11 desktops (including XFCE, MATE, Cinnamon, GNOME Xorg and Plasma X11) | Uses `xprop` (`x11-utils` on Debian/Ubuntu) |

Other Wayland compositors may not expose a safe, portable active-window API.
Kolboo shows an explicit unavailable message rather than an empty list. You can
still enter an executable path manually, but automatic matching requires one
of the integrations above. When matching is unavailable, Kolboo uses the
default profile. It never guesses a program from an incomplete Xwayland list.

On GNOME Wayland, install and enable the extension yourself from the GNOME
Extensions site, then reopen Kolboo. Shell extensions run inside GNOME Shell
and can access window metadata; review that permission before enabling one.
Kolboo does not install or enable it automatically.

## Paste shortcuts

In **Settings → UI → Paste shortcut**, choose the shortcut used to insert a
transcript. Select **Default** in the profile picker to change it globally, or
select a program profile to override it only for that program. For example,
choose **Ctrl+Shift+V** for a terminal while leaving other apps at **System
default** (Ctrl+V on Linux/Windows, ⌘+V on macOS). Ctrl+V, Ctrl+Shift+V,
Shift+Insert and ⌘+V are also explicit choices; the target app must support the
chosen shortcut. Resetting an override restores inheritance from Default.

The shortcut applies to Paste and Both output modes, including live output and
paste/retry actions. Copy-only output sends no keys. Windows still uses its
existing accessibility insertion path where available, and uses the selected
shortcut when it falls back to clipboard paste. This does not change clipboard
privacy or desktop permission requirements. **Context Grab Shortcut** is
separate: it copies highlighted text, rather than pasting a transcript.
