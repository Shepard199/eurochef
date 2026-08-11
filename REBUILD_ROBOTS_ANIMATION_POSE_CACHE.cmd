@echo off
setlocal

set "ROOT=%~dp0"
if not defined ROBOTS_GAME_ROOT set "ROBOTS_GAME_ROOT=D:\Games\Robots"
if not defined ROBOTS_POSE_CACHE_WORKERS set "ROBOTS_POSE_CACHE_WORKERS=8"

set "ROBOTS_EXE=%ROBOTS_GAME_ROOT%\Robots.exe"
set "MANIFEST=%ROBOTS_GAME_ROOT%\_eurotools_out\edb\manifest.tsv"
set "BINDINGS=%ROOT%target\anim_binding_corpus_v1\animation_skin_bindings.tsv"
set "ANIMSKINS=%ROOT%target\anim_binding_corpus_v1\animskin_rows.tsv"
set "SCRIPT_HEALTH=%ROOT%target\script_health\script_health_report.json"
set "OUTPUT=%ROOT%target\robots_animation_pose_cache"
set "GENERATOR=%ROOT%tools\robots_animation_pose_cache.py"

if not exist "%ROBOTS_EXE%" (
    echo [pose-cache] Missing Robots executable: %ROBOTS_EXE%
    exit /b 2
)
if not exist "%MANIFEST%" (
    echo [pose-cache] Missing canonical EDB manifest: %MANIFEST%
    exit /b 3
)
if not exist "%BINDINGS%" (
    echo [pose-cache] Missing Animation binding corpus: %BINDINGS%
    exit /b 4
)
if not exist "%ANIMSKINS%" (
    echo [pose-cache] Missing AnimSkin corpus: %ANIMSKINS%
    exit /b 5
)
if not exist "%GENERATOR%" (
    echo [pose-cache] Missing RAPCV003 generator: %GENERATOR%
    exit /b 6
)

rem Always refresh Script resource resolution before deriving exact pose pairs.
rem This prevents stale script-health JSON from silently resurrecting old bindings.
echo [pose-cache] Refreshing canonical Script health report...
call "%ROOT%CARGO.cmd" --offline run -p eurochef-cli -- edb script-health "%MANIFEST%" "%ROOT%target\script_health"
if errorlevel 1 exit /b %ERRORLEVEL%
if not exist "%SCRIPT_HEALTH%" (
    echo [pose-cache] Script health report was not produced: %SCRIPT_HEALTH%
    exit /b 7
)

rem Rebuild only Script-resolved non-native Animation/AnimSkin variants.
rem The generator validates existing RAPCV003 files and automatically replaces
rem stale RAPCV001/RAPCV002 sidecars. Extra arguments are passed through, e.g.
rem   REBUILD_ROBOTS_ANIMATION_POSE_CACHE.cmd --max-clips 5 --workers 1
py -3.13 "%GENERATOR%" "%ROBOTS_EXE%" "%BINDINGS%" "%ANIMSKINS%" "%OUTPUT%" ^
  --script-health-report "%SCRIPT_HEALTH%" ^
  --script-bound-only ^
  --workers %ROBOTS_POSE_CACHE_WORKERS% ^
  %*
exit /b %ERRORLEVEL%
