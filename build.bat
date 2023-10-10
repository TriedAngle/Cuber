@echo off
cd glyphers_ffi
call cargo build --release
if errorlevel 1 (
    echo Error: 'cargo build --release' failed.
    exit /b %errorlevel%
)
move target\release\glyphers.dll ..\sdfui\glue\glyphers.dll
if errorlevel 1 (
    echo Error: Failed to move glyphers.dll.
    exit /b %errorlevel%
)
echo Successfully moved glyphers.dll to ..\sdfui\glue\glyphers.dll