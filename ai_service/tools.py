"""Read-only operations over the Rust-validated evidence snapshot."""
import json
import re
from collections import Counter, defaultdict
from datetime import date, datetime, timezone

from .graph import connected_paths
from .models import Filters, Related


def activity_date(record):
    value = (record.get("details") or {}).get("attemptedDate")
    if not value:
        value = record.get("completedAt") or record.get("startedAt") or record.get("createdAt")
    if isinstance(value, dict):
        value = value.get("$date")
    if isinstance(value, dict):
        value = datetime.fromtimestamp(int(value["$numberLong"]) / 1000, timezone.utc).isoformat()
    return str(value or "")[:10]


def filtered(records, args):
    if args.startDate:
        date.fromisoformat(args.startDate)
    if args.endDate:
        date.fromisoformat(args.endDate)
    if args.startDate and args.endDate and args.startDate > args.endDate:
        raise ValueError("startDate must not follow endDate")
    selected = []
    for record in records:
        when = activity_date(record)
        if args.types and record["type"] not in args.types:
            continue
        if args.status and record.get("status") != args.status:
            continue
        if args.startDate and (not when or when < args.startDate):
            continue
        if args.endDate and (not when or when > args.endDate):
            continue
        if args.maxScore is not None and (record.get("score") is None or record["score"] > args.maxScore):
            continue
        if args.text and args.text.casefold() not in json.dumps(record, ensure_ascii=False).casefold():
            continue
        selected.append(record)
    return sorted(selected, key=lambda r: (activity_date(r), r["_id"]), reverse=True)


def summary(records):
    grouped = defaultdict(list)
    for record in records:
        grouped[record["type"]].append(record)
    result = []
    for kind, values in sorted(grouped.items()):
        scores = [r["score"] for r in values if r.get("score") is not None]
        result.append(dict(type=kind, count=len(values), scoredCount=len(scores),
            averageScore=round(sum(scores) / len(scores), 2) if scores else None,
            statusCounts=dict(Counter(r.get("status", "unknown") for r in values)),
            durationMinutes=sum(r.get("durationMinutes") or 0 for r in values)))
    return result


TOOL_MODELS = {"search_activities": Filters, "summarize_activities": Filters, "related_activities": Related}
DESCRIPTIONS = {
    "search_activities": "Find activities across all types by type, text, date, status, or maximum score. Returns newest first. Dates are inclusive YYYY-MM-DD. Text is one literal phrase. No types means all types.",
    "summarize_activities": "Count and compare activity types, average scores excluding unscored records, time spent, and statuses. Uses all matching snapshot rows, not just displayed rows. Returns coverage and chart data.",
    "related_activities": "Find paths connecting activities via recorded concepts, explicit weaknesses, shared entities, and user-confirmed links. Use activity IDs from search, or text to select seeds. Hops 4 allows two shared-concept joins. Associations are not causal proof.",
}
TOOLS = [dict(type="function", function=dict(name=name, description=DESCRIPTIONS[name], parameters=model.model_json_schema())) for name, model in TOOL_MODELS.items()]


async def execute_tool(name, raw, request, graph, projection, graph_ready):
    if name not in TOOL_MODELS:
        raise ValueError("Unapproved operation")
    args = TOOL_MODELS[name].model_validate_json(raw) if isinstance(raw, str) else TOOL_MODELS[name].model_validate(raw)
    coverage = dict(complete=request.complete, loaded=len(request.records), total=request.totalCount)
    if name in ("search_activities", "summarize_activities"):
        records = filtered(request.records, args)
        result = dict(matchedCount=len(records), coverage=coverage, records=records[:args.limit])
        if name == "summarize_activities":
            groups = summary(records)
            result["groups"] = groups
            result["chart"] = dict(title="Recorded activity counts" if request.complete else "Activity counts in loaded snapshot", labels=[g["type"] for g in groups], values=[g["count"] for g in groups])
        return result
    ids = {r["_id"] for r in request.records}
    if any(key not in ids for key in args.activityIds):
        raise ValueError("Unknown or out-of-scope activity ID")
    seeds = args.activityIds
    if not seeds:
        # Natural-language seed descriptions often span several records. Rank their
        # substantive words rather than requiring the entire sentence in one row.
        words = set(re.findall(r"[a-z0-9]+", args.text.lower())) - {"my", "the", "and", "to", "of", "with", "how", "is", "are", "related", "activities"}
        ranked = sorted(request.records, key=lambda r: len(words & set(re.findall(r"[a-z0-9]+", json.dumps(r).lower()))), reverse=True)
        seeds = [r["_id"] for r in ranked if not words or words & set(re.findall(r"[a-z0-9]+", json.dumps(r).lower()))][:10]
    paths = await projection.paths(request.scope, graph, seeds, args.hops) if graph_ready else connected_paths(graph, seeds, args.hops)
    used = {n["id"] for path in paths for n in path["nodes"]} | set(seeds)
    return dict(paths=paths, coverage=coverage, records=[r for r in request.records if r["_id"] in used][:50], graphBackend="neo4j" if graph_ready else "derived")
