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
