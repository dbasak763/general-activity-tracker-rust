// Field name, label, input type, required, choices or numeric bounds.
const activityTypes = {
  leet_code: { label: "LeetCode attempt", fields: [
    ["problem", "Problem", "text", true], ["problemNumber", "Problem number", "number", false, [1,5000]],
    ["difficulty", "Difficulty", "select", true, ["easy","medium","hard"]], ["language", "Language", "text"],
    ["runtimeMs", "Runtime (ms)", "number", false, [0]], ["memoryKb", "Memory (KB)", "number", false, [0]],
    ["accepted", "Accepted", "boolean", true], ["techniques", "Techniques", "list"]
  ]},
  codeforces: { label: "Codeforces attempt", fields: [
    ["contestId", "Contest ID", "text", true], ["problemIndex", "Problem index", "text", true],
    ["rating", "Problem rating", "number", false, [800,4000]], ["verdict", "Verdict", "text", true],
    ["language", "Language", "text"], ["tags", "Problem tags", "list"]
  ]},
  logic_puzzle: { label: "Logic puzzle attempt", fields: [
    ["source", "Puzzle source", "text", true], ["solved", "Solved", "boolean", true],
    ["attempts", "Number of attempts", "number", false, [1]], ["solutionSummary", "Solution / approach", "textarea"]
  ]},
  ai_ml_topic: { label: "AI / ML study attempt", fields: [
    ["topic", "Topic", "text", true], ["learningResource", "Learning resource", "text"],
    ["masteryPercent", "Mastery (%)", "number", false, [0,100]], ["concepts", "Concepts", "list"]
  ]},
  research_paper: { label: "Research paper reading", fields: [
    ["paperTitle", "Paper title", "text", true], ["authors", "Authors", "list"],
    ["publicationYear", "Publication year", "number", false, [0,65535]], ["paperId", "Paper ID / DOI", "text"],
    ["pagesRead", "Pages read", "number", false, [1]], ["keyTakeaways", "Key takeaways", "textarea"]
  ]},
  model_experiment: { label: "Model experiment", fields: [
    ["experimentName", "Experiment name", "text", true], ["experimentId", "Experiment ID", "text"],
    ["modelName", "Model name", "text", true], ["dataset", "Dataset", "text"],
    ["metrics", "Metrics (JSON numbers)", "json"], ["parameters", "Parameters (JSON)", "json"]
  ]},
  project_milestone: { label: "Project milestone", fields: [
    ["projectId", "Project ID", "text"], ["milestone", "Milestone", "text", true],
    ["completionPercent", "Completion (%)", "number", true, [0,100]], ["release", "Release", "text"]
  ]},
  job_application: { label: "Job application", fields: [
    ["applicationId", "Application ID", "text"], ["company", "Company", "text", true],
    ["role", "Role", "text", true], ["stage", "Stage", "select", true, ["saved","applied","recruiter_screen","interview","offer","rejected","withdrawn"]], ["location", "Location", "text"]
  ]},
  networking_interaction: { label: "Networking interaction", fields: [
    ["personId", "Person ID", "text"], ["personName", "Person name", "text", true],
    ["interactionType", "Interaction type", "select", true, ["message","call","meeting","event","referral","follow_up"]],
    ["organization", "Organization", "text"], ["followUpAt", "Follow-up date", "datetime"]
  ]}
};
