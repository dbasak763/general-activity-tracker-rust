const form = document.querySelector("#attempt-form");
const formStatus = document.querySelector("#form-status");
const rows = document.querySelector("#attempt-rows");
const summary = document.querySelector("#summary");
const topicFilter = document.querySelector("#topic-filter");

function setFormDefaults() {
  const now = new Date();
  form.elements.attemptedDate.value = now.toISOString().slice(0, 10);
  form.elements.startedAt.value = new Date(now.getTime() - now.getTimezoneOffset() * 60000).toISOString().slice(0, 16);
  form.elements.status.value = "complete";
  form.elements.attemptSource.value = "manual";
}
setFormDefaults();

async function api(path, options) {
  const response = await fetch(path, options);
  if (!response.ok) {
    const body = await response.json().catch(() => ({}));
    throw new Error(body.detail || `Request failed (${response.status})`);
  }
  return response.status === 204 ? null : response.json();
}

async function loadHealth() {
  try {
    const result = await api("/health/ready");
    document.querySelector("#health").textContent = `MongoDB ready · ${result.database}`;
  } catch (error) {
    document.querySelector("#health").textContent = error.message;
  }
}

async function loadAttempts() {
  rows.innerHTML = '<tr><td colspan="7">Loading…</td></tr>';
  const query = new URLSearchParams({ limit: "500" });
  if (topicFilter.value) query.set("topic", topicFilter.value);
  try {
    const attempts = await api(`/api/attempts?${query}`);
    rows.replaceChildren(...attempts.map(renderAttempt));
    summary.textContent = `${attempts.length} ${attempts.length === 1 ? "attempt" : "attempts"}`;
    if (!topicFilter.value) {
      const selected = topicFilter.value;
      const topics = [...new Set(attempts.map((attempt) => attempt.topic))].sort();
      topicFilter.replaceChildren(new Option("All topics", ""), ...topics.map((topic) => new Option(topic, topic)));
      topicFilter.value = selected;
    }
    if (!attempts.length) rows.innerHTML = '<tr><td colspan="7">No attempts found.</td></tr>';
  } catch (error) {
    const row = document.createElement("tr");
    const cell = document.createElement("td");
    cell.colSpan = 7;
    cell.textContent = error.message;
    row.append(cell);
    rows.replaceChildren(row);
  }
}

function renderAttempt(attempt) {
  const row = document.createElement("tr");
  const values = [attempt.attemptedDate, attempt.topic, [attempt.company, attempt.role].filter(Boolean).join(" · ") || "—", attempt.status, attempt.score ?? "—", attempt.notes || "—"];
  values.forEach((value) => { const cell = document.createElement("td"); cell.textContent = value; row.append(cell); });
  const action = document.createElement("td");
  const button = document.createElement("button");
  button.className = "danger"; button.textContent = "Delete";
  button.addEventListener("click", async () => {
    if (!confirm(`Delete attempt ${attempt.id}?`)) return;
    button.disabled = true;
    formStatus.textContent = `Deleting attempt ${attempt.id}…`;
    try {
      await api(`/api/attempts/${attempt.id}`, { method: "DELETE" });
      formStatus.textContent = `Deleted attempt ${attempt.id}.`;
      await loadAttempts();
    } catch (error) {
      formStatus.textContent = error.message;
      button.disabled = false;
    }
  });
  action.append(button); row.append(action); return row;
}

form.addEventListener("submit", async (event) => {
  event.preventDefault();
  const submitButton = form.querySelector('button[type="submit"]');
  const data = new FormData(form);
  const status = data.get("status");
  const rawScore = data.get("score");
  if (status === "complete" && rawScore === "") {
    form.elements.score.setCustomValidity("A completed attempt requires a score.");
    form.reportValidity();
    form.elements.score.setCustomValidity("");
    return;
  }
  formStatus.textContent = "Saving…";
  submitButton.disabled = true;
  const startedAt = new Date(data.get("startedAt")).toISOString();
  const payload = { attemptedDate: data.get("attemptedDate"), topic: data.get("topic"), company: data.get("company") || null, role: data.get("role") || null, score: rawScore === "" ? null : Number(rawScore), notes: data.get("notes") || null, status, attemptSource: data.get("attemptSource"), startedAt };
  try {
    await api("/api/attempts", { method: "POST", headers: { "Content-Type": "application/json" }, body: JSON.stringify(payload) });
    formStatus.textContent = "Saved and persisted in MongoDB."; form.reset();
    setFormDefaults();
    await loadAttempts();
  } catch (error) {
    formStatus.textContent = error.message;
  } finally {
    submitButton.disabled = false;
  }
});

document.querySelector("#refresh").addEventListener("click", loadAttempts);
topicFilter.addEventListener("change", loadAttempts);
loadHealth(); loadAttempts();
