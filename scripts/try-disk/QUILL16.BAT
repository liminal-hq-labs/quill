@ECHO OFF
PATH=\;\QUILL
CD \QUILL
CTMOUSE /R13 >NUL
QUILLDOS.EXE /G16 SAMPLES\DEMO.QLD
CD \
ECHO.
ECHO Quill has ended. This is FreeDOS 1.3. Type QUILLDOS /G16 to start it again, QUILLDOS /G for 256 colours, or QUILLDOS /T for text.
ECHO There is a note about FreeDOS and its licence in FREEDOS.TXT.
