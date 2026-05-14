// drive-warp.swift
//
// Playwright-style driver for the Warp.app window. Brings Warp to the
// front, types a command via synthesized keyboard events, presses Return,
// then exits. Used by orchestrate-demo.sh to make the recorded window
// actually do something during a ScreenCaptureKit capture.
//
// Usage:
//   swift drive-warp.swift <command-to-type>
//
// Requires:
//   - macOS (anything modern; uses CGEventPost / NSWorkspace)
//   - Accessibility permission for the parent app (Terminal / VSCode).
//     System Settings → Privacy & Security → Accessibility.

import AppKit
import CoreGraphics

private let app = NSApplication.shared
_ = app.setActivationPolicy(.accessory)

// 1. Find Warp.app and activate it (foreground).
guard let warp = NSWorkspace.shared.runningApplications.first(where: {
    $0.localizedName == "Warp"
}) else {
    FileHandle.standardError.write("drive-warp: Warp is not running.\n".data(using: .utf8)!)
    exit(1)
}

let activated = warp.activate(options: [.activateAllWindows])
if !activated {
    FileHandle.standardError.write("drive-warp: could not activate Warp.\n".data(using: .utf8)!)
    exit(1)
}

// Give the window-server a moment to surface Warp.
Thread.sleep(forTimeInterval: 0.6)

guard CommandLine.arguments.count >= 2 else {
    FileHandle.standardError.write("usage: drive-warp.swift <command-to-type>\n".data(using: .utf8)!)
    exit(2)
}
let typed = CommandLine.arguments[1]

// 2. Type the command character-by-character via Unicode keyboard events.
//    Using kCGEventKeyDown with setUnicodeString lets us avoid keymap
//    games for `$`, `~`, etc. — macOS interprets the string we set, not
//    the (virtual) keycode 0 we synthesize.
let source = CGEventSource(stateID: .hidSystemState)

func postString(_ s: String) {
    for ch in s {
        let str = String(ch)
        if let down = CGEvent(keyboardEventSource: source, virtualKey: 0, keyDown: true) {
            down.keyboardSetUnicodeString(stringLength: str.utf16.count, unicodeString: Array(str.utf16))
            down.post(tap: .cgAnnotatedSessionEventTap)
        }
        if let up = CGEvent(keyboardEventSource: source, virtualKey: 0, keyDown: false) {
            up.keyboardSetUnicodeString(stringLength: str.utf16.count, unicodeString: Array(str.utf16))
            up.post(tap: .cgAnnotatedSessionEventTap)
        }
        // Human-paced typing makes the recording readable.
        Thread.sleep(forTimeInterval: 0.04)
    }
}

func postReturn() {
    // Return is keycode 36 on US layout — bypass setUnicodeString for it.
    if let down = CGEvent(keyboardEventSource: source, virtualKey: 36, keyDown: true) {
        down.post(tap: .cgAnnotatedSessionEventTap)
    }
    if let up = CGEvent(keyboardEventSource: source, virtualKey: 36, keyDown: false) {
        up.post(tap: .cgAnnotatedSessionEventTap)
    }
}

postString(typed)
Thread.sleep(forTimeInterval: 0.2)
postReturn()
FileHandle.standardError.write("drive-warp: typed \(typed.count) chars + Return\n".data(using: .utf8)!)
