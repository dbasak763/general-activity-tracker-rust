import os
from dataclasses import dataclass, field


@dataclass
class Settings:
    token: str = field(default_factory=lambda: os.getenv("CHAT_INTERNAL_TOKEN", ""))
    local_enabled: bool = field(default_factory=lambda: os.getenv("LOCAL_LLM_ENABLED", "true").lower() == "true")
    local_url: str = field(default_factory=lambda: os.getenv("LOCAL_LLM_BASE_URL", "http://127.0.0.1:11434/v1"))
    local_model: str = field(default_factory=lambda: os.getenv("LOCAL_LLM_MODEL", "qwen3:8b"))
    cloud_key: str = field(default_factory=lambda: os.getenv("GROQ_API_KEY", ""))
    cloud_url: str = field(default_factory=lambda: os.getenv("GROQ_BASE_URL", "https://api.groq.com/openai/v1"))
    neo4j_uri: str = field(default_factory=lambda: os.getenv("NEO4J_URI", ""))
    neo4j_user: str = field(default_factory=lambda: os.getenv("NEO4J_USER", "neo4j"))
    neo4j_password: str = field(default_factory=lambda: os.getenv("NEO4J_PASSWORD", ""))
    neo4j_database: str = field(default_factory=lambda: os.getenv("NEO4J_DATABASE", "neo4j"))

    def providers(self, intent):
        preferred = os.getenv("LLM_" + intent.upper() + "_PROVIDER", "local" if intent == "lookup" else "groq")
        available = (["local"] if self.local_enabled else []) + (["groq"] if self.cloud_key else [])
        return list(dict.fromkeys([p for p in [preferred, *available] if p in available]))

    def provider(self, name, intent):
        if name == "local":
            return self.local_url, self.local_model, "ollama"
        model = os.getenv("GROQ_" + intent.upper() + "_MODEL", "openai/gpt-oss-20b" if intent == "lookup" else "openai/gpt-oss-120b")
        return self.cloud_url, model, self.cloud_key
