# Android one-round chat and restart acceptance

Not executed. Use only the specifically approved isolated NativeSync R2.1
service, synthetic Topic/account and nonpaid model/test execution arrangement.
Do not substitute the Python TLS fixture for a real NativeSync server. Do not
change production, real history, SSH, security settings, credentials or signing.

Before starting, record the exact client commit/APK SHA256 and server R2.1
source/material identities, service origin, Android device/OS and build/run IDs.
Confirm the existing authorized Topic has ready central history and executable
configuration. An existing trusted service operator must provide one fresh
pairing code; the client must not create credentials or reuse desktop Cookies.

With an authorized phone and complete verified APK, the operator can run:

```sh
adb devices -l
sha256sum /absolute/path/to/verified-development.apk
adb -s APPROVED_DEVICE_SERIAL install -r /absolute/path/to/verified-development.apk
adb -s APPROVED_DEVICE_SERIAL shell am start -n org.vcptoolbox.nativeclient.dev/org.vcptoolbox.nativeclient.MainActivity
```

Select the exact device; confirm it is physical. The current cloud candidate
does not supply a complete APK; wait for separately approved build verification.
Enter the approved HTTPS origin and fresh code in the app, pair once, choose
the synthetic authorized Topic and read history. Send one distinct synthetic
message with **Send and receive**. Record its stable original request ID using
the local pending inventory and approved server execution evidence. Do not log
Cookie values, pairing codes, message payloads or raw vault exports.

Verify a user history row and assistant result appear without a second POST.
This candidate observes history updates; token-by-token streaming is still a
separate unimplemented contract. A 202, accepted or running state does not pass
AA final-result acceptance. Stop local reception and verify it issues no remote
cancel and no new model/tool/AA execution. Resume reception by explicit action.

For restart after an original was attempted:

```sh
adb -s APPROVED_DEVICE_SERIAL shell am force-stop org.vcptoolbox.nativeclient.dev
adb -s APPROVED_DEVICE_SERIAL shell am start -n org.vcptoolbox.nativeclient.dev/org.vcptoolbox.nativeclient.MainActivity
```

Do not clear app data, uninstall or remove its Keystore key. Startup must make
no POST. Explicitly restore the saved session, select the same Topic, inspect
pending and query the original receipt. Exact accepted identity may close only
that original; missing/ambiguous/changed authority retains it. Cross-session
recovery is read-only and never acquires task-control permission.

Using an isolated service's approved fault hook/proxy, drop the original HTTP
ACK **after acceptance**, then repeat restart/recovery. A second independent
server-side counter must prove exactly one original POST and no duplicate
model/tool/AA invocation. The current client does not configure such a fault
hook on a real service. Do not simulate ACK loss by guessing that every timeout
means the server accepted it. Repeat with temporary network loss, Topic switch,
credential replacement/removal and late replies; content must not cross views
and no stale response may clear pending.

Keep outcomes separate: build, synthetic TLS, simulator, actual NativeSync,
physical Android and AA final result. Record each failed/not-run item with exact
conditions. iOS Complete protection remains PENDING_REAL_DEVICE_ACCEPTANCE until
an iPhone and authorized signing/install path are available.
