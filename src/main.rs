use dotenv::dotenv;
use reqwest;
use serde::{Deserialize, Serialize};
use std::env;
use std::error::Error;
use std::io::{self, Write};
use std::path::PathBuf;
use std::process::Command; // Add this line

#[derive(Serialize)]
struct GroqRequest {
    model: String,
    messages: Vec<Message>,
    temperature: f32,
    max_tokens: i32,
}

#[derive(Serialize)]
struct Message {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct GroqResponse {
    id: String,
    choices: Vec<Choice>,
}

#[derive(Deserialize)]
struct Choice {
    message: ResponseMessage,
}

#[derive(Deserialize)]
struct ResponseMessage {
    content: String,
}

fn get_git_diff(path: &str) -> Result<String, Box<dyn Error>> {
    let output = Command::new("git")
        .current_dir(path)
        .args(&["diff", "--staged"])
        .output()?;

    Ok(String::from_utf8(output.stdout)?)
}

async fn generate_commit_message(git_diff: &str) -> Result<String, Box<dyn Error>> {
    let client = reqwest::Client::new();

    let groq_api_key = std::env::var("GROQ_API_KEY")?;
    let request = GroqRequest {
        model: "mixtral-8x7b-32768".to_string(), // or another model
        messages: vec![Message {
            role: "user".to_string(),
            content: format!(
                "Generate a concise commit message for the following git diff:\n{}\n\nDO NOT INCLUDE ANYTHING BUT THE COMMIT MESSAGE IN YOUR RESPONSE.",
                git_diff
            ),
        }],
        temperature: 0.7,
        max_tokens: 1000,
    };

    let response = client
        .post("https://api.groq.com/openai/v1/chat/completions")
        .header("Authorization", format!("Bearer {}", groq_api_key))
        .json(&request)
        .send()
        .await?;

    let response_text = response.text().await?;

    let response: GroqResponse = serde_json::from_str(&response_text)?;
    Ok(response.choices[0].message.content.trim().to_string())
}

fn make_commit(path: &str, message: &str, edit: bool) -> Result<(), Box<dyn Error>> {
    let mut args = vec!["commit"];

    if edit {
        // Opens in editor for message modification
        args.push("-e");
        args.push("-m");
        args.push(message);
    } else {
        args.push("-m");
        args.push(message);
    }

    let output = Command::new("git").current_dir(path).args(&args).output()?;

    if !output.status.success() {
        let error = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Failed to commit: {}", error).into());
    }

    println!("Successfully committed changes!");
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Get the directory where the script is being executed from
    let execution_dir = env::current_dir()?;

    // Get the project root (where your .env file is)
    let project_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));

    dotenv::from_path(project_root.join(".env"))?;

    let git_diff = get_git_diff(&execution_dir.to_str().unwrap())?;

    if git_diff.is_empty() {
        println!("No changes detected in the file.");
        return Ok(());
    }
    let commit_message = generate_commit_message(&git_diff).await?;
    println!("Generated commit message: {}", commit_message);
    println!("What would you do like to do?");
    println!("1.) Commit with this message");
    println!("2.) Edit the message in the editor");
    //println!("3.) Re-generate the message");
    println!("4.) Abort");

    io::stdout().flush()?;
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    match input.trim() {
        "1" => make_commit(execution_dir.to_str().unwrap(), &commit_message, false)?,
        "2" => make_commit(execution_dir.to_str().unwrap(), &commit_message, true)?,
        //"3" => make_commit(execution_dir.to_str().unwrap(), &commit_message, true)?,
        _ => println!("Commit aborted."),
    }

    Ok(())
}
