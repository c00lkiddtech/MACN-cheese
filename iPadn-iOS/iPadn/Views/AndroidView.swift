// MACN-cheese: Android tab. Downloads free Google Play apps (base + split APKs)
// through the bundled `macn-play` helper (see Services/PlayHelperClient.swift).
// Anonymous Aurora OSS sessions only; no personal Google account is used.
import SwiftUI
import UniformTypeIdentifiers

@MainActor
final class AndroidModel: ObservableObject {
    static let shared = AndroidModel()
    let client = PlayHelperClient(sessions: InMemoryPlaySessionStore())

    @Published var query = ""
    @Published var app: PlayApp?
    @Published var results: [PlayApp] = []
    @Published var busy = false
    @Published var downloading = false
    @Published var progress = PlayProgress()
    @Published var format: PlayPackageFormat = .auto
    @Published var includeSplits = true
    @Published var lastFile: PlayDownloadResult?
    @Published var error: String?
    private var task: Task<Void, Never>?

    var helperMissing: Bool { client.executable == nil }

    func lookUp() {
        let text = query.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !text.isEmpty else { return }
        error = nil; busy = true; app = nil; results = []
        task = Task {
            defer { busy = false }
            do {
                if let pkg = PlayQuery.packageID(from: text) {
                    app = try await client.details(pkg)
                } else {
                    results = try await client.search(text)
                    if results.isEmpty { error = "No Google Play apps matched “\(text)”." }
                }
            } catch { self.error = error.localizedDescription }
        }
    }

    func select(_ item: PlayApp) {
        query = item.package
        lookUp()
    }

    func download() {
        guard let app else { return }
        error = nil; downloading = true; progress = PlayProgress(); lastFile = nil
        let folder = MacPort.exportFolder
        try? FileManager.default.createDirectory(at: folder, withIntermediateDirectories: true)
        task = Task {
            defer { downloading = false }
            do {
                let result = try await client.download(app.package, to: folder, includeSplits: includeSplits, format: format) { p in
                    Task { @MainActor in AndroidModel.shared.progress = p }
                }
                lastFile = result
            } catch { self.error = error.localizedDescription }
        }
    }

    func convert(_ file: URL) {
        error = nil; downloading = true; progress = PlayProgress(stage: "merge", message: "Merging split APKs into one .apk…"); lastFile = nil
        task = Task {
            defer { downloading = false }
            let scoped = file.startAccessingSecurityScopedResource()
            defer { if scoped { file.stopAccessingSecurityScopedResource() } }
            do { lastFile = try await client.merge(file) } catch { self.error = error.localizedDescription }
        }
    }

    func cancel() {
        client.cancelAll()
        task?.cancel()
    }
}

struct AndroidView: View {
    @ObservedObject private var model = AndroidModel.shared
    @State private var picking = false

