mod stats;
mod git;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::process::Command;
use std::io::{BufRead, BufReader};
use std::fs;
use std::path::Path;
use stats::BugStats;
use git::PhoenixGit;

#[derive(Debug, Deserialize)]
#[serde(tag = "reason")]
enum CargoMessage {
    #[serde(rename = "compiler-message")]
    CompilerMessage { message: Diagnostic },
    #[serde(rename = "test")]
    Test { name: String, event: String, stdout: Option<String> },
    #[serde(other)]
    Other,
}

#[derive(Debug, Deserialize)]
struct Diagnostic {
    message: String,
    spans: Vec<DiagnosticSpan>,
}

#[derive(Debug, Deserialize)]
struct DiagnosticSpan {
    file_name: String,
    line_start: usize,
}

#[derive(Debug, Serialize, Deserialize)]
struct LlmRequest {
    model: String,
    messages: Vec<LlmMessage>,
}

#[derive(Debug, Serialize, Deserialize)]
struct LlmMessage {
    role: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct LlmResponse {
    choices: Vec<LlmChoice>,
}

#[derive(Debug, Deserialize)]
struct LlmChoice {
    message: LlmMessage,
}

async fn get_ai_fix(error: &str, code: &str, file: &str) -> Result<String> {
    println!("🤖 Asking AI for a fix in {}...", file);
    
    let client = reqwest::Client::new();
    let prompt = format!(
        "You are Project Phoenix, a self-healing Rust agent.\n\
        The file `{}` has the following error:\n\n\
        ERROR: {}\n\n\
        FULL SOURCE CODE:\n\n\
        {}\n\n\
        Provide ONLY the corrected source code. Do not include markdown formatting or explanations.",
        file, error, code
    );

    let (url, body) = if let Ok(api_key) = std::env::var("PHOENIX_API_KEY") {
        println!("☁️ Using cloud LLM API...");
        (
            "https://api.openai.com/v1/chat/completions".to_string(),
            serde_json::json!({
                "model": "gpt-4o",
                "messages": [{"role": "user", "content": prompt}],
                "stream": false
            })
        )
    } else {
        println!("🏠 Using local Ollama...");
        (
            "http://localhost:11434/api/chat".to_string(),
            serde_json::json!({
                "model": "llama2",
                "messages": [{"role": "user", "content": prompt}],
                "stream": false
            })
        )
    };

    let mut request = client.post(&url);
    if let Ok(api_key) = std::env::var("PHOENIX_API_KEY") {
        request = request.bearer_auth(api_key);
    }

    let res = request.json(&body).send().await?;
    let json: serde_json::Value = res.json().await?;

    let fix = if url.contains("openai") {
        json["choices"][0]["message"]["content"].as_str().unwrap_or("").to_string()
    } else {
        json["message"]["content"].as_str().unwrap_or("").to_string()
    };
    
    Ok(fix.trim_matches('`').trim_start_matches("rust\n").to_string())
}

#[tokio::main]
async fn main() -> Result<()> {
    println!("🔥 Project Phoenix: Initializing Self-Healing Loop...");

    let git = PhoenixGit::open()?;
    let mut stats = BugStats::load();
    let max_attempts = 3;
    let mut current_attempt = 0;

    loop {
        current_attempt += 1;
        if current_attempt > max_attempts {
            println!("🚫 Max repair attempts reached. Manual intervention required.");
            break;
        }

        println!("🧐 Attempt {}/{}: Running tests...", current_attempt, max_attempts);
        let mut failures = Vec::new();

        let mut child = Command::new("cargo")
            .args(["test", "--message-format=json"])
            .stdout(std::process::Stdio::piped())
            .spawn()?;

        let reader = BufReader::new(child.stdout.take().unwrap());
        for line in reader.lines() {
            let line = line?;
            if let Ok(msg) = serde_json::from_str::<CargoMessage>(&line) {
                match msg {
                    CargoMessage::CompilerMessage { message } => {
                        if let Some(span) = message.spans.first() {
                            failures.push((span.file_name.clone(), message.message.clone()));
                        }
                    }
                    CargoMessage::Test { name, event, stdout } => {
                        if event == "failed" {
                            failures.push(("src/lib.rs".to_string(), format!("Test {} failed: {}", name, stdout.unwrap_or_default())));
                        }
                    }
                    _ => {}
                }
            }
        }

        child.wait()?;

        if failures.is_empty() {
            println!("✅ All tests passed. Your code is stable.");
            break;
        }

        let mut fix_applied = false;
        for (file, error) in failures {
            println!("⚠️ Failure detected in {}: {}", file, error);
            if Path::new(&file).exists() {
                let code = fs::read_to_string(&file)?;
                match get_ai_fix(&error, &code, &file).await {
                    Ok(new_code) => {
                        if !new_code.is_empty() {
                            fs::write(&file, &new_code)?;
                            println!("✨ Applied AI fix to {}.", file);
                            
                            // Automated Git Commit
                            let msg = format!("fix(phoenix): autonomous repair of {} (Attempt {})", file, current_attempt);
                            if let Err(e) = git.commit_fix(&file, &msg) {
                                println!("❌ Git commit failed: {}", e);
                            }

                            stats.add_fix(&file, &error);
                            fix_applied = true;
                        }
                    }
                    Err(e) => println!("❌ Failed to get AI fix: {}", e),
                }
            }
        }

        if fix_applied {
            stats.save()?;
        } else {
            println!("🚫 No fix could be applied. Stopping...");
            break;
        }
    }

    Ok(())
}
