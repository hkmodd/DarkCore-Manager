@echo off
REM ============================================
REM  WATCHER TEST SCRIPT - Force Outdated Game
REM ============================================
REM This script modifies a local appmanifest to
REM simulate an outdated game for testing.
REM
REM USAGE: force_outdate_test.bat <STEAM_PATH> <APPID>
REM EXAMPLE: force_outdate_test.bat "C:\Program Files (x86)\Steam" 431960
REM ============================================

if "%~1"=="" (
    echo ERROR: Missing STEAM_PATH argument
    echo USAGE: force_outdate_test.bat "STEAM_PATH" APPID
    exit /b 1
)

if "%~2"=="" (
    echo ERROR: Missing APPID argument
    echo USAGE: force_outdate_test.bat "STEAM_PATH" APPID
    exit /b 1
)

set STEAM_PATH=%~1
set APPID=%~2
set ACF_FILE=%STEAM_PATH%\steamapps\appmanifest_%APPID%.acf
set BACKUP_FILE=%STEAM_PATH%\steamapps\appmanifest_%APPID%.acf.backup

echo.
echo [TEST] Forcing outdated state for AppID: %APPID%
echo [TEST] ACF File: %ACF_FILE%
echo.

if not exist "%ACF_FILE%" (
    echo ERROR: ACF file not found!
    echo Make sure the game is installed.
    exit /b 1
)

REM Create backup
echo [1/3] Creating backup...
copy "%ACF_FILE%" "%BACKUP_FILE%" >nul
echo       Backup saved: %BACKUP_FILE%

REM Modify buildid to 1 (very old)
echo [2/3] Setting buildid to 1 (simulating outdated)...
powershell -Command "(Get-Content '%ACF_FILE%') -replace '\"buildid\"\s*\"[0-9]+\"', '\"buildid\"		\"1\"' | Set-Content '%ACF_FILE%'"

echo [3/3] Done!
echo.
echo ============================================
echo  TEST READY
echo ============================================
echo The game %APPID% now has buildid=1.
echo.
echo NEXT STEPS:
echo 1. Launch DarkCore-Manager
echo 2. Go to Library tab
echo 3. Click "CHECK UPDATES"
echo 4. You should see the orange indicator on %APPID%
echo 5. Click "AGGIORNA" to test download
echo.
echo TO RESTORE:
echo copy "%BACKUP_FILE%" "%ACF_FILE%"
echo ============================================
