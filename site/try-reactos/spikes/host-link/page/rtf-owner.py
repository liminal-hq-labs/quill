# Owns the X11 clipboard on a private Xvfb with text/rtf, text/html and text/plain, as LibreOffice or Word would after a copy, so that a browser
# running on the same display can be asked what its paste event and async Clipboard API offer for a native RTF clipboard. Runs until killed.
#
# (c) Copyright 2026 Liminal HQ, Scott Morris
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# usage: DISPLAY=:97 QT_QPA_PLATFORM=xcb python3 -I rtf-owner.py
import sys
from PyQt6.QtCore import QMimeData, QTimer
from PyQt6.QtGui import QGuiApplication

RTF = rb"{\rtf1\ansi{\fonttbl{\f0 Arial;}}\f0\fs28 plain {\b bold} {\i italic}\par}"
app = QGuiApplication(sys.argv)
mime = QMimeData()
mime.setData("text/rtf", RTF)
mime.setData("application/rtf", RTF)
mime.setHtml("<p>plain <b>bold</b> <i>italic</i></p>")
mime.setText("plain bold italic")
app.clipboard().setMimeData(mime)
print("owner ready", flush=True)
QTimer.singleShot(600_000, app.quit)
app.exec()
