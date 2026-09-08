"""Live relationship lifecycle test. Removes only records it creates."""
import json
import os
import urllib.error
import urllib.parse
import urllib.request
import uuid

BASE = os.environ.get("TEST_API_URL", "http://127.0.0.1:18080")


def request(path, method="GET", body=None):
    data = None if body is None else json.dumps(body).encode()
    with urllib.request.urlopen(urllib.request.Request(
        BASE + path, data=data, method=method,
        headers={"Content-Type": "application/json"},
    ), timeout=30) as response:
        raw = response.read()
        return json.loads(raw) if raw else None


def run():
    ids = []
    owner = "relationship-test-" + str(uuid.uuid4())
    try:
        for name in ("Practice DFS", "Tree interview"):
            saved = request("/api/activities", "POST", {
                "userId": owner, "type": "ai_ml_topic", "title": name,
                "details": {"kind": "ai_ml_topic", "topic": "DFS"},
            })
            ids.append(saved["_id"])
        payload = {"sourceId": ids[0], "targetId": ids[1], "kind": "addresses", "reason": "Practice addresses the recorded weakness"}
        saved = request("/api/relationships", "POST", payload)
        assert request("/api/relationships", "POST", payload)["_id"] == saved["_id"]
        assert len(request("/api/relationships?activityId=" + ids[0])) == 1
        for invalid in ({**payload, "targetId": ids[0]}, {**payload, "kind": "execute"}, {**payload, "targetId": "missing"}):
            try:
                request("/api/relationships", "POST", invalid)
                raise AssertionError("Invalid relationship was accepted")
            except urllib.error.HTTPError as error:
                assert error.code in (404, 422)
        path = "/api/relationships/" + urllib.parse.quote(saved["_id"], safe="")
        request(path, "DELETE")
        assert request("/api/relationships?activityId=" + ids[0]) == []
        request("/api/relationships", "POST", payload)
        request("/api/activities/" + ids.pop(), "DELETE")
        assert request("/api/relationships?activityId=" + ids[0]) == []
        print("PASS: relationship create, idempotency, read, validation, delete and activity cleanup")
    finally:
        for activity_id in ids:
            request("/api/activities/" + activity_id, "DELETE")


if __name__ == "__main__":
    run()
