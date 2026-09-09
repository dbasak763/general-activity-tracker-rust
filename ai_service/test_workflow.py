import json
import unittest
from unittest.mock import patch

import httpx

from ai_service.graph import Neo4jProjection, build_graph, connected_paths
from ai_service.main import create_app
from ai_service.models import ChatInput, Filters
from ai_service.settings import Settings
from ai_service.tools import execute_tool, filtered, summary
from ai_service.workflow import Workflow, classify


def fixture(**overrides):
    records = [
        {"_id": "interview-1", "title": "Tree interview", "type": "interview", "status": "completed", "score": 45, "tags": ["DFS"], "details": {"attemptedDate": "2026-09-01"}, "metadata": {"weaknesses": ["DFS"]}},
        {"_id": "coding-1", "title": "Tree practice", "type": "leet_code", "status": "completed", "score": 80, "tags": ["dfs"], "details": {"accepted": True}, "startedAt": {"$date": "2026-09-02T12:00:00Z"}},
        {"_id": "paper-1", "title": "Embedding paper", "type": "research_paper", "status": "in_progress", "score": None, "tags": ["embeddings"], "details": {}},
        {"_id": "experiment-1", "title": "Embedding baseline", "type": "model_experiment", "status": "completed", "score": 90, "tags": ["embeddings"], "details": {"modelName": "baseline"}},
    ]
    return ChatInput.model_validate(dict(message="Compare my interviews and LeetCode", records=records, relationships=[], totalCount=4, complete=True, scope="test", today="2026-09-07", **overrides))


