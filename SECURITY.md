# Security Policy

## Supported Versions

| Version | Supported |
|---------|-----------|
| 0.1.x   | Yes       |

## Reporting a Vulnerability

If you discover a security vulnerability in cc-dj, please report it responsibly:

1. **Do not** open a public GitHub issue for security vulnerabilities.
2. Email the maintainer directly or use [GitHub's private vulnerability reporting](https://github.com/diomandeee/cc-dj/security/advisories/new).
3. Include:
   - Description of the vulnerability
   - Steps to reproduce
   - Potential impact
   - Suggested fix (if any)

You should receive a response within 48 hours. We will work with you to understand and address the issue before any public disclosure.

## Scope

Security-relevant areas of cc-dj include:

- **AppleScript bridge**: Keyboard automation on macOS — input validation prevents command injection
- **API key handling**: Gemini API keys are sanitized in all error messages and logs
- **Configuration parsing**: YAML configs are validated at parse time
- **MIDI output**: Direct hardware communication

## Security Measures

- Key input validated against injection (max 2 ASCII chars)
- API keys redacted from error messages and log output
- `.gitignore` blocks `.env*`, `*.pem`, `*.key` files
- `deny.toml` configured for license and advisory checks
- Dependencies audited — `serde_yaml` replaced with maintained `serde_yml`
