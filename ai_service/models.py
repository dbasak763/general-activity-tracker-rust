from typing import Any, Literal

from pydantic import BaseModel, ConfigDict, Field

ActivityType = Literal["interview", "leet_code", "codeforces", "logic_puzzle", "ai_ml_topic", "research_paper", "model_experiment", "project_milestone", "job_application", "networking_interaction"]
LinkKind = Literal["related_to", "addresses", "applies", "inspired_by", "part_of", "prerequisite_for"]


class StrictModel(BaseModel):
    model_config = ConfigDict(extra="forbid")


class Message(StrictModel):
    role: Literal["user", "assistant"]
    content: str = Field(min_length=1, max_length=8000)


class ChatInput(StrictModel):
    message: str = Field(min_length=1, max_length=8000)
    history: list[Message] = Field(default_factory=list, max_length=10)
    records: list[dict[str, Any]] = Field(max_length=2000)
    relationships: list[dict[str, Any]] = Field(default_factory=list, max_length=2000)
    totalCount: int = Field(ge=0)
    complete: bool
    scope: str = Field(min_length=1, max_length=250)
    today: str


class Filters(StrictModel):
    types: list[ActivityType] = Field(default_factory=list, max_length=10)
    text: str = Field(default="", max_length=200)
    startDate: str | None = Field(default=None, pattern=r"^\d{4}-\d{2}-\d{2}$")
    endDate: str | None = Field(default=None, pattern=r"^\d{4}-\d{2}-\d{2}$")
    status: Literal["planned", "in_progress", "completed", "incomplete", "invalidated", "skipped"] | None = None
    maxScore: float | None = Field(default=None, ge=0, le=100)
    limit: int = Field(default=20, ge=1, le=50)


class Related(StrictModel):
    activityIds: list[str] = Field(default_factory=list, max_length=10)
    text: str = Field(default="", max_length=200)
    hops: int = Field(default=4, ge=1, le=4)


class Proposal(StrictModel):
    sourceId: str
    targetId: str
    kind: LinkKind
    reason: str = Field(min_length=1, max_length=1000)


class ModelAnswer(StrictModel):
    reply: str = Field(min_length=1, max_length=16000)
    citations: list[str] = Field(default_factory=list, max_length=30)
    relationshipProposals: list[Proposal] = Field(default_factory=list, max_length=5)
