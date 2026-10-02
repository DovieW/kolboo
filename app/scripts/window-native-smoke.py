"""Observe/close only the synthetic Kolboo window on the isolated Xvfb display."""
import ctypes
import os
import re
import subprocess
import time

assert os.environ.get("KOLBOO_NATIVE_WINDOW_TEST") == "1"
x11 = ctypes.CDLL("libX11.so.6")
x11.XOpenDisplay.restype = ctypes.c_void_p
x11.XInternAtom.argtypes = [ctypes.c_void_p, ctypes.c_char_p, ctypes.c_int]
x11.XInternAtom.restype = ctypes.c_ulong
x11.XSendEvent.argtypes = [ctypes.c_void_p, ctypes.c_ulong, ctypes.c_int, ctypes.c_long, ctypes.c_void_p]
x11.XFlush.argtypes = [ctypes.c_void_p]
x11.XCloseDisplay.argtypes = [ctypes.c_void_p]
display = x11.XOpenDisplay(None)
assert display

class Data(ctypes.Union):
    _fields_ = [("longs", ctypes.c_long * 5), ("bytes", ctypes.c_char * 20)]

class ClientMessage(ctypes.Structure):
    _fields_ = [("type", ctypes.c_int), ("serial", ctypes.c_ulong), ("send_event", ctypes.c_int), ("display", ctypes.c_void_p), ("window", ctypes.c_ulong), ("message_type", ctypes.c_ulong), ("format", ctypes.c_int), ("data", Data)]

class Event(ctypes.Union):
    _fields_ = [("client", ClientMessage), ("padding", ctypes.c_long * 24)]

deadline = time.monotonic() + 600  # Allows the first instrumented binary build.
window = None
while window is None:
    assert time.monotonic() < deadline, "Kolboo startup window was not shown"
    listing = subprocess.run(["xprop", "-root", "_NET_CLIENT_LIST"], capture_output=True, text=True, check=True)
    for candidate in re.findall(r"0x[0-9a-f]+", listing.stdout):
        name = subprocess.run(["xprop", "-id", candidate, "_NET_WM_NAME"], capture_output=True, text=True)
        if name.stdout.rstrip().endswith('= "Kolboo"'):
            window = int(candidate, 16)
            break
    time.sleep(0.03)

details = subprocess.run(["xwininfo", "-id", hex(window)], capture_output=True, text=True, check=True).stdout
width = int(re.search(r"Width: (\d+)", details)[1])
height = int(re.search(r"Height: (\d+)", details)[1])
assert (width, height) == (1280, 800), (width, height)
event = Event()
event.client = ClientMessage(33, 0, 1, display, window, x11.XInternAtom(display, b"WM_PROTOCOLS", 0), 32, Data())
event.client.data.longs[0] = x11.XInternAtom(display, b"WM_DELETE_WINDOW", 0)
assert x11.XSendEvent(display, window, 0, 0, ctypes.byref(event))
x11.XFlush(display)
x11.XCloseDisplay(display)
print("Real Kolboo startup size and close-to-exit verified (isolated, offline).")
