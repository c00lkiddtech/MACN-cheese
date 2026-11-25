// MACN-cheese: helpers for the macOS (Mac Catalyst) build.
// On the Mac nothing gets installed: finished downloads are copied into a
// folder the user picks (Settings › Mac › Save Folder), default
// ~/Desktop, and can be revealed in Finder.
import Foundation
import UIKit

enum MacPort {
    #if targetEnvironment(macCatalyst)
    static let appName = "MACN-cheese"
    static let isMac = true
    #else
    static let appName = "iPadn"
    static let isMac = false
    #endif

    static let noInstallMessage = "MACN-cheese doesn't install apps. The file was saved to your Desktop instead."
    private static let folderKey = "macnExportFolder"
    private static let autoSaveKey = "macnAutoSave"

    static var defaultFolder: URL {
        // Finished .ipa / .apk files go straight to the Desktop by default.
        FileManager.default.urls(for: .desktopDirectory, in: .userDomainMask).first
            ?? URL(fileURLWithPath: NSHomeDirectory()).appendingPathComponent("Desktop", isDirectory: true)
    }

    static var exportFolder: URL {
        get {
            if let path = UserDefaults.standard.string(forKey: folderKey), !path.isEmpty {
                return URL(fileURLWithPath: path, isDirectory: true)
            }
            return defaultFolder
        }
        set { UserDefaults.standard.set(newValue.path, forKey: folderKey) }
    }

    /// Copy finished downloads to the save folder automatically (on by default).
    static var autoSave: Bool {
        get { UserDefaults.standard.object(forKey: autoSaveKey) as? Bool ?? true }
        set { UserDefaults.standard.set(newValue, forKey: autoSaveKey) }
    }

    /// Copies `file` into the save folder as `name` (keeping the extension),
    /// never overwriting another file. Returns the saved copy.
    @discardableResult
    static func save(_ file: URL, as name: String? = nil) throws -> URL {
        let fm = FileManager.default
        let folder = exportFolder
        try fm.createDirectory(at: folder, withIntermediateDirectories: true)
        let ext = file.pathExtension
        let base = (name?.isEmpty == false ? name! : file.deletingPathExtension().lastPathComponent)
        var target = folder.appendingPathComponent(base).appendingPathExtension(ext)
        if fm.fileExists(atPath: target.path), sameSize(target, file) { return target }
        var n = 2
        while fm.fileExists(atPath: target.path) {
            target = folder.appendingPathComponent("\(base) (\(n))").appendingPathExtension(ext); n += 1
        }
        try fm.copyItem(at: file, to: target)   // APFS clone: instant, no extra space
        return target
    }

    private static func sameSize(_ a: URL, _ b: URL) -> Bool {
        let fm = FileManager.default
        guard let x = try? fm.attributesOfItem(atPath: a.path)[.size] as? NSNumber,
              let y = try? fm.attributesOfItem(atPath: b.path)[.size] as? NSNumber else { return false }
        return x == y
    }

    /// Finder window with the file selected (AppKit's NSWorkspace, reached
    /// through the Objective-C runtime because Catalyst doesn't expose it).
    static func reveal(_ url: URL) {
        if let cls = NSClassFromString("NSWorkspace") as? NSObject.Type,
           let ws = cls.value(forKey: "sharedWorkspace") as? NSObject {
            let sel = NSSelectorFromString("activateFileViewerSelectingURLs:")
            if ws.responds(to: sel) { ws.perform(sel, with: [url]); return }
        }
        UIApplication.shared.open(url.deletingLastPathComponent())
    }

    static func openFolder() {
        try? FileManager.default.createDirectory(at: exportFolder, withIntermediateDirectories: true)
        UIApplication.shared.open(exportFolder)
    }

    /// Saves a finished download (named "App Version.ipa") and shows it in Finder.
    @MainActor
    static func saveAndReveal(_ job: DownloadJob, manager: DownloadManager) throws {
        guard let file = manager.fileURL(job) else { return }
        let named = manager.exportURL(job) ?? file
        reveal(try save(file, as: named.deletingPathExtension().lastPathComponent))
    }

    @MainActor
    static func autoSaveFinished(_ job: DownloadJob, manager: DownloadManager) {
        guard isMac, autoSave, let file = manager.fileURL(job) else { return }
        let named = manager.exportURL(job) ?? file
        _ = try? save(file, as: named.deletingPathExtension().lastPathComponent)
    }
}
