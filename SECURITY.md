# Security Policy

## Reporting a vulnerability

Report privately through GitHub's private vulnerability reporting:
[Report a vulnerability](https://github.com/metaneutrons/bups/security/advisories/new).

Please do not open a public issue for a security problem.

Include what you have: affected version, printer model, how to reproduce, and
what an attacker gains. A partial report is better than none.

## Response

You get an acknowledgement within 72 hours and an assessment within 7 days. If
the report is valid, we agree a disclosure date with you before publishing.

## Scope

bups exposes three network services: a raw print port (TCP 9100 by default), an
SNMP responder (UDP 161 by default) and mDNS advertisement. None of them
authenticate, and that is by design for a print server on a trusted LAN.

Reports about **unauthenticated access from the network bups is bound to** are
therefore in scope only where they exceed what the service is meant to do, for
instance memory corruption, a crash, or reaching data outside the printer
status. Simply being able to print, or to query printer status, is the intended
function.

Do not expose bups to an untrusted network.

## Supported versions

The latest release. There is no backport branch.
