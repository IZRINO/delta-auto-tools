Start all services and open PM2 monitor.
```bash
pm2 start ecosystem.config.cjs && start wt.exe -d "." pwsh -NoExit -c "pm2 monit"
```
