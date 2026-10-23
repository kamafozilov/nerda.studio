//! Stands in for `claude` in the tests. It plays `fake-agent.jsonl` from its
//! working folder: agent lines go to stdout, with `{{uuid}}` and `{{request}}`
//! filled in from what the app wrote; at a `{"host": …}` line it waits until
//! the app writes that kind of line. After the script it waits for its input
//! to close, as the agent does.
//!
//! Lines of its own: `{"agent": "exit", "code": N}` quits; `{"agent":
//! "stderr", "text": …}` writes to stderr; `{"agent": "raw", "text": …}`
//! writes text that needn't be JSON; `{"agent": "hang"}` stops reading and
//! never quits; `{"agent": "orphan"}` starts a process that holds its stderr
//! open for five seconds; `{"agent": "spawn", "name": N, "group": "own"}`
//! starts a process that sleeps a minute, in a process group of its own as
//! the agent's background shells are, or in the agent's without `group`, and
//! writes its pid to `N.pid`. It writes its arguments to `fake-agent.args`, one a line, its
//! PATH to `fake-agent.path`, and each line it reads to `fake-agent.stdin.jsonl`.

use std::io::{BufRead, Write};
use std::sync::mpsc;
use std::time::Duration;

use serde_json::Value;

fn main() {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    if arguments == ["--linger"] {
        std::thread::sleep(Duration::from_secs(5));
        return;
    }
    if arguments == ["--sleep"] {
        std::thread::sleep(Duration::from_secs(60));
        return;
    }
    std::fs::write("fake-agent.args", arguments.join("\n")).unwrap();
    let path = std::env::var("PATH").unwrap_or_default();
    std::fs::write("fake-agent.path", path).unwrap();
    let Ok(script) = std::fs::read_to_string("fake-agent.jsonl") else {
        eprintln!("fake-agent: no fake-agent.jsonl in this folder");
        std::process::exit(2);
    };

    let (sender, input) = mpsc::channel::<Value>();
    std::thread::spawn(move || {
        let mut log = std::fs::File::create("fake-agent.stdin.jsonl").unwrap();
        for line in std::io::stdin().lock().lines() {
            let Ok(line) = line else { break };
            writeln!(log, "{line}").unwrap();
            let value = serde_json::from_str(&line).unwrap_or(Value::Null);
            if sender.send(value).is_err() {
                break;
            }
        }
    });

    let (mut uuid, mut request) = (String::new(), String::new());
    for line in script.lines().filter(|line| !line.trim().is_empty()) {
        let step: Value = serde_json::from_str(line).unwrap();
        let text = || step["text"].as_str().unwrap_or_default().to_owned();
        if let Some(host) = step["host"].as_str() {
            let Some(written) = wait_for(&input, host) else {
                std::process::exit(0);
            };
            match host {
                "user" => uuid = written["uuid"].as_str().unwrap_or_default().to_owned(),
                "answer" => {}
                _ => {
                    request = written["request_id"]
                        .as_str()
                        .unwrap_or_default()
                        .to_owned()
                }
            }
            continue;
        }
        match step["agent"].as_str() {
            Some("exit") => std::process::exit(step["code"].as_i64().unwrap_or(0) as i32),
            Some("stderr") => eprintln!("{}", text()),
            Some("raw") => emit(&text()),
            Some("orphan") => {
                #[expect(clippy::zombie_processes, reason = "it has to outlive the fake agent")]
                let _orphan = std::process::Command::new(std::env::current_exe().unwrap())
                    .arg("--linger")
                    .stdin(std::process::Stdio::null())
                    .stdout(std::process::Stdio::null())
                    .spawn()
                    .unwrap();
            }
            Some("spawn") => {
                let mut command = std::process::Command::new(std::env::current_exe().unwrap());
                command
                    .arg("--sleep")
                    .stdin(std::process::Stdio::null())
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null());
                #[cfg(unix)]
                if step["group"] == "own" {
                    use std::os::unix::process::CommandExt;
                    command.process_group(0);
                }
                #[expect(clippy::zombie_processes, reason = "the session has to end it")]
                let child = command.spawn().unwrap();
                let name = step["name"].as_str().unwrap();
                std::fs::write(format!("{name}.pid"), child.id().to_string()).unwrap();
            }
            Some("hang") => loop {
                std::thread::sleep(Duration::from_secs(60));
            },
            _ => emit(
                &line
                    .replace("{{uuid}}", &uuid)
                    .replace("{{request}}", &request),
            ),
        }
    }
    while input.recv().is_ok() {}
}

/// The next line the app writes of the kind a host line names, skipping others;
/// `None` once the app closes its end.
fn wait_for(input: &mpsc::Receiver<Value>, host: &str) -> Option<Value> {
    loop {
        let written = input.recv().ok()?;
        let fits = match host {
            "user" => written["type"] == "user",
            "answer" => written["type"] == "control_response",
            subtype => {
                written["type"] == "control_request" && written["request"]["subtype"] == subtype
            }
        };
        if fits {
            return Some(written);
        }
    }
}

fn emit(line: &str) {
    let mut stdout = std::io::stdout().lock();
    if writeln!(stdout, "{line}")
        .and_then(|()| stdout.flush())
        .is_err()
    {
        std::process::exit(0);
    }
}
