mod support;

use std::path::PathBuf;

use agent::protocol::MAX_IMAGE;
use agent::protocol::{encode, encode_decline, encode_initialize};
use agent::{Attachment, Command, Decision, Image, ImageFormat, Message, MessageId, RequestId};
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
fn images_go_first_and_the_text_last_so_a_leading_command_still_expands() {
    let png = Image::new(vec![0x89, b'P', b'N', b'G']).unwrap();
    let attachments = vec![
        Attachment::File(PathBuf::from("/Users/me/project/note.txt")),
        Attachment::Image(png),
    ];
    let sent = line(
        &Command::Send(message("/review what fruit is named?", attachments)),
        "unused",
    );
    assert_eq!(
        sent["message"]["content"],
        json!([
            {"type": "image", "source": {"type": "base64", "media_type": "image/png", "data": "iVBORw=="}},
            {"type": "text", "text": "/review what fruit is named?\n\nAttached files:\n/Users/me/project/note.txt"},
        ])
    );
}

#[test]
fn a_message_of_whitespace_sends_no_text_block() {
    let jpeg = Image::new(vec![0xff, 0xd8, 0xff]).unwrap();
    let sent = line(
        &Command::Send(message(" \n\t", vec![Attachment::Image(jpeg)])),
        "unused",
    );
    assert_eq!(
        sent["message"]["content"],
        json!([{"type": "image", "source": {"type": "base64", "media_type": "image/jpeg", "data": "/9j/"}}])
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
            permissions: Vec::new(),
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
            interrupt: false,
        },
    };
    assert_eq!(
        line(&deny, "unused")["response"]["response"],
        json!({"behavior": "deny", "message": "Not now"})
    );
    let rule = json!({"type": "addRules", "rules": [{"toolName": "Bash"}], "behavior": "allow", "destination": "session"});
    let for_session = Command::Answer {
        request: RequestId("a6c22aa7".to_owned()),
        decision: Decision::Allow {
            input: input.clone(),
            permissions: vec![rule.clone()],
        },
    };
    assert_eq!(
        line(&for_session, "unused")["response"]["response"],
        json!({"behavior": "allow", "updatedInput": input, "updatedPermissions": [rule]})
    );
    let cancel = Command::Answer {
        request: RequestId("a6c22aa7".to_owned()),
        decision: Decision::Deny {
            message: "Stop".to_owned(),
            interrupt: true,
        },
    };
    assert_eq!(
        line(&cancel, "unused")["response"]["response"],
        json!({"behavior": "deny", "message": "Stop", "interrupt": true})
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

const PNG: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];

fn image_in(attachment: Attachment) -> Option<ImageFormat> {
    match attachment {
        Attachment::Image(image) => Some(image.format()),
        Attachment::File(_) => None,
    }
}

#[test]
fn an_image_is_known_by_its_bytes_and_any_other_file_goes_by_absolute_path() {
    let dir = TempDir::new();
    let jpeg_named_png = dir.path().join("Shot.PNG");
    std::fs::write(&jpeg_named_png, [0xff, 0xd8, 0xff, 0xe0, 0, 0x10]).unwrap();
    let png_without_extension = dir.path().join("pasted");
    std::fs::write(&png_without_extension, PNG).unwrap();
    let text_named_png = dir.path().join("notes.png");
    std::fs::write(&text_named_png, "# notes").unwrap();

    assert_eq!(
        image_in(Attachment::open(&jpeg_named_png)),
        Some(ImageFormat::Jpeg)
    );
    assert_eq!(
        image_in(Attachment::open(&png_without_extension)),
        Some(ImageFormat::Png)
    );
    let Attachment::File(path) = Attachment::open(&text_named_png) else {
        panic!("text read as an image");
    };
    assert!(path.is_absolute());
    assert_eq!(path, text_named_png);
    let formats = [
        (&b"GIF89a"[..], ImageFormat::Gif),
        (&b"RIFF\x10\0\0\0WEBPVP8 "[..], ImageFormat::Webp),
    ];
    for (bytes, format) in formats {
        assert_eq!(
            Image::new(bytes.to_vec()).map(|image| image.format()),
            Some(format)
        );
    }
}

#[test]
fn an_image_too_large_or_that_cannot_be_read_goes_by_its_path() {
    let dir = TempDir::new();
    let mut bytes = PNG.to_vec();
    bytes.resize(MAX_IMAGE as usize, 0);
    let largest = dir.path().join("largest.png");
    std::fs::write(&largest, &bytes).unwrap();
    bytes.push(0);
    let too_large = dir.path().join("too-large.png");
    std::fs::write(&too_large, &bytes).unwrap();
    let missing = dir.path().join("missing.png");

    assert_eq!(image_in(Attachment::open(&largest)), Some(ImageFormat::Png));
    assert_eq!(
        Attachment::open(&too_large),
        Attachment::File(too_large.clone())
    );
    assert_eq!(
        Attachment::open(&missing),
        Attachment::File(missing.clone())
    );
    assert!(Image::new(bytes).is_none());
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let locked = dir.path().join("locked.png");
        std::fs::write(&locked, PNG).unwrap();
        std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o000)).unwrap();
        assert_eq!(Attachment::open(&locked), Attachment::File(locked.clone()));
    }
}
