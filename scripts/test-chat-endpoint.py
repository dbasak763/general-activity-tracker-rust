#!/usr/bin/env python3
"""Exercise browser-facing Rust chat with synthetic cross-activity data."""

import json
import os
import urllib.error
import urllib.request
import uuid


BASE_URL = os.getenv("TEST_API_URL", "http://127.0.0.1:8080").rstrip("/")


def request(method, path, body=None, timeout=180):
    data = json.dumps(body).encode() if body is not None else None
    response = urllib.request.urlopen(
        urllib.request.Request(
            BASE_URL + path,
            data=data,
            method=method,
            headers={"Content-Type": "application/json"},
        ),
        timeout=timeout,
    )
    if response.status == 204:
        return None
    return json.load(response)


def main():
    run = uuid.uuid4().hex[:10]
    user = "chat-e2e-" + run
    ids = []
    try:
        config = request("GET", "/api/dashboard/chat/config")
        assert "local" in config["availableProviders"], config

        paper = request(
            "POST",
            "/api/activities",
            {
                "userId": user,
                "type": "research_paper",
                "title": "Synthetic embedding paper " + run,
                "status": "completed",
                "startedAt": {"$date": "2026-09-01T12:00:00Z"},
                "tags": ["embeddings", "retrieval"],
                "details": {
                    "kind": "research_paper",
                    "paperTitle": "Synthetic Dense Retrieval Study",
                    "pagesRead": 12,
                    "keyTakeaways": "Embedding quality changes retrieval precision.",
                },
            },
        )
        ids.append(paper["_id"])
        experiment = request(
            "POST",
            "/api/activities",
            {
                "userId": user,
                "type": "model_experiment",
                "title": "Synthetic embedding experiment " + run,
                "status": "completed",
                "score": 84,
                "startedAt": {"$date": "2026-09-02T12:00:00Z"},
                "tags": ["embeddings", "retrieval"],
                "details": {
                    "kind": "model_experiment",
                    "experimentName": "Synthetic retriever evaluation",
                    "modelName": "Local embedding model",
                    "dataset": "Synthetic passages",
                    "metrics": {"recall": 0.84},
                    "parameters": {"dimensions": 768},
                },
            },
        )
        ids.append(experiment["_id"])
        request(
            "POST",
            "/api/relationships",
            {
                "sourceId": paper["_id"],
                "targetId": experiment["_id"],
                "kind": "inspired_by",
                "reason": "The experiment applies the paper's retrieval idea.",
            },
        )

        answer = request(
            "POST",
            "/api/dashboard/chat",
            {
                "message": "How is my embeddings paper related to my embeddings model experiment? Use the activity records.",
                "userId": user,
                "history": [],
            },
        )
        assert answer["provider"] != "database", answer
        assert answer["route"] == "relationships", answer
        assert answer["coverage"] == {"total": 2, "loaded": 2, "complete": True}, answer
        cited = {item["id"] for item in answer["citations"]}
        assert cited and cited <= set(ids), answer
        assert answer["relationships"], answer
        print(
            json.dumps(
                {
                    "status": "PASS",
                    "provider": answer["provider"],
                    "model": answer["model"],
                    "route": answer["route"],
                    "graphBackend": answer["graphBackend"],
                    "citations": sorted(cited),
                    "relationshipPaths": len(answer["relationships"]),
                },
                indent=2,
            )
        )
    finally:
        for activity_id in reversed(ids):
            try:
                request("DELETE", "/api/activities/" + activity_id)
            except urllib.error.HTTPError as error:
                if error.code != 404:
                    raise


if __name__ == "__main__":
    main()
