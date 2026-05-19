// warp-driver.swift
//
// Visual-anchored UI driver for Warp / warp-oss windows. Reads a recipe
// (JSON) describing a sequence of steps — activates, waits, OCR checks,
// clicks, types, key presses — and executes them in order against a
// running target process.
//
// The primary safety invariant: typed text and clicks are gated on
// `expect_text` / `click_text` steps that use macOS Vision OCR against a
// window-scoped CGImage snapshot. If the expected text isn't visible,
// the step fails and the driver exits non-zero WITHOUT typing anything.
// This guards against keystroke leakage into the wrong UI surface
// (shell command palette, universal search, etc.) — the failure mode we
// hit with blind CGEventPost drivers.
//
// Usage:
//   swift warp-driver.swift <recipe.json>
//
// Exit codes:
//   0   recipe completed
//   1   recipe step failed (assertion miss, window not found, etc.)
//   2   bad invocation / unparseable recipe
//
// Requirements:
//   - macOS 12+ (Vision text recognition, ScreenCaptureKit if needed)
//   - Screen Recording permission for the parent process
//   - Accessibility permission for the parent process (CGEventPost)
//
// Recipe JSON schema:
//
//   {
//     "name": "9853-mcp-fresh-conversation",
//     "target_process": "warp-oss",
//     "steps": [
//       {"op": "activate"},
//       {"op": "wait",       "ms": 1500},
//       {"op": "expect_text","regex": "Warp anything", "timeout_ms": 5000},
//       {"op": "abort_if_text","regex": "(Search sessions|Search tabs)"},
//       {"op": "click_text", "regex": "Warp anything"},
//       {"op": "wait",       "ms": 400},
//       {"op": "type",       "text": "use the whoami tool from proof-mcp"},
//       {"op": "wait",       "ms": 250},
//       {"op": "press",      "key": "escape"},
//       {"op": "wait",       "ms": 150},
//       {"op": "press",      "key": "return"},
//       {"op": "wait",       "ms": 18000},
//       {"op": "screenshot", "path": "/tmp/final.png"}
//     ]
//   }
//
// Step types:
//   - activate                         bring target_process to front
//   - wait        ms:Int               sleep
//   - expect_text regex, timeout_ms?   abort if no OCR match within timeout
//   - abort_if_text regex              abort if OCR match found (defensive)
//   - click_text  regex                OCR-find then click center of bbox
//   - click_at    x:Double, y:Double   click at window-relative coords (0..1)
//   - type        text                 unicode keystrokes
//   - press       key                  named key (return, escape, tab, ...)
//   - chord       mods:[String], key   keyboard chord
//   - screenshot  path:String          dump current window image to file

import AppKit
import CoreGraphics
import Foundation
import Vision

// MARK: - Recipe model

struct Recipe: Decodable {
    let name: String
    let target_process: String
    let steps: [Step]
}

enum Step {
    case activate
    case wait(ms: Int)
    case expectText(regex: String, timeoutMs: Int)
    case abortIfText(regex: String)
    case clickText(regex: String)
    case clickOffsetFromText(regex: String, dx: Double, dy: Double)
    case clickAt(x: Double, y: Double)
    case clickAtIfText(regex: String, x: Double, y: Double)
    case clickAtIfNotText(regex: String, x: Double, y: Double)
    case type(text: String)
    case press(key: String)
    case chord(mods: [String], key: String)
    case osChord(mods: [String], key: String)
    case osKeyCode(code: Int, mods: [String])
    case pasteText(text: String)
    case resizeWindow(x: Int, y: Int, width: Int, height: Int)
    case screenshot(path: String)
}

