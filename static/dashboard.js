const form = document.querySelector("#attempt-form");
const formStatus = document.querySelector("#form-status");
const rows = document.querySelector("#attempt-rows");
const summary = document.querySelector("#summary");
const typeFilter = document.querySelector("#type-filter");

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

let loadVersion = 0;
async function loadAttempts() {
  const version = ++loadVersion;
  rows.innerHTML = '<tr><td colspan="7">Loading…</td></tr>';
  const query = new URLSearchParams({ limit: "500" });
  if (typeFilter.value) query.set("type", typeFilter.value);
  try {
    const [activities, count] = await Promise.all([api(`/api/activities?${query}`), api(`/api/activities/count?${query}`)]);
    if (version !== loadVersion) return;
    rows.replaceChildren(...activities.map(renderActivity));
    summary.textContent = `Showing ${activities.length} of ${count.count} activities · ordered by start time`;
    if (!activities.length) rows.innerHTML = '<tr><td colspan="7">No activities found.</td></tr>';
  } catch (error) {
    if (version !== loadVersion) return;
    rows.replaceChildren();
    summary.textContent = error.message;
  }
}
function dateText(value) {
  if (!value) return "—";
  const raw = value.$date ?? value;
  return new Date(typeof raw === "object" ? Number(raw.$numberLong) : raw).toLocaleString();
}
function renderActivity(activity) {
  const row = document.createElement("tr");
  const values = [activity.details.attemptedDate || dateText(activity.completedAt || activity.startedAt || activity.createdAt), activityTypes[activity.type]?.label || "Interview", activity.title, activity.status, activity.score ?? "—", activity.notes || "—"];
  values.forEach((value) => { const cell = document.createElement("td"); cell.textContent = value; row.append(cell); });
  const details = document.createElement("details");
  const heading = document.createElement("summary"); heading.textContent = "View record";
  const pre = document.createElement("pre"); pre.textContent = JSON.stringify(activity, null, 2);
  details.append(heading, pre); row.children[2].append(details);
  const action = document.createElement("td");
  const button = document.createElement("button");
  button.className = "danger"; button.textContent = "Delete";
  button.addEventListener("click", async () => {
    if (!confirm(`Delete ${activity.title}?`)) return;
    button.disabled = true;
    try {
      await api(`/api/activities/${encodeURIComponent(activity._id)}`, { method: "DELETE" });
      await loadAttempts();
    } catch (error) { summary.textContent = error.message; button.disabled = false; }
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
typeFilter.addEventListener("change", loadAttempts);
loadHealth(); loadAttempts();

const activityForm = document.querySelector("#activity-form");
const activityType = document.querySelector("#activity-type");
const activityStatus = document.querySelector("#activity-status");
for (const [kind, schema] of Object.entries(activityTypes)) {
  activityType.add(new Option(schema.label, kind));
  typeFilter.add(new Option(schema.label, kind));
}
function renderDetails() {
  const kind = activityType.value;
  document.querySelector("#interview-panel").hidden = kind !== "interview";
  document.querySelector("#activity-panel").hidden = kind === "interview";
  if (kind === "interview") return;
  document.querySelector("#activity-heading").textContent = `Record ${activityTypes[kind].label}`;
  const container = document.querySelector("#activity-details");
  container.replaceChildren();
  const legend = document.createElement("legend"); legend.textContent = "Activity details"; container.append(legend);
  for (const [name, label, type, required, options] of activityTypes[kind].fields) {
    const wrapper = document.createElement("label"); wrapper.textContent = label + (required ? " *" : "");
    const input = document.createElement(type === "select" || type === "boolean" ? "select" : type === "json" || type === "textarea" ? "textarea" : "input");
    input.name = `detail_${name}`; input.required = Boolean(required);
    if (type === "boolean") { input.add(new Option("No", "false")); input.add(new Option("Yes", "true")); }
    else if (type === "select") for (const value of options) input.add(new Option(value.replaceAll("_", " "), value));
    else if (type === "number") { input.type = "number"; input.step = "1"; if(options) { input.min = options[0]; if(options[1] != null) input.max = options[1]; } }
    else if(type === "datetime") input.type = "datetime-local";
    else if(type === "json") input.value = "{}";
    else if(type !== "textarea") input.type = "text";
    if(type === "list") input.placeholder = "Comma separated";
    wrapper.append(input); container.append(wrapper);
  }
  if (!activityForm.elements.startedAt.value) {
    const now = new Date();
    activityForm.elements.startedAt.value = new Date(now.getTime() - now.getTimezoneOffset() * 60000).toISOString().slice(0, 16);
  }
  updatePreview();
}
const splitList = value => value.split(",").map(x => x.trim()).filter(Boolean);
function jsonObject(value, label) {
  const parsed = JSON.parse(value || "{}");
  if (!parsed || Array.isArray(parsed) || typeof parsed !== "object") throw new Error(`${label} must be a JSON object.`);
  return parsed;
}
function activityPayload() {
  const data = new FormData(activityForm), kind = activityType.value;
  const payload = { type: kind, details: { kind } };
  for (const key of ["title", "userId", "status", "priority", "category", "sourceUrl", "description", "notes", "feedback"]) payload[key] = data.get(key)?.trim() || null;
  for (const key of ["score", "rating", "durationMinutes"]) payload[key] = data.get(key) === "" ? null : Number(data.get(key));
  for (const key of ["plannedAt", "startedAt", "completedAt"]) payload[key] = data.get(key) ? { $date: new Date(data.get(key)).toISOString() } : null;
  payload.tags = splitList(data.get("tags")); payload.metadata = jsonObject(data.get("metadata"), "Metadata");
  for (const [name, label, type] of activityTypes[kind].fields) {
    const value = data.get(`detail_${name}`).trim();
    payload.details[name] = type === "boolean" ? value === "true" : type === "list" ? splitList(value) : type === "json" ? jsonObject(value, label) : !value ? null : type === "number" ? Number(value) : type === "datetime" ? { $date: new Date(value).toISOString() } : value;
  }
  return payload;
}
function updatePreview() {
  if(activityType.value === "interview") return;
  try { document.querySelector("#payload-preview").textContent = JSON.stringify(activityPayload(), null, 2); }
  catch(error) { document.querySelector("#payload-preview").textContent = error.message; }
}
activityType.addEventListener("change", renderDetails);
activityForm.addEventListener("input", updatePreview);
activityForm.addEventListener("submit", async event => {
  event.preventDefault();
  const button = activityForm.querySelector('button[type="submit"]'); button.disabled = true;
  try {
    const payload = activityPayload();
    activityStatus.textContent = "Saving…";
    await api("/api/activities", { method: "POST", headers: { "Content-Type": "application/json" }, body: JSON.stringify(payload) });
    activityStatus.textContent = "Saved and persisted in MongoDB.";
    activityForm.reset(); renderDetails(); await loadAttempts();
  } catch(error) { activityStatus.textContent = error.message; }
  finally { button.disabled = false; }
});
