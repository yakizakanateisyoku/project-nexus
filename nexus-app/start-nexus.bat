@echo off
REM Refresh env and start Nexus
for /f "tokens=2*" %%a in ('reg query "HKCU\Environment" /v ANTHROPIC_API_KEY 2^>nul') do set ANTHROPIC_API_KEY=%%b
start "" "C:\Users\annih\Documents\GitRepository\project-nexus\nexus-app\src-tauri\target\debug\nexus-app.exe"