extension Step: Decodable {
    private enum Keys: String, CodingKey {
        case op, regex, ms, text, key, code, mods, path, timeout_ms, x, y, dx, dy, width, height
    }

    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: Keys.self)
        let op = try c.decode(String.self, forKey: .op)
        switch op {
        case "activate":
            self = .activate
        case "wait":
            self = .wait(ms: try c.decode(Int.self, forKey: .ms))
        case "expect_text":
            self = .expectText(
                regex: try c.decode(String.self, forKey: .regex),
                timeoutMs: try c.decodeIfPresent(Int.self, forKey: .timeout_ms) ?? 5000)
        case "abort_if_text":
            self = .abortIfText(regex: try c.decode(String.self, forKey: .regex))
        case "click_text":
            self = .clickText(regex: try c.decode(String.self, forKey: .regex))
        case "click_offset_from_text":
            self = .clickOffsetFromText(
                regex: try c.decode(String.self, forKey: .regex),
                dx: try c.decode(Double.self, forKey: .dx),
                dy: try c.decodeIfPresent(Double.self, forKey: .dy) ?? 0)
        case "click_at":
            self = .clickAt(
                x: try c.decode(Double.self, forKey: .x),
                y: try c.decode(Double.self, forKey: .y))
        case "click_at_if_text":
            self = .clickAtIfText(
                regex: try c.decode(String.self, forKey: .regex),
                x: try c.decode(Double.self, forKey: .x),
                y: try c.decode(Double.self, forKey: .y))
        case "click_at_if_not_text":
            self = .clickAtIfNotText(
                regex: try c.decode(String.self, forKey: .regex),
                x: try c.decode(Double.self, forKey: .x),
                y: try c.decode(Double.self, forKey: .y))
        case "type":
            self = .type(text: try c.decode(String.self, forKey: .text))
        case "press":
            self = .press(key: try c.decode(String.self, forKey: .key))
        case "chord":
            self = .chord(
                mods: try c.decode([String].self, forKey: .mods),
                key: try c.decode(String.self, forKey: .key))
        case "os_chord":
            self = .osChord(
                mods: try c.decode([String].self, forKey: .mods),
                key: try c.decode(String.self, forKey: .key))
        case "os_key_code":
            self = .osKeyCode(
                code: try c.decode(Int.self, forKey: .code),
                mods: try c.decodeIfPresent([String].self, forKey: .mods) ?? [])
        case "paste_text":
            self = .pasteText(text: try c.decode(String.self, forKey: .text))
        case "resize_window":
            self = .resizeWindow(
                x: try c.decode(Int.self, forKey: .x),
                y: try c.decode(Int.self, forKey: .y),
                width: try c.decode(Int.self, forKey: .width),
                height: try c.decode(Int.self, forKey: .height))
        case "screenshot":
            self = .screenshot(path: try c.decode(String.self, forKey: .path))
        default:
            throw DecodingError.dataCorruptedError(
                forKey: .op, in: c, debugDescription: "unknown op '\(op)'")
        }
    }
}

// MARK: - Logging

func info(_ s: String) {
    FileHandle.standardError.write("warp-driver: \(s)\n".data(using: .utf8)!)
}

func die(_ s: String, code: Int32 = 1) -> Never {
    FileHandle.standardError.write("warp-driver: FAIL: \(s)\n".data(using: .utf8)!)
    exit(code)
}

// MARK: - Window discovery + capture

struct TargetWindow {
    let id: CGWindowID
    let frame: CGRect  // screen coords, top-left origin (points)
}

func findTargetWindow(processName: String) -> TargetWindow? {
    guard
        let info = CGWindowListCopyWindowInfo(
            [.optionOnScreenOnly, .excludeDesktopElements], kCGNullWindowID)
            as? [[String: Any]]
    else { return nil }

    let candidates = info.filter { entry in
        let owner = entry[kCGWindowOwnerName as String] as? String
        let layer = entry[kCGWindowLayer as String] as? Int ?? 0
        return owner == processName && layer == 0
    }

    // Pick the largest on-screen window (skip menubar / utility windows).
    let best =
        candidates
        .compactMap { entry -> (CGWindowID, CGRect)? in
            guard let id = entry[kCGWindowNumber as String] as? CGWindowID,
                let boundsDict = entry[kCGWindowBounds as String] as? [String: Any],
                let r = CGRect(dictionaryRepresentation: boundsDict as CFDictionary)
            else { return nil }
            return (id, r)
        }
        .max(by: { $0.1.width * $0.1.height < $1.1.width * $1.1.height })

    return best.map { TargetWindow(id: $0.0, frame: $0.1) }
}

func captureWindow(_ w: TargetWindow) -> CGImage? {
    return CGWindowListCreateImage(
        .null,
        [.optionIncludingWindow],
        w.id,
        [.boundsIgnoreFraming, .bestResolution]
    )
}

// MARK: - OCR

struct OCRMatch {
    let text: String
    // Window-relative point coords, top-left origin.
    let centerInWindow: CGPoint
    // Screen-space point coords, top-left origin (computed via window.frame).
    let centerOnScreen: CGPoint
}

