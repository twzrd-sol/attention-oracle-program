# TWZRD Incident Log

All production and near-production incidents are documented here for auditing, learning, and grant/security review.

---

## Active Incidents

| Date       | ID             | Title                                | Status   | Severity |
|------------|----------------|--------------------------------------|----------|----------|
| _None_     | —              | —                                    | —        | —        |

---

## Resolved Incidents

| Date       | ID             | Title                                | Status   | Severity | Link |
|------------|----------------|--------------------------------------|----------|----------|------|
| 2025-11-17 | DB-2025-11-17  | Schema Mismatch: token_group column  | Resolved | Medium   | [Full Post-Mortem](./incidents/INCIDENT_RESPONSE_2025-11-17.md) |

---

## Severity Definitions

- **Critical**: Service outage, data loss, security breach
- **High**: Significant degradation, user-visible impact
- **Medium**: Internal pipeline disruption, no user impact
- **Low**: Minor issues, cosmetic bugs

---

## Incident Response Process

1. **Detection**: Logs, alerts, user reports
2. **Diagnosis**: Rapid root cause analysis with minimal assumptions
3. **Resolution**: Non-destructive fixes prioritized; data integrity validated
4. **Documentation**: Full post-mortem within 24 hours
5. **Prevention**: Concrete action items tracked in follow-up tasks

---

**Maintainer**: twzrd
**Last Updated**: 2025-11-17
