# InterviewStack NDJSON import

The importer reads one JSON object per line with the exact source fields:
`sourceKey`, `date`, `company`, `role`, `level`, `topic`, `score`, `type`, and
`status`. Unknown fields, malformed dates, duplicate source keys, scores outside
0–100, a missing score on a complete row, or a score on an incomplete row fail
validation with a line number.

No absent facts are guessed. The original object is retained in
`metadata.sourceRecord`; source identity is retained in
`metadata.importSource`, `metadata.sourceKey`, and `externalAttemptId`. Each
record uses `_id = interviewstack:<sourceKey>`. The compatibility `startedAt`
response falls back to ingestion time because the source supplies a date but no
time; the stored activity does not invent a source start timestamp.

## Commands

Validate the bundled fixture without a database:

```bash
cargo run --locked --bin import-interviewstack -- \
  --input fixtures/interviewstack_attempts.ndjson --dry-run
```

Import it:

```bash
MONGODB_URI='mongodb://127.0.0.1:27017' \
MONGODB_DATABASE='activity_tracker' \
cargo run --locked --bin import-interviewstack -- \
  --input fixtures/interviewstack_attempts.ndjson
```

Run the same import again. Expected reports for this fixture are:

| Run | Total | Inserted | Updated | Skipped | Failed | Verified |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| dry run | 68 | 0 | 0 | 0 | 0 | 0 |
| first live import | 68 | 68 | 0 | 0 | 0 | 68 |
| unchanged rerun | 68 | 0 | 0 | 68 | 0 | 68 |

The live importer reads back every source key and checks the retained source
object exactly. It additionally reports verification of the first, middle, and
last rows. A changed row with the same key is updated; an unchanged row is
skipped.

## Recovery

The operation is restart-safe: rerun it after correcting input or restoring
MongoDB availability. It never deletes source data. A mixed failure report
identifies failing source keys while successful stable-key writes remain safe
to revisit.