/// Run Vision text recognition on a window snapshot, returning all
/// observations matching `regex` mapped into window/screen coordinates.
func ocrFindAll(in window: TargetWindow, image: CGImage, regex: NSRegularExpression)
    -> [OCRMatch]
{
    var results: [OCRMatch] = []
    let request = VNRecognizeTextRequest()
    request.recognitionLevel = .accurate
    request.usesLanguageCorrection = false

    let handler = VNImageRequestHandler(cgImage: image, options: [:])
    do { try handler.perform([request]) } catch {
        info("OCR perform failed: \(error)")
        return []
    }
    guard let obs = request.results else { return [] }

    let wFrame = window.frame  // screen points

    for o in obs {
        guard let top = o.topCandidates(1).first else { continue }
        let s = top.string
        let range = NSRange(s.startIndex..<s.endIndex, in: s)
        guard regex.firstMatch(in: s, options: [], range: range) != nil else { continue }

        // Vision's boundingBox: normalized 0..1 image coords, bottom-left origin.
        let bb = o.boundingBox

        // Convert to window-relative POINT coords (top-left origin).
        let relX = (bb.origin.x + bb.width / 2.0) * wFrame.width
        let relYBottomUp = (bb.origin.y + bb.height / 2.0) * wFrame.height
        let relY = wFrame.height - relYBottomUp
        let centerWin = CGPoint(x: relX, y: relY)

        // Convert to screen-space coords by adding window origin.
        let centerScr = CGPoint(x: wFrame.origin.x + relX, y: wFrame.origin.y + relY)

        results.append(OCRMatch(text: s, centerInWindow: centerWin, centerOnScreen: centerScr))
    }
    return results
}

func ocrFirstMatch(processName: String, regex: NSRegularExpression) -> OCRMatch? {
    guard let w = findTargetWindow(processName: processName) else { return nil }
    guard let img = captureWindow(w) else { return nil }
    return ocrFindAll(in: w, image: img, regex: regex).first
}

// MARK: - Input synthesis

let eventSource = CGEventSource(stateID: .hidSystemState)

// US-layout virtual key codes for keys addressable by `press` / `chord`.
let namedKeys: [String: CGKeyCode] = [
    "return": 36,
    "enter": 36,
    "escape": 53,
    "esc": 53,
    "tab": 48,
    "space": 49,
    "delete": 51,
    "backspace": 51,
    "left": 123,
    "right": 124,
    "up": 126,
    "down": 125,
    "grave": 50,
    "backtick": 50,
    "n": 45,
    "p": 35,
    "i": 34,
    "k": 40,
    "l": 37,
    "v": 9,
    "t": 17,
    "a": 0,
    "c": 8,
    "w": 13,
    "f": 3,
    ",": 43,
    "comma": 43,
    ".": 47,
    "period": 47,
    "slash": 44,
]

func cgFlags(for mods: [String]) -> CGEventFlags {
    var flags: CGEventFlags = []
    for m in mods {
        switch m.lowercased() {
        case "cmd", "command": flags.insert(.maskCommand)
        case "ctrl", "control": flags.insert(.maskControl)
        case "opt", "option", "alt": flags.insert(.maskAlternate)
        case "shift": flags.insert(.maskShift)
        default: info("unknown modifier '\(m)' — ignored")
        }
    }
    return flags
}

func postKey(_ vk: CGKeyCode, flags: CGEventFlags = []) {
    if let down = CGEvent(keyboardEventSource: eventSource, virtualKey: vk, keyDown: true) {
        down.flags = flags
        down.post(tap: .cgAnnotatedSessionEventTap)
    }
    if let up = CGEvent(keyboardEventSource: eventSource, virtualKey: vk, keyDown: false) {
        up.flags = flags
        up.post(tap: .cgAnnotatedSessionEventTap)
    }
}

func postString(_ s: String) {
    for ch in s {
        let str = String(ch)
        if let down = CGEvent(keyboardEventSource: eventSource, virtualKey: 0, keyDown: true) {
            down.keyboardSetUnicodeString(
                stringLength: str.utf16.count, unicodeString: Array(str.utf16))
            down.post(tap: .cgAnnotatedSessionEventTap)
        }
        if let up = CGEvent(keyboardEventSource: eventSource, virtualKey: 0, keyDown: false) {
            up.keyboardSetUnicodeString(
                stringLength: str.utf16.count, unicodeString: Array(str.utf16))
            up.post(tap: .cgAnnotatedSessionEventTap)
        }
        Thread.sleep(forTimeInterval: 0.035)
    }
}