class ToolsTests(unittest.IsolatedAsyncioTestCase):
    async def asyncSetUp(self):
        self.request = fixture()
        self.settings = Settings(token="test-token", local_enabled=False, cloud_key="", neo4j_uri="")
        self.projection = Neo4jProjection(self.settings)
        self.graph = build_graph(self.request.records, [])

    def test_classifies_relationships_analysis_and_visualization(self):
        self.assertEqual(classify("How is this paper related to my project?"), "relationships")
        self.assertEqual(classify("What should I practice next?"), "analysis")
        self.assertEqual(classify("Plot progress"), "visualization")

    def test_shared_concepts_connect_activity_types(self):
        paths = connected_paths(self.graph, ["interview-1"])
        self.assertTrue(any(p["nodes"][-1]["id"] == "coding-1" for p in paths))
        self.assertFalse(any(p["nodes"][-1]["id"] == "paper-1" for p in paths))
        self.assertTrue(any(e["kind"] == "identified_weakness" for e in self.graph["edges"]))

    def test_explicit_links_are_distinct_and_deleted_records_do_not_link(self):
        links = [{"sourceId": "coding-1", "targetId": "paper-1", "kind": "related_to", "reason": "Confirmed connection"}, {"sourceId": "missing", "targetId": "paper-1", "kind": "related_to", "reason": "stale"}]
        graph = build_graph(self.request.records, links)
        self.assertEqual(len([e for e in graph["edges"] if e["origin"] == "user_confirmed"]), 1)

    def test_date_score_filters_and_unscored_summary(self):
        self.assertEqual([r["_id"] for r in filtered(self.request.records, Filters(startDate="2026-09-02", endDate="2026-09-02"))], ["coding-1"])
        self.assertEqual([r["_id"] for r in filtered(self.request.records, Filters(maxScore=50))], ["interview-1"])
        groups = summary(self.request.records)
        paper = next(g for g in groups if g["type"] == "research_paper")
        self.assertIsNone(paper["averageScore"])
        with self.assertRaises(ValueError):
            filtered(self.request.records, Filters(startDate="2026-02-31"))

    async def test_rejects_model_writes_bad_filters_and_out_of_scope_ids(self):
        for name, args in [("delete_activity", {}), ("search_activities", {"userId": "someone-else"}), ("related_activities", {"activityIds": ["missing"]}), ("search_activities", {"limit": 5000})]:
            with self.assertRaises(ValueError):
                await execute_tool(name, args, self.request, self.graph, self.projection, False)

    async def test_summary_coverage_is_not_page_count(self):
        self.request.complete = False
        self.request.totalCount = 9000
        result = await execute_tool("summarize_activities", {"limit": 1}, self.request, self.graph, self.projection, False)
        self.assertEqual(result["matchedCount"], 4)
        self.assertEqual(len(result["records"]), 1)
        self.assertEqual(result["coverage"], {"complete": False, "loaded": 4, "total": 9000})

    async def test_related_seed_description_can_span_multiple_records(self):
        result = await execute_tool("related_activities", {"text": "tree coding practice interview weaknesses"}, self.request, self.graph, self.projection, False)
        self.assertTrue(result["paths"])
        self.assertTrue(any(r["_id"] == "coding-1" for r in result["records"]))

    async def test_unavailable_models_produce_labelled_database_answer(self):
        workflow = Workflow(self.settings, self.projection)
        try:
            result = await workflow.run(self.request)
            self.assertEqual(result["provider"], "database")
            self.assertEqual(result["graphBackend"], "derived")
            self.assertIn("unavailable", result["reply"])
            self.assertTrue(result["citations"])
        finally:
            await workflow.close()

    async def test_provider_tools_followup_and_cloud_failover(self):
        self.settings.local_enabled = True
        self.settings.cloud_key = "test-key"
        self.settings.local_url = "http://local.test/v1"
        self.settings.cloud_url = "http://cloud.test/v1"
        seen = []

        def respond(request):
            data = json.loads(request.content)
            seen.append((request.url.host, data))
            if request.url.host == "local.test":
                return httpx.Response(503)
            if "tools" in data and not any(m["role"] == "tool" for m in data["messages"]):
                message = {"role": "assistant", "content": None, "tool_calls": [{"id": "call-1", "type": "function", "function": {"name": "related_activities", "arguments": '{"activityIds":["interview-1"]}'}}]}
            elif "tools" in data:
                message = {"role": "assistant", "content": "I have enough evidence."}
            else:
                message = {"role": "assistant", "content": json.dumps({"reply": "Tree practice and the interview share recorded DFS evidence.", "citations": ["interview-1", "coding-1"], "relationshipProposals": []})}
            return httpx.Response(200, json={"choices": [{"message": message}]})

        workflow = Workflow(self.settings, self.projection, httpx.MockTransport(respond))
        self.request.history = []
        try:
            with patch.dict("os.environ", {"LLM_ANALYSIS_PROVIDER": "local"}):
                result = await workflow.run(self.request)
            self.assertEqual(result["provider"], "groq")
            self.assertEqual(result["operations"], ["related_activities"])
            self.assertTrue(result["relationships"])
            self.assertEqual(seen[0][0], "local.test")
            self.assertTrue(any("local" in warning for warning in result["warnings"]))
        finally:
            await workflow.close()

    async def test_hallucinated_citation_falls_back(self):
        self.settings.local_enabled = True
        def respond(request):
            data = json.loads(request.content)
            if "tools" in data:
                message = {"role": "assistant", "tool_calls": [{"id": "one", "type": "function", "function": {"name": "search_activities", "arguments": "{}"}}]}
            else:
                message = {"role": "assistant", "content": '{"reply":"Invented fact", "citations":["not-a-record"]}'}
            return httpx.Response(200, json={"choices": [{"message": message}]})
        workflow = Workflow(self.settings, self.projection, httpx.MockTransport(respond))
        try:
            self.assertEqual((await workflow.run(self.request))["provider"], "database")
        finally:
            await workflow.close()

    async def test_internal_endpoint_rejects_missing_token(self):
        app = create_app(self.settings)
        async with httpx.AsyncClient(transport=httpx.ASGITransport(app=app), base_url="http://test") as client:
            self.assertEqual((await client.post("/chat", json=self.request.model_dump())).status_code, 401)
            response = await client.post("/chat", json=self.request.model_dump(), headers={"Authorization": "Bearer test-token"})
            self.assertEqual(response.status_code, 200)
            self.assertEqual(response.json()["provider"], "database")


if __name__ == "__main__":
    unittest.main()
