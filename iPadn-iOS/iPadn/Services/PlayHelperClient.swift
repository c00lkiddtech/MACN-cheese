// MACN-cheese: Google Play (APK) support.
//
// Talks to the bundled `macn-play` helper (mac/PlayHelper, Rust) over its
// JSON-lines stdout. The helper speaks the Play protocol through gpapi /
// googleplay-protobuf from EFForg/rs-google-play (MIT) and follows the
// download flow of EFForg/apkeep (MIT). Sessions are anonymous ones from the
// Aurora OSS token dispenser; no personal Google account is used.
//
// Pure Foundation + POSIX so it also type-checks on Linux (see APPLY.md).

import Foundation
#if canImport(Darwin)
import Darwin
#elseif canImport(Glibc)
import Glibc
#endif

// MARK: - Models

struct PlayApp: Decodable, Identifiable, Hashable, Sendable {
    var package: String
    var title: String?
    var developer: String?
    var versionCode: Int64?
    var versionName: String?
    var size: Int64?
    var updated: String?
    var downloads: String?
    var targetSdk: Int?
    var category: String?
    var rating: Double?
    var icon: String?
    var free: Bool?
    var price: String?
    var splits: [String]?
    var description: String?

    var id: String { package }
    var displayTitle: String { (title?.isEmpty == false ? title : nil) ?? package }
    var isPaid: Bool { free == false }
    /// Google image URLs take a size suffix; 128 px is crisp at 64 pt on Retina.
    var iconURL: URL? {
        guard let icon, !icon.isEmpty else { return nil }
        return URL(string: icon.contains("=") ? icon : icon + "=s128")
    }
    var sizeLabel: String? {
        guard let size, size > 0 else { return nil }
        return ByteCountFormatter.string(fromByteCount: size, countStyle: .file)
    }
    var ratingLabel: String? {
        guard let rating, rating > 0 else { return nil }
        return String(format: "%.1f ★", rating)
    }
}

enum PlayPackageFormat: String, CaseIterable, Identifiable, Sendable {
    case auto, apk, apks, xapk, folder
    var id: String { rawValue }
    var title: String {
        switch self {
        case .auto: return "Automatic"
        case .apk: return "Single .apk"
        case .apks: return ".apks (SAI / bundletool)"
        case .xapk: return ".xapk (APKPure)"
        case .folder: return "Folder of APKs"
        }
    }
    var footnote: String {
        switch self {
        case .auto: return "A single .apk when the app has no splits, otherwise an .apks bundle."
        case .apk: return "Apps that ship as split APKs can't be merged into one file, so they fall back to .apks."
        case .apks: return "Base and split APKs in one archive. Install with SAI or `adb install-multiple`."
        case .xapk: return "Base and split APKs plus a manifest, for APKPure-style installers."
        case .folder: return "The raw base and split APKs side by side."
        }
    }
}

struct PlayDownloadResult: Sendable, Equatable {
    var path: String
    var format: String
    var size: Int64
    var package: String
    var title: String
    var versionName: String
    var splitCount: Int
    var url: URL { URL(fileURLWithPath: path) }
}

struct PlayProgress: Sendable, Equatable {
    var stage: String = ""
    var message: String = ""
    var done: Int64 = 0
    var total: Int64 = 0
    var fraction: Double? { total > 0 ? min(1, Double(done) / Double(total)) : nil }
}

struct PlayHelperError: LocalizedError, Equatable {
    var kind: String
    var message: String
    var errorDescription: String? {
        switch kind {
        case "paid": return "This is a paid app. MACN-cheese downloads free Google Play apps only."
        case "notFound": return message
        case "notAvailable": return message
        case "rateLimited", "dispenser": return "The anonymous Google Play sign-in service is busy. Try again in a minute."
        case "network": return "Couldn't reach Google Play. Check your connection. (\(message))"
        case "missingHelper": return "The Google Play helper (macn-play) isn't inside MACN-cheese.app. Rebuild with mac/scripts/build_play_helper.sh."
        case "cancelled": return "Cancelled."
        default: return message
        }
    }
}

/// Where the anonymous Play session lives between launches (Keychain on Apple platforms).
protocol PlaySessionStoring: AnyObject, Sendable {
    func load() -> (email: String, token: String)?
    func save(email: String, token: String)
    func clear()
}

