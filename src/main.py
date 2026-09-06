"""Single entry point for the packaged (PyInstaller) build of passoflow.

With no arguments, starts the API server (which also serves the built web UI, see
api_server.py's static file mount). Invoked as `--run-scenario <path> [--start N] [--end N]`,
it instead runs one scenario and exits -- this lets api_server.py re-invoke this same frozen
exe (via sys.executable) to run scenarios, instead of shelling out to a bare `python`, which
won't exist on a target PC without a dev install.

Not used in development: `run_webapp.bat` still runs `python src/api_server.py` directly.
"""

import sys


def main() -> None:
    if len(sys.argv) > 1 and sys.argv[1] == "--run-scenario":
        import argparse

        from logging_config import setup_logging
        from run_scenario import run_scenario

        parser = argparse.ArgumentParser()
        parser.add_argument("yaml_path")
        parser.add_argument("--start", type=int, default=None)
        parser.add_argument("--end", type=int, default=None)
        args = parser.parse_args(sys.argv[2:])

        setup_logging()
        run_scenario(args.yaml_path, start=args.start, end=args.end)
        return

    import uvicorn

    from api_server import app

    uvicorn.run(app, host="127.0.0.1", port=8000)


if __name__ == "__main__":
    main()
