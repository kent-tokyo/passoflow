import sys
from pathlib import Path

from fastapi.testclient import TestClient

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "src"))

import api_server


client = TestClient(api_server.app)


def test_manual_defaults_to_general_guide():
    response = client.get("/docs/manual?lang=en")

    assert response.status_code == 200
    assert "Using PassoFlow" in response.text
    assert "audience=advanced" in response.text


def test_advanced_manual_remains_available():
    response = client.get("/docs/manual?lang=en&audience=advanced")

    assert response.status_code == 200
    assert "Action reference" in response.text


def test_manual_asset_is_allowlisted():
    response = client.get("/docs/assets/screenshot_passoflow_01.png")

    assert response.status_code == 200
    assert response.headers["content-type"].startswith("image/png")


def test_manual_asset_rejects_arbitrary_paths():
    response = client.get("/docs/assets/api_server.py")

    assert response.status_code == 404
