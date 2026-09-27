use solo::{
    approval::{ApprovalPlan, ApprovalRequest},
    auto_approval::{self, CodexReviewer, ReviewInput},
    codex_subscription::{Authentication, DEFAULT_MODEL},
    command_rules::{RuleList, RuleStore},
    harness::{
        self, Cancellation, Limits, Message, StopReason, ToolCall, Update,
        workspace::{WorkspaceTools, is_read_only_tool},
    },
};
use std::{
    env,
    io::{self, Write},
    path::PathBuf,
};

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
                    "使い方: solo-subscription [--cwd PATH] [--model MODEL] [--login] [PROMPT...]\nSolo のハーネスと ChatGPT Codex モデルで実行します。\n承認設定: ~/.config/solo/config.yml"
                );
                return Ok(());
            }
            _ => prompt.push(arg),
        }
    }
    let rule_store = RuleStore::for_workspace(&cwd)?;
    if !prompt.is_empty() {
        rule_store.approval_policy()?;
    }
    let auth = Authentication::new()?;
    let cancellation = Cancellation::default();
    let cancel = cancellation.child_token();
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
    let mut model = auth.model(model_name.clone(), cancel.clone())?;
    let mut tools = WorkspaceTools::new(&cwd)?.with_cancellation(cancellation.clone());
    let user_prompt = prompt.join(" ");
    let review_prompt = user_prompt.clone();
    let mut policy = |call: &ToolCall| {
        let tool_decision = rule_store
            .load()
            .ok()
            .and_then(|rules| rules.tool_decision(&call.name));
        match tool_decision {
            Some(solo::command_rules::Decision::Allow) => return true,
            Some(solo::command_rules::Decision::Deny) => return false,
            _ if is_read_only_tool(&call.name) => return true,
            _ => {}
        }
        let request = ApprovalRequest::tool(call, &cwd);
        let plan = match request.plan(&rule_store) {
            Ok(plan) => plan,
            Err(error) => {
                eprintln!("承認設定を読めないため実行しません: {error}");
                return false;
            }
        };
        match plan {
            ApprovalPlan::Allow(source) => {
                eprintln!("\n[{source}] 許可: {}", request.title);
                return true;
            }
            ApprovalPlan::Deny(source) => {
                eprintln!("\n[{source}] 拒否: {}", request.title);
                return false;
            }
            ApprovalPlan::Auto(settings) => {
                eprintln!("\n[Auto] {} で承認を判定中", settings.model);
                let input = ReviewInput::new(&request, &review_prompt, &cwd);
                let result = auto_approval::review(&CodexReviewer, &settings, &input, &cancel);
                let current = match request.plan(&rule_store) {
                    Ok(plan) => plan,
                    Err(error) => {
                        eprintln!("承認設定を再確認できません: {error}");
                        return false;
                    }
                };
                match &result {
                    Ok(result) => eprintln!("[Auto] {:?}: {}", result.decision, result.reason),
                    Err(error) => eprintln!("[Auto] {error} · 手動確認に戻ります"),
                }
                if let Some(accepted) =
                    auto_approval::resolve_result(&current, &settings, result.as_ref().ok())
                {
                    return accepted;
                }
            }
            ApprovalPlan::Manual => {}
        }
        eprintln!("\n{}", request.title);
        if let Some(command) = &request.display_command {
            eprintln!(
                "実行元: {}\n作業ディレクトリ: {}\n$ {}",
                request.executor,
                cwd.display(),
                command
            );
        } else {
            eprintln!("{}", request.details);
        }
        eprint!(
            "{}",
            if request.command.is_some() {
                "[y] 今回だけ実行 / [a] Allow に完全一致で登録 / [b] Deny に完全一致で登録して拒否 / [N] 拒否: "
            } else {
                "今回だけ許可しますか? [y/N] "
            }
        );
        let _ = io::stderr().flush();
        let mut answer = String::new();
        if io::stdin().read_line(&mut answer).is_err() {
            return false;
        }
        let recheck = || match request.plan(&rule_store) {
            Ok(ApprovalPlan::Allow(_) | ApprovalPlan::Manual | ApprovalPlan::Auto(_)) => true,
            Ok(ApprovalPlan::Deny(_)) => {
                eprintln!("Deny に一致するため実行を拒否しました");
                false
            }
            Err(error) => {
                eprintln!("ルールを再確認できないため実行しません: {error}");
                false
            }
        };
        match answer.trim().to_ascii_lowercase().as_str() {
            "y" => recheck(),
            "a" | "b" if request.command.is_some() => {
                let list = if answer.trim().eq_ignore_ascii_case("a") {
                    RuleList::Allow
                } else {
                    RuleList::Deny
                };
                let command = request.command.as_ref().expect("command approval");
                if let Err(error) = rule_store.add(list, command.clone()) {
                    eprintln!("ルールを保存できないため実行しません: {error}");
                    return false;
                }
                list == RuleList::Allow && recheck()
            }
            _ => false,
        }
    };
    let run = harness::run(
        &mut model,
        &mut tools,
        &mut policy,
        vec![Message::User { text: user_prompt }],
        &Limits::default(),
        &cancellation,
        |update| match update {
            Update::Assistant(text) | Update::AssistantDelta(text) => {
                print!("{text}");
                let _ = io::stdout().flush();
            }
            Update::ToolProposed(call) => {
                eprintln!("\nツール要求: {} {}", call.name, call.arguments)
            }
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
