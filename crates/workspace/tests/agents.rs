mod support;

use support::{TempDir, load_json};
use workspace::{
    Access, AddedProject, AgentChoice, AgentId, ConversationId, Effort, ModelId, ProjectId,
    SessionId, Timestamp, Workspace,
};

fn choice(model: &str, effort: Effort, access: Access) -> AgentChoice {
    AgentChoice {
        agent: AgentId("claude".to_owned()),
        model: ModelId(model.to_owned()),
        effort,
        access,
    }
}

fn add(workspace: &mut Workspace, temp: &TempDir, name: &str) -> ProjectId {
    match workspace
        .add_project(&temp.dir(name), Timestamp(1))
        .unwrap()
    {
        AddedProject::Added(id) => id,
        AddedProject::Existing(_) => panic!("expected a new project"),
    }
}

/// A file with one project, a session that saw work and a draft.
const TWO_SESSIONS: &str = r#"{
  "version": 1, "next_id": 3,
  "projects": [{"id": 0, "title": "web", "path": "/web", "created_at": 1}],
  "sessions": [
    {"id": 1, "project": 0, "title": "Worked", "created_at": 1, "last_activity_at": 2,
     "choice": {"agent": "claude", "model": "claude-opus-4-1", "effort": "ultra", "access": "yolo"}},
    {"id": 2, "project": 0, "title": "New session", "created_at": 3, "last_activity_at": null,
     "choice": {"agent": "other-agent", "model": "other-model", "effort": "low", "access": "plan"}}
  ],
  "tabs": [1, 2]
}"#;

#[test]
fn sessions_start_on_opus_at_high_in_full_access() {
    let temp = TempDir::new();
    let mut workspace = Workspace::default();
    let project = add(&mut workspace, &temp, "web");
    let session = workspace.new_session(project, Timestamp(2)).unwrap();
    let expected = choice("opus", Effort::High, Access::FullAccess);
    assert_eq!(workspace.last_choice(), &expected);
    assert_eq!(workspace.session(session).unwrap().choice, expected);

    let agent = workspace::agent(&expected.agent).unwrap();
    assert_eq!(agent.name, "Claude Code");
    assert_eq!(agent.model(&expected.model).unwrap().label, "Opus 5.5");
    assert_eq!(
        agent.access_mode(Access::FullAccess).unwrap().label,
        "Full access"
    );
}

#[test]
fn a_new_session_starts_from_the_last_choice() {
    let temp = TempDir::new();
    let mut workspace = Workspace::default();
    let web = add(&mut workspace, &temp, "web");
    let api = add(&mut workspace, &temp, "api");
    let first = workspace.new_session(web, Timestamp(2)).unwrap();
    let draft = workspace.new_session(api, Timestamp(2)).unwrap();
    let picked = choice("haiku", Effort::Max, Access::Plan);
    workspace.set_choice(first, picked.clone());
    assert_eq!(workspace.session(first).unwrap().choice, picked);
    assert_eq!(workspace.last_choice(), &picked);

    // The other project's draft, opened again by New session, takes it too.
    assert_eq!(workspace.new_session(api, Timestamp(3)), Some(draft));
    assert_eq!(workspace.session(draft).unwrap().choice, picked);

    // A choice for a session that isn't there changes nothing.
    workspace.set_choice(SessionId(99), AgentChoice::default());
    assert_eq!(workspace.last_choice(), &picked);
}

