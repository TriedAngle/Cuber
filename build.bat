@echo off
call cargo build --release --package glyphers_ffi
if errorlevel 1 (
    echo Error: 'cargo build --release' failed.
    exit /b %errorlevel%
)
copy target\release\glyphers.dll sdfui\glue\glyphers.dll
if errorlevel 1 (
    echo Error: Failed to move glyphers.dll.
    exit /b %errorlevel%
)
echo Successfully moved glyphers.dll to sdfui\glue\glyphers.dll