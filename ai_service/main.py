import asyncio
import secrets
from contextlib import asynccontextmanager

from fastapi import Depends, FastAPI, Header, HTTPException

from .graph import Neo4jProjection
from .models import ChatInput
from .settings import Settings
from .workflow import Workflow


def create_app(settings=None, workflow=None):
    settings = settings or Settings()
    projection = Neo4jProjection(settings)
    workflow = workflow or Workflow(settings, projection)
    semaphore = asyncio.Semaphore(2)

    @asynccontextmanager
    async def lifespan(app):
        if not settings.token:
            raise RuntimeError("CHAT_INTERNAL_TOKEN is required for the internal AI service")
        yield
        await workflow.close()
        await projection.close()

    app = FastAPI(title="Internal activity AI worker", lifespan=lifespan)

    async def authorize(authorization: str = Header(default="")):
        if not settings.token or not secrets.compare_digest(authorization, "Bearer " + settings.token):
            raise HTTPException(401, "Invalid internal service token")

    @app.get("/health")
    async def health():
        return {"status": "ready", "orchestrator": "langgraph"}

    @app.get("/config", dependencies=[Depends(authorize)])
    async def config():
        return dict(orchestrator="langgraph", availableProviders=list(dict.fromkeys(settings.providers("lookup"))),
            routes={intent: {"providerOrder": settings.providers(intent)} for intent in ("lookup", "analysis", "relationships", "visualization")},
            graphConfigured=bool(settings.neo4j_uri))

    @app.post("/chat", dependencies=[Depends(authorize)])
    async def chat(request: ChatInput):
        try:
            async with asyncio.timeout(155):
                async with semaphore:
                    return await workflow.run(request)
        except TimeoutError as error:
            raise HTTPException(503, "AI workflow timed out") from error

    return app


app = create_app()
