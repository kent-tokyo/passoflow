# -*- mode: python ; coding: utf-8 -*-
"""PyInstaller spec for a --onedir build of passoflow. Build with: pyinstaller passoflow.spec
(or run build_exe.bat, which also builds web-ui/dist first)."""

from PyInstaller.utils.hooks import collect_data_files

datas = [
    ("web-ui/dist", "web-ui/dist"),
    ("scenarios", "scenarios"),
    ("docs", "docs"),
    ("VERSION", "."),
    (".env.example", "."),
]
datas += collect_data_files("certifi")  # anthropic/httpx need certifi's cacert.pem bundled

a = Analysis(
    ["src/main.py"],
    pathex=["src"],
    datas=datas,
)
pyz = PYZ(a.pure)

# contents_directory="." keeps the pre-6.0 flat onedir layout (everything directly beside
# passoflow.exe) instead of PyInstaller 6+'s default `_internal` subfolder, so app_root()
# (based on sys.executable's own directory) also finds the bundled web-ui/dist alongside it.
exe = EXE(
    pyz,
    a.scripts,
    [],
    exclude_binaries=True,
    name="passoflow",
    console=True,
    contents_directory=".",
)

coll = COLLECT(
    exe,
    a.binaries,
    a.datas,
    strip=False,
    upx=False,
    name="passoflow",
)
