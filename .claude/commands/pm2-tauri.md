Start delta-auto-tools-tauri (app) and open logs.
```bash
pm2 start ecosystem.config.cjs --only delta-auto-tools-tauri && start wt.exe -d "." pwsh -NoExit -c "pm2 logs delta-auto-tools-tauri"
```
