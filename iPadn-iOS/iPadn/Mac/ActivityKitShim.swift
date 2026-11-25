// MACN-cheese: ActivityKit (Live Activities) is unavailable in Mac Catalyst.
// These no-op stand-ins shadow the ActivityKit names so shared code compiles;
// nothing is ever shown and `request` always fails.
#if targetEnvironment(macCatalyst)
import Foundation
import SwiftUI

protocol ActivityAttributes {
    associatedtype ContentState: Codable & Hashable
}

struct ActivityContent<State> {
    var state: State
    var staleDate: Date?
    init(state: State, staleDate: Date?, relevanceScore: Double = 0) {
        self.state = state; self.staleDate = staleDate
    }
}

enum ActivityState { case active, ended, dismissed, stale, pending }

enum ActivityUIDismissalPolicy {
    case immediate, `default`
    case after(Date)
}

enum PushType { case token }

struct AlertConfiguration {
    enum AlertSound { case `default`; static func named(_ name: String) -> AlertSound { .default } }
    init(title: LocalizedStringResource, body: LocalizedStringResource, sound: AlertSound) {}
}

struct ActivityAuthorizationInfo {
    var areActivitiesEnabled: Bool { false }
}

struct ActivityUnavailableOnMac: Error {}

final class Activity<Attributes: ActivityAttributes>: @unchecked Sendable {
    let id = UUID().uuidString
    var activityState: ActivityState { .ended }
    let content: ActivityContent<Attributes.ContentState>
    private init(content: ActivityContent<Attributes.ContentState>) { self.content = content }
    static var activities: [Activity<Attributes>] { [] }
    static func request(attributes: Attributes, content: ActivityContent<Attributes.ContentState>,
                        pushType: PushType? = nil) throws -> Activity<Attributes> {
        throw ActivityUnavailableOnMac()
    }
    func update(_ content: ActivityContent<Attributes.ContentState>, alertConfiguration: AlertConfiguration? = nil) async {}
    func end(_ content: ActivityContent<Attributes.ContentState>?, dismissalPolicy: ActivityUIDismissalPolicy = .default) async {}
}
#endif
