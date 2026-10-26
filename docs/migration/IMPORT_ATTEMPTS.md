# Import attempts

First run 40286659089 failed before publishing a staging branch. The source HEAD, tree and 59-file manifest passed; Git 2.55 fast-export rejected the native/ path in the no-checkout source copy. The next revision checks out only native and LICENSE in that disposable copy before export. Both ordinary and no-checkout synthetic repository regressions passed locally; real rerun is still required. No source repository or product code was modified, no check was disabled, and the first failure remains in Actions.
