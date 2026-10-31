import Foundation
import Photos
import PhotosUI
import SwiftRs
import Tauri
import UIKit
import UniformTypeIdentifiers
import WebKit

struct PickArgs: Decodable {
  let dir: String
  var multiple: Bool?
  var images: Bool?
}

struct DirArgs: Decodable {
  let dir: String
}

struct ExportArgs: Decodable {
  let source: String
  let name: String
  var mime: String?
}

struct SaveDirArgs: Decodable {
  var persist: Bool?
}

struct PathArgs: Decodable {
  let path: String
}

struct PathsArgs: Decodable {
  let paths: [String]
}

struct ServiceArgs: Decodable {
  let enabled: Bool
  var title: String?
  var text: String?
}

struct PublishArgs: Decodable {
  let name: String
  let port: Int
  let txt: [String: String]
}

struct PathsResult: Encodable {
  var paths: [String] = []
  var cancelled: Bool = false
}

struct PathResult: Encodable {
  var path: String? = nil
  var cancelled: Bool = false
}

struct SavedResult: Encodable {
  var saved: Bool = false
  var cancelled: Bool = false
}

struct DeviceInfoResult: Encodable {
  let name: String
  let downloadDir: String
}

struct BonjourServiceInfo: Encodable {
  let fullname: String
  let addresses: [String]
  let port: Int
  let txt: [String: String]
}

struct BonjourSnapshot: Encodable {
  let services: [BonjourServiceInfo]
}

final class DocumentPickerDelegate: NSObject, UIDocumentPickerDelegate {
  private let completion: ([URL]?) -> Void

  init(_ completion: @escaping ([URL]?) -> Void) {
    self.completion = completion
  }

  func documentPicker(_ controller: UIDocumentPickerViewController, didPickDocumentsAt urls: [URL]) {
    completion(urls)
  }

  func documentPickerWasCancelled(_ controller: UIDocumentPickerViewController) {
    completion(nil)
  }
}

@available(iOS 14.0, *)
final class MediaPickerDelegate: NSObject, PHPickerViewControllerDelegate {
  private let completion: ([PHPickerResult]) -> Void

  init(_ completion: @escaping ([PHPickerResult]) -> Void) {
    self.completion = completion
  }

  func picker(_ picker: PHPickerViewController, didFinishPicking results: [PHPickerResult]) {
    picker.dismiss(animated: true)
    completion(results)
  }
}

final class PreviewDelegate: NSObject, UIDocumentInteractionControllerDelegate {
  weak var host: UIViewController?

  init(_ host: UIViewController?) {
    self.host = host
  }

  func documentInteractionControllerViewControllerForPreview(
    _ controller: UIDocumentInteractionController
  ) -> UIViewController {
    return host ?? UIViewController()
  }
}

final class Bonjour: NSObject, NetServiceDelegate, NetServiceBrowserDelegate {
  static let serviceType = "_zwormhole._tcp."
  static let domain = "local."

  private var published: NetService?
  private var browser: NetServiceBrowser?
  private var found: [String: NetService] = [:]
  private var resolved: [String: BonjourServiceInfo] = [:]
  private let lock = NSLock()

  private func key(_ service: NetService) -> String {
    return "\(service.name).\(service.type)\(service.domain)"
  }

  func publish(name: String, port: Int, txt: [String: String]) {
    DispatchQueue.main.async {
      self.published?.stop()
      let service = NetService(
        domain: Bonjour.domain, type: Bonjour.serviceType, name: name, port: Int32(port))
      var record: [String: Data] = [:]
      for (k, v) in txt {
        record[k] = Data(v.utf8)
      }
      service.setTXTRecord(NetService.data(fromTXTRecord: record))
      service.delegate = self
      service.publish()
      self.published = service
    }
  }

  func unpublish() {
    DispatchQueue.main.async {
      self.published?.stop()
      self.published = nil
    }
  }

  func browse() {
    DispatchQueue.main.async {
      if self.browser != nil {
        return
      }
      let browser = NetServiceBrowser()
      browser.delegate = self
      browser.searchForServices(ofType: Bonjour.serviceType, inDomain: Bonjour.domain)
      self.browser = browser
    }
  }

  func stopBrowsing() {
    DispatchQueue.main.async {
      self.browser?.stop()
      self.browser = nil
      for service in self.found.values {
        service.stop()
        service.stopMonitoring()
      }
      self.found.removeAll()
      self.lock.lock()
      self.resolved.removeAll()
      self.lock.unlock()
    }
  }

  func snapshot() -> [BonjourServiceInfo] {
    lock.lock()
    defer { lock.unlock() }
    return Array(resolved.values)
  }

