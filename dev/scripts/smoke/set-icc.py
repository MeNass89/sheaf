"""Sets (or with no file, deletes) the X root window's _ICC_PROFILE to a file's bytes, as colord and
xcalib do for a calibrated display. Usage: python3 set-icc.py [profile.icc]   (DISPLAY chosen)."""
import ctypes
import sys

x = ctypes.CDLL("libX11.so.6")
x.XOpenDisplay.restype = ctypes.c_void_p
x.XOpenDisplay.argtypes = [ctypes.c_char_p]
x.XDefaultRootWindow.restype = ctypes.c_ulong
x.XDefaultRootWindow.argtypes = [ctypes.c_void_p]
x.XInternAtom.restype = ctypes.c_ulong
x.XInternAtom.argtypes = [ctypes.c_void_p, ctypes.c_char_p, ctypes.c_int]
x.XChangeProperty.argtypes = [ctypes.c_void_p, ctypes.c_ulong, ctypes.c_ulong, ctypes.c_ulong, ctypes.c_int, ctypes.c_int, ctypes.c_char_p, ctypes.c_int]
x.XDeleteProperty.argtypes = [ctypes.c_void_p, ctypes.c_ulong, ctypes.c_ulong]
x.XFlush.argtypes = [ctypes.c_void_p]
x.XCloseDisplay.argtypes = [ctypes.c_void_p]
d = x.XOpenDisplay(None)
if not d:
    sys.exit("no display")
root = x.XDefaultRootWindow(d)
atom = x.XInternAtom(d, b"_ICC_PROFILE", 0)
if len(sys.argv) > 1:
    data = open(sys.argv[1], "rb").read()
    cardinal = x.XInternAtom(d, b"CARDINAL", 0)
    x.XChangeProperty(d, root, atom, cardinal, 8, 0, data, len(data))  # PropModeReplace
else:
    x.XDeleteProperty(d, root, atom)
x.XFlush(d)
x.XCloseDisplay(d)