func clickAtScreen(_ p: CGPoint) {
    let move = CGEvent(
        mouseEventSource: eventSource, mouseType: .mouseMoved, mouseCursorPosition: p,
        mouseButton: .left)
    move?.post(tap: .cghidEventTap)
    Thread.sleep(forTimeInterval: 0.05)
    let down = CGEvent(
        mouseEventSource: eventSource, mouseType: .leftMouseDown, mouseCursorPosition: p,
        mouseButton: .left)
    down?.post(tap: .cghidEventTap)
    Thread.sleep(forTimeInterval: 0.05)
    let up = CGEvent(
        mouseEventSource: eventSource, mouseType: .leftMouseUp, mouseCursorPosition: p,
        mouseButton: .left)
    up?.post(tap: .cghidEventTap)
}

// MARK: - Activation

func activate(processName: String) -> Bool {
    // Force frontmost via AppleScript — NSWorkspace.activate() alone is
    // unreliable enough that follow-up CGEventPost input regularly lands
    // in the script's parent terminal/IDE instead of warp-oss. See
    // feedback_warp_oss_input_routing.md.
    return ensureFrontmost(processName: processName, settle: 0.6)
}

/// Drives `System Events` to make `processName` the frontmost app and
/// returns once focus has settled. Every input-emitting op should call
/// this immediately before posting events.
@discardableResult
func ensureFrontmost(processName: String, settle: TimeInterval = 0.3) -> Bool {
    let script =
        "tell application \"System Events\" to set frontmost of (first process whose name is \"\(processName)\") to true"
    var err: NSDictionary?
    if let s = NSAppleScript(source: script) {
        s.executeAndReturnError(&err)
    }
    if let err {
        info("ensureFrontmost(\(processName)): \(err)")
        return false
    }
    Thread.sleep(forTimeInterval: settle)
    return true
}

/// Send a key + modifiers via AppleScript `keystroke ... using {...}`.
/// Routes through the macOS keyboard event pipeline, which Warp's
/// Settings JSON editor accepts where raw CGEventPost is ignored.
func osChord(processName: String, mods: [String], key: String) {
    ensureFrontmost(processName: processName, settle: 0.2)
    let modList = mods.map { m -> String in
        switch m.lowercased() {
        case "cmd", "command": return "command down"
        case "ctrl", "control": return "control down"
        case "opt", "option", "alt": return "option down"
        case "shift": return "shift down"
        default:
            info("osChord: unknown modifier '\(m)' — ignored")
            return ""
        }
    }.filter { !$0.isEmpty }
    let modSegment = modList.isEmpty ? "" : " using {\(modList.joined(separator: ", "))}"
    let escapedKey = key.replacingOccurrences(of: "\"", with: "\\\"")
    let script =
        "tell application \"System Events\" to keystroke \"\(escapedKey)\"\(modSegment)"
    var err: NSDictionary?
    if let s = NSAppleScript(source: script) {
        s.executeAndReturnError(&err)
    }
    if let err {
        die("osChord(\(mods)+\(key)): \(err)")
    }
}

/// Send a specific virtual key code via AppleScript `key code N using {...}`.
/// Use this for keys not addressable by `keystroke` (e.g. forward-delete = 117).
func osKeyCode(processName: String, code: Int, mods: [String] = []) {
    ensureFrontmost(processName: processName, settle: 0.2)
    let modList = mods.map { m -> String in
        switch m.lowercased() {
        case "cmd", "command": return "command down"
        case "ctrl", "control": return "control down"
        case "opt", "option", "alt": return "option down"
        case "shift": return "shift down"
        default: return ""
        }
    }.filter { !$0.isEmpty }
    let modSegment = modList.isEmpty ? "" : " using {\(modList.joined(separator: ", "))}"
    let script = "tell application \"System Events\" to key code \(code)\(modSegment)"
    var err: NSDictionary?
    if let s = NSAppleScript(source: script) {
        s.executeAndReturnError(&err)
    }
    if let err {
        die("osKeyCode(\(code) + \(mods)): \(err)")
    }
}

// MARK: - PNG dump for screenshot step

func dumpPNG(_ image: CGImage, to path: String) {
    let rep = NSBitmapImageRep(cgImage: image)
    guard let data = rep.representation(using: .png, properties: [:]) else {
        info("could not encode PNG at \(path)")
        return
    }
    let url = URL(fileURLWithPath: path)
    try? FileManager.default.createDirectory(
        at: url.deletingLastPathComponent(), withIntermediateDirectories: true)
    try? data.write(to: url)
}

// MARK: - Step execution

func compileRegex(_ pattern: String) -> NSRegularExpression {
    do {
        return try NSRegularExpression(pattern: pattern, options: [.caseInsensitive])
    } catch {
        die("bad regex /\(pattern)/: \(error)", code: 2)
    }
}

