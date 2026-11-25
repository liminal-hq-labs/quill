@ECHO OFF
PATH=\;\QUILL
CD \QUILL
CTMOUSE /R13 >NUL
QUILLDOS.EXE /T SAMPLES\DEMO.QLD
CD \
ECHO.
ECHO Quill has ended. This is FreeDOS 1.3. Type QUILLDOS to start it again, or QUILLDOS /G for graphics, or QUILLDOS /G16 for 16 colours.
ECHO There is a note about FreeDOS and its licence in FREEDOS.TXT.
