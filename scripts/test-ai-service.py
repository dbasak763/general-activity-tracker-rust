"""Opt-in live provider / graph smoke test using only synthetic activity records."""
import argparse
import asyncio
import os
from pathlib import Path
import runpy
import sys
import uuid

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT))

from ai_service.graph import Neo4jProjection, build_graph
from ai_service.settings import Settings
from ai_service.test_workflow import fixture
from ai_service.workflow import Workflow


async def test(args):
    if args.provider_env:
        runpy.run_path(str(ROOT / "scripts/run-ai-service.py"))["load_provider_env"](args.provider_env)
    settings = Settings()
    if args.provider == "local":
        settings.cloud_key = ""
        settings.local_enabled = True
    else:
        settings.local_enabled = False
        if not settings.cloud_key:
            raise RuntimeError("Groq is not configured")
    projection = Neo4jProjection(settings)
    workflow = Workflow(settings, projection)
    request = fixture()
    request.scope = "verification-" + str(uuid.uuid4())
    request.message = "How is my tree coding practice related to my interview weaknesses? Use the stored relationship paths."
    try:
        if args.neo4j:
            graph = build_graph(request.records, [])
            assert await projection.sync(request.scope, graph, True)
            paths = await projection.paths(request.scope, graph, ["interview-1"], 4)
            assert any(p["nodes"][-1]["id"] == "coding-1" for p in paths)
            changed = build_graph([r for r in request.records if r["_id"] != "coding-1"], [])
            await projection.sync(request.scope, changed, True)
            assert not await projection.paths(request.scope, changed, ["interview-1"], 4)
            assert not await projection.paths(request.scope + "-other", graph, ["interview-1"], 4)
            print("PASS Neo4j: concept paths, deleted activity reconciliation, scope isolation")
        result = await workflow.run(request)
        print("Provider:", result["provider"], "Model:", result["model"], "Graph:", result["graphBackend"])
        print("Operations:", result["operations"])
        print("Reply:", result["reply"])
        print("Warnings:", result["warnings"])
        assert result["provider"] == args.provider, "Expected a live model answer, not a fallback"
        assert result["citations"] and result["relationships"]
        if args.neo4j:
            assert result["graphBackend"] == "neo4j"
        print("PASS live model: evidence-backed answer with cross-activity relationships")
    finally:
        if args.neo4j:
            # Remove only this run's isolated, synthetic graph projection.
            await projection.sync(request.scope, {"nodes": [], "edges": []}, True)
        await workflow.close()
        await projection.close()


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--provider", choices=["local", "groq"], required=True)
    parser.add_argument("--provider-env")
    parser.add_argument("--neo4j", action="store_true")
    asyncio.run(test(parser.parse_args()))
