# Security Policy

## Supported Versions

This project uses CalVer (`YY.M.patch`). Only the latest released version
receives security fixes.

| Version        | Supported          |
| -------------- | ------------------ |
| latest release | :white_check_mark: |
| older          | :x:                |

## Reporting a Vulnerability

Please **do not** open a public issue for security problems.

Report privately to **xpointsh@gmail.com** with:

- a description of the issue and its impact,
- steps to reproduce (a minimal proof-of-concept if possible),
- any suggested remediation.

You can expect an acknowledgement within **5 business days**. Once a fix is
available we will coordinate disclosure and credit reporters who wish to be
named.

## Known Non-Issues

- Crates in this ecosystem use local `path` dependencies on sibling repos; a
  build that fails in isolation (without the siblings checked out alongside) is
  expected and is **not** a security issue.
