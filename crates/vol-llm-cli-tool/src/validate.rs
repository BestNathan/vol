//! First-token binaries whitelist check.

/// Extract the first whitespace-delimited token from a command string.
pub fn first_token(command: &str) -> Option<&str> {
    command.split_whitespace().next()
}

/// Validate that the first token is in the allowed list.
pub fn validate_first_token<'a>(
    command: &'a str,
    binaries: &[String],
) -> Result<&'a str, crate::CliToolError> {
    let token = first_token(command).ok_or_else(|| {
        crate::CliToolError::InvalidArguments("command is empty".into())
    })?;
    if binaries.iter().any(|b| b == token) {
        Ok(token)
    } else {
        Err(crate::CliToolError::BinaryNotAllowed {
            token: token.to_string(),
            allowed: binaries.to_vec(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bins() -> Vec<String> {
        vec!["ansible".into(), "ansible-playbook".into()]
    }

    #[test]
    fn valid_first_token() {
        let tok = validate_first_token("ansible-playbook site.yml --limit web", &bins()).unwrap();
        assert_eq!(tok, "ansible-playbook");
    }

    #[test]
    fn invalid_first_token() {
        let err = validate_first_token("rm -rf /", &bins()).unwrap_err();
        match err {
            crate::CliToolError::BinaryNotAllowed { token, allowed } => {
                assert_eq!(token, "rm");
                assert_eq!(allowed, bins());
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn empty_command_rejected() {
        let err = validate_first_token("", &bins()).unwrap_err();
        assert!(matches!(err, crate::CliToolError::InvalidArguments(_)));
    }

    #[test]
    fn whitespace_only_rejected() {
        let err = validate_first_token("   \t  ", &bins()).unwrap_err();
        assert!(matches!(err, crate::CliToolError::InvalidArguments(_)));
    }

    #[test]
    fn leading_whitespace_still_finds_token() {
        let tok = validate_first_token("   ansible all -m ping", &bins()).unwrap();
        assert_eq!(tok, "ansible");
    }
}
