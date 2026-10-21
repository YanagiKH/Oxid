This is the unchanged `typed-project-unit2-evidence` artifact from the successful
[original Repository CI run 39226443899, attempt 1, job 117973774217](https://github.com/YanagiKH/Oxid/actions/runs/39226443899/job/117973774217).
It ran on the real `schedule` event for commit
`9ff8ef90a26f003f4616838dfc186fa3bfb6845c`, tree
`50512c0ff428a802afcb1b834ee11f981811db1f`.

- GitHub artifact ID: `12180618442`.
- Exact ZIP size: `2251614` bytes.
- GitHub archive SHA256: `a3395ec1a79a7bb28f1372c1cf40b0745b6583a575d18b0428a1d475c577f88f`.

The original ZIP is retained so GitHub artifact expiration cannot remove the
evidence. `scripts/verify_historical_event_evidence.py` checks the complete ZIP
digest, bounded member roster, all 53 source-control index entries, and the six
unchanged Unit2 inputs required by the original completion rule. It reads members
without extracting or executing them. The historical replay runner separately
verifies the original successful GitHub run, attempt, job and test step.

The delegated `test_published_formatter_source` suite completed 92 source-control
tests: 37 unit, 15 integration, 14 view, 20 CI transport and six public lifecycle
tests. Its receipt explicitly records zero compiler executions,
`execution_qualified: false`, and 36 additional lifecycle tests pending. These are
results from that original event; they do not qualify current source or a release.
The remaining historical script tests run again on the pinned checkout.
