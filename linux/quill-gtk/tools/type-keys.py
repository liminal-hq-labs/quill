# Types keysyms into the focused window through mutter's RemoteDesktop API. It sends to whatever has focus, so run it only
# while the spike window is the active window (the spike logs "window: is-active true").
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
"""Usage: type-keys.py KEYSYM_HEX..."""
import sys, time
from gi.repository import Gio, GLib
bus = Gio.bus_get_sync(Gio.BusType.SESSION)
D='org.gnome.Mutter.RemoteDesktop'
def call(path, iface, m, args=None, sig=None):
	return bus.call_sync(D, path, iface, m, GLib.Variant(sig,args) if sig else None, None, Gio.DBusCallFlags.NONE, 10000).unpack()
s = call('/org/gnome/Mutter/RemoteDesktop', D, 'CreateSession')[0]
call(s, D+'.Session', 'Start')
time.sleep(0.5)
def key(sym):
	call(s, D+'.Session', 'NotifyKeyboardKeysym', (sym, True), '(ub)'); time.sleep(0.08)
	call(s, D+'.Session', 'NotifyKeyboardKeysym', (sym, False), '(ub)'); time.sleep(0.2)
for sym in [int(x, 16) for x in sys.argv[1:]]:
	key(sym)
time.sleep(0.5)
call(s, D+'.Session', 'Stop')
