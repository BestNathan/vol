//! CLI-based approval channel - prompts user in terminal.

use crate::react::hitl::*;
use std::io::{self, Write};

/// CLI-based approval channel - prompts user in terminal
pub struct CliApprovalChannel;

#[async_trait::async_trait]
impl ApprovalChannel for CliApprovalChannel {
    async fn request_approval(
        &self,
        request: ApprovalRequest,
        timeout: Option<std::time::Duration>,
    ) -> Result<Option<ApprovalResponse>, ApprovalError> {
        println!("\n════════════════════════════════════════");
        println!("Approval Request");
        println!("════════════════════════════════════════");
        println!("Tool: {}", request.tool_name);
        println!("Reason: {}", request.reason);
        println!("════════════════════════════════════════");
        println!("[A]pprove / [R]eject / [S]top");
        print!("Your choice: ");
        io::stdout()
            .flush()
            .map_err(|e| ApprovalError::Transport(e.to_string()))?;

        if let Some(timeout_dur) = timeout {
            let result = tokio::time::timeout(timeout_dur, async { read_line_async().await }).await;

            match result {
                Ok(Ok(input)) => Ok(parse_approval_input(&input)),
                Ok(Err(_)) => Ok(None),
                Err(_) => Ok(None), // Timeout
            }
        } else {
            let input = read_line_async()
                .await
                .map_err(|e| ApprovalError::Transport(e.to_string()))?;
            Ok(parse_approval_input(&input))
        }
    }
}

async fn read_line_async() -> io::Result<String> {
    tokio::task::spawn_blocking(move || {
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        Ok::<String, io::Error>(input.trim().to_string())
    })
    .await
    .unwrap_or_else(|_| Ok(String::new()))
}

fn parse_approval_input(input: &str) -> Option<ApprovalResponse> {
    match input.to_lowercase().as_str() {
        "a" | "approve" | "y" | "yes" => Some(ApprovalResponse::Approved),
        "r" | "reject" | "n" | "no" => Some(ApprovalResponse::Rejected {
            reason: "User rejected".to_string(),
        }),
        "s" | "stop" => Some(ApprovalResponse::Rejected {
            reason: "User stopped execution".to_string(),
        }),
        _ => {
            println!("Invalid choice. Please try again.");
            None
        }
    }
}
