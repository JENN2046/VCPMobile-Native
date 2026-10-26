# Pairing fixture startup repair

Candidate against `c711ad53880bff740cb8b4002da4b96557b84ab8`, run40295036441, macOS job121478919316. This is not a passed fix until its own CI executes.

## Observed failure

Rust compiled successfully; 41 library unit tests passed, one real-TLS contract failed at the bounded Python fixture readiness check (ready file absent while the child remained alive). The later integration tests, Swift host and iOS simulator were skipped in this macOS job, not passed. The Linux core/check/binding step and Kotlin host step passed; Android was still building when the repair was prepared. Original failure remains in run40295036441.

## Narrow change

Python HTTPServer.server_bind calls socket.getfqdn(host) even for an explicitly bound loopback fixture. This test has no need for reverse DNS: the certificate hostname and expected Origin are already fixed. Replace that implicit lookup with TCPServer.server_bind and explicit fixture-only server_name/server_port. Publish the readiness file by atomic rename after TLS initialization to avoid reading a newly created empty file.

This removes a concrete unnecessary external timing dependency. The macOS log alone does not prove reverse DNS is the unique root cause; the original test must actually pass after this change. No startup timeout is extended, no TLS certificate/hostname check is bypassed, no production code, dependency, transport contract, assertion or test is removed.

Local validation deliberately made socket.getfqdn raise. All five fixture modes still initialized within the original five-second bound (under one second locally), used real CA and hostname verification, returned the expected response/connection close and counted exactly one POST. This validates the Python fixture only, not the Rust test or actual NativeSync service.

## Next

Wait for the current Android job to finish without cancellation or duplicate CI. Commit this bounded fix only if HEAD still matches the expected parent; inspect any additional failures before the one targeted repair rerun. Keep M2 service acceptance and physical-device/signing results separate.
