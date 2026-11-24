import pytest
from fastapi.testclient import TestClient
from pydantic import ValidationError

from my_his_chatbot.config import Settings
from my_his_chatbot.main import create_app


def test_settings_read_chatbot_prefixed_env(monkeypatch: pytest.MonkeyPatch, settings: Settings) -> None:
    monkeypatch.setenv("CHATBOT_WORKERS", "3")
    monkeypatch.setenv("CHATBOT_LOG_LEVEL", "debug")

    loaded = Settings(_env_file=None)

    assert loaded.workers == 3
    assert loaded.log_level == "debug"


def test_settings_ignore_unprefixed_database_url(monkeypatch: pytest.MonkeyPatch, settings: Settings) -> None:
    # DATABASE_URL ของ backend ต้องไม่ถูกหยิบมาใช้โดยบังเอิญ
    monkeypatch.setenv("DATABASE_URL", "postgres://backend:secret@db:5432/my_his")

    assert Settings(_env_file=None).database_url is None


def test_empty_database_url_means_not_configured(monkeypatch: pytest.MonkeyPatch, settings: Settings) -> None:
    monkeypatch.setenv("CHATBOT_DATABASE_URL", "")

    assert Settings(_env_file=None).database_url is None


def test_settings_reject_invalid_worker_count(monkeypatch: pytest.MonkeyPatch, settings: Settings) -> None:
    monkeypatch.setenv("CHATBOT_WORKERS", "0")

    with pytest.raises(ValidationError):
        Settings(_env_file=None)


def test_database_password_does_not_leak_through_repr(monkeypatch: pytest.MonkeyPatch, settings: Settings) -> None:
    monkeypatch.setenv("CHATBOT_DATABASE_URL", "postgres://chatbot:topsecret@db:5432/my_his")

    loaded = Settings(_env_file=None)

    assert "topsecret" not in repr(loaded)
    assert "topsecret" not in str(loaded)


def test_app_starts_without_database_pool(settings: Settings) -> None:
    app = create_app(settings)

    with TestClient(app):
        assert app.state.db is None
