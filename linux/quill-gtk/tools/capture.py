# Grabs one frame of a monitor through mutter's ScreenCast and the PipeWire source, at the monitor's native resolution.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
"""Usage: capture.py CONNECTOR OUT.png"""
import sys, subprocess
from gi.repository import Gio, GLib
connector, out = sys.argv[1], sys.argv[2]
bus = Gio.bus_get_sync(Gio.BusType.SESSION)
loop = GLib.MainLoop()
D = 'org.gnome.Mutter.ScreenCast'
def call(path, iface, method, args, sig):
	return bus.call_sync(D, path, iface, method, GLib.Variant(sig, args) if args is not None else None, None, Gio.DBusCallFlags.NONE, -1).unpack()
session = call('/org/gnome/Mutter/ScreenCast', D, 'CreateSession', ({},), '(a{sv})')[0]
stream = call(session, D + '.Session', 'RecordMonitor', (connector, {'cursor-mode': GLib.Variant('u', 0)}), '(sa{sv})')[0]
node = []
def on_added(conn, sender, obj, iface, sig, params):
	node.append(params.unpack()[0]); loop.quit()
bus.signal_subscribe(D, D + '.Stream', 'PipeWireStreamAdded', stream, None, Gio.DBusSignalFlags.NONE, on_added)
call(session, D + '.Session', 'Start', None, None)
GLib.timeout_add_seconds(10, loop.quit)
loop.run()
print('node', node)
if node:
	p = subprocess.run(['gst-launch-1.0', '-q', 'pipewiresrc', 'path=%d' % node[0], 'num-buffers=3', '!', 'videoconvert', '!', 'pngenc', 'snapshot=true', '!', 'filesink', 'location=' + out], timeout=30, capture_output=True, text=True)
	print(p.returncode, p.stderr[-300:])
call(session, D + '.Session', 'Stop', None, None)
