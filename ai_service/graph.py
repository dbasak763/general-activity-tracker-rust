"""Evidence-based graph construction and a rebuildable Neo4j projection."""
import hashlib
import json
import re
from collections import deque

from neo4j import AsyncGraphDatabase


def normalize(value):
    return re.sub(r"\s+", " ", str(value).strip().lower())


def stable_id(*parts):
    return hashlib.sha256(json.dumps(parts, ensure_ascii=False).encode()).hexdigest()[:32]


def build_graph(records, relationships):
    nodes, edges = {}, {}

    def edge(source, target, kind, reason, origin):
        key = stable_id(source, target, kind, reason)
        edges[key] = dict(id=key, source=source, target=target, kind=kind, reason=reason, origin=origin)

    for record in records:
        activity_id = record["_id"]
        nodes[activity_id] = dict(id=activity_id, label=record["title"], kind=record["type"])
        details, metadata = record.get("details") or {}, record.get("metadata") or {}
        concepts = list(record.get("tags") or [])
        for key in ("concepts", "techniques", "tags"):
            concepts.extend(details.get(key) or [])
        for key in ("topic", "focusTopic"):
            if details.get(key):
                concepts.append(details[key])
        for key in ("concepts", "topics"):
            if isinstance(metadata.get(key), list):
                concepts.extend(metadata[key])
        relation = {"leet_code": "tests", "codeforces": "tests", "logic_puzzle": "tests", "research_paper": "covers", "model_experiment": "uses", "project_milestone": "uses"}.get(record["type"], "covers")
        for concept in concepts[:40]:
            if not isinstance(concept, str) or not normalize(concept):
                continue
            key = "concept:" + stable_id(normalize(concept))
            nodes[key] = dict(id=key, label=normalize(concept), kind="concept")
            edge(activity_id, key, relation, "Recorded topic/tag: " + concept, "recorded_field")
        # Only explicit structured weaknesses get this stronger semantic label.
        for weakness in metadata.get("weaknesses", []) if isinstance(metadata.get("weaknesses"), list) else []:
            if isinstance(weakness, str) and normalize(weakness):
                key = "concept:" + stable_id(normalize(weakness))
                nodes[key] = dict(id=key, label=normalize(weakness), kind="concept")
                edge(activity_id, key, "identified_weakness", "Recorded weakness: " + weakness, "recorded_field")
        entities = dict(record.get("entityRefs") or {})
        for field, entity in (("projectId", "project"), ("paperId", "paper"), ("applicationId", "application"), ("personId", "person"), ("company", "company"), ("organization", "company")):
            if details.get(field):
                entities[entity] = details[field]
        for kind, value in list(entities.items())[:20]:
            if not isinstance(value, str) or not normalize(value):
                continue
            key = "entity:" + stable_id(kind, normalize(value))
            nodes[key] = dict(id=key, label=value, kind=kind)
            edge(activity_id, key, "references", f"Recorded {kind}: {value}", "recorded_field")
    for link in relationships:
        source, target = link.get("sourceId"), link.get("targetId")
        if source in nodes and target in nodes:
            edge(source, target, link["kind"], link["reason"], "user_confirmed")
    return dict(nodes=list(nodes.values()), edges=list(edges.values()))


def connected_paths(graph, seeds, hops=4):
    adjacency = {}
    nodes = {n["id"]: n for n in graph["nodes"]}
    for edge in graph["edges"]:
        adjacency.setdefault(edge["source"], []).append((edge["target"], edge))
        adjacency.setdefault(edge["target"], []).append((edge["source"], edge))
    paths = []
    for seed in seeds[:10]:
        queue = deque([(seed, [seed], [])])
        visited = {seed}
        while queue and len(paths) < 30:
            current, node_ids, route = queue.popleft()
            if len(route) >= hops:
                continue
            for neighbor, edge in adjacency.get(current, []):
                if neighbor in visited:
                    continue
                visited.add(neighbor)
                new_route = route + [edge]
                if not neighbor.startswith(("concept:", "entity:")) and neighbor != seed:
                    paths.append(dict(nodes=[nodes[n] for n in node_ids + [neighbor]], edges=new_route))
                    if len(paths) >= 30:
                        break
                queue.append((neighbor, node_ids + [neighbor], new_route))
    return paths


class Neo4jProjection:
    def __init__(self, settings):
        self.database = settings.neo4j_database
        self.driver = AsyncGraphDatabase.driver(settings.neo4j_uri, auth=(settings.neo4j_user, settings.neo4j_password), connection_timeout=3, max_transaction_retry_time=3) if settings.neo4j_uri else None

    async def sync(self, scope, graph, complete):
        if not self.driver:
            return False
        generation = stable_id(graph)

        async def update(tx):
            # This scope node serializes competing rebuilds within the transaction.
            await (await tx.run("MERGE (s:TrackerScope {id: $scope}) SET s.generation = $generation", scope=scope, generation=generation)).consume()
            await (await tx.run("UNWIND $nodes AS n MERGE (a:TrackerNode {scope: $scope, id: n.id}) SET a.label=n.label, a.kind=n.kind, a.generation=$generation", scope=scope, nodes=graph["nodes"], generation=generation)).consume()
            await (await tx.run("UNWIND $edges AS e MATCH (a:TrackerNode {scope:$scope, id:e.source}), (b:TrackerNode {scope:$scope, id:e.target}) MERGE (a)-[r:TRACKER_LINK {id:e.id}]->(b) SET r.kind=e.kind, r.reason=e.reason, r.origin=e.origin, r.generation=$generation", scope=scope, edges=graph["edges"], generation=generation)).consume()
            if complete:
                await (await tx.run("MATCH (:TrackerNode {scope:$scope})-[r:TRACKER_LINK]->() WHERE r.generation <> $generation DELETE r", scope=scope, generation=generation)).consume()
                await (await tx.run("MATCH (n:TrackerNode {scope:$scope}) WHERE n.generation <> $generation DETACH DELETE n", scope=scope, generation=generation)).consume()
        async with self.driver.session(database=self.database) as session:
            await session.execute_write(update)
        return True

    async def paths(self, scope, graph, seeds, hops):
        # No model-written Cypher. Restrict every hop to current source-of-truth evidence.
        query = """MATCH p=(a:TrackerNode)-[:TRACKER_LINK*1..4]-(b:TrackerNode)
            WHERE a.scope=$scope AND a.id IN $seeds AND b.id IN $activities AND b.id <> a.id
            AND length(p) <= $hops
            AND all(n IN nodes(p) WHERE n.scope=$scope AND n.id IN $nodes)
            AND all(r IN relationships(p) WHERE r.id IN $edges)
            RETURN [n IN nodes(p) | {id:n.id, label:n.label, kind:n.kind}] AS nodes,
            [r IN relationships(p) | {id:r.id, source:startNode(r).id, target:endNode(r).id, kind:r.kind, reason:r.reason, origin:r.origin}] AS edges
            ORDER BY length(p) LIMIT 30"""
        records, _, _ = await self.driver.execute_query(query, scope=scope, seeds=seeds, hops=hops,
            activities=[n["id"] for n in graph["nodes"] if not n["id"].startswith(("concept:", "entity:"))],
            nodes=[n["id"] for n in graph["nodes"]], edges=[e["id"] for e in graph["edges"]], database_=self.database)
        return [dict(r) for r in records]

    async def close(self):
        if self.driver:
            await self.driver.close()
