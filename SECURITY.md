# Security Policy

## Supported Versions

| Version | Supported |
|---------|-----------|
| 1.0.x   | Yes       |

## Reporting a Vulnerability

Please do not report security vulnerabilities through public GitHub issues.

Instead, email **security@momidala.com** with:

- A description of the vulnerability and its potential impact
- Steps to reproduce or a proof-of-concept
- Any suggested mitigations if known

You should receive an acknowledgement within 2 business days. We will keep you informed as we investigate and address the issue.

## Scope

This policy covers:

- The `graydr` compiler (`graydr` crate)
- The `graydr-registry` server (`graydr-registry` crate)
- The graydr VSCode extension

## Out of Scope

- Vulnerabilities in third-party dependencies should be reported upstream
- Issues in generated IaC output (CloudFormation, Bicep, Terraform) are the responsibility of the consuming cloud provider
