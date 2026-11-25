// MACN-cheese: Settings › Downloads › "Mac" section (save folder for .ipa/.pkg files).
import SwiftUI
import UniformTypeIdentifiers

struct MacSaveFolderSection: View {
    @State private var folder = MacPort.exportFolder
    @State private var autoSave = MacPort.autoSave
    @State private var picking = false

    var body: some View {
        Section {
            LabeledContent("Save Folder") {
                Text(folder.path.replacingOccurrences(of: NSHomeDirectory(), with: "~"))
                    .lineLimit(1).truncationMode(.middle).foregroundStyle(.secondary)
            }
            Toggle("Copy finished downloads to this folder", isOn: $autoSave)
                .onChange(of: autoSave) { _, on in MacPort.autoSave = on }
            HStack {
                Button("Change Folder…") { picking = true }
                Button("Show in Finder") { MacPort.openFolder() }
                if folder != MacPort.defaultFolder {
                    Button("Use Default") { MacPort.exportFolder = MacPort.defaultFolder; folder = MacPort.defaultFolder }
                }
            }
            .buttonStyle(.bordered)
        } header: {
            Text(verbatim: "Mac")
        } footer: {
            Text(verbatim: "MACN-cheese doesn't install apps. Downloaded .ipa and .pkg files are saved here; use \"Show in Finder\" on a download to find it.")
        }
        .fileImporter(isPresented: $picking, allowedContentTypes: [.folder]) { result in
            if case .success(let url) = result {
                _ = url.startAccessingSecurityScopedResource()
                MacPort.exportFolder = url; folder = url
            }
        }
    }
}
