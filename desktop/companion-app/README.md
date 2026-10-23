# StepX伴侣

Native phone runtime for the 17 × 23 (379 physical LED) StepX rear display. It has no network permission, boot receiver, general shell runner, file browser or arbitrary Binder entry point. Sensor handling, game rules and rendering run on the phone. Desktop is only needed to install/adapt/configure it.

Build with `bash scripts/build-companion-app.sh`. Android SDK 36 build tools/platform and the locally configured JDK are used without Gradle or downloaded dependencies. The persistent signing key and password file live outside the repository in `~/.config/stepx-companion/signing/` with restricted permissions. Keep that key to update an installed copy.

## Android lifecycle and permissions

Package: `com.stepx.companion.app`; launcher label: **StepX伴侣**. The exported foreground service is `.CompanionService`, protected by `android.permission.DUMP`; the app's own Activity and notification action can call it under their own UID. It requests `com.stepos.permission.MANAGE_BACK_SCREEN`, foreground service/specialUse, notifications, phone state and wake lock. It does not request WRITE_SECURE_SETTINGS, root, system UID or internet. The desktop separately performs the explicitly authorized, package/signature-specific Step AOS permission adaptation.

The app reads/writes the observed `android.hardware.display.IStepOSDisplayManager` Binder interface. It checks its descriptor and reports failures rather than changing system security policy. It keeps an original display mode, native type, brightness mode/value and auto-rotation baseline in private preferences, then verifies restoration. The pending baseline is recovered on a later status/start/stop after a process crash; Android does not guarantee `onDestroy` after a force kill. It is not a system-integrated crash watchdog.

Nothing starts at boot. Standby is only armed by the user; it holds no wake lock. This device's accelerometer/gravity/gyro are **non-wakeup** sensors, so triple knocks cannot guarantee waking a deeply suspended processor. Pick the phone up first if necessary. During an active game or multi-frame/dynamic animation, a 60-second partial wake lock is renewed only while posture, call, power and thermal checks allow output. It is released on pause/stop. Real games continue until a stable flip to the front or an explicit stop; each individual CPU lease remains bounded. A static image needs no app wake lock. Animation duration 0 means continue until explicitly stopped (subject to system policies).

## Command protocol (version 1)

Use a fresh bounded request identifier for every command:

```
adb -s SERIAL shell am start-foreground-service \
  -n com.stepx.companion.app/.CompanionService \
  --es op status --es request_id REQUEST_ID
```

Supported operations:

* `status`: creates the external files directory, recovers any pending baseline, writes status, and stops itself if otherwise idle.
* `standby`: arms triple-knock entry. If an animation is playing, it stays visible.
* `game`: starts a real sensor-controlled game and enables standby. The current applied animation resumes when the game exits.
* `stop`: disables standby/output and restores original phone settings.
* `apply --es config_name incoming.json`: reads only the fixed app-owned `files/incoming.json`, validates it before applying, and stores the accepted configuration internally.
* `test --ez test_mode true`: explicitly synthetic alignment through five levels while sending frames to the real display. This does not prove physical knock/tilt correctness. It stops/restores after the bounded test and reports `lastTest` separately from real sensor counters.

The app-owned paths are:

```
/sdcard/Android/data/com.stepx.companion.app/files/status.json
/sdcard/Android/data/com.stepx.companion.app/files/incoming.json
```

`status.json` is atomically replaced no more often than every 250 ms. Check `requestId` equality and `commandReady`, then `lastError`. It includes service instance creation time, start count/id and the actual foreground service type for lifecycle diagnosis, plus mode, phase, pausedReason, jobId, appliedTitle, settingsRestored/restorePending, live sensor counts, actual tap trigger count, raw/calibrated tilt, game bar/slot positions and equal widths, score and submitted frame count. `outputAccepted` means Binder submission; `outputVerified` remains false because the app cannot independently read the protected HAL dump. Desktop must verify actual HAL power/frame ownership when claiming real output.

Frame configuration, maximum 256 KiB, 1–120 frames, 391 integer bytes per frame, all 12 absent corner slots zero:

```json
{"requestId":"REQUEST_ID","title":"旋转太空人","kind":"frames","fps":6,"brightness":5,"durationSecs":0,"frames":[[0]]}
```

The `[0]` above is schematic: every actual frame must contain exactly 391 values. Provide full-range pixels; brightness is applied once by the phone driver. `fps` is 1–12, `brightness` 1–8, and `durationSecs` 0–3600. `requestId` in the config, if supplied, must match the command. Only a regular non-symlink file at the fixed path is accepted.

Dynamic presets use `kind: "preset"`, `key: "clock" | "battery" | "countdown"` instead of frames. Phone time and battery are read locally; countdown requires a positive duration and displays minutes/seconds above 99 seconds. Other unsupported dynamic presets are rejected rather than silently frozen.

## Game and validation

Both falling bar and bottom target slot progress through widths 5, 4, 3, 2, 1. The first two levels allow one column of landing tolerance and snap to the slot; later levels require exact alignment. Positive Android gravity X maps to increasing back-view LED columns, with hypot(Y,Z) normalization; the firmware preserves column order when rotation is locked. Gravity is remapped for the back view, the game's output is locked to natural portrait, a three-second neutral calibration precedes play, and movement uses smoothing, a dead zone and column hysteresis. Each drop takes 12, 11, 10, 9 and 8 seconds respectively; later rounds repeat these speeds. Completing width 1 starts another round. Misses restart at width 5 and preserve cumulative score. The actual backscreen shows the total in four 3×5 digits at the top (K/M abbreviation for large values), with the playfield below. Best and last-session scores are kept locally. During play knocks are ignored; turning to the front for 600 ms ends the session and restores the applied animation/standby. A game-rotation-vector normal detects upright 180° turns; its reference follows gradual heading changes within 45° with a 1.5-second low-pass filter, while fast flips freeze that reference. A rapid whole-body turn is physically indistinguishable from turning only the phone using these sensors. front-up gravity above 5.5 m/s² is a fallback. Exit arms only after calibration and a back-facing/vertical pose (gravity Z below 2), so a desktop-started game initially lying front-up waits for the user.

Knock recognition uses separated Z-axis impulses relative to gravity, lateral/gyro rejection, a 4.5 m/s² minimum normal impulse, 120 ms debounce, 650 ms maximum inter-knock gap, 1600 ms total window and post-trigger cooldown. Front-up gravity above 7.5 m/s² prevents entry and shows a posture hint; held angled poses are accepted. Status includes the last ten bounded impulse candidates and rejection reasons. These tuning values do not imply universal recognition accuracy. Physically test bare phone/case, deliberate triple knocks, single/double knocks, placing on a table, walking and opposite tilt directions.

`GameCoreTest` covers knock counting/debounce/rejection, synthetic five-level completion, continuous real-input multi-round progression beyond five minutes, score LED rendering, 12-to-8-second drop timing, miss recovery, direction mapping, upright rotation/gravity flip debounce and 379 LED masking. Real sensor streaming, real output, restoration, foreground-service behavior, unplugged operation and physical input must be validated separately on the paired phone.
