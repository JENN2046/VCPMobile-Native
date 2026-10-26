# Import attempts

Run 40286659089 failed before publishing a staging branch. Source HEAD/tree and the 59-file manifest passed, but Git 2.55 fast-export rejected native/ in a no-checkout clone. The repaired importer materializes only native and LICENSE in its disposable source copy. Ordinary and no-checkout synthetic regressions passed locally.

Run 40286916129 SUCCESS: source branch was checked before fetch and again before publication. All 59 source file SHA256 values matched the reviewed package; filtered native/LICENSE history contains16 commits with original author/committer/date/message, and final paths/modes/blob IDs match. Staging commit da2f29398dbfbbaa13cf7191e6e0befa063d24f0 was published non-forced to migration/native-history; source and main stayed unchanged during that job. The initial failure remains in Actions.

The one-time write-capable import workflow is removed from the final repository tree. Its script and bootstrap history are retained for audit. Do not rerun the importer against the populated repository. The final main initialization joins the scoped history and bootstrap history without rewriting either. Subsequent product development uses codex/native-rust-m0-m1 only.