final class InMemoryPlaySessionStore: PlaySessionStoring, @unchecked Sendable {
    private var value: (String, String)?
    func load() -> (email: String, token: String)? { value.map { (email: $0.0, token: $0.1) } }
    func save(email: String, token: String) { value = (email, token) }
    func clear() { value = nil }
}

// MARK: - Events

/// One line of helper output.
struct PlayEvent: @unchecked Sendable {  // JSON values are immutable once parsed
    let raw: [String: Any]
    var name: String { raw["event"] as? String ?? "" }
    func string(_ key: String) -> String? { raw[key] as? String }
    func int(_ key: String) -> Int64? {
        if let n = raw[key] as? NSNumber { return n.int64Value }
        return nil
    }

    static func parse(_ line: Data) -> PlayEvent? {
        guard !line.isEmpty,
              let obj = try? JSONSerialization.jsonObject(with: line) as? [String: Any] else { return nil }
        return PlayEvent(raw: obj)
    }

    func decode<T: Decodable>(_ type: T.Type, key: String) -> T? {
        guard let value = raw[key], JSONSerialization.isValidJSONObject([value]),
              let data = try? JSONSerialization.data(withJSONObject: [value]),
              let list = try? JSONDecoder().decode([T].self, from: data) else { return nil }
        return list.first
    }
}

/// Splits a byte stream into newline-terminated lines.
struct LineBuffer {
    private var pending = Data()
    mutating func append(_ chunk: Data) -> [Data] {
        pending.append(chunk)
        var lines: [Data] = []
        while let nl = pending.firstIndex(of: 0x0A) {
            lines.append(pending.subdata(in: pending.startIndex..<nl))
            pending.removeSubrange(pending.startIndex...nl)
        }
        return lines
    }
    mutating func finish() -> [Data] {
        defer { pending.removeAll() }
        return pending.isEmpty ? [] : [pending]
    }
}

// MARK: - Client

final class PlayHelperClient: @unchecked Sendable {
    let executable: URL?
    let sessions: PlaySessionStoring
    private let lock = NSLock()
    private var running: Set<pid_t> = []

    init(executable: URL? = PlayHelperClient.locateHelper(), sessions: PlaySessionStoring) {
        self.executable = executable
        self.sessions = sessions
    }

    /// macn-play ships in Contents/MacOS next to the app binary (Bundle's
    /// "auxiliary executable" spot); a few fallbacks keep dev builds working.
    static func locateHelper(bundle: Bundle = .main) -> URL? {
        let fm = FileManager.default
        if let override = ProcessInfo.processInfo.environment["MACN_PLAY_HELPER"], fm.isExecutableFile(atPath: override) {
            return URL(fileURLWithPath: override)
        }
        var candidates: [URL] = []
        if let aux = bundle.url(forAuxiliaryExecutable: "macn-play") { candidates.append(aux) }
        let root = bundle.bundleURL
        candidates += [
            root.appendingPathComponent("Contents/MacOS/macn-play"),
            root.appendingPathComponent("Contents/Helpers/macn-play"),
            root.appendingPathComponent("Contents/Resources/macn-play"),
            root.appendingPathComponent("macn-play"),
        ]
        return candidates.first { fm.isExecutableFile(atPath: $0.path) }
    }

    // MARK: Commands

    func search(_ query: String, limit: Int = 25) async throws -> [PlayApp] {
        let events = try await run(["search", query, "--limit", String(limit)])
        guard let e = events.last(where: { $0.name == "search" }) else { return [] }
        return e.decode([PlayApp].self, key: "results") ?? []
    }

    func details(_ packageOrLink: String) async throws -> PlayApp {
        let events = try await run(["details", packageOrLink])
        guard let app = events.last(where: { $0.name == "details" })?.decode(PlayApp.self, key: "app") else {
            throw PlayHelperError(kind: "other", message: "Google Play didn't send app details.")
        }
        return app
    }