  func netServiceBrowser(
    _ browser: NetServiceBrowser, didFind service: NetService, moreComing: Bool
  ) {
    found[key(service)] = service
    service.delegate = self
    service.resolve(withTimeout: 10)
    service.startMonitoring()
  }

  func netServiceBrowser(
    _ browser: NetServiceBrowser, didRemove service: NetService, moreComing: Bool
  ) {
    let k = key(service)
    found[k]?.stop()
    found[k]?.stopMonitoring()
    found.removeValue(forKey: k)
    lock.lock()
    resolved.removeValue(forKey: k)
    lock.unlock()
  }

  func netServiceDidResolveAddress(_ sender: NetService) {
    update(sender)
  }

  func netService(_ sender: NetService, didUpdateTXTRecord data: Data) {
    update(sender)
  }

  private func update(_ service: NetService) {
    guard found[key(service)] != nil else { return }
    var txt: [String: String] = [:]
    if let data = service.txtRecordData() {
      for (k, v) in NetService.dictionary(fromTXTRecord: data) {
        txt[k] = String(data: v, encoding: .utf8) ?? ""
      }
    }
    let addresses = (service.addresses ?? []).compactMap { Bonjour.ipString($0) }
    if addresses.isEmpty || service.port <= 0 {
      return
    }
    let info = BonjourServiceInfo(
      fullname: key(service), addresses: addresses, port: service.port, txt: txt)
    lock.lock()
    resolved[key(service)] = info
    lock.unlock()
  }

  static func ipString(_ data: Data) -> String? {
    return data.withUnsafeBytes { (raw: UnsafeRawBufferPointer) -> String? in
      guard let base = raw.baseAddress else { return nil }
      let sa = base.assumingMemoryBound(to: sockaddr.self)
      var host = [CChar](repeating: 0, count: Int(NI_MAXHOST))
      let result = getnameinfo(
        sa, socklen_t(data.count), &host, socklen_t(host.count), nil, 0, NI_NUMERICHOST)
      return result == 0 ? String(cString: host) : nil
    }
  }
}

@available(iOS 14.0, *)
class ZWormHoleMobilePlugin: Plugin {
  private var documentDelegate: DocumentPickerDelegate?
  private var mediaDelegate: MediaPickerDelegate?
  private var previewDelegate: PreviewDelegate?
  private var interaction: UIDocumentInteractionController?
  private var saveDirURL: URL?
  private var transientURLs: [URL] = []
  private var backgroundTask: UIBackgroundTaskIdentifier = .invalid
  private let bonjour = Bonjour()
  private let saveDirKey = "zwormhole.saveDirBookmark"
  private var deviceName = "iPhone"

  override func load(webview: WKWebView) {
    super.load(webview: webview)
    deviceName = UIDevice.current.name
    _ = restoreSaveDirURL()
    NotificationCenter.default.addObserver(
      self, selector: #selector(didEnterBackground),
      name: UIApplication.didEnterBackgroundNotification, object: nil)
    NotificationCenter.default.addObserver(
      self, selector: #selector(willEnterForeground),
      name: UIApplication.willEnterForegroundNotification, object: nil)
  }

  @objc private func didEnterBackground() {
    endBackgroundTask()
    backgroundTask = UIApplication.shared.beginBackgroundTask(withName: "zwormhole-transfer") {
      [weak self] in
      self?.endBackgroundTask()
    }
  }

  @objc private func willEnterForeground() {
    endBackgroundTask()
  }

  private func endBackgroundTask() {
    if backgroundTask != .invalid {
      UIApplication.shared.endBackgroundTask(backgroundTask)
      backgroundTask = .invalid
    }
  }

  private func present(_ controller: UIViewController) {
    guard let host = manager.viewController else { return }
    var top = host
    while let next = top.presentedViewController {
      top = next
    }
    top.present(controller, animated: true, completion: nil)
  }

  private static func sanitize(_ name: String) -> String {
    let cleaned = name.replacingOccurrences(of: "/", with: "_").trimmingCharacters(
      in: .whitespacesAndNewlines)
    return cleaned.isEmpty || cleaned == "." || cleaned == ".." ? "file" : cleaned
  }

  private static func unique(_ dir: URL, _ name: String) -> URL {
    let fm = FileManager.default
    var candidate = dir.appendingPathComponent(name)
    if !fm.fileExists(atPath: candidate.path) {
      return candidate
    }
    let ext = (name as NSString).pathExtension
    let base = (name as NSString).deletingPathExtension
    var i = 2
    while fm.fileExists(atPath: candidate.path) {
      let next = ext.isEmpty ? "\(base) (\(i))" : "\(base) (\(i)).\(ext)"
      candidate = dir.appendingPathComponent(next)
      i += 1
    }
    return candidate
  }

