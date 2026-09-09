"""Launch the local worker, optionally reusing provider settings from the old tracker."""
import argparse
import os
from pathlib import Path
import shlex
import sys

import uvicorn

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT))


def load_provider_env(path):
    # Parse values as data; never source/evaluate a configuration file as shell code.
    for line in Path(path).read_text().splitlines():
        key, separator, value = line.partition("=")
        key = key.strip()
        if separator and key.startswith(("GROQ_", "LOCAL_LLM_", "LLM_")):
            parts = shlex.split(value, comments=True)
            if parts:
                os.environ.setdefault(key, " ".join(parts))


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--provider-env", help="Existing tracker .env; only model-provider settings are loaded")
    parser.add_argument("--port", type=int, default=8090)
    args = parser.parse_args()
    if args.provider_env:
        load_provider_env(args.provider_env)
    uvicorn.run("ai_service.main:app", host="127.0.0.1", port=args.port)