    func download(_ package: String, to folder: URL, versionCode: Int64? = nil, includeSplits: Bool = true,
                  includeOBB: Bool = false, format: PlayPackageFormat = .auto,
                  progress: @escaping @Sendable (PlayProgress) -> Void) async throws -> PlayDownloadResult {
        var args = ["download", package, "--out", folder.path, "--format", format.rawValue]
        if let versionCode { args += ["--version-code", String(versionCode)] }
        if !includeSplits { args.append("--no-splits") }
        if includeOBB { args.append("--obb") }
        let state = ProgressState()
        let events = try await run(args) { event in
            switch event.name {
            case "status":
                progress(state.update { $0.stage = event.string("stage") ?? ""; $0.message = event.string("message") ?? "" })
            case "progress":
                progress(state.update { $0.done = event.int("done") ?? 0; $0.total = event.int("total") ?? 0 })
            default: break
            }
        }
        guard let done = events.last(where: { $0.name == "done" }), let path = done.string("path") else {
            throw PlayHelperError(kind: "other", message: "The download ended without a file.")
        }
        return PlayDownloadResult(path: path, format: done.string("format") ?? format.rawValue, size: done.int("size") ?? 0,
                                  package: done.string("package") ?? package, title: done.string("title") ?? package,
                                  versionName: done.string("versionName") ?? "", splitCount: Int(done.int("splitCount") ?? 0))
    }

    func cancelAll() {
        lock.lock(); let pids = running; lock.unlock()
        for pid in pids { kill(pid, SIGTERM) }
    }

    // MARK: Process plumbing

    private final class ProgressState: @unchecked Sendable {
        private let lock = NSLock()
        private var value = PlayProgress()
        func update(_ change: (inout PlayProgress) -> Void) -> PlayProgress {
            lock.lock(); defer { lock.unlock() }
            change(&value)
            return value
        }
    }

    /// Runs the helper to completion, delivering each event as it arrives.
    /// Stores any session the helper dispensed; throws the helper's error event.
    @discardableResult
    func run(_ arguments: [String], onEvent: (@Sendable (PlayEvent) -> Void)? = nil) async throws -> [PlayEvent] {
        guard let executable else { throw PlayHelperError(kind: "missingHelper", message: "macn-play not found") }
        var env = ProcessInfo.processInfo.environment
        env["MACN_PLAY_EMAIL"] = nil
        env["MACN_PLAY_AUTH_TOKEN"] = nil
        if let s = sessions.load() {
            env["MACN_PLAY_EMAIL"] = s.email
            env["MACN_PLAY_AUTH_TOKEN"] = s.token
        }
        let sessions = self.sessions
        let environment = env
        let pidBox = PidBox()
        return try await withTaskCancellationHandler {
            try await withCheckedThrowingContinuation { (cont: CheckedContinuation<[PlayEvent], Error>) in
                DispatchQueue.global(qos: .userInitiated).async {
                    do {
                        let (events, status) = try Self.spawnAndCollect(executable: executable, arguments: arguments, environment: environment,
                                                                         started: { pid in pidBox.set(pid); self.track(pid, true) },
                                                                         onEvent: { event in
                            if event.name == "session", let e = event.string("email"), let t = event.string("authToken") {
                                sessions.save(email: e, token: t)
                            }
                            onEvent?(event)
                        })
                        if let pid = pidBox.get() { self.track(pid, false) }
                        if let err = events.last(where: { $0.name == "error" }) {
                            if err.string("kind") == "auth" { sessions.clear() }
                            throw PlayHelperError(kind: err.string("kind") ?? "other", message: err.string("message") ?? "Unknown error")
                        }
                        if pidBox.cancelled { throw PlayHelperError(kind: "cancelled", message: "Cancelled") }
                        guard status == 0 else {
                            throw PlayHelperError(kind: "other", message: "macn-play exited with status \(status)")
                        }
                        cont.resume(returning: events)
                    } catch {
                        cont.resume(throwing: error)
                    }
                }
            }
        } onCancel: {
            pidBox.cancel()
        }
    }

    private func track(_ pid: pid_t, _ add: Bool) {
        lock.lock(); defer { lock.unlock() }
        if add { running.insert(pid) } else { running.remove(pid) }
    }

    private final class PidBox: @unchecked Sendable {
        private let lock = NSLock()
        private var pid: pid_t?
        private(set) var cancelled = false
        func set(_ p: pid_t) {
            lock.lock(); pid = p; let c = cancelled; lock.unlock()
            if c { kill(p, SIGTERM) }
        }
        func get() -> pid_t? { lock.lock(); defer { lock.unlock() }; return pid }
        func cancel() {
            lock.lock(); cancelled = true; let p = pid; lock.unlock()
            if let p { kill(p, SIGTERM) }
        }
    }

