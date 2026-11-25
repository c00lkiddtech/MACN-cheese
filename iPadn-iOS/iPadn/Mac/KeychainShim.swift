// MACN-cheese: Mac Catalyst always uses the data-protection keychain, which
// needs a provisioning-profile entitlement that an ad-hoc signed build can't
// have (SecItem* fails with -34018). These module-level functions shadow the
// Security ones for generic passwords and store them in the user's login
// keychain through the classic file-keychain API instead (looked up at runtime
// because Catalyst's headers don't expose it). No entitlement needed.
#if targetEnvironment(macCatalyst)
import Foundation
import Security
import Darwin

private enum LegacyKeychain {
    typealias Find = @convention(c) (CFTypeRef?, UInt32, UnsafePointer<CChar>, UInt32, UnsafePointer<CChar>,
                                     UnsafeMutablePointer<UInt32>?, UnsafeMutablePointer<UnsafeMutableRawPointer?>?,
                                     UnsafeMutablePointer<Unmanaged<CFTypeRef>?>?) -> OSStatus
    typealias Add = @convention(c) (CFTypeRef?, UInt32, UnsafePointer<CChar>, UInt32, UnsafePointer<CChar>,
                                    UInt32, UnsafeRawPointer, UnsafeMutablePointer<Unmanaged<CFTypeRef>?>?) -> OSStatus
    typealias Modify = @convention(c) (CFTypeRef, UnsafeRawPointer?, UInt32, UnsafeRawPointer) -> OSStatus
    typealias Free = @convention(c) (UnsafeMutableRawPointer?, UnsafeMutableRawPointer?) -> OSStatus
    typealias Delete = @convention(c) (CFTypeRef) -> OSStatus

    static let handle = dlopen("/System/Library/Frameworks/Security.framework/Security", RTLD_NOW)
    static func sym<T>(_ name: String, _: T.Type) -> T? {
        guard let p = dlsym(handle, name) else { return nil }
        return unsafeBitCast(p, to: T.self)
    }
    static let find = sym("SecKeychainFindGenericPassword", Find.self)
    static let add = sym("SecKeychainAddGenericPassword", Add.self)
    static let modify = sym("SecKeychainItemModifyAttributesAndData", Modify.self)
    static let free = sym("SecKeychainItemFreeContent", Free.self)
    static let delete = sym("SecKeychainItemDelete", Delete.self)
    static var available: Bool { find != nil && add != nil && modify != nil && free != nil && delete != nil }

    /// Returns the item (retained) and/or its data.
    static func lookup(_ service: String, _ account: String, wantData: Bool) -> (OSStatus, CFTypeRef?, Data?) {
        var length: UInt32 = 0
        var bytes: UnsafeMutableRawPointer?
        var item: Unmanaged<CFTypeRef>?
        let s = Array(service.utf8CString), a = Array(account.utf8CString)
        let status: OSStatus = withUnsafeMutablePointer(to: &length) { lp in
            withUnsafeMutablePointer(to: &bytes) { bp in
                find!(nil, UInt32(s.count - 1), s, UInt32(a.count - 1), a,
                      wantData ? lp : nil, wantData ? bp : nil, &item)
            }
        }
        var data: Data?
        if status == errSecSuccess, let bytes { data = Data(bytes: bytes, count: Int(length)); _ = free!(nil, bytes) }
        return (status, item?.takeRetainedValue(), data)
    }
}

private func genericKey(_ query: CFDictionary) -> (service: String, account: String)? {
    let q = query as NSDictionary
    guard (q[kSecClass as String] as? String) == (kSecClassGenericPassword as String), LegacyKeychain.available,
          let service = q[kSecAttrService as String] as? String else { return nil }
    return (service, (q[kSecAttrAccount as String] as? String) ?? "")
}

func SecItemCopyMatching(_ query: CFDictionary, _ result: UnsafeMutablePointer<CFTypeRef?>?) -> OSStatus {
    guard let key = genericKey(query) else { return Security.SecItemCopyMatching(query, result) }
    let wantData = ((query as NSDictionary)[kSecReturnData as String] as? Bool) == true
    let (status, _, data) = LegacyKeychain.lookup(key.service, key.account, wantData: wantData)
    if status == errSecSuccess, wantData { result?.pointee = data as CFData? }
    return status
}

func SecItemAdd(_ attributes: CFDictionary, _ result: UnsafeMutablePointer<CFTypeRef?>?) -> OSStatus {
    guard let key = genericKey(attributes) else { return Security.SecItemAdd(attributes, result) }
    let data = ((attributes as NSDictionary)[kSecValueData as String] as? Data) ?? Data()
    let s = Array(key.service.utf8CString), a = Array(key.account.utf8CString)
    return data.withUnsafeBytes { raw in
        LegacyKeychain.add!(nil, UInt32(s.count - 1), s, UInt32(a.count - 1), a, UInt32(raw.count),
                            raw.baseAddress ?? UnsafeRawPointer(bitPattern: 1)!, nil)
    }
}

func SecItemUpdate(_ query: CFDictionary, _ attributesToUpdate: CFDictionary) -> OSStatus {
    guard let key = genericKey(query) else { return Security.SecItemUpdate(query, attributesToUpdate) }
    let (status, item, _) = LegacyKeychain.lookup(key.service, key.account, wantData: false)
    guard status == errSecSuccess, let item else { return status }
    guard let data = (attributesToUpdate as NSDictionary)[kSecValueData as String] as? Data else { return errSecSuccess }
    return data.withUnsafeBytes { raw in
        LegacyKeychain.modify!(item, nil, UInt32(raw.count), raw.baseAddress ?? UnsafeRawPointer(bitPattern: 1)!)
    }
}

@discardableResult
func SecItemDelete(_ query: CFDictionary) -> OSStatus {
    guard let key = genericKey(query) else { return Security.SecItemDelete(query) }
    let (status, item, _) = LegacyKeychain.lookup(key.service, key.account, wantData: false)
    guard status == errSecSuccess, let item else { return status }
    return LegacyKeychain.delete!(item)
}
#endif
