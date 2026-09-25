use solo::{
    codex_subscription::{Authentication, DEFAULT_MODEL},
    harness::{
        self, Cancellation, Limits, Message, StopReason, ToolCall, Update,
        workspace::WorkspaceTools,
    },
};
use std::{
    env,
    io::{self, Write},
    path::PathBuf,
};
use tokio_util::sync::CancellationToken;

fn main() {
    if let Err(error) = run() {
        eprintln!("solo-subscription: {error}");
        std::process::exit(1);
    }
}

fn run() -> io::Result<()> {
    let mut args = env::args().skip(1);
    let mut cwd = env::current_dir()?.canonicalize()?;
    let mut login = false;
    let mut model_name = env::var("SOLO_MODEL").unwrap_or_else(|_| DEFAULT_MODEL.into());
    let mut prompt = Vec::new();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--cwd" => {
                cwd = PathBuf::from(args.next().ok_or_else(|| usage("--cwd にパスが必要です"))?)
                    .canonicalize()?
            }
            "--model" => {
                model_name = args
                    .next()
                    .ok_or_else(|| usage("--model に名前が必要です"))?
            }
            "--login" => login = true,
            "--help" => {
                println!(
                    "使い方: solo-subscription [--cwd PATH] [--model MODEL] [--login] [PROMPT...]\nSolo のハーネスと ChatGPT Codex モデルで実行します。"
                );
                return Ok(());
            }
            _ => prompt.push(arg),
        }
    }
    let auth = Authentication::new()?;
    let cancel = CancellationToken::new();
    if login || !auth.is_logged_in()? {
        auth.login_device(
            |device| {
                eprintln!(
                    "ブラウザーで {} を開き、コード {} を入力してください。",
                    device.verification_url, device.user_code
                );
            },
            &cancel,
        )?;
    }
    if prompt.is_empty() {
        return Ok(());
    }
    let mut model = auth.model(model_name.clone(), cancel)?;
    let mut tools = WorkspaceTools::new(cwd)?;
    let mut policy = |call: &ToolCall| {
        if matches!(call.name.as_str(), "read" | "search") {
            return true;
        }
        eprintln!("\n承認要求: {} {}", call.name, call.arguments);
        eprint!("今回だけ許可しますか? [y/N] ");
        let _ = io::stderr().flush();
        let mut answer = String::new();
        io::stdin().read_line(&mut answer).is_ok() && answer.trim().eq_ignore_ascii_case("y")
    };
    let run = harness::run(
        &mut model,
        &mut tools,
        &mut policy,
        vec![Message::User {
            text: prompt.join(" "),
        }],
        &Limits::default(),
        &Cancellation::default(),
        |update| match update {
            Update::Assistant(text) | Update::AssistantDelta(text) => {
                print!("{text}");
                let _ = io::stdout().flush();
            }
            Update::ToolProposed(call) => eprintln!("\n実行: {} {}", call.name, call.arguments),
            Update::ToolFinished { result, .. } => eprintln!("{}", result.content),
            _ => {}
        },
    );
    println!();
    match run.stop {
        StopReason::Completed => Ok(()),
        other => Err(usage(format!("実行終了: {other:?}"))),
    }
}

fn usage(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message.into())
}