    /// posix_spawn (Foundation's Process isn't available to Mac Catalyst apps).
    static func spawnAndCollect(executable: URL, arguments: [String], environment: [String: String],
                                started: (pid_t) -> Void, onEvent: (PlayEvent) -> Void) throws -> ([PlayEvent], Int32) {
        var fds: [Int32] = [0, 0]
        guard pipe(&fds) == 0 else { throw PlayHelperError(kind: "other", message: "pipe() failed: \(errno)") }
        let readEnd = fds[0], writeEnd = fds[1]

        #if canImport(Darwin)
        var actions: posix_spawn_file_actions_t?
        #else
        var actions = posix_spawn_file_actions_t()
        #endif
        posix_spawn_file_actions_init(&actions)
        defer { posix_spawn_file_actions_destroy(&actions) }
        posix_spawn_file_actions_adddup2(&actions, writeEnd, STDOUT_FILENO)
        posix_spawn_file_actions_addopen(&actions, STDERR_FILENO, "/dev/null", O_WRONLY, 0)
        posix_spawn_file_actions_addopen(&actions, STDIN_FILENO, "/dev/null", O_RDONLY, 0)
        posix_spawn_file_actions_addclose(&actions, readEnd)

        let argv: [UnsafeMutablePointer<CChar>?] = ([executable.path] + arguments).map { strdup($0) } + [nil]
        let envp: [UnsafeMutablePointer<CChar>?] = environment.map { strdup("\($0.key)=\($0.value)") } + [nil]
        defer {
            argv.forEach { free($0) }
            envp.forEach { free($0) }
        }

        var pid: pid_t = 0
        let rc = argv.withUnsafeBufferPointer { a in
            envp.withUnsafeBufferPointer { e in
                posix_spawn(&pid, executable.path, &actions, nil, a.baseAddress!, e.baseAddress!)
            }
        }
        close(writeEnd)
        guard rc == 0 else {
            close(readEnd)
            throw PlayHelperError(kind: "missingHelper", message: "couldn't start macn-play (\(rc))")
        }
        started(pid)

        var events: [PlayEvent] = []
        var lines = LineBuffer()
        var buffer = [UInt8](repeating: 0, count: 64 * 1024)
        while true {
            let n = buffer.withUnsafeMutableBytes { read(readEnd, $0.baseAddress, $0.count) }
            if n < 0 && errno == EINTR { continue }
            if n <= 0 { break }
            for line in lines.append(Data(buffer[0..<n])) {
                if let e = PlayEvent.parse(line) { events.append(e); onEvent(e) }
            }
        }
        for line in lines.finish() {
            if let e = PlayEvent.parse(line) { events.append(e); onEvent(e) }
        }
        close(readEnd)

        var status: Int32 = 0
        while waitpid(pid, &status, 0) < 0 && errno == EINTR {}
        // WEXITSTATUS / WIFEXITED are macros; decode by hand.
        let exited = (status & 0x7f) == 0
        let code: Int32 = exited ? (status >> 8) & 0xff : 128 + (status & 0x7f)
        return (events, code)
    }
}

// MARK: - Input helpers

enum PlayQuery {
    /// A package id from `com.example.app`, any Play Store link, or `market://details?id=`.
    static func packageID(from input: String) -> String? {
        let trimmed = input.trimmingCharacters(in: .whitespacesAndNewlines)
        var candidate = trimmed
        if let range = trimmed.range(of: "id=") {
            candidate = String(trimmed[range.upperBound...].prefix { $0 != "&" && $0 != "#" && $0 != "/" })
        }
        let parts = candidate.split(separator: ".", omittingEmptySubsequences: false)
        guard parts.count >= 2, let first = parts.first?.first, first.isLetter else { return nil }
        let allowed = CharacterSet.alphanumerics.union(CharacterSet(charactersIn: "_"))
        for p in parts {
            if p.isEmpty || p.unicodeScalars.contains(where: { !allowed.contains($0) || !$0.isASCII }) { return nil }
        }
        return candidate
    }
}
