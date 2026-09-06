# PassoFlow web UI

The web UI is a React + Vite scenario editor backed by the FastAPI service in `src/api_server.py`.

## Run locally

From the project root:

```bash
pip install -r requirements.txt
python -m playwright install chromium  # needed for browser_* DOM actions
cd web-ui
npm install
npm run dev
```

In another terminal, start the API with `python src/api_server.py`. On Windows, `run_webapp.bat` starts both services; on macOS, `run_webapp.command` attempts the same when backend dependencies are available. Open the URL printed by Vite.

Useful checks:

```bash
npm run build
npm run lint
npm run test:smoke
```

## UI conventions

- The action palette is searchable and grouped by purpose.
- The parameter panel is schema-driven; required fields, units, paths, variables, image regions, and selectors include inline guidance.
- Visual browser actions operate the visible default browser through screen images. DOM browser actions use a separate Playwright-controlled browser and CSS selectors.
- Flow nodes use a light background tint, visible focus, and status cues that do not rely on color alone.
- The editor supports keyboard navigation, undo/redo, validation, image capture/cropping, table preview, loops, branches, and run logs.
- Update UI-facing strings in all three locales (`en`, `ja`, `zh`) in `src/i18n/translations.ts`.

The runtime remains primarily Windows-oriented because some actions use Win32 and Excel COM. The browser editor can still be opened on macOS when its backend dependencies are available.