  private static func importItem(_ url: URL, into dir: URL) throws -> URL {
    let fm = FileManager.default
    try fm.createDirectory(at: dir, withIntermediateDirectories: true)
    let target = unique(dir, sanitize(url.lastPathComponent))
    let scoped = url.startAccessingSecurityScopedResource()
    defer {
      if scoped { url.stopAccessingSecurityScopedResource() }
    }
    do {
      try fm.moveItem(at: url, to: target)
    } catch {
      try fm.copyItem(at: url, to: target)
    }
    return target
  }

  @objc public func pickFiles(_ invoke: Invoke) throws {
    let args = try invoke.parseArgs(PickArgs.self)
    let dir = URL(fileURLWithPath: args.dir, isDirectory: true)
    let multiple = args.multiple ?? true
    DispatchQueue.main.async {
      if args.images ?? false {
        self.presentMediaPicker(invoke, dir: dir, multiple: multiple, imagesOnly: true)
        return
      }
      let sheet = UIAlertController(title: nil, message: nil, preferredStyle: .actionSheet)
      sheet.addAction(
        UIAlertAction(title: "Photos & Videos", style: .default) { _ in
          self.presentMediaPicker(invoke, dir: dir, multiple: multiple, imagesOnly: false)
        })
      sheet.addAction(
        UIAlertAction(title: "Files", style: .default) { _ in
          self.presentDocumentPicker(invoke, dir: dir, multiple: multiple)
        })
      sheet.addAction(
        UIAlertAction(title: "Cancel", style: .cancel) { _ in
          invoke.resolve(PathsResult(cancelled: true))
        })
      UIUtils.centerPopover(rootViewController: self.manager.viewController, popoverController: sheet)
      self.present(sheet)
    }
  }

  private func presentDocumentPicker(_ invoke: Invoke, dir: URL, multiple: Bool) {
    let picker = UIDocumentPickerViewController(forOpeningContentTypes: [UTType.item], asCopy: true)
    picker.allowsMultipleSelection = multiple
    documentDelegate = DocumentPickerDelegate { urls in
      guard let urls = urls, !urls.isEmpty else {
        invoke.resolve(PathsResult(cancelled: true))
        return
      }
      DispatchQueue.global(qos: .userInitiated).async {
        do {
          let paths = try urls.map { try ZWormHoleMobilePlugin.importItem($0, into: dir).path }
          invoke.resolve(PathsResult(paths: paths))
        } catch {
          invoke.reject(error.localizedDescription)
        }
      }
    }
    picker.delegate = documentDelegate
    present(picker)
  }

  private func presentMediaPicker(_ invoke: Invoke, dir: URL, multiple: Bool, imagesOnly: Bool) {
    var configuration = PHPickerConfiguration(photoLibrary: PHPhotoLibrary.shared())
    configuration.selectionLimit = multiple ? 0 : 1
    configuration.preferredAssetRepresentationMode = imagesOnly ? .compatible : .current
    if imagesOnly {
      configuration.filter = .images
    }
    let picker = PHPickerViewController(configuration: configuration)
    mediaDelegate = MediaPickerDelegate { results in
      if results.isEmpty {
        invoke.resolve(PathsResult(cancelled: true))
        return
      }
      try? FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
      let group = DispatchGroup()
      let lock = NSLock()
      var paths: [String] = []
      var failure: String?
      for result in results {
        let provider = result.itemProvider
        let typeId: String
        if provider.hasItemConformingToTypeIdentifier(UTType.movie.identifier) {
          typeId = UTType.movie.identifier
        } else if let first = provider.registeredTypeIdentifiers.first {
          typeId = first
        } else {
          continue
        }
        group.enter()
        provider.loadFileRepresentation(forTypeIdentifier: typeId) { url, error in
          defer { group.leave() }
          guard let url = url else {
            lock.lock()
            failure = error?.localizedDescription ?? "Could not load the selected item"
            lock.unlock()
            return
          }
          var name = url.lastPathComponent
          if let suggested = provider.suggestedName, !suggested.isEmpty {
            name = url.pathExtension.isEmpty ? suggested : "\(suggested).\(url.pathExtension)"
          }
          lock.lock()
          defer { lock.unlock() }
          let target = ZWormHoleMobilePlugin.unique(dir, ZWormHoleMobilePlugin.sanitize(name))
          do {
            try FileManager.default.copyItem(at: url, to: target)
            paths.append(target.path)
          } catch {
            failure = error.localizedDescription
          }
        }
      }
      group.notify(queue: .global(qos: .userInitiated)) {
        if paths.isEmpty, let failure = failure {
          invoke.reject(failure)
        } else {
          invoke.resolve(PathsResult(paths: paths, cancelled: paths.isEmpty))
        }
      }
    }
    picker.delegate = mediaDelegate
    present(picker)
  }