func run(step: Step, target processName: String, idx: Int) {
    switch step {

    case .activate:
        info("[\(idx)] activate '\(processName)'")
        if !activate(processName: processName) {
            die("could not activate '\(processName)'")
        }

    case .wait(let ms):
        info("[\(idx)] wait \(ms)ms")
        Thread.sleep(forTimeInterval: Double(ms) / 1000.0)

    case .expectText(let pattern, let timeoutMs):
        info("[\(idx)] expect_text /\(pattern)/ within \(timeoutMs)ms")
        let re = compileRegex(pattern)
        let deadline = Date().addingTimeInterval(Double(timeoutMs) / 1000.0)
        var found = false
        while Date() < deadline {
            if ocrFirstMatch(processName: processName, regex: re) != nil {
                found = true
                break
            }
            Thread.sleep(forTimeInterval: 0.3)
        }
        if !found { die("expect_text /\(pattern)/ not satisfied within \(timeoutMs)ms") }

    case .abortIfText(let pattern):
        info("[\(idx)] abort_if_text /\(pattern)/")
        let re = compileRegex(pattern)
        if let m = ocrFirstMatch(processName: processName, regex: re) {
            die("abort_if_text matched '\(m.text)' — refusing to continue")
        }

    case .clickText(let pattern):
        info("[\(idx)] click_text /\(pattern)/")
        let re = compileRegex(pattern)
        guard let m = ocrFirstMatch(processName: processName, regex: re) else {
            die("click_text /\(pattern)/ — no match; refusing to click")
        }
        info("    matched '\(m.text)' at screen \(m.centerOnScreen)")
        ensureFrontmost(processName: processName, settle: 0.2)
        clickAtScreen(m.centerOnScreen)

    case .clickOffsetFromText(let pattern, let dx, let dy):
        // Anchor on OCR-matched text, then click at a window-pixel offset
        // from the match's center. Used for hitting UI elements (toggles,
        // icons) that sit alongside a label on the same row — the label
        // gives us a stable Y anchor and the dx/dy locates the widget.
        info("[\(idx)] click_offset_from_text /\(pattern)/ +(\(dx), \(dy))")
        let re = compileRegex(pattern)
        guard let m = ocrFirstMatch(processName: processName, regex: re) else {
            die("click_offset_from_text /\(pattern)/ — no match; refusing to click")
        }
        let p = CGPoint(
            x: m.centerOnScreen.x + dx,
            y: m.centerOnScreen.y + dy)
        info("    matched '\(m.text)' — clicking at \(p)")
        ensureFrontmost(processName: processName, settle: 0.2)
        clickAtScreen(p)

    case .clickAt(let nx, let ny):
        info("[\(idx)] click_at (\(nx), \(ny))")
        guard let w = findTargetWindow(processName: processName) else {
            die("click_at — target window not found")
        }
        let p = CGPoint(
            x: w.frame.origin.x + nx * w.frame.width,
            y: w.frame.origin.y + ny * w.frame.height)
        ensureFrontmost(processName: processName, settle: 0.2)
        clickAtScreen(p)

    case .clickAtIfText(let pattern, let nx, let ny):
        // Conditional click: click at window-relative coords ONLY when the
        // OCR regex finds a match. No-op otherwise (with a logged note).
        // Lets recipes write idempotent state-normalization preambles —
        // e.g. "click the redaction toggle to OFF only if the dropdown is
        // currently visible (i.e. toggle is currently ON)".
        info("[\(idx)] click_at_if_text /\(pattern)/ then (\(nx), \(ny))")
        let re = compileRegex(pattern)
        if ocrFirstMatch(processName: processName, regex: re) != nil {
            guard let w = findTargetWindow(processName: processName) else {
                die("click_at_if_text — target window not found")
            }
            let p = CGPoint(
                x: w.frame.origin.x + nx * w.frame.width,
                y: w.frame.origin.y + ny * w.frame.height)
            info("    matched — clicking at \(p)")
            ensureFrontmost(processName: processName, settle: 0.2)
            clickAtScreen(p)
        } else {
            info("    /\(pattern)/ not visible — skipping click")
        }

    case .clickAtIfNotText(let pattern, let nx, let ny):
        // Inverse of click_at_if_text: click only when the regex does NOT
        // match. Useful for "ensure toggle is ON: click only if the
        // dropdown is NOT currently visible".
        info("[\(idx)] click_at_if_not_text /\(pattern)/ then (\(nx), \(ny))")
        let re = compileRegex(pattern)
        if ocrFirstMatch(processName: processName, regex: re) == nil {
            guard let w = findTargetWindow(processName: processName) else {
                die("click_at_if_not_text — target window not found")
            }
            let p = CGPoint(
                x: w.frame.origin.x + nx * w.frame.width,
                y: w.frame.origin.y + ny * w.frame.height)
            info("    /\(pattern)/ not visible — clicking at \(p)")
            ensureFrontmost(processName: processName, settle: 0.2)
            clickAtScreen(p)
        } else {
            info("    matched — skipping click")
        }

    case .type(let text):
        info("[\(idx)] type '\(text)' (\(text.count) chars)")
        ensureFrontmost(processName: processName, settle: 0.2)
        postString(text)

    case .press(let keyName):
        info("[\(idx)] press \(keyName)")
        guard let vk = namedKeys[keyName.lowercased()] else {
            die("press: unknown key '\(keyName)' — add to namedKeys table")
        }
        ensureFrontmost(processName: processName, settle: 0.2)
        postKey(vk)

    case .chord(let mods, let keyName):
        info("[\(idx)] chord \(mods)+\(keyName)")
        guard let vk = namedKeys[keyName.lowercased()] else {
            die("chord: unknown key '\(keyName)' — add to namedKeys table")
        }
        ensureFrontmost(processName: processName, settle: 0.2)
        postKey(vk, flags: cgFlags(for: mods))

    case .osChord(let mods, let key):
        info("[\(idx)] os_chord \(mods)+\(key)")
        osChord(processName: processName, mods: mods, key: key)

    case .osKeyCode(let code, let mods):
        info("[\(idx)] os_key_code \(code) mods=\(mods)")
        osKeyCode(processName: processName, code: code, mods: mods)

    case .pasteText(let text):
        // Set the system clipboard, then post Cmd+V via AppleScript so
        // the keystroke routes through the macOS event pipeline that
        // Warp's Settings JSON editor honors. The CGEventPost variant
        // works in terminal/agent input fields but is silently ignored
        // by the code editor.
        info("[\(idx)] paste_text (\(text.count) chars)")
        let pb = NSPasteboard.general
        pb.clearContents()
        pb.setString(text, forType: .string)
        Thread.sleep(forTimeInterval: 0.15)
        osChord(processName: processName, mods: ["cmd"], key: "v")

    case .resizeWindow(let x, let y, let w, let h):
        // Set the front-window bounds of the target process via AppleScript.
        // Smaller, predictable windows mean less downscale when converting
        // captured .mov → .gif, which keeps recorded text readable.
        info("[\(idx)] resize_window {x:\(x),y:\(y),w:\(w),h:\(h)}")
        let script = """
            tell application "System Events"
              tell process "\(processName)"
                set position of front window to {\(x), \(y)}
                set size of front window to {\(w), \(h)}
              end tell
            end tell
            """
        var err: NSDictionary?
        if let s = NSAppleScript(source: script) {
            s.executeAndReturnError(&err)
        }
        if let err {
            die("resize_window failed: \(err)")
        }
        Thread.sleep(forTimeInterval: 0.4)

    case .screenshot(let path):
        info("[\(idx)] screenshot \(path)")
        guard let w = findTargetWindow(processName: processName) else {
            die("screenshot — target window not found")
        }
        guard let img = captureWindow(w) else { die("screenshot — capture failed") }
        dumpPNG(img, to: path)
    }
}

// MARK: - Main

private let appShared = NSApplication.shared
_ = appShared.setActivationPolicy(.accessory)

guard CommandLine.arguments.count >= 2 else {
    FileHandle.standardError.write(
        "usage: warp-driver.swift <recipe.json>\n".data(using: .utf8)!)
    exit(2)
}
let recipePath = CommandLine.arguments[1]
guard let recipeData = FileManager.default.contents(atPath: recipePath) else {
    die("cannot read recipe at \(recipePath)", code: 2)
}
let recipe: Recipe
do {
    recipe = try JSONDecoder().decode(Recipe.self, from: recipeData)
} catch {
    die("recipe parse error: \(error)", code: 2)
}

info("recipe '\(recipe.name)' against process '\(recipe.target_process)' — \(recipe.steps.count) steps")

for (i, step) in recipe.steps.enumerated() {
    run(step: step, target: recipe.target_process, idx: i)
}

info("recipe '\(recipe.name)' completed")