#[test]
fn choices_survive_saving_and_loading() {
    let temp = TempDir::new();
    let path = temp.path().join("workspace.json");
    let mut workspace = Workspace::default();
    let web = add(&mut workspace, &temp, "web");
    let api = add(&mut workspace, &temp, "api");
    let first = workspace.new_session(web, Timestamp(2)).unwrap();
    workspace.set_choice(first, choice("sonnet", Effort::Low, Access::AcceptEdits));
    let second = workspace.new_session(api, Timestamp(3)).unwrap();
    workspace.set_choice(
        second,
        choice("fable", Effort::ExtraHigh, Access::FullAccess),
    );
    workspace.save(&path).unwrap();

    let loaded = Workspace::load(&path).unwrap();
    for session in [first, second] {
        assert_eq!(
            loaded.session(session).unwrap().choice,
            workspace.session(session).unwrap().choice
        );
    }
    assert_eq!(loaded.last_choice(), workspace.last_choice());
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.contains(r#""effort": "extra-high""#), "{text}");
    assert!(text.contains(r#""access": "full-access""#), "{text}");
}

#[test]
fn an_unknown_agent_or_model_is_kept_as_written() {
    let temp = TempDir::new();
    let path = temp.path().join("workspace.json");
    let workspace = load_json(TWO_SESSIONS);
    let worked = &workspace.session(SessionId(1)).unwrap().choice;
    assert_eq!(worked.model, ModelId("claude-opus-4-1".to_owned()));
    // An effort or access a newer build wrote reads as the default.
    assert_eq!(worked.effort, Effort::High);
    assert_eq!(worked.access, Access::FullAccess);
    let draft = &workspace.session(SessionId(2)).unwrap().choice;
    assert_eq!(draft.agent, AgentId("other-agent".to_owned()));
    assert!(workspace::agent(&draft.agent).is_none());

    workspace.save(&path).unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.contains(r#""model": "claude-opus-4-1""#), "{text}");
    assert!(text.contains(r#""agent": "other-agent""#), "{text}");
    assert!(text.contains(r#""model": "other-model""#), "{text}");
}

#[test]
fn a_file_without_choices_loads_with_the_defaults() {
    let workspace = load_json(
        r#"{"version": 1, "next_id": 2,
            "projects": [{"id": 0, "title": "web", "path": "/web", "created_at": 1}],
            "sessions": [{"id": 1, "project": 0, "title": "Old", "created_at": 1,
                          "last_activity_at": 2}]}"#,
    );
    assert_eq!(workspace.last_choice(), &AgentChoice::default());
    assert_eq!(
        workspace.session(SessionId(1)).unwrap().choice,
        AgentChoice::default()
    );
}

#[test]
fn the_choice_becomes_the_agents_arguments() {
    let claude = &workspace::AGENTS[0];
    assert_eq!(
        claude.arguments(&AgentChoice::default()),
        [
            "--model",
            "opus",
            "--effort",
            "high",
            "--permission-mode",
            "bypassPermissions"
        ]
    );
    assert_eq!(
        claude.arguments(&choice(
            "claude-opus-4-1",
            Effort::ExtraHigh,
            Access::FullAccess
        )),
        [
            "--model",
            "claude-opus-4-1",
            "--effort",
            "xhigh",
            "--permission-mode",
            "bypassPermissions"
        ]
    );
}

#[test]
fn a_session_s_conversation_is_saved_and_kept_as_written() {
    let temp = TempDir::new();
    let path = temp.path().join("workspace.json");
    let mut workspace = Workspace::default();
    let project = add(&mut workspace, &temp, "web");
    let first = workspace.new_session(project, Timestamp(2)).unwrap();
    assert_eq!(workspace.session(first).unwrap().conversation, None);
    let id = ConversationId::new();
    let groups: Vec<usize> = id.0.split('-').map(str::len).collect();
    assert_eq!(groups, [8, 4, 4, 4, 12], "a UUID: {id}");
    workspace.set_conversation(first, id.clone());
    workspace.set_conversation(SessionId(99), ConversationId::new());
    workspace.save(&path).unwrap();
    let loaded = Workspace::load(&path).unwrap();
    assert_eq!(loaded.session(first).unwrap().conversation, Some(id));

    let written = r#"{"version": 1, "next_id": 3,
      "projects": [{"id": 0, "title": "web", "path": "/web", "created_at": 1}],
      "sessions": [
        {"id": 1, "project": 0, "title": "Odd", "created_at": 1, "last_activity_at": 2, "conversation": "not-a-uuid"},
        {"id": 2, "project": 0, "title": "Older", "created_at": 1, "last_activity_at": 2}
      ]}"#;
    let odd = load_json(written);
    let kept = Some(ConversationId("not-a-uuid".to_owned()));
    assert_eq!(odd.session(SessionId(1)).unwrap().conversation, kept);
    assert_eq!(odd.session(SessionId(2)).unwrap().conversation, None);
    odd.save(&path).unwrap();
    let saved = std::fs::read_to_string(&path).unwrap();
    assert!(saved.contains(r#""conversation": "not-a-uuid""#), "{saved}");
    assert_eq!(
        saved.matches("\"conversation\"").count(),
        1,
        "none is written for a session without one"
    );
}