  @objc public func pickFolder(_ invoke: Invoke) throws {
    let args = try invoke.parseArgs(DirArgs.self)
    let dir = URL(fileURLWithPath: args.dir, isDirectory: true)
    DispatchQueue.main.async {
      let picker = UIDocumentPickerViewController(forOpeningContentTypes: [UTType.folder])
      picker.allowsMultipleSelection = false
      self.documentDelegate = DocumentPickerDelegate { urls in
        guard let url = urls?.first else {
          invoke.resolve(PathResult(cancelled: true))
          return
        }
        DispatchQueue.global(qos: .userInitiated).async {
          let scoped = url.startAccessingSecurityScopedResource()
          defer {
            if scoped { url.stopAccessingSecurityScopedResource() }
          }
          do {
            try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
            let target = ZWormHoleMobilePlugin.unique(
              dir, ZWormHoleMobilePlugin.sanitize(url.lastPathComponent))
            let coordinator = NSFileCoordinator()
            var coordError: NSError?
            var copyError: Error?
            coordinator.coordinate(readingItemAt: url, options: [], error: &coordError) { readURL in
              do {
                try FileManager.default.copyItem(at: readURL, to: target)
              } catch {
                copyError = error
              }
            }
            if let e = coordError ?? copyError.map({ $0 as NSError }) {
              invoke.reject(e.localizedDescription)
            } else {
              invoke.resolve(PathResult(path: target.path))
            }
          } catch {
            invoke.reject(error.localizedDescription)
          }
        }
      }
      picker.delegate = self.documentDelegate
      self.present(picker)
    }
  }

  private func restoreSaveDirURL() -> URL? {
    guard let data = UserDefaults.standard.data(forKey: saveDirKey) else { return nil }
    var stale = false
    guard
      let url = try? URL(
        resolvingBookmarkData: data, options: [], relativeTo: nil, bookmarkDataIsStale: &stale)
    else {
      return nil
    }
    if url.startAccessingSecurityScopedResource() {
      saveDirURL?.stopAccessingSecurityScopedResource()
      saveDirURL = url
    }
    if stale, let fresh = try? url.bookmarkData(options: [], includingResourceValuesForKeys: nil, relativeTo: nil) {
      UserDefaults.standard.set(fresh, forKey: saveDirKey)
    }
    return url
  }

  @objc public func pickSaveDir(_ invoke: Invoke) throws {
    let persist = (try? invoke.parseArgs(SaveDirArgs.self))?.persist ?? true
    DispatchQueue.main.async {
      let picker = UIDocumentPickerViewController(forOpeningContentTypes: [UTType.folder])
      picker.allowsMultipleSelection = false
      self.documentDelegate = DocumentPickerDelegate { urls in
        guard let url = urls?.first else {
          invoke.resolve(PathResult(cancelled: true))
          return
        }
        guard url.startAccessingSecurityScopedResource() else {
          invoke.reject("ZWormHole wasn't granted access to that folder.")
          return
        }
        if !persist {
          self.transientURLs.append(url)
          invoke.resolve(PathResult(path: url.path))
          return
        }
        self.saveDirURL?.stopAccessingSecurityScopedResource()
        self.saveDirURL = url
        if let bookmark = try? url.bookmarkData(options: [], includingResourceValuesForKeys: nil, relativeTo: nil) {
          UserDefaults.standard.set(bookmark, forKey: self.saveDirKey)
        }
        invoke.resolve(PathResult(path: url.path))
      }
      picker.delegate = self.documentDelegate
      self.present(picker)
    }
  }

  @objc public func restoreSaveDir(_ invoke: Invoke) throws {
    if let url = saveDirURL ?? restoreSaveDirURL() {
      invoke.resolve(PathResult(path: url.path))
    } else {
      invoke.resolve(PathResult())
    }
  }

  @objc public func clearSaveDir(_ invoke: Invoke) throws {
    saveDirURL?.stopAccessingSecurityScopedResource()
    saveDirURL = nil
    UserDefaults.standard.removeObject(forKey: saveDirKey)
    invoke.resolve()
  }

