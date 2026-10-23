mod support;

use std::path::PathBuf;

use agent::protocol::{encode, encode_decline, encode_initialize};
use agent::{
    Attachment, Command, ConversationId, Decision, Image, ImageFormat, Message, MessageId,
    RequestId,
};
use serde_json::{Value, json};
use support::TempDir;
use workspace::{Access, AgentId, Effort, ModelId};

fn line(command: &Command, request: &str) -> Value {
    let claude = workspace::agent(&AgentId("claude".to_owned())).unwrap();
    let line = encode(command, claude, &RequestId(request.to_owned())).unwrap();
    assert!(!line.contains('\n'), "one command is one line");
    serde_json::from_str(&line).unwrap()
}

fn message(text: &str, attachments: Vec<Attachment>) -> Message {
    Message {
        id: MessageId("5be0d9a7-61fb-4fbd-a79f-4c718bfcb386".to_owned()),
        text: text.to_owned(),
        attachments,
    }
}

#[test]
fn a_message_goes_as_a_user_line_under_its_id() {
    let text = "Use the Bash tool to run exactly this command: touch probe.txt";
    assert_eq!(
        line(&Command::Send(message(text, vec![])), "unused"),
        json!({
            "type": "user",
            "uuid": "5be0d9a7-61fb-4fbd-a79f-4c718bfcb386",
            "parent_tool_use_id": null,
            "message": {"role": "user", "content": [{"type": "text", "text": text}]},
        })
    );
}

#[test]
fn images_go_inline_and_other_files_by_path() {
    let png = Image {
        format: ImageFormat::Png,
        bytes: vec![0x89, b'P', b'N', b'G'].into(),
    };
    let attachments = vec![
        Attachment::File(PathBuf::from("/Users/me/project/note.txt")),
        Attachment::Image(png),
    ];
    let sent = line(
        &Command::Send(message("What fruit is named?", attachments)),
        "unused",
    );
    assert_eq!(
        sent["message"]["content"],
        json!([
            {"type": "text", "text": "What fruit is named?\n\nAttached files:\n/Users/me/project/note.txt"},
            {"type": "image", "source": {"type": "base64", "media_type": "image/png", "data": "iVBORw=="}},
        ])
    );
}

#[test]
fn a_message_of_whitespace_sends_no_text_block() {
    let jpeg = Image {
        format: ImageFormat::Jpeg,
        bytes: vec![0xff, 0xd8].into(),
    };
    let sent = line(
        &Command::Send(message(" \n\t", vec![Attachment::Image(jpeg)])),
        "unused",
    );
    assert_eq!(
        sent["message"]["content"],
        json!([{"type": "image", "source": {"type": "base64", "media_type": "image/jpeg", "data": "/9g="}}])
    );
}

#[test]
fn text_in_any_script_goes_as_written() {
    let text = "Привет, oʻzbekcha matn\nikkinchi qator";
    let sent = line(&Command::Send(message(text, vec![])), "unused");
    assert_eq!(sent["message"]["content"][0]["text"], text);
}

#[test]
fn mid_session_changes_go_as_the_recorded_control_requests() {
    let control = |request: &str, body: Value| json!({"type": "control_request", "request_id": request, "request": body});
    assert_eq!(
        line(&Command::SetAccess(Access::AcceptEdits), "host_2"),
        control(
            "host_2",
            json!({"subtype": "set_permission_mode", "mode": "acceptEdits"})
        )
    );
    assert_eq!(
        line(&Command::SetModel(ModelId("haiku".to_owned())), "host_3"),
        control("host_3", json!({"subtype": "set_model", "model": "haiku"}))
    );
    assert_eq!(
        line(&Command::SetEffort(Effort::Low), "host_4"),
        control(
            "host_4",
            json!({"subtype": "apply_flag_settings", "settings": {"effortLevel": "low"}})
        )
    );
    assert_eq!(
        line(&Command::SetEffort(Effort::ExtraHigh), "host_6")["request"]["settings"],
        json!({"effortLevel": "xhigh"})
    );
    assert_eq!(
        line(&Command::SetAccess(Access::AskFirst), "host_7")["request"]["mode"],
        "manual"
    );
}

#[test]
fn stop_interrupts_the_turn_and_the_queue_behind_it() {
    assert_eq!(
        line(&Command::Interrupt, "host_5"),
        json!({
            "type": "control_request",
            "request_id": "host_5",
            "request": {"subtype": "interrupt", "cancel_queued": true},
        })
    );
}

#[test]
fn an_answer_goes_back_under_the_prompt_s_id() {
    let input = json!({
        "command": "touch probe.txt",
        "description": "Create empty probe.txt file in working directory",
    });
    let allow = Command::Answer {
        request: RequestId("a6c22aa7-22c2-43d3-9358-c0477a9b497f".to_owned()),
        decision: Decision::Allow {
            input: input.clone(),
        },
    };
    assert_eq!(
        line(&allow, "unused"),
        json!({"type": "control_response", "response": {
            "subtype": "success",
            "request_id": "a6c22aa7-22c2-43d3-9358-c0477a9b497f",
            "response": {"behavior": "allow", "updatedInput": input},
        }})
    );
    let deny = Command::Answer {
        request: RequestId("a6c22aa7".to_owned()),
        decision: Decision::Deny {
            message: "Not now".to_owned(),
        },
    };
    assert_eq!(
        line(&deny, "unused")["response"]["response"],
        json!({"behavior": "deny", "message": "Not now"})
    );
}

#[test]
fn the_session_s_own_lines_open_and_decline() {
    let initialize: Value = serde_json::from_str(&encode_initialize()).unwrap();
    assert_eq!(
        initialize,
        json!({"type": "control_request", "request_id": "initialize", "request": {"subtype": "initialize"}})
    );
    let decline: Value =
        serde_json::from_str(&encode_decline(&RequestId("r1".to_owned()))).unwrap();
    assert_eq!(decline["response"]["subtype"], "error");
    assert_eq!(decline["response"]["request_id"], "r1");
}

#[test]
fn an_image_file_attaches_inline_and_any_other_by_absolute_path() {
    let dir = TempDir::new();
    let png = dir.path().join("Shot.PNG");
    std::fs::write(&png, [1, 2, 3]).unwrap();
    let notes = dir.path().join("notes.md");
    std::fs::write(&notes, "# notes").unwrap();

    assert_eq!(
        Attachment::open(&png).unwrap(),
        Attachment::Image(Image {
            format: ImageFormat::Png,
            bytes: vec![1, 2, 3].into(),
        })
    );
    let Attachment::File(path) = Attachment::open(&notes).unwrap() else {
        panic!("a markdown file read as an image");
    };
    assert!(path.is_absolute());
    assert_eq!(path, notes);
    assert!(Attachment::open(&dir.path().join("missing.png")).is_err());
}

#[test]
fn a_conversation_id_reads_back_as_written() {
    let id = ConversationId::new();
    assert_eq!(id.to_string().parse::<ConversationId>().unwrap(), id);
    assert!("not-a-uuid".parse::<ConversationId>().is_err());
}