    var body: some View {
        Form {
            Section {
                HStack {
                    TextField(text: $model.query, prompt: Text(verbatim: "com.example.app, a Play Store link, or a search")) { Text(verbatim: "Package") }
                        .textInputAutocapitalization(.never).autocorrectionDisabled()
                        .onSubmit { model.lookUp() }
                    Button { model.lookUp() } label: { Text(verbatim: "Look Up") }
                        .buttonStyle(.borderedProminent)
                        .disabled(model.busy || model.downloading || model.query.trimmingCharacters(in: .whitespaces).isEmpty)
                }
                if model.busy { ProgressView() }
                Button { picking = true } label: {
                    Label { Text(verbatim: "Convert .apks / .xapk to .apk…") } icon: { Image(systemName: "arrow.triangle.merge") }
                }
                .disabled(model.downloading || model.helperMissing)
            } header: { Text(verbatim: "Google Play") } footer: {
                Text(verbatim: model.helperMissing
                     ? "The Google Play helper is missing from this build."
                     : "Free apps only. Uses an anonymous Google Play session, never your Google account.")
            }

            if let err = model.error {
                Section { Label { Text(verbatim: err) } icon: { Image(systemName: "exclamationmark.triangle.fill").foregroundStyle(.orange) } }
            }

            if !model.results.isEmpty {
                Section { ForEach(model.results) { item in
                    Button { model.select(item) } label: { row(item) }.buttonStyle(.plain)
                } } header: { Text(verbatim: "Results") }
            }

            if let app = model.app {
                Section {
                    row(app)
                    if let v = app.versionName { LabeledContent { Text(verbatim: v) } label: { Text(verbatim: "Version") } }
                    if let s = app.sizeLabel { LabeledContent { Text(verbatim: s) } label: { Text(verbatim: "Size") } }
                    if let d = app.downloads { LabeledContent { Text(verbatim: d) } label: { Text(verbatim: "Installs") } }
                    if let u = app.updated { LabeledContent { Text(verbatim: u) } label: { Text(verbatim: "Updated") } }
                    if let splits = app.splits, !splits.isEmpty {
                        LabeledContent { Text(verbatim: "\(splits.count)") } label: { Text(verbatim: "Split APKs") }
                    }
                    if let desc = app.description, !desc.isEmpty {
                        Text(verbatim: desc).font(.callout).foregroundStyle(.secondary).lineLimit(4)
                    }
                } header: { Text(verbatim: "App") }

                Section {
                    Picker(selection: $model.format) {
                        ForEach(PlayPackageFormat.allCases) { Text(verbatim: $0.title).tag($0) }
                    } label: { Text(verbatim: "Format") }
                    Toggle(isOn: $model.includeSplits) { Text(verbatim: "Include split APKs") }
                    if model.downloading {
                        VStack(alignment: .leading, spacing: 6) {
                            if let f = model.progress.fraction { ProgressView(value: f) } else { ProgressView() }
                            Text(verbatim: progressText).font(.caption).foregroundStyle(.secondary)
                        }
                        Button(role: .cancel) { model.cancel() } label: { Text(verbatim: "Cancel") }
                    } else {
                        Button { model.download() } label: {
                            Label { Text(verbatim: "Download") } icon: { Image(systemName: "arrow.down.circle.fill") }
                        }
                        .buttonStyle(.borderedProminent)
                        .disabled(app.isPaid || model.helperMissing)
                    }
                } header: { Text(verbatim: "Download") } footer: {
                    Text(verbatim: model.format.footnote + " Saves to " + MacPort.exportFolder.path.replacingOccurrences(of: NSHomeDirectory(), with: "~") + ".")
                }
            }

            if let file = model.lastFile {
                Section {
                    Label { Text(verbatim: file.url.lastPathComponent) } icon: { Image(systemName: "checkmark.circle.fill").foregroundStyle(.green) }
                    Text(verbatim: ByteCountFormatter.string(fromByteCount: file.size, countStyle: .file) + (file.format == "apk" ? " · single installable .apk" : " · \(file.splitCount) split APK(s)"))
                        .font(.caption).foregroundStyle(.secondary)
                    Button { MacPort.reveal(file.url) } label: {
                        Label { Text(verbatim: "Show in Finder") } icon: { Image(systemName: "folder") }
                    }
                } header: { Text(verbatim: "Downloaded") }
            }
        }
        .navigationTitle(Text(verbatim: "Android"))
        .fileImporter(isPresented: $picking, allowedContentTypes: [.data, .zip, .folder]) { result in
            if case .success(let url) = result { model.convert(url) }
        }
    }

    private var progressText: String {
        let p = model.progress
        if p.total > 0 {
            return "\(p.message.isEmpty ? "Downloading…" : p.message) \(ByteCountFormatter.string(fromByteCount: p.done, countStyle: .file)) of \(ByteCountFormatter.string(fromByteCount: p.total, countStyle: .file))"
        }
        return p.message.isEmpty ? "Starting…" : p.message
    }

    @ViewBuilder private func row(_ app: PlayApp) -> some View {
        HStack(spacing: 12) {
            AsyncImage(url: app.iconURL) { $0.resizable().scaledToFit() } placeholder: { Color.secondary.opacity(0.15) }
                .frame(width: 48, height: 48).clipShape(RoundedRectangle(cornerRadius: 11, style: .continuous))
            VStack(alignment: .leading, spacing: 2) {
                Text(verbatim: app.displayTitle).font(.headline)
                Text(verbatim: [app.developer, app.package].compactMap { $0 }.joined(separator: " · ")).font(.caption).foregroundStyle(.secondary)
                if app.isPaid { Text(verbatim: "Paid app: not supported").font(.caption2).foregroundStyle(.orange) }
            }
            Spacer()
            if let r = app.ratingLabel { Text(verbatim: r).font(.caption).foregroundStyle(.secondary) }
        }
    }
}