  @objc public func exportFile(_ invoke: Invoke) throws {
    let args = try invoke.parseArgs(ExportArgs.self)
    let source = URL(fileURLWithPath: args.source)
    DispatchQueue.main.async {
      let picker = UIDocumentPickerViewController(forExporting: [source], asCopy: true)
      self.documentDelegate = DocumentPickerDelegate { urls in
        if urls == nil {
          invoke.resolve(SavedResult(cancelled: true))
        } else {
          invoke.resolve(SavedResult(saved: true))
        }
      }
      picker.delegate = self.documentDelegate
      self.present(picker)
    }
  }

  @objc public func clipboardImage(_ invoke: Invoke) throws {
    let args = try invoke.parseArgs(DirArgs.self)
    let dir = URL(fileURLWithPath: args.dir, isDirectory: true)
    DispatchQueue.main.async {
      guard let image = UIPasteboard.general.image, let data = image.pngData() else {
        invoke.resolve(PathResult())
        return
      }
      do {
        try FileManager.default.createDirectory(at: dir, withIntermediateDirectories: true)
        let stamp = Int(Date().timeIntervalSince1970 * 1000)
        let target = ZWormHoleMobilePlugin.unique(dir, "clipboard-\(stamp).png")
        try data.write(to: target)
        invoke.resolve(PathResult(path: target.path))
      } catch {
        invoke.reject(error.localizedDescription)
      }
    }
  }

  @objc public func deviceInfo(_ invoke: Invoke) throws {
    let documents =
      FileManager.default.urls(for: .documentDirectory, in: .userDomainMask).first?.path ?? ""
    invoke.resolve(DeviceInfoResult(name: deviceName, downloadDir: documents))
  }

  @objc public func openFile(_ invoke: Invoke) throws {
    let args = try invoke.parseArgs(PathArgs.self)
    let url = URL(fileURLWithPath: args.path)
    var isDir: ObjCBool = false
    if FileManager.default.fileExists(atPath: url.path, isDirectory: &isDir), isDir.boolValue {
      reveal(url)
      invoke.resolve()
      return
    }
    DispatchQueue.main.async {
      let controller = UIDocumentInteractionController(url: url)
      self.previewDelegate = PreviewDelegate(self.manager.viewController)
      controller.delegate = self.previewDelegate
      self.interaction = controller
      if !controller.presentPreview(animated: true) {
        if let view = self.manager.viewController?.view {
          controller.presentOptionsMenu(from: view.bounds, in: view, animated: true)
        }
      }
      invoke.resolve()
    }
  }

  @objc public func revealFile(_ invoke: Invoke) throws {
    let args = try invoke.parseArgs(PathArgs.self)
    let url = URL(fileURLWithPath: args.path)
    var isDir: ObjCBool = false
    let exists = FileManager.default.fileExists(atPath: url.path, isDirectory: &isDir)
    reveal(exists && isDir.boolValue ? url : url.deletingLastPathComponent())
    invoke.resolve()
  }

  private func reveal(_ dir: URL) {
    let encoded = dir.path.addingPercentEncoding(withAllowedCharacters: .urlPathAllowed) ?? dir.path
    guard let target = URL(string: "shareddocuments://\(encoded)") else { return }
    DispatchQueue.main.async {
      UIApplication.shared.open(target, options: [:], completionHandler: nil)
    }
  }

  @objc public func mediaScan(_ invoke: Invoke) throws {
    invoke.resolve()
  }

  @objc public func setBackgroundService(_ invoke: Invoke) throws {
    invoke.resolve()
  }

  @objc public func bonjourPublish(_ invoke: Invoke) throws {
    let args = try invoke.parseArgs(PublishArgs.self)
    bonjour.publish(name: args.name, port: args.port, txt: args.txt)
    invoke.resolve()
  }

  @objc public func bonjourUnpublish(_ invoke: Invoke) throws {
    bonjour.unpublish()
    invoke.resolve()
  }

  @objc public func bonjourBrowse(_ invoke: Invoke) throws {
    bonjour.browse()
    invoke.resolve()
  }

  @objc public func bonjourStopBrowse(_ invoke: Invoke) throws {
    bonjour.stopBrowsing()
    invoke.resolve()
  }

  @objc public func bonjourSnapshot(_ invoke: Invoke) throws {
    invoke.resolve(BonjourSnapshot(services: bonjour.snapshot()))
  }
}

@_cdecl("init_plugin_zwormhole_mobile")
func initPlugin() -> Plugin {
  if #available(iOS 14.0, *) {
    return ZWormHoleMobilePlugin()
  }
  return Plugin()
}
