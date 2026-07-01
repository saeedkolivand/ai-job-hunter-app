# Eval corpus

Synthetic, **no-PII** resume fixtures for the export/extraction eval harness
(`tests/eval.rs`). Each `<name>.txt` is a fake resume; its `<name>.tags` sidecar
declares the expected extraction in a simple line format:

```
# comment line
key: value
```

Recognized keys (repeatable keys accumulate in order):

| key       | repeatable | required | meaning                                    |
| --------- | ---------- | -------- | ------------------------------------------ |
| `name`    | no         | yes      | candidate full name                        |
| `email`   | no         | yes      | contact email (must be a synthetic domain) |
| `phone`   | no         | no       | contact phone                              |
| `section` | yes        | yes      | each expected section heading              |
| `link`    | yes        | no       | each expected profile/portfolio label      |

## Rules

- Emails MUST use a reserved example domain (`example.test` / `example.com`) —
  the harness asserts this and that the email also appears verbatim in the `.txt`.
- No real names, phone numbers, or addresses.

## Status

Phase 1 only validates corpus **shape** (sidecar present, tags parse, required
fields, synthetic emails). Field-level precision/recall against the real
extractor is computed in `tests/eval.rs` in **Phase 6**, using these same tags as
ground truth.
