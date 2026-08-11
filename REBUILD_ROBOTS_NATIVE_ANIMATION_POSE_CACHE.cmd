@echo off
setlocal

set "ROOT=%~dp0"
if not defined ROBOTS_GAME_ROOT set "ROBOTS_GAME_ROOT=D:\Games\Robots"
if not defined ROBOTS_POSE_CACHE_WORKERS set "ROBOTS_POSE_CACHE_WORKERS=8"

set "ROBOTS_EXE=%ROBOTS_GAME_ROOT%\Robots.exe"
set "BINDINGS=%ROOT%target\anim_binding_corpus_v1\animation_skin_bindings.tsv"
set "ANIMSKINS=%ROOT%target\anim_binding_corpus_v1\animskin_rows.tsv"
set "OUTPUT=%ROOT%target\robots_animation_pose_cache"
set "GENERATOR=%ROOT%tools\robots_animation_pose_cache.py"

if not exist "%ROBOTS_EXE%" (
    echo [pose-cache] Missing Robots executable: %ROBOTS_EXE%
    exit /b 2
)
if not exist "%BINDINGS%" (
    echo [pose-cache] Missing Animation binding corpus: %BINDINGS%
    exit /b 3
)
if not exist "%ANIMSKINS%" (
    echo [pose-cache] Missing AnimSkin corpus: %ANIMSKINS%
    exit /b 4
)
if not exist "%GENERATOR%" (
    echo [pose-cache] Missing RAPCV003 generator: %GENERATOR%
    exit /b 5
)

rem Rebuild native Animation.skin_num -> AnimSkin.base_skin_num RAPCV003 caches.
rem Existing valid V3 files are reused; stale RAPCV001/RAPCV002 files are replaced.
rem Extra generator arguments are passed through, for example:
rem   REBUILD_ROBOTS_NATIVE_ANIMATION_POSE_CACHE.cmd --edb-uid 0x0100001F
py -3.13 "%GENERATOR%" "%ROBOTS_EXE%" "%BINDINGS%" "%ANIMSKINS%" "%OUTPUT%" ^
  --workers %ROBOTS_POSE_CACHE_WORKERS% ^
  %*
exit /b %ERRORLEVEL%
