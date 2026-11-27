@ECHO OFF
PATH=A:\;A:\QUILL
SET QUILL_FNT=A:\QUILL\QUILL.FNT
REM C: is yours (Quill saves there); D: is the page's. SYNC makes each see the other's files, then the page's date is set.
SYNC
IF EXIST D:\SETDATE.BAT CALL D:\SETDATE.BAT
A:
CD \QUILL
CTMOUSE /R13 >NUL
QUILLDOS.EXE /G16 SAMPLES\DEMO.QLD
C:
ECHO.
ECHO Quill has ended. This is FreeDOS 1.3. Type QUILLDOS /G16 to start it again, QUILLDOS /G for 256 colours, or QUILLDOS /T for text.
ECHO Your files are on C:. If you copy one onto C: here, run SYNC so the page can see it.
ECHO There is a note about FreeDOS and its licence in FREEDOS.TXT.
