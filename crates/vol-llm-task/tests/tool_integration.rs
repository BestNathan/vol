//! Integration test: register_all + full tool workflow.

use std::sync::Arc;

use vol_llm_task::InMemoryTaskStore;
use vol_llm_task::tools;
use vol_llm_task::TaskStore;
use vol_llm_tool::ToolRegistry;

#[tokio::test]
async fn test_register_all() {
    let store = Arc::new(InMemoryTaskStore::new());
    let mut registry = ToolRegistry::new();
    tools::register_all(&mut registry, store);

    let defs = registry.definitions();
    assert_eq!(defs.len(), 7);

    let names: Vec<_> = defs.iter().map(|d| &d.name).collect();
    assert!(names.contains(&&"task_create".to_string()));
    assert!(names.contains(&&"task_list".to_string()));
    assert!(names.contains(&&"task_get".to_string()));
    assert!(names.contains(&&"task_update".to_string()));
    assert!(names.contains(&&"task_output".to_string()));
    assert!(names.contains(&&"task_stop".to_string()));
}

#[tokio::test]
async fn test_full_workflow() {
    use vol_llm_tool::ExecutableTool;
    use vol_llm_tool::ToolContext;

    let store = Arc::new(InMemoryTaskStore::new());
    let create_tool = tools::TaskCreate::new(store.clone());
    let list_tool = tools::TaskList::new(store.clone());
    let get_tool = tools::TaskGet::new(store.clone());
    let update_tool = tools::TaskUpdate::new(store.clone());
    let ctx = ToolContext::default();

    // Create a task
    let result = create_tool
        .execute(
            &serde_json::json!({
                "subject": "build feature",
                "description": "implement the main feature"
            }),
            &ctx,
        )
        .await
        .unwrap();
    assert!(result.success);

    // List tasks
    let result = list_tool
        .execute(&serde_json::json!({}), &ctx)
        .await
        .unwrap();
    assert!(result.success);
    let data = result.data.unwrap();
    let tasks = data.get("tasks").unwrap().as_array().unwrap();
    assert_eq!(tasks.len(), 1);

    // Get the task
    let result = get_tool
        .execute(&serde_json::json!({ "taskId": "1" }), &ctx)
        .await
        .unwrap();
    assert!(result.success);
    let data = result.data.unwrap();
    let task = data.get("task").unwrap();
    assert_eq!(task.get("subject").unwrap(), "build feature");

    // Update task status
    let result = update_tool
        .execute(
            &serde_json::json!({
                "taskId": "1",
                "status": "completed"
            }),
            &ctx,
        )
        .await
        .unwrap();
    assert!(result.success);

    // Verify status changed
    let task: vol_llm_task::Task = store
        .get(&vol_llm_task::TaskId(1))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(task.status, vol_llm_task::TaskStatus::Completed);
}
