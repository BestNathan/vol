CREATE TABLE IF NOT EXISTS tasks (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    status TEXT NOT NULL,
    kind TEXT NOT NULL,
    publisher TEXT NULL,
    assignee TEXT NULL,
    subject TEXT NOT NULL,
    description TEXT NOT NULL,
    active_form TEXT NULL,
    dependencies_json TEXT NOT NULL,
    blocks_json TEXT NOT NULL,
    result_json TEXT NULL,
    summary TEXT NULL,
    output_file TEXT NULL,
    created_at_secs INTEGER NOT NULL,
    started_at_secs INTEGER NULL,
    completed_at_secs INTEGER NULL
);

CREATE INDEX IF NOT EXISTS idx_tasks_status ON tasks(status);
